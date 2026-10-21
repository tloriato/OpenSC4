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

// The occupant manager's GZCOM class (docs/specs/occupant-manager.md §1,
// CLSID 0x098F964D): the city's spatial index of occupants. The grid, the
// cell lists, the message order and the v1 save record live in the Rust core
// (fc4_occmgr); this class adapts the three interfaces to it and implements
// the host callbacks over the game's occupants.
//
// Layout (§1): cISC4OccupantManager and cIGZUnknown at +0, the cIGZSerializable
// sub-object at +4, cIGZMessageTarget2 at +8. QueryInterface answers 1 and
// 0xC6BB8334 at +0, 0xE4FDA3D4 at +4 and 0x452294AA at +8 (the core's
// fc4_occmgr_query_interface gives the sub-object offset). NOTE the order is
// the reverse of the dispatch manager's (+4 is Serializable here, not the
// message target); a save/load through the wrong view writes the wrong record.
//
// The slots are the game's compiled order (cFC4ISC4OccupantManager,
// FC4Interfaces.h): every overload pair has a unique name, and slots 46/47 are
// declared typed-first because the game puts the typed overload first for that
// pair alone (§9.4).
//
// Threading (§14): the core owns one recursive lock and takes it at exactly the
// §14 table points (slot 42's callbacks unlocked, slot 44's under the outer
// lock). Getters run from a second thread while OnTick runs (HANDOFF §5.1), so
// nothing of ours is behind a RefCell; every entry point may be called from any
// thread.
//
// Refaudit (HANDOFF §5.1b): the core AddRefs the occupants it holds and
// Releases them through the host; a list query hands the caller one reference
// per element and slot 48 gives them back. Temporaries the game creates for us
// go through FC4_RELEASE_CREATED.
//
// OFF by default: FreeCity4.ini `enable = occmgr`; `disable` still wins.
#pragma once

#include "FC4Interfaces.h"
#include "cIGZMessageTarget2.h"
#include "cIGZSerializable.h"
#include "fc4.h"

#include <atomic>
#include <cstdint>
#include <vector>

class cISC4City;

static const uint32_t kFC4_CLSID_cSC4OccupantManager = 0x098F964D;
static const uint32_t kFC4_IID_cISC4OccupantManager = 0xC6BB8334;

class cFC4OccMgr final : public cFC4ISC4OccupantManager,
                         public cIGZSerializable,
                         public cIGZMessageTarget2 {
public:
    cFC4OccMgr();
    ~cFC4OccMgr();

    // cIGZUnknown (slot 0-2)
    bool QueryInterface(uint32_t riid, void** ppvObj) override;
    uint32_t AddRef() override;
    uint32_t Release() override;

    // cISC4OccupantManager (§11, the compiled order)
    bool Init() override;
    bool Shutdown() override;
    bool SetCitySize(float fx, float fz) override;
    bool GetOccupantManagerCellSizes(float& x, float& z) override;
    bool GetWorldCellCount(int32_t& x, int32_t& z) override;
    bool GetOccupantManagerCellCount(int32_t& x, int32_t& z) override;
    bool GetOccupantManagerCellBounds(int32_t& x0, int32_t& z0, int32_t& x1, int32_t& z1) override;
    bool WorldCellToOccupantManagerCell(int32_t x, int32_t z, int32_t& ox, int32_t& oz) override;
    bool OccupantManagerCellToWorldCell(int32_t x, int32_t z, int32_t& ox, int32_t& oz) override;
    bool WorldCellToStandardCityCell(int32_t x, int32_t z, int32_t& ox, int32_t& oz) override;
    bool StandardCityCellToWorldCell(int32_t x, int32_t z, int32_t& ox, int32_t& oz) override;
    bool WorldCellToPosition(int32_t x, int32_t z, float& fx, float& fz) override;
    bool PositionToWorldCell(float fx, float fz, int32_t& x, int32_t& z) override;
    bool OccupantManagerCellToPosition(int32_t x, int32_t z, float& fx, float& fz) override;
    bool PositionToOccupantManagerCell(float fx, float fz, int32_t& x, int32_t& z) override;
    bool StandardCityCellToPosition(int32_t x, int32_t z, float& fx, float& fz) override;
    bool PositionToStandardCityCell(float fx, float fz, int32_t& x, int32_t& z) override;
    bool InsertOccupant(cISC4Occupant* occ, uint32_t data4) override;
    bool RemoveOccupant(cISC4Occupant* occ, bool shutdown, uint32_t data4) override;
    bool RemoveOccupants(cISC4OccupantFilter* filter, bool shutdown, uint32_t data4) override;
    bool RemoveOccupantsOfType(uint32_t occType, bool shutdown, uint32_t data4) override;
    bool MoveOccupant(cISC4Occupant* occ, bool post) override;
    bool IsCellEmpty(int32_t x, int32_t z) override;
    int32_t GetBoundingCells(int32_t x, int32_t z, int32_t (*cells)[2], int32_t& n) override;
    bool FindOccupant(cISC4Occupant* occ, bool searchAll, int32_t* x, int32_t* z) override;
    bool GetFirstOccupantByPositionFilter(cISC4Occupant*& out, float fx, float fz,
                                          cISC4OccupantFilter* filter) override;
    bool GetFirstOccupantByPositionTyped(cISC4Occupant*& out, float fx, float fz, uint32_t occType) override;
    bool GetFirstOccupantFilter(cISC4Occupant*& out, int32_t x, int32_t z, cISC4OccupantFilter* filter) override;
    bool GetFirstOccupantTyped(cISC4Occupant*& out, int32_t x, int32_t z, uint32_t occType) override;
    bool GetFirstOccupantByStandardCityCellFilter(cISC4Occupant*& out, int32_t x, int32_t z,
                                                  cISC4OccupantFilter* filter) override;
    bool GetFirstOccupantByStandardCityCellTyped(cISC4Occupant*& out, int32_t x, int32_t z,
                                                 uint32_t occType) override;
    bool GetFirstOccupantByStandardCityCellsFilter(cISC4Occupant*& out, SC4Point<int> const* min,
                                                   SC4Point<int> const* max, cISC4OccupantFilter* filter) override;
    bool GetFirstOccupantByStandardCityCellsTyped(cISC4Occupant*& out, SC4Point<int> const* min,
                                                  SC4Point<int> const* max, uint32_t occType) override;
    bool GetOccupantsByBBoxFilter(SC4List<cISC4Occupant*>& list, float const* min3, float const* max3,
                                  cISC4OccupantFilter* filter, uint32_t maxCount) override;
    bool GetOccupantsByBBoxTyped(SC4List<cISC4Occupant*>& list, float const* min3, float const* max3,
                                 uint32_t occType, uint32_t maxCount) override;
    bool GetOccupantsByOccupantManagerCellsFilter(SC4List<cISC4Occupant*>& list, int32_t const* min,
                                                  int32_t const* max, cISC4OccupantFilter* filter,
                                                  uint32_t maxCount) override;
    bool GetOccupantsByOccupantManagerCellsTyped(SC4List<cISC4Occupant*>& list, int32_t const* min,
                                                 int32_t const* max, uint32_t occType, uint32_t maxCount) override;
    bool GetOccupantsByStandardCityCellsFilter(SC4List<cISC4Occupant*>& list, SC4Point<int> const* min,
                                               SC4Point<int> const* max, cISC4OccupantFilter* filter,
                                               uint32_t maxCount) override;
    bool GetOccupantsByStandardCityCellsTyped(SC4List<cISC4Occupant*>& list, SC4Point<int> const* min,
                                              SC4Point<int> const* max, uint32_t occType, uint32_t maxCount) override;
    bool IterateOccupantsByBBoxFilter(bool (*fn)(cISC4Occupant*, void*), void* data, float const* min3,
                                      float const* max3, cISC4OccupantFilter* filter) override;
    bool IterateOccupantsByBBoxTyped(bool (*fn)(cISC4Occupant*, void*), void* data, float const* min3,
                                     float const* max3, uint32_t occType) override;
    bool IterateOccupantsFilter(bool (*fn)(cISC4Occupant*, void*), void* data, int32_t const* min,
                                int32_t const* max, cISC4OccupantFilter* filter) override;
    bool IterateOccupantsTyped(bool (*fn)(cISC4Occupant*, void*), void* data, int32_t const* min,
                               int32_t const* max, uint32_t occType) override;
    bool IterateOccupantsByStandardCityCellTyped(bool (*fn)(cISC4Occupant*, void*), void* data,
                                                 int32_t const* min, int32_t const* max, uint32_t occType) override;
    bool IterateOccupantsByStandardCityCellFilter(bool (*fn)(cISC4Occupant*, void*), void* data,
                                                  int32_t const* min, int32_t const* max,
                                                  cISC4OccupantFilter* filter) override;
    bool ReleaseOccupantList(SC4List<cISC4Occupant*>& list) override;

    // cIGZMessageTarget2 (§1: a shared stub, no message is handled)
    bool DoMessage(cIGZMessage2* pMessage) override;

    // cIGZSerializable (§13)
    bool Write(cIGZOStream& gzOut) override;
    bool Read(cIGZIStream& gzIn) override;
    uint32_t GetGZCLSID() override;

    // The host callbacks (cFC4OccMgr.cpp) and the sub-object views.
    cIGZUnknown* Primary() {
        return static_cast<cIGZUnknown*>(static_cast<cFC4ISC4OccupantManager*>(this));
    }
    cIGZSerializable* Serializable() { return static_cast<cIGZSerializable*>(this); }
    cIGZMessageTarget2* Target() { return static_cast<cIGZMessageTarget2*>(this); }

    // The city this manager serves (set at the first Init).
    void SetCity(cISC4City* c);
    cISC4City* City() const { return city_; }

private:
    // One list query's results, in the core's order, until they are appended.
    struct ListOut {
        std::vector<uint64_t> occs;
    };
    static void ListSink(void* ctx, uint64_t occ);
    // One iteration callback, wrapped from the game's `bool (cISC4Occupant*, void*)`.
    struct IterIn {
        bool (*fn)(cISC4Occupant*, void*) = nullptr;
        void* data = nullptr;
    };
    static bool IterThunk(void* ctx, uint64_t occ);

    // `occ` by reference: the callers pass the out variable of the core call in
    // the same argument list, and argument evaluation order is unspecified
    // (MSVC reads a by-value copy before the core call writes it).
    bool FirstOut(bool ok, const uint64_t& occ, cISC4Occupant*& out);
    bool FillList(bool ok, const ListOut& out, SC4List<cISC4Occupant*>& list);

    uint32_t refCount_ = 0;
    fc4_occmgr_host host_{};  // copied into the core at create
    fc4_occmgr* core_ = nullptr;
    cISC4City* city_ = nullptr;
};

// Factory for the subsystem table (reference count 0).
cIGZUnknown* CreateFC4OccupantManager();
