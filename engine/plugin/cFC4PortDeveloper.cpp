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

#include "cFC4PortDeveloper.h"

#include "FC4CoreProps.h"
#include "FC4Diag.h"
#include "FC4Fpu.h"
#include "FC4GameList.h"
#include "FC4Interfaces.h"
#include "FC4Log.h"
#include "FC4LotDevInterfaces.h"
#include "FC4Properties.h"
#include "FC4RefAudit.h"
#include "FC4Streams.h"
#include "FC4TractInterfaces.h"
#include "cFC4Message2Standard.h"

#include "GZServPtrs.h"
#include "SC4CellRegion.h"
#include "SC4Rect.h"
#include "SC4Vector.h"
#include "cIGZApp.h"
#include "cIGZFrameWork.h"
#include "cIGZMessage2.h"
#include "cIGZMessage2Standard.h"
#include "cIGZMessageServer2.h"
#include "cISC4AdvisorSystem.h"
#include "cISC4App.h"
#include "cISC4BudgetSimulator.h"
#include "cISC4BuildingDevelopmentSimulator.h"
#include "cISC4City.h"
#include "cISC4DemandSimulator.h"
#include "cISC4DepartmentBudget.h"
#include "cISC4LineItem.h"
#include "cISC4Lot.h"
#include "cISC4LotConfiguration.h"
#include "cISC4LotConfigurationManager.h"
#include "cISC4LotDeveloper.h"
#include "cISC4LotManager.h"
#include "cISC4Occupant.h"
#include "cISC4Simulator.h"
#include "cISC4TrafficSimulator.h"
#include "cISC4ZoneManager.h"
#include "cISCPropertyHolder.h"
#include "cISTETerrain.h"
#include "cRZBaseString.h"
#include "cRZCOMDllDirector.h"

#include <cstdio>
#include <cstring>
#include <string>

using fc4::FromToken;
using fc4::TokenOf;

namespace {

constexpr uint32_t kIID_cIGZMessageTarget2 = 0x452294AA;
constexpr uint32_t kIID_cIGZSerializable = 0xE4FDA3D4;
constexpr uint32_t kIID_cISC4Occupant = 0x26BB767E;
constexpr uint32_t kIID_cISC4Lot = 0xE818FCED;

// Messages (§3).
constexpr uint32_t kMsgSimNewMonth = 0x66956816;
constexpr uint32_t kMsgOccupantInserted = 0x99EF1142;
constexpr uint32_t kMsgLotStateChanged = 0xE8BDA2E7;
constexpr uint32_t kMsgPostCityInit = 0x26D31EC1;
constexpr uint32_t kMsgTerrainChanged = 0x6A3B0AFD;  // seaport only
constexpr uint32_t kMsgHistoryRecord = 0x89EFA536;
constexpr uint32_t kAirportGrowth[] = {0xEA394B74, 0xCB3BE815, 0xEB3BE415, 0x2B3BE44A, 0x6B3BE6D8, 0x0B3BE81A};
constexpr uint32_t kSeaportGrowth = 0x8A394DC4;

// GetSimulatorType (§1).
constexpr uint32_t kSimulatorType[] = {0xEA3955C4, 0xEA3957AD};

// The tuning exemplars (§6): type and group; the instance comes from the core.
constexpr uint32_t kTuningType = 0x6534284A;
constexpr uint32_t kTuningGroup = 0x67CD5FA1;

// The occupant preview marker (§3).
constexpr uint32_t kPropPreview = 0x029244DB;

fc4::PortContext* Ctx(void* u) { return static_cast<fc4::PortContext*>(u); }

cISC4Occupant* OccOf(uint64_t t) { return reinterpret_cast<cISC4Occupant*>(FromToken(t)); }
cISC4Lot* SdkLotOf(uint64_t t) { return reinterpret_cast<cISC4Lot*>(FromToken(t)); }
cFC4ISC4Lot* LotOf(uint64_t t) { return t ? fc4::AsGame(SdkLotOf(t)) : nullptr; }
cISC4LotConfiguration* CfgOf(uint64_t t) { return reinterpret_cast<cISC4LotConfiguration*>(FromToken(t)); }
cFC4ISC4DepartmentBudget* DeptOf(uint64_t t) {
    return t ? fc4::AsGame(reinterpret_cast<cISC4DepartmentBudget*>(FromToken(t))) : nullptr;
}

template <class T>
T* Query(cIGZUnknown* u, uint32_t iid) {
    if (u == nullptr) return nullptr;
    void* p = nullptr;
    if (!u->QueryInterface(iid, &p) || p == nullptr) return nullptr;
    T* t = static_cast<T*>(p);
    if (!fc4::IsOurObject(t)) FC4_ACQUIRED(reinterpret_cast<cIGZUnknown*>(t), "QueryInterface");
    return t;
}

template <class T>
void Drop(T* p) {
    if (p == nullptr) return;
    cIGZUnknown* u = reinterpret_cast<cIGZUnknown*>(p);
    if (fc4::IsOurObject(u)) {
        u->Release();
    } else {
        FC4_RELEASE(u);
    }
}

void HoldObject(cIGZUnknown* o) {
    if (o == nullptr) return;
    if (fc4::IsOurObject(o)) {
        o->AddRef();
    } else {
        FC4_ADDREF(o);
    }
}

void DropObject(cIGZUnknown* o) {
    if (o == nullptr) return;
    if (fc4::IsOurObject(o)) {
        o->Release();
    } else {
        FC4_RELEASE(o);
    }
}

void NoteOnce(bool& done, const char* text) {
    if (done) return;
    done = true;
    fc4::Log("%s", text);
}

bool g_noteAccessProps = false;
bool g_noteNoTraffic = false;

cISC4City* AppCity() {
    cIGZFrameWork* fw = RZGetFrameWork();
    cIGZApp* app = fw ? fw->Application() : nullptr;
    cISC4App* sc4 = nullptr;
    if (app == nullptr || !app->QueryInterface(GZIID_cISC4App, reinterpret_cast<void**>(&sc4)) || sc4 == nullptr) {
        return nullptr;
    }
    FC4_ACQUIRED(sc4, "QueryInterface");
    cISC4City* city = sc4->GetCity();
    FC4_RELEASE(sc4);
    return city;
}

cFC4ISC4BudgetSimulator* Budget() {
    cISC4City* c = AppCity();
    cISC4BudgetSimulator* b = c ? c->GetBudgetSimulator() : nullptr;  // borrowed
    return b ? fc4::AsGame(b) : nullptr;
}

cFC4ISC4LotManager* LotManager() {
    cISC4City* c = AppCity();
    cISC4LotManager* lm = c ? c->GetLotManager() : nullptr;  // borrowed
    return lm ? fc4::AsGame(lm) : nullptr;
}

cFC4ISC4LotDeveloper* LotDeveloper() {
    cISC4City* c = AppCity();
    cISC4BuildingDevelopmentSimulator* bds = c ? c->GetBuildingDevelopmentSimulator() : nullptr;
    cISC4LotDeveloper* ld = bds ? bds->GetLotDeveloper() : nullptr;  // BDS slot 36, borrowed
    return ld ? fc4::AsGame(ld) : nullptr;
}

cISTETerrain* Terrain() {
    cISC4City* c = AppCity();
    return c ? c->GetTerrain() : nullptr;  // borrowed
}

// [abi] The traffic simulator's vector slots fill a std::vector with the
// game's default allocator (B): without it nothing is asked.
cFC4ISC4TrafficSimulator* Traffic() {
    cISC4City* c = AppCity();
    cISC4TrafficSimulator* t = c ? c->GetTrafficSimulator() : nullptr;
    return t ? reinterpret_cast<cFC4ISC4TrafficSimulator*>(t) : nullptr;
}

// ---- host callbacks (fc4_port_host) ---------------------------------------------------------------

fc4_props* Exemplar(void*, uint32_t instance) {
    fc4::props::Exemplar ex;
    if (!ex.Load(kTuningType, kTuningGroup, instance)) return nullptr;
    return fc4::props::ToCoreAll(ex.Holder());
}

// §3: simulator slot 19 AddAgent(target = +8, level, name, flags).
void AddAgent(void* u, uint32_t level, const char* name, uint32_t flags) {
    fc4::PortContext* c = Ctx(u);
    if (c->target == nullptr || name == nullptr) return;
    cISC4City* city = AppCity();
    cISC4Simulator* raw = city ? city->GetSimulator() : nullptr;  // city slot 50, borrowed
    if (raw == nullptr) {
        fc4::Log("ports %s: no simulator; the month agent is not registered", fc4::PortKindName(c->kind));
        return;
    }
    if (c->agentSim == nullptr) {
        c->agentSim = raw;
        HoldObject(c->agentSim);
    }
    const cRZBaseString n(name);
    fc4::AsGame(raw)->AddAgent(c->target, level, n, flags);
    c->agentRegistered = true;
}

// §3: Shutdown removes the agent from every level.
void RemoveAgent(void* u, const char*) {
    fc4::PortContext* c = Ctx(u);
    if (c->agentSim == nullptr) return;
    if (c->agentRegistered) fc4::AsGame(c->agentSim)->RemoveAgentAllLevels(c->target);
    c->agentRegistered = false;
    DropObject(c->agentSim);
    c->agentSim = nullptr;
}

// §4.2: budget slot 28 GetDepartmentBudget(id), borrowed.
uint64_t Department(void*, uint32_t id) {
    cFC4ISC4BudgetSimulator* b = Budget();
    return b ? TokenOf(b->GetDepartmentBudget(id)) : 0;
}

// §4.2: CreateDepartmentBudget(id, group) (slot 25), SetDepartmentName
// (department slot 5), SetMaxAllowedFundingPercentage (slot 13), line item
// `line` not locally funded with display flag 1 cleared. [question PD12]
// "create line item 1": department slot 17 CreateLineItem(line, false), the
// literal reading (slot 18 CreateLineItemForBuildingType is the other
// candidate).
uint64_t CreateDepartment(void* u, uint32_t id, uint32_t group, uint32_t nameGroup, uint32_t nameInstance,
                          float maxFunding, uint32_t line) {
    cFC4ISC4BudgetSimulator* b = Budget();
    cISC4DepartmentBudget* raw = b ? b->CreateDepartmentBudget(id, group) : nullptr;
    cFC4ISC4DepartmentBudget* d = raw ? fc4::AsGame(raw) : nullptr;
    if (d == nullptr) {
        fc4::Log("ports %s: the budget department 0x%08X could not be created", fc4::PortKindName(Ctx(u)->kind), id);
        return 0;
    }
    d->SetDepartmentName(nameGroup, nameInstance);
    SC4Percentage p;
    p.percentage = maxFunding;
    d->SetMaxAllowedFundingPercentage(p);
    if (cISC4LineItem* li = d->CreateLineItem(line, false)) {
        li->SetDisplayFlag(static_cast<cISC4LineItem::DisplayFlag>(1), false);
    }
    ++Ctx(u)->departments;
    return TokenOf(raw);
}

// Department slot 20 GetLineItem (borrowed); line item slots 9 and 10.
bool LineExpenses(void*, uint64_t dept, uint32_t line, int64_t* full, int64_t* current) {
    cFC4ISC4DepartmentBudget* d = DeptOf(dept);
    cISC4LineItem* li = d ? d->GetLineItem(line) : nullptr;
    if (li == nullptr) return false;
    if (full != nullptr) *full = li->GetFullExpenses();
    if (current != nullptr) *current = li->GetCurrentExpenses();
    return true;
}

void SetFullExpenses(void*, uint64_t dept, uint32_t line, int64_t amount) {
    cFC4ISC4DepartmentBudget* d = DeptOf(dept);
    cISC4LineItem* li = d ? d->GetLineItem(line) : nullptr;
    if (li == nullptr) return;
    HoldObject(li);
    li->SetFullExpenses(amount);
    DropObject(li);
}

// §4.5 step 2: budget slot 8; the result is ignored by the developer.
bool WithdrawFunds(void* u, int64_t amount) {
    cFC4ISC4BudgetSimulator* b = Budget();
    ++Ctx(u)->withdrawals;
    return b != nullptr && b->WithdrawFunds(amount);
}

bool OccupantIsPreview(void*, uint64_t occ) {
    cISCPropertyHolder* h = occ ? OccOf(occ)->AsPropertyHolder() : nullptr;  // occupant slot 6
    return h != nullptr && fc4::props::Has(h, kPropPreview);
}

uint32_t OccupantType(void*, uint64_t occ) { return occ ? static_cast<uint32_t>(OccOf(occ)->GetType()) : 0; }

bool OccupantInGroup(void*, uint64_t occ, uint32_t group) { return occ != 0 && OccOf(occ)->IsOccupantGroup(group); }

// Lot manager slot 14 GetOccupantLot (borrowed).
uint64_t OccupantLot(void*, uint64_t occ) {
    cFC4ISC4LotManager* lm = LotManager();
    return (lm != nullptr && occ != 0) ? TokenOf(lm->GetOccupantLot(OccOf(occ))) : 0;
}

// Lot slot 9, configuration slots 3 and 11.
bool LotConfig(void*, uint64_t lot, uint32_t* id, uint32_t* stage) {
    cFC4ISC4Lot* l = LotOf(lot);
    cISC4LotConfiguration* cfg = l ? l->GetLotConfiguration() : nullptr;
    if (cfg == nullptr) return false;
    if (id != nullptr) *id = cfg->GetID();
    if (stage != nullptr) *stage = cfg->GetGrowthStage();
    return true;
}

void LotLocation(void*, uint64_t lot, int16_t* x, int16_t* z) {
    int32_t lx = 0, lz = 0;
    if (cFC4ISC4Lot* l = LotOf(lot)) l->GetLocation(lx, lz);  // slot 21
    if (x != nullptr) *x = static_cast<int16_t>(lx);
    if (z != nullptr) *z = static_cast<int16_t>(lz);
}

// §2: a loaded port's lot, fetched from the lot manager at (x, z): slot 6
// GetLot(x, z, false) [inferred: the flag the parks glue passes].
uint64_t LotAt(void*, int16_t x, int16_t z) {
    cFC4ISC4LotManager* lm = LotManager();
    return lm ? TokenOf(lm->GetLot(x, z, false)) : 0;
}

uint32_t LotState(void*, uint64_t lot) {
    cFC4ISC4Lot* l = LotOf(lot);
    return l ? static_cast<uint32_t>(l->GetState()) : 0;  // slot 33
}

uint32_t LotCondition(void*, uint64_t lot) {
    cFC4ISC4Lot* l = LotOf(lot);
    return l ? l->GetCondition() : 0;  // slot 35
}

void SetLotCondition(void* u, uint64_t lot, uint32_t condition) {
    if (cFC4ISC4Lot* l = LotOf(lot)) {
        l->SetCondition(condition);  // slot 36
        ++Ctx(u)->conditions;
    }
}

uint32_t LotDays(void*, uint64_t lot) {
    cFC4ISC4Lot* l = LotOf(lot);
    return l ? l->GetDaysSinceLastStateChange() : 0;  // slot 40
}

void LotRect(void*, uint64_t lot, fc4_cell_rect* out) {
    if (out == nullptr) return;
    *out = fc4_cell_rect{};
    SC4Rect<int32_t> r(0, 0, 0, 0);
    cFC4ISC4Lot* l = LotOf(lot);
    if (l == nullptr || !l->GetBoundingRect(r)) return;  // slot 14
    out->x1 = r.topLeftX;
    out->z1 = r.topLeftY;
    out->x2 = r.bottomRightX;
    out->z2 = r.bottomRightY;
}

uint32_t LotFacing(void*, uint64_t lot) {
    cFC4ISC4Lot* l = LotOf(lot);
    return l ? static_cast<uint32_t>(l->GetFacing()) : 0;  // slot 23
}

// §4.6: lot configuration slot 15.
bool LotZoneCompatible(void*, uint64_t lot, uint16_t zone) {
    cFC4ISC4Lot* l = LotOf(lot);
    cISC4LotConfiguration* cfg = l ? l->GetLotConfiguration() : nullptr;
    return cfg != nullptr && cfg->IsCompatibleWithZoneType(static_cast<cISC4ZoneManager::ZoneType>(zone));
}

// The lot configuration manager (city slot 37) slot 8: no reference.
uint64_t LotConfiguration(void*, uint32_t id) {
    cISC4City* c = AppCity();
    cISC4LotConfigurationManager* m = c ? c->GetLotConfigurationManager() : nullptr;
    return m ? TokenOf(fc4::AsGame(m)->GetLotConfiguration(id)) : 0;
}

void StartDemolishLot(void* u, uint64_t lot) {
    if (cFC4ISC4LotDeveloper* ld = LotDeveloper()) {
        ld->StartDemolishLot(SdkLotOf(lot));  // slot 12
        ++Ctx(u)->swaps;
    }
}

void EndDemolishLot(void*, uint64_t lot) {
    if (cFC4ISC4LotDeveloper* ld = LotDeveloper()) ld->EndDemolishLot(SdkLotOf(lot));  // slot 13
}

// Lot manager slot 23: the lot comes with one reference for the caller
// (zones.md §8.5), which becomes the developer's hold.
uint64_t CreateLot(void* u, int32_t x, int32_t z, int32_t width, int32_t depth, uint32_t facing) {
    cFC4ISC4LotManager* lm = LotManager();
    if (lm == nullptr) return 0;
    cISC4Lot* lot = nullptr;
    if (!lm->CreateLot(x, z, width, depth, static_cast<int32_t>(facing), lot) || lot == nullptr) return 0;
    if (!fc4::IsOurObject(lot)) FC4_ACQUIRED(reinterpret_cast<cIGZUnknown*>(lot), "CreateLot");
    Ctx(u)->createdLots.push_back(TokenOf(lot));
    return TokenOf(lot);
}

// Lot slots 10 and 32.
void SetLotConfiguration(void*, uint64_t lot, uint64_t cfg) {
    cFC4ISC4Lot* l = LotOf(lot);
    if (l == nullptr) return;
    l->SetLotConfiguration(CfgOf(cfg));
    l->UpdateZoneType();
}

// Lot developer slot 7 ConstructDefaultLot(lot, cfg, 0, true).
bool ConstructDefaultLot(void*, uint64_t lot, uint64_t cfg) {
    cFC4ISC4LotDeveloper* ld = LotDeveloper();
    return ld != nullptr && ld->ConstructDefaultLot(SdkLotOf(lot), CfgOf(cfg), 0, true);
}

// The cRZCellMap<bool> layout with a vtable of our own (as cFC4Landfill.cpp).
class CellMapStorage {
public:
    virtual ~CellMapStorage() = default;
    uint32_t width = 0;
    uint32_t height = 0;
    uint32_t words = 0;
    uint32_t** rows = nullptr;
};

// A full rectangle in the game's SC4CellRegion<long> layout (zones.md); the
// memory is ours. The zone manager clears the bits of rejected cells.
class RegionBuffer {
public:
    bool Build(const fc4_cell_rect& r) {
        const int64_t w = int64_t{r.x2} - r.x1 + 1;
        const int64_t h = int64_t{r.z2} - r.z1 + 1;
        if (w <= 0 || h <= 0 || w * h > (1 << 20)) return false;
        layout_.x1 = r.x1;
        layout_.z1 = r.z1;
        layout_.x2 = r.x2;
        layout_.z2 = r.z2;
        const uint32_t words = static_cast<uint32_t>((h + 31) / 32);
        bits_.assign(static_cast<size_t>(w) * words, 0);
        rows_.resize(static_cast<size_t>(w));
        for (int64_t dx = 0; dx < w; ++dx) {
            rows_[static_cast<size_t>(dx)] = &bits_[static_cast<size_t>(dx) * words];
            for (int64_t dz = 0; dz < h; ++dz) rows_[static_cast<size_t>(dx)][dz / 32] |= 1u << (dz % 32);
        }
        layout_.map.width = static_cast<uint32_t>(w);
        layout_.map.height = static_cast<uint32_t>(h);
        layout_.map.words = words;
        layout_.map.rows = rows_.data();
        return true;
    }
    SC4CellRegion<int32_t>& Zoning() { return *reinterpret_cast<SC4CellRegion<int32_t>*>(&layout_); }

private:
    struct Layout {
        int32_t x1 = 0, z1 = 0, x2 = 0, z2 = 0;
        CellMapStorage map;
    };
    static_assert(sizeof(void*) != 4 || sizeof(Layout) == 0x24, "SC4CellRegion<long> layout");
    Layout layout_;
    std::vector<uint32_t> bits_;
    std::vector<uint32_t*> rows_;
};

// §4.6 step 2: zone manager slot 11 PlaceZone(region, 0, commit, skip funds,
// no slope check, skip size, lot need not be inside, null, null, 0).
void Dezone(void* u, const fc4_cell_rect* rect) {
    cISC4City* c = AppCity();
    cISC4ZoneManager* zm = c ? c->GetZoneManager() : nullptr;  // city slot 36
    if (zm == nullptr || rect == nullptr) return;
    RegionBuffer rb;
    if (!rb.Build(*rect)) return;
    ++Ctx(u)->dezones;
    zm->PlaceZone(rb.Zoning(), static_cast<cISC4ZoneManager::ZoneType>(0), true, true, false, true, false, nullptr,
                  nullptr, 0);
}

// §4.3 (PD2): demand slot 7 UpdateOccupantEffects(lot.GetBuildingType(false),
// &old, &new).
void UpdateOccupantEffects(void* u, uint64_t lot, float oldEfficiency, float newEfficiency) {
    cFC4ISC4Lot* l = LotOf(lot);
    cISC4City* c = AppCity();
    cISC4DemandSimulator* d = c ? c->GetDemandSimulator() : nullptr;
    if (l == nullptr || d == nullptr) return;
    const uint32_t bt = l->GetBuildingType(false);  // lot slot 46
    SC4Percentage a, b;
    a.percentage = oldEfficiency;
    b.percentage = newEfficiency;
    fc4::AsGame(d)->UpdateOccupantEffects(bt, a, b);
    ++Ctx(u)->effects;
}

// §3: MessageSend (synchronous) of 0x89EFA536 with data1 = key, data2 = value.
void RecordHistory(void* u, uint32_t key, int32_t value) {
    cIGZMessageServer2Ptr ms;
    if (!ms) return;
    cFC4Message2Standard* msg =
        FC4MakeMessage(kMsgHistoryRecord, static_cast<intptr_t>(key), static_cast<intptr_t>(value), 0);
    ms->MessageSend(msg);
    FC4_X87_CHECK("ports history MessageSend (after)");  // the game's handlers ran synchronously (FC4Fpu.h)
    msg->Release();
    ++Ctx(u)->histories;
}

// §3: city slot 81 GetAdvisorSystem, then PushEvent(event, null) (slot 15).
void PushEvent(void* u, uint32_t event) {
    cISC4City* c = AppCity();
    cISC4AdvisorSystem* as = c ? c->GetAdvisorSystem() : nullptr;
    if (as == nullptr) return;
    as->PushEvent(static_cast<int32_t>(event), nullptr);
    ++Ctx(u)->events;
}

cFC4ISC4TrafficSimulator* TrafficForVectors() {
    if (!fc4::StlNodeAllocator::Available()) {
        NoteOnce(g_noteNoTraffic,
                 "ports: the game's vector allocator is not available; the traffic simulator's subnetworks are not "
                 "asked (no office trips)");
        return nullptr;
    }
    return Traffic();
}

// §4.4: traffic slot 42 GetOccupantCountForAllSubnetworks(type, &counts).
void OfficeCounts(void*, uint32_t type, fc4_u32s_sink sink, void* ctx) {
    if (sink == nullptr) return;
    cFC4ISC4TrafficSimulator* t = TrafficForVectors();
    if (t == nullptr) return;
    fc4::GameVector<uint32_t> counts;
    t->GetOccupantCountForAllSubnetworks(type, counts);
    if (!counts.empty()) sink(ctx, counts.begin(), counts.size());
}

// §4.4: traffic slot 39 GetSubnetworksForLot(lot, &ids).
void LotSubnetworks(void*, uint64_t lot, fc4_u32s_sink sink, void* ctx) {
    if (sink == nullptr || lot == 0) return;
    cFC4ISC4TrafficSimulator* t = TrafficForVectors();
    if (t == nullptr) return;
    fc4::GameVector<uint32_t> ids;
    t->GetSubnetworksForLot(SdkLotOf(lot), ids);
    if (!ids.empty()) sink(ctx, ids.begin(), ids.size());
}

// §4.4 seaport: traffic slot 21 GetTrafficArrived(lot.AsPropertyHolder(), type).
uint32_t TrafficArrived(void*, uint64_t lot, uint32_t type) {
    cFC4ISC4Lot* l = LotOf(lot);
    cFC4ISC4TrafficSimulator* t = Traffic();
    cISCPropertyHolder* h = l ? l->AsPropertyHolder() : nullptr;  // lot slot 3
    return (t != nullptr && h != nullptr) ? t->GetTrafficArrived(h, type) : 0;
}

// §4.4 seaport: traffic slot 19 SetMaxTripCapacity(holder, type, capacity).
void SetMaxTripCapacity(void* u, uint64_t lot, uint32_t type, uint32_t capacity) {
    cFC4ISC4Lot* l = LotOf(lot);
    cFC4ISC4TrafficSimulator* t = Traffic();
    cISCPropertyHolder* h = l ? l->AsPropertyHolder() : nullptr;
    if (t == nullptr || h == nullptr) return;
    t->SetMaxTripCapacity(h, type, capacity);
    ++Ctx(u)->capacities;
}

// §4.8: terrain slots 7 and 8 (cells).
void TerrainSize(void*, uint32_t* width, uint32_t* depth) {
    cISTETerrain* t = Terrain();
    if (width != nullptr) *width = t ? t->CellCountX() : 0;
    if (depth != nullptr) *depth = t ? t->CellCountZ() : 0;
}

bool CellHasWater(void*, uint32_t x, uint32_t z) {
    cISTETerrain* t = Terrain();
    return t != nullptr && t->CellHasWater(x, z);  // slot 34
}

// §4.4 step 3: presentation only (the props' state 1). [choice] Not wired:
// logged once; the advisor event is still pushed by the core.
uint32_t SetAccessPropsState(void* u, uint64_t lot, uint32_t property, uint32_t state) {
    char b[200];
    snprintf(b, sizeof(b),
             "ports %s: a seaport lot %p has no sea access: its props with property 0x%08X are not switched to state "
             "%u (presentation; not wired)",
             fc4::PortKindName(Ctx(u)->kind), static_cast<void*>(FromToken(lot)), property, state);
    NoteOnce(g_noteAccessProps, b);
    return 0;
}

// §4.8 New seaport: SetAltitudeAtVertex (never reached with shipped data).
void SetAltitude(void*, int32_t x, int32_t z, float altitude) {
    if (cISTETerrain* t = Terrain()) t->SetAltitudeAtVertex(x, z, altitude);
}

std::string DateText(uint32_t julian) {
    if (julian == 0) return "0000-00-00";
    fc4_date d{};
    fc4_date_from_julian(static_cast<int32_t>(julian), &d);
    char b[24];
    snprintf(b, sizeof(b), "%04d-%02d-%02d", d.year, d.month, d.day_of_month);
    return b;
}

float FromBits(uint32_t b) {
    float f;
    std::memcpy(&f, &b, sizeof(f));
    return f;
}

uint32_t ToBits(float f) {
    uint32_t b;
    std::memcpy(&b, &f, sizeof(b));
    return b;
}

}  // namespace

namespace fc4 {

const char* PortKindName(uint32_t kind) { return kind == 0 ? "airport" : "seaport"; }

fc4_port_host MakePortHost(PortContext* ctx) {
    fc4_port_host h{};
    h.struct_size = sizeof(h);
    h.user = ctx;
    // Reads (both tables).
    h.exemplar = &Exemplar;
    h.department = &Department;
    h.line_expenses = &LineExpenses;
    h.occupant_is_preview = &OccupantIsPreview;
    h.occupant_type = &OccupantType;
    h.occupant_in_group = &OccupantInGroup;
    h.occupant_lot = &OccupantLot;
    h.lot_config = &LotConfig;
    h.lot_location = &LotLocation;
    h.lot_at = &LotAt;
    h.lot_state = &LotState;
    h.lot_condition = &LotCondition;
    h.lot_days_since_state_change = &LotDays;
    h.lot_rect = &LotRect;
    h.lot_facing = &LotFacing;
    h.lot_zone_compatible = &LotZoneCompatible;
    h.lot_configuration = &LotConfiguration;
    h.office_counts = &OfficeCounts;
    h.lot_subnetworks = &LotSubnetworks;
    h.traffic_arrived = &TrafficArrived;
    h.terrain_size = &TerrainSize;
    h.cell_has_water = &CellHasWater;
    if (ctx->readOnly) return h;
    // Writes.
    h.add_agent = &AddAgent;
    h.remove_agent = &RemoveAgent;
    h.create_department = &CreateDepartment;
    h.set_full_expenses = &SetFullExpenses;
    h.withdraw_funds = &WithdrawFunds;
    h.set_lot_condition = &SetLotCondition;
    h.start_demolish_lot = &StartDemolishLot;
    h.end_demolish_lot = &EndDemolishLot;
    h.create_lot = &CreateLot;
    h.set_lot_configuration = &SetLotConfiguration;
    h.construct_default_lot = &ConstructDefaultLot;
    h.dezone = &Dezone;
    h.update_occupant_effects = &UpdateOccupantEffects;
    h.record_history = &RecordHistory;
    h.push_event = &PushEvent;
    h.set_max_trip_capacity = &SetMaxTripCapacity;
    h.set_access_props_state = &SetAccessPropsState;
    h.set_altitude = &SetAltitude;
    return h;
}

}  // namespace fc4

// ---- the developer --------------------------------------------------------------------------------

template <class Primary>
cFC4PortDeveloperT<Primary>::cFC4PortDeveloperT(uint32_t kind) : kind_(kind) {
    InitializeCriticalSection(&lock_);
    mainTid_ = GetCurrentThreadId();  // the class store creates city components on the main thread
    ctx_.kind = kind;
    ctx_.target = static_cast<cIGZMessageTarget2*>(this);
    host_ = fc4::MakePortHost(&ctx_);
    core_ = fc4_port_developer_create(&host_, kind, TokenOf(PrimaryUnknown()));
    if (core_ == nullptr) fc4::Log("cFC4PortDeveloper(%s): core creation failed", fc4::PortKindName(kind));
    fc4::Log("cFC4PortDeveloper(%s, %p) created, core %p", fc4::PortKindName(kind), static_cast<void*>(PrimaryUnknown()),
             static_cast<void*>(core_));
}

template <class Primary>
cFC4PortDeveloperT<Primary>::~cFC4PortDeveloperT() {
    fc4_port_developer_destroy(core_);
    core_ = nullptr;
    ReleaseHolds();
    RemoveAgent(&ctx_, nullptr);
    fc4::Log("cFC4PortDeveloper(%s) destroyed (%u months, %u fallbacks)", fc4::PortKindName(kind_), months_,
             fallbacks_.load());
    DeleteCriticalSection(&lock_);
}

// §1: the core's offset (0, 4, 8, 0xC); anything else is false and the out
// pointer is left untouched. QueryInterface(1) gives +4 [compat].
template <class Primary>
bool cFC4PortDeveloperT<Primary>::QueryInterface(uint32_t riid, void** ppvObj) {
    uint32_t off = 0;
    if (ppvObj == nullptr || !fc4_port_developer_query_interface(kind_, riid, &off)) return false;
    switch (off) {
        case 0:
            *ppvObj = static_cast<Primary*>(this);
            break;
        case 4:
            *ppvObj = static_cast<cFC4ISC4PortSubObject*>(this);
            break;
        case 8:
            *ppvObj = static_cast<cIGZMessageTarget2*>(this);
            break;
        case 0xC:
            *ppvObj = static_cast<cIGZSerializable*>(this);
            break;
        default:
            return false;
    }
    ++refCount_;
    return true;
}

template <class Primary>
uint32_t cFC4PortDeveloperT<Primary>::AddRef() {
    return ++refCount_;
}

// §1: the release of the last reference calls Shutdown and the destructor
// (at a count of 1 or below, returning 0).
template <class Primary>
uint32_t cFC4PortDeveloperT<Primary>::Release() {
    if (refCount_ > 1) return --refCount_;
    refCount_ = 0;
    if (destroying_) return 0;
    destroying_ = true;
    Shutdown();
    delete this;
    return 0;
}

// ---- the lock -------------------------------------------------------------------------------------

template <class Primary>
bool cFC4PortDeveloperT<Primary>::Enter(bool wait) {
    const DWORD me = GetCurrentThreadId();
    if (wait || mainTid_ == 0 || me == mainTid_) {
        EnterCriticalSection(&lock_);
        return true;
    }
    if (TryEnterCriticalSection(&lock_)) return true;
    const ULONGLONG until = GetTickCount64() + kLockWaitMs;
    while (GetTickCount64() < until) {
        Sleep(1);
        if (TryEnterCriticalSection(&lock_)) return true;
    }
    ++fallbacks_;
    return false;
}

template <class Primary>
void cFC4PortDeveloperT<Primary>::Leave() {
    LeaveCriticalSection(&lock_);
}

template <class Primary>
cFC4PortDeveloperT<Primary>::Section::Section(const cFC4PortDeveloperT* s, bool wait)
    : s_(const_cast<cFC4PortDeveloperT*>(s)), ok_(s_->Enter(wait)) {
    if (ok_) ++s_->depth_;
}

template <class Primary>
cFC4PortDeveloperT<Primary>::Section::~Section() {
    if (!ok_) return;
    if (--s_->depth_ == 0) s_->AfterCall();
    s_->Leave();
}

// The outermost call: the holds follow the core's lists (main thread only:
// the game's reference counts are not atomic), and the values other threads
// fall back to are refreshed (the core's own state; no host calls).
template <class Primary>
void cFC4PortDeveloperT<Primary>::AfterCall() {
    if (core_ == nullptr) return;
    if (GetCurrentThreadId() == mainTid_) SyncHolds();
    snap_.efficiencyBits = ToBits(fc4_port_developer_avg_efficiency(core_));
    snap_.trips = fc4_port_developer_num_trips(core_);
    snap_.ports = fc4_port_developer_num_ports(core_, 0xFFFFFFFFu, 0xFFFFFFFFu);
    snap_.sea = fc4_port_developer_city_has_sea(core_);
}

// §2: one reference per port lot the core lists (a lot listed twice is held
// twice) and one on the department.
template <class Primary>
void cFC4PortDeveloperT<Primary>::SyncHolds() {
    // CreateLot's references for the caller are holds already.
    for (uint64_t t : ctx_.createdLots) ++heldLots_[t];
    ctx_.createdLots.clear();
    std::map<uint64_t, uint32_t> want;
    fc4_port_lot l{};
    l.struct_size = sizeof(l);
    for (uint32_t i = 0; fc4_port_developer_get_port(core_, i, &l); ++i) {
        if (l.lot != 0) ++want[l.lot];
    }
    for (auto it = heldLots_.begin(); it != heldLots_.end();) {
        const uint32_t w = want.count(it->first) ? want[it->first] : 0;
        while (it->second > w) {
            DropObject(FromToken(it->first));
            --it->second;
        }
        it = it->second == 0 ? heldLots_.erase(it) : std::next(it);
    }
    for (const auto& [t, w] : want) {
        uint32_t& h = heldLots_[t];
        while (h < w) {
            HoldObject(FromToken(t));
            ++h;
        }
    }
    fc4_port_state st{};
    st.struct_size = sizeof(st);
    const uint64_t dept = fc4_port_developer_get_state(core_, &st) ? st.department : 0;
    if (dept != heldDepartment_) {
        HoldObject(FromToken(dept));
        DropObject(FromToken(heldDepartment_));
        heldDepartment_ = dept;
    }
}

template <class Primary>
void cFC4PortDeveloperT<Primary>::ReleaseHolds() {
    for (uint64_t t : ctx_.createdLots) ++heldLots_[t];
    ctx_.createdLots.clear();
    for (auto& [t, n] : heldLots_) {
        for (; n > 0; --n) DropObject(FromToken(t));
    }
    heldLots_.clear();
    DropObject(FromToken(heldDepartment_));
    heldDepartment_ = 0;
}

// [inferred] §3's subscriptions, made at Init and removed at Shutdown.
template <class Primary>
void cFC4PortDeveloperT<Primary>::Subscribe(bool on) {
    std::vector<uint32_t> ids = {kMsgOccupantInserted, kMsgLotStateChanged, kMsgPostCityInit};
    if (kind_ == 0) {
        ids.insert(ids.end(), std::begin(kAirportGrowth), std::end(kAirportGrowth));
    } else {
        ids.push_back(kSeaportGrowth);
        ids.push_back(kMsgTerrainChanged);
    }
    cIGZMessageTarget2* target = static_cast<cIGZMessageTarget2*>(this);
    if (on) {
        if (messageServer_ != nullptr) return;
        cIGZMessageServer2Ptr ms;
        cIGZMessageServer2* p = ms;
        if (p == nullptr) {
            fc4::Log("ports %s: no message server; nothing subscribed", fc4::PortKindName(kind_));
            return;
        }
        HoldObject(p);
        messageServer_ = p;
        for (uint32_t id : ids) {
            if (messageServer_->AddNotification(target, id)) ++subscriptions_;
        }
    } else {
        if (messageServer_ == nullptr) return;
        for (uint32_t id : ids) {
            if (messageServer_->RemoveNotification(target, id) && subscriptions_ > 0) --subscriptions_;
        }
        DropObject(messageServer_);
        messageServer_ = nullptr;
    }
}

template <class Primary>
void cFC4PortDeveloperT<Primary>::LogState(const char* what, bool ok) {
    fc4_port_state st{};
    st.struct_size = sizeof(st);
    if (core_ == nullptr || !fc4_port_developer_get_state(core_, &st)) {
        fc4::Log("ports %s %s: ok=%d (no state)", fc4::PortKindName(kind_), what, ok ? 1 : 0);
        return;
    }
    size_t held = 0;
    for (const auto& kv : heldLots_) held += kv.second;
    fc4::Log("ports %s %s: ok=%d init=%u tuning=0x%08X loaded=%u types=%u/%u ports=%u zone=%u dept=%p sea=%u "
             "subscriptions=%u agent=%d held=%u fallbacks=%u",
             fc4::PortKindName(kind_), what, ok ? 1 : 0, st.initialized, st.tuning_instance, st.tuning_loaded,
             st.types_loaded, st.type_count, st.port_count, unsigned{st.zone_type},
             static_cast<void*>(FromToken(st.department)), st.city_has_sea, subscriptions_,
             ctx_.agentRegistered ? 1 : 0, static_cast<unsigned>(held), fallbacks_.load());
}

template <class Primary>
void cFC4PortDeveloperT<Primary>::LogMonth(uint32_t julian) {
    fc4_port_state st{};
    st.struct_size = sizeof(st);
    if (!fc4_port_developer_get_state(core_, &st)) return;
    fc4::Log("ports %s month %s: ports=%u ran=%u skipped=%u history=%d event=0x%08X avg=%.9g trips=%u fr=%.9g "
             "required=%lld writes: conditions=%u effects=%u capacities=%u swaps=%u withdrawals=%u dezones=%u "
             "histories=%u events=%u",
             fc4::PortKindName(kind_), DateText(julian).c_str(), st.port_count, st.month_ran, st.month_skipped,
             st.month_history, st.month_event, static_cast<double>(fc4_port_developer_avg_efficiency(core_)),
             fc4_port_developer_num_trips(core_), static_cast<double>(fc4_port_developer_funding_ratio(core_)),
             static_cast<long long>(fc4_port_developer_required_budget(core_)), ctx_.conditions, ctx_.effects,
             ctx_.capacities, ctx_.swaps, ctx_.withdrawals, ctx_.dezones, ctx_.histories, ctx_.events);
}

// ---- lifecycle ------------------------------------------------------------------------------------

template <class Primary>
bool cFC4PortDeveloperT<Primary>::Init() {
    FC4_TRACE("PortDeveloper::Init");
    mainTid_ = GetCurrentThreadId();
    bool ok = false;
    {
        Section sec(this);
        ok = fc4_port_developer_init(core_);  // tuning, department, agent; the seaport's map
        Subscribe(true);
        LogState("Init", ok);
    }
    return true;  // §8.1: AL
}

template <class Primary>
bool cFC4PortDeveloperT<Primary>::Shutdown() {
    FC4_TRACE("PortDeveloper::Shutdown");
    // Keep this object alive while the simulator and the message server drop
    // their references (unless this already is the final Release).
    if (!destroying_) PrimaryUnknown()->AddRef();
    {
        Section sec(this);
        Subscribe(false);
        if (core_ != nullptr) fc4_port_developer_shutdown(core_);  // removes the agent
        LogState("Shutdown", true);
    }
    if (!destroying_) PrimaryUnknown()->Release();
    return true;
}

// ---- slots 5-9 and 11-16 (§4.7) -------------------------------------------------------------------

// Slot 5: by-value SC4Percentage through the hidden result pointer.
template <class Primary>
SC4Percentage* cFC4PortDeveloperT<Primary>::GetAvgEfficiency(SC4Percentage* out) {
    Section sec(this);
    float v = FromBits(snap_.efficiencyBits.load());
    if (sec.ok() && core_ != nullptr) v = fc4_port_developer_avg_efficiency(core_);
    if (out != nullptr) out->percentage = v;
    return out;
}

template <class Primary>
uint32_t cFC4PortDeveloperT<Primary>::GetSimulatorType() {
    return kSimulatorType[kind_ & 1];
}

// [choice] (§4.7, PD1) the minimum efficiency (first on ties), not the
// original's endless loop with two or more ports.
template <class Primary>
bool cFC4PortDeveloperT<Primary>::GetLeastEfficientPort(SC4Percentage& pct, SC4Point<int32_t>& point) {
    Section sec(this);
    if (!sec.ok() || core_ == nullptr) return false;
    float e = 0.0f;
    int16_t x = 0, z = 0;
    if (!fc4_port_developer_least_efficient(core_, &e, &x, &z)) return false;
    pct.percentage = e;
    point.x = x;
    point.y = z;
    return true;
}

// Slot 8: appends every port's lot (resolving it), one reference each.
template <class Primary>
void cFC4PortDeveloperT<Primary>::GetAllPortLots(fc4::GameVector<cISC4Lot*>& lots) {
    Section sec(this);
    if (!sec.ok() || core_ == nullptr) return;
    const size_t n = fc4_port_developer_all_port_lots(core_, nullptr, 0);
    if (n == 0) return;
    std::vector<uint64_t> tokens(n);
    fc4_port_developer_all_port_lots(core_, tokens.data(), tokens.size());
    for (uint64_t t : tokens) {
        cISC4Lot* lot = SdkLotOf(t);
        if (!lots.push_back(lot)) break;
        HoldObject(reinterpret_cast<cIGZUnknown*>(lot));
    }
}

// Slot 9: size bit 1 << size class (§4.7).
template <class Primary>
uint32_t cFC4PortDeveloperT<Primary>::GetNumPorts(int32_t sizeFlags, int32_t stageFlags) {
    Section sec(this);
    if (!sec.ok() || core_ == nullptr) return 0;
    return fc4_port_developer_num_ports(core_, static_cast<uint32_t>(sizeFlags), static_cast<uint32_t>(stageFlags));
}

template <class Primary>
uint32_t cFC4PortDeveloperT<Primary>::PortCount() {
    Section sec(this);
    if (!sec.ok() || core_ == nullptr) return snap_.ports.load();
    return kind_ == 0 ? fc4_port_developer_num_airports(core_, 0) : fc4_port_developer_num_seaports(core_);
}

template <class Primary>
bool cFC4PortDeveloperT<Primary>::UsageValues(cISC4Lot* lot, uint32_t& usage, uint32_t& capacity) {
    Section sec(this);
    if (!sec.ok() || core_ == nullptr) {
        usage = 0;
        capacity = 0;
        return false;
    }
    return fc4_port_developer_usage_values(core_, TokenOf(lot), &usage, &capacity);
}

template <class Primary>
uint32_t cFC4PortDeveloperT<Primary>::NumTrips() {
    Section sec(this);
    if (!sec.ok() || core_ == nullptr) return snap_.trips.load();
    return fc4_port_developer_num_trips(core_);
}

template <class Primary>
int64_t cFC4PortDeveloperT<Primary>::Maintenance(cISC4Lot* lot) {
    Section sec(this);
    if (!sec.ok() || core_ == nullptr) return 0;
    return fc4_port_developer_maintenance_cost(core_, TokenOf(lot));
}

uint32_t cFC4AirportDeveloper::GetNumAirports(int32_t size) {
    if (size == 0) return PortCount();
    Section sec(this);
    if (!sec.ok() || core_ == nullptr) return 0;
    return fc4_port_developer_num_airports(core_, static_cast<uint32_t>(size));
}

bool cFC4SeaportDeveloper::CityHasSea() {
    Section sec(this);
    if (!sec.ok() || core_ == nullptr) return snap_.sea.load();
    return fc4_port_developer_city_has_sea(core_);
}

uint8_t cFC4SeaportDeveloper::GetCellSeaAccess(int32_t x, int32_t z) {
    Section sec(this);
    if (!sec.ok() || core_ == nullptr) return 0;
    return fc4_port_developer_cell_sea_access(core_, x, z);
}

// ---- messages (§3) --------------------------------------------------------------------------------

template <class Primary>
bool cFC4PortDeveloperT<Primary>::DoMessage(cIGZMessage2* pMessage) {
    if (pMessage == nullptr || core_ == nullptr) return true;
    FC4_X87_SCOPE("ports DoMessage", pMessage->GetType());  // x87 leftover guard (FC4Fpu.h)
    if (mainTid_ != 0 && GetCurrentThreadId() != mainTid_) {
        const uint32_t n = ++offThreadMessages_;
        if (n <= 8) {
            fc4::Log("ports %s: DoMessage 0x%08X on thread %lu (not the main thread); it waits for the lock",
                     fc4::PortKindName(kind_), pMessage->GetType(), static_cast<unsigned long>(GetCurrentThreadId()));
        }
    }
    Section sec(this, true);
    const uint32_t type = pMessage->GetType();
    uint64_t data = 0;
    uint32_t oldState = 0, newState = 0, julian = 0;
    cIGZMessage2Standard* s = Query<cIGZMessage2Standard>(pMessage, GZIID_cIGZMessage2Standard);
    cIGZUnknown* heldForCall = nullptr;
    if (s != nullptr) {
        if (type == kMsgOccupantInserted) {
            // void1 = the occupant (held for the call), as its cISC4Occupant.
            cISC4Occupant* o = Query<cISC4Occupant>(static_cast<cIGZUnknown*>(s->GetVoid1()), kIID_cISC4Occupant);
            data = TokenOf(o);
            heldForCall = reinterpret_cast<cIGZUnknown*>(o);
        } else if (type == kMsgLotStateChanged) {
            // The lot = IGZUnknown queried for cISC4Lot; old = data1, new = data2.
            cISC4Lot* l = Query<cISC4Lot>(s->GetIGZUnknown(), kIID_cISC4Lot);
            data = TokenOf(l);
            heldForCall = reinterpret_cast<cIGZUnknown*>(l);
            oldState = static_cast<uint32_t>(s->GetData1());
            newState = static_cast<uint32_t>(s->GetData2());
        } else if (type == kMsgSimNewMonth) {
            julian = static_cast<uint32_t>(s->GetData1());
        }
    }
    const fc4::PortContext before = ctx_;
    fc4_port_developer_do_message(core_, type, data, oldState, newState);
    if (type == kMsgSimNewMonth) {
        ++months_;
        fc4_port_state st{};
        st.struct_size = sizeof(st);
        // A month with ports, or the first one: the status line.
        if (fc4_port_developer_get_state(core_, &st) && (st.port_count > 0 || months_ == 1)) LogMonth(julian);
    } else if (type != kMsgOccupantInserted && type != kMsgLotStateChanged) {
        fc4::Log("ports %s: message 0x%08X handled", fc4::PortKindName(kind_), type);
    } else if (ctx_.dezones != before.dezones || ctx_.swaps != before.swaps) {
        LogState("lot change", true);
    }
    Drop(heldForCall);
    Drop(s);
    return true;  // §3: every handler returns true
}

// ---- persistence (§7) -----------------------------------------------------------------------------

template <class Primary>
bool cFC4PortDeveloperT<Primary>::Write(cIGZOStream& s) {
    FC4_TRACE("PortDeveloper::Write");
    Section sec(this, true);
    fc4::OStreamAdapter a(s);
    const bool ok = core_ != nullptr && fc4_port_developer_write(core_, a.Get());
    fc4::Log("ports %s Write -> %d (ports=%u)", fc4::PortKindName(kind_), ok ? 1 : 0,
             core_ ? fc4_port_developer_num_ports(core_, 0xFFFFFFFFu, 0xFFFFFFFFu) : 0u);
    return ok;
}

// Runs before Init: the ports are appended with their lots to look up (§7).
template <class Primary>
bool cFC4PortDeveloperT<Primary>::Read(cIGZIStream& s) {
    FC4_TRACE("PortDeveloper::Read");
    Section sec(this, true);
    fc4::IStreamAdapter a(s);
    const bool ok = core_ != nullptr && fc4_port_developer_read(core_, a.Get());
    LogState("Read", ok);
    return ok;
}

template <class Primary>
uint32_t cFC4PortDeveloperT<Primary>::GetGZCLSID() {
    return kind_ == 0 ? kFC4_CLSID_cSC4AirportDeveloper : kFC4_CLSID_cSC4SeaportDeveloper;
}

template class cFC4PortDeveloperT<cFC4ISC4AirportDeveloper>;
template class cFC4PortDeveloperT<cFC4ISC4SeaportDeveloper>;

// ---- factories ------------------------------------------------------------------------------------

cIGZUnknown* CreateFC4AirportDeveloper() {
    return static_cast<cIGZUnknown*>(static_cast<cFC4ISC4AirportDeveloper*>(new cFC4AirportDeveloper()));
}

cIGZUnknown* CreateFC4SeaportDeveloper() {
    return static_cast<cIGZUnknown*>(static_cast<cFC4ISC4SeaportDeveloper*>(new cFC4SeaportDeveloper()));
}

// ---- tooling: the harness script's `port` action (cFC4PortDeveloper.h) ----------------------------
std::string PortScriptBuild(uint32_t kind, uint32_t type, int32_t x, int32_t z, uint32_t facing) {
    char b[256];
    if (kind > 1) return "skipped: kind is not 0 (airport) or 1 (seaport)";
    // §2: the tuning exemplar instance of each developer.
    const uint32_t tuning = kind == 0 ? 0xCA270E9Du : 0xEA29AD92u;
    std::vector<uint32_t> types;
    std::vector<uint32_t> configs;
    {
        fc4::props::Exemplar ex;
        if (!ex.Load(kTuningType, kTuningGroup, tuning) || !fc4::props::GetU32s(ex.Holder(), 0x8A270FC3, types)) {
            return "skipped: no port type list in the tuning exemplar";
        }
    }
    if (type >= types.size()) {
        std::snprintf(b, sizeof(b), "skipped: type %u of %u", type, static_cast<unsigned>(types.size()));
        return b;
    }
    {
        fc4::props::Exemplar ex;
        if (!ex.Load(kTuningType, kTuningGroup, types[type]) ||
            !fc4::props::GetU32s(ex.Holder(), 0xCA271781, configs) || configs.empty()) {
            std::snprintf(b, sizeof(b), "skipped: type exemplar 0x%08X has no lot configuration list", types[type]);
            return b;
        }
    }
    const uint32_t cfgId = configs[0];
    cISC4City* c = AppCity();
    cISC4LotConfigurationManager* m = c ? c->GetLotConfigurationManager() : nullptr;  // borrowed
    cISC4LotConfiguration* cfg = m ? fc4::AsGame(m)->GetLotConfiguration(cfgId) : nullptr;  // no reference
    cFC4ISC4LotManager* lm = LotManager();
    cFC4ISC4LotDeveloper* ld = LotDeveloper();
    if (cfg == nullptr || lm == nullptr || ld == nullptr) {
        std::snprintf(b, sizeof(b), "skipped: cfg 0x%08X %s, lot manager %s, lot developer %s", cfgId,
                      cfg ? "ok" : "missing", lm ? "ok" : "missing", ld ? "ok" : "missing");
        return b;
    }
    uint32_t w = 0;
    uint32_t d = 0;
    cfg->GetSize(w, d);  // lot configuration slot 7
    // [choice] tooling: a script lists several candidate sites; one port of
    // each kind per session, and a site with any lot on it is skipped
    // (lot manager slot 6, borrowed).
    static bool s_built[2] = {false, false};
    if (s_built[kind]) return "skipped: a port of this kind was built already";
    for (uint32_t i = 0; i < w; ++i) {
        for (uint32_t j = 0; j < d; ++j) {
            if (lm->GetLot(x + static_cast<int32_t>(i), z + static_cast<int32_t>(j), false) != nullptr) {
                std::snprintf(b, sizeof(b), "skipped: cell (%u, %u) of the %ux%u site at (%d, %d) has a lot",
                              x + i, z + j, w, d, x, z);
                return b;
            }
        }
    }
    cISC4Lot* lot = nullptr;
    if (!lm->CreateLot(x, z, static_cast<int32_t>(w), static_cast<int32_t>(d), static_cast<int32_t>(facing), lot) ||
        lot == nullptr) {
        std::snprintf(b, sizeof(b), "failed: CreateLot(%d, %d, %u, %u, %u) for cfg 0x%08X (type 0x%08X)", x, z, w,
                      d, facing, cfgId, types[type]);
        return b;
    }
    cFC4ISC4Lot* l = fc4::AsGame(lot);
    l->SetLotConfiguration(cfg);  // slot 10
    l->UpdateZoneType();          // slot 32
    const bool built = ld->ConstructDefaultLot(lot, cfg, 0, true);  // slot 7
    s_built[kind] = built;
    std::snprintf(b, sizeof(b), "ok=%d: cfg 0x%08X (type 0x%08X) lot %ux%u at (%d, %d) facing %u", built ? 1 : 0,
                  cfgId, types[type], w, d, x, z, facing);
    // CreateLot's reference is the caller's (zones.md §8.5): acquired by us
    // (as the swap's CreateLot host above), dropped here; the lot manager
    // keeps its own.
    if (!fc4::IsOurObject(lot)) FC4_ACQUIRED(reinterpret_cast<cIGZUnknown*>(lot), "CreateLot");
    FC4_RELEASE(reinterpret_cast<cIGZUnknown*>(lot));
    return b;
}
