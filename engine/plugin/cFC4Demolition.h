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

// The demolition utility's GZCOM class (docs/specs/demolition.md §1): city
// component 2, the cISC4City slot 31 getter. The filter Φ, the pricing and
// the removals live in the Rust core (fc4_demolition); this class adapts the
// interfaces to it and owns the game services it uses, through the full host
// table of FC4DemolitionHost.h (the 86 callbacks glue17 left unwired).
//
// Layout (§1): cISC4Demolition and cIGZUnknown at +0, the cIGZMessageTarget2
// sub-object at +4, cIGZSerializable at +8. QueryInterface answers 1 and
// 0x26DDE2EA at +0, 0x452294AA at +4 and 0xE4FDA3D4 at +8 (the core's
// fc4_demolition_query_interface gives the sub-object offset).
//
// Threading: the core is single-threaded (RefCell); the plugin serialises
// every call on this one object. Getters run from a second thread while OnTick
// runs (HANDOFF §5.1), so nothing here lives in a RefCell.
//
// OFF by default: FreeCity4.ini `enable = demolition`; `disable` still wins.
#pragma once

#include "FC4DemolitionHost.h"
#include "FC4Interfaces.h"
#include "cIGZMessageTarget2.h"
#include "cIGZSerializable.h"
#include "fc4.h"

#include <atomic>
#include <cstdint>

class cIGZMessageServer2;
class cISC4City;

static const uint32_t kFC4_CLSID_cSC4Demolition = 0x0990C38A;
static const uint32_t kFC4_IID_cISC4Demolition = 0x26DDE2EA;
// The GZCOM class factory (FC4Subsystems.cpp, `enable = demolition`).
cIGZUnknown* CreateFC4Demolition();

namespace fc4 {
bool DemolitionRegionToC(const SC4CellRegion<int32_t>& region, fc4_zone_region& out,
                        std::vector<uint8_t>& mask);
}

class cFC4Demolition final : public cFC4ISC4Demolition,
                             public cIGZMessageTarget2,
                             public cIGZSerializable {
public:
    cFC4Demolition();
    ~cFC4Demolition();

    // cIGZUnknown
    bool QueryInterface(uint32_t riid, void** ppvObj) override;
    uint32_t AddRef() override;
    uint32_t Release() override;

    // cISC4Demolition (§8, the compiled order)
    bool Init() override;
    bool Shutdown() override;
    void SetDefaultOccupantFilter(cISC4OccupantFilter* filter) override;  // 5  does nothing
    bool DemolishRegion(bool doIt, SC4CellRegion<int32_t> const& region, int32_t privilege, uint32_t flags,
                        bool dezone, cISC4OccupantFilter* filter, int64_t* cost, intptr_t demolished,
                        cISC4Occupant* clicked, long x, long y) override;                          // 6
    bool DemolishLots(bool doIt, SC4List<cISC4Lot*> const& lots, int32_t privilege, uint32_t flags, bool dezone,
                      cISC4OccupantFilter* filter, int64_t* cost, intptr_t demolished) override;   // 7
    bool DemolishOccupant(bool doIt, cISC4Occupant* occ, int32_t privilege, uint32_t flags, int64_t* cost,
                          bool effects, SC4List<cISC4Occupant*>* orphans) override;                // 8
    int32_t ModifyTerrainHeight(bool doIt, bool perVertex, int32_t x0, int32_t z0, int32_t x1, int32_t z1, float h,
                                cISC4Lot* exempt, uint32_t reason, int64_t* cost) override;        // 9
    void DoDemolishOccupantEffect(cISC4Occupant* occ, uint32_t flags, cISC4Lot* lot, bool clicked, long x,
                                  long y) override;                                                // 10
    void DoDemolishGroupEffect(uint32_t cause, int32_t x, int32_t z, SC4Rect<int32_t> const& r) override;  // 11

    // cIGZMessageTarget2 (§3) and cIGZSerializable (§7)
    bool DoMessage(cIGZMessage2* pMessage) override;
    bool Write(cIGZOStream& gzOut) override;
    bool Read(cIGZIStream& gzIn) override;
    uint32_t GetGZCLSID() override;

    // The city this utility belongs to (borrowed; set by the subsystems).
    void SetCity(cISC4City* city);
    cISC4City* City() const { return ctx_.city; }

    fc4_demolition* Core() const { return core_; }

private:
    std::atomic<uint32_t> refCount_{1};
    bool destroying_ = false;
    fc4::DemolitionHostContext ctx_{};
    ::fc4_demolition_host host_{};
    fc4_demolition* core_ = nullptr;
};
