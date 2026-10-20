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

// The tract developer's host callbacks (fc4.h fc4_tract_host) over the
// game's interfaces, in the order of docs/specs/tract-developer.md §8, and
// the configuration filter (§7.4). Every token handed to the core is
// BORROWED (fc4.h): references the game's lists give are kept in the context
// until the outermost call into the core returns (TractReleaseHeld).
//
// Main thread, or another thread holding the tract developer's lock; the
// shadow (FC4TractShadow.cpp) runs on the main thread.

#include "cFC4TractDeveloper.h"

#include "FC4Config.h"
#include "FC4CoreProps.h"
#include "FC4DevelopmentInterfaces.h"
#include "FC4Fpu.h"
#include "FC4GameList.h"
#include "FC4GameMap.h"
#include "FC4Interfaces.h"
#include "FC4Log.h"
#include "FC4Properties.h"
#include "FC4RefAudit.h"
#include "FC4Streams.h"
#include "FC4TractInterfaces.h"

#include "GZServPtrs.h"
#include "SC4List.h"
#include "SC4Rect.h"
#include "cGZPersistResourceKey.h"
#include "cIGZApp.h"
#include "cIGZFrameWork.h"
#include "cIGZMessageServer2.h"
#include "cISC4App.h"
#include "cISC4AuraSimulator.h"
#include "cISC4BuildingDevelopmentSimulator.h"
#include "cISC4BuildingOccupant.h"
#include "cISC4City.h"
#include "cISC4Demolition.h"
#include "cISC4FireProtectionSimulator.h"
#include "cISC4GrowthDeveloper.h"
#include "cISC4LandValueSimulator.h"
#include "cISC4Lot.h"
#include "cISC4LotConfiguration.h"
#include "cISC4LotConfigurationManager.h"
#include "cISC4LotDeveloper.h"
#include "cISC4LotManager.h"
#include "cISC4NetworkOccupant.h"
#include "cISC4PlumbingSimulator.h"
#include "cISC4PowerSimulator.h"
#include "cISC4SimGrid.h"
#include "cISC4TrafficSimulator.h"
#include "cISC4ZoneManager.h"
#include "cISTETerrain.h"
#include "cRZBaseString.h"
#include "cSC4LotConfigurationObject.h"

#include <algorithm>
#include <atomic>
#include <cstring>
#include <mutex>
#include <unordered_map>
#include <vector>

using fc4::FromToken;
using fc4::TokenOf;

namespace {

// §1.3 step 4: the agent's name and flags.
constexpr const char* kAgentName = "cSC4TractDeveloper";
constexpr uint32_t kAgentFlags = 2;
// §4.10.2: FindNetworkTile(x, z, 0xC4B, 1).
constexpr uint32_t kNetworkTileMask = 0xC4B;
// §4.15.1: DemolishLots(true, [lot], 0, 0x800, false, 0, 0, 0).
constexpr uint32_t kDemolishFlags = 0x800;
// §4.6 / §4.16: the local hash sets are built with bucket hint 100 (193 buckets).
constexpr uint32_t kSetHint = 100;
// Building exemplar properties (§4.15.2, §4.16, §4.17).
constexpr uint32_t kPropOccupantGroups = 0xAA1DD396;
constexpr uint32_t kPropMonopoly = 0x2781283F;
constexpr uint32_t kPropFieldIds = 0x29B55F73;
// A style map has a handful of entries; a longer walk is not a style map.
constexpr uint32_t kMaxStyleEntries = 4096;
// The most nodes a lot list may grow to in one call (a guard against a
// corrupted list; a city has far fewer lots).
constexpr uint32_t kMaxListNodes = 1u << 20;

fc4::TractContext* Ctx(void* u) { return static_cast<fc4::TractContext*>(u); }
cISC4City* City(void* u) { return Ctx(u)->city; }

// The tract_* switches; the shadow's context reads the game as it is.
bool Gate(void* u, fc4::Sw s, const char* what) {
    if (!Ctx(u)->gated || fc4::On(s)) return true;
    fc4::NoteSkipped(s, what);
    return false;
}

// Writes reach the game only from the replacement (the shadow core swallows
// them before they reach the table, and its table has them NULL anyway).
bool Writes(void* u) { return !Ctx(u)->readOnly; }

// Test seam op 45: a callback of the test table answers instead of the game.
#define TRACT_TEST(name, ...)                                                                  \
    do {                                                                                       \
        const fc4_tract_host* t_ = Ctx(u)->test;                                               \
        if (t_ != nullptr && t_->name != nullptr) return t_->name(t_->user, __VA_ARGS__);      \
    } while (0)
#define TRACT_TEST0(name)                                                                      \
    do {                                                                                       \
        const fc4_tract_host* t_ = Ctx(u)->test;                                               \
        if (t_ != nullptr && t_->name != nullptr) return t_->name(t_->user);                   \
    } while (0)

// Main thread only (host callbacks run under the tract developer's lock or
// in the shadow's comparison).
void NoteOnce(bool& done, const char* text) {
    if (done) return;
    done = true;
    fc4::Log("%s", text);
}
bool g_noteNoAllocator = false;
bool g_noteRoads = false;
bool g_noteStyleMap = false;
bool g_noteFamilySet = false;

// The app's current city (borrowed).
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

// A service still null takes `value` with a reference of its own.
template <class T>
void TakeIfNull(T*& slot, T* value) {
    if (slot != nullptr || value == nullptr) return;
    slot = value;
    FC4_ADDREF(slot);
}

template <class T>
void ReleaseSlot(T*& slot) {
    if (slot != nullptr) FC4_RELEASE(slot);
    slot = nullptr;
}

cFC4ISC4BuildingDevelopmentSimulator* Bds(void* u) {
    cISC4BuildingDevelopmentSimulator* b = Ctx(u)->bds;
    if (b == nullptr && Ctx(u)->readOnly && City(u) != nullptr) b = City(u)->GetBuildingDevelopmentSimulator();
    return b ? fc4::AsGame(b) : nullptr;
}

cFC4ISC4GrowthDeveloper* DevOf(uint64_t t) {
    return fc4::AsGame(reinterpret_cast<cISC4GrowthDeveloper*>(FromToken(t)));
}
cFC4ISC4Lot* LotOf(uint64_t t) { return fc4::AsGame(reinterpret_cast<cISC4Lot*>(FromToken(t))); }
cISC4Lot* SdkLotOf(uint64_t t) { return reinterpret_cast<cISC4Lot*>(FromToken(t)); }
cISC4LotConfiguration* CfgOf(uint64_t t) { return reinterpret_cast<cISC4LotConfiguration*>(FromToken(t)); }

// ---- lifecycle (§1.3, §1.4) --------------------------------------------------------------------------

// §1.3 step 3: the city and the nine services, each with a reference. The
// original reads them from the city's globals; FreeCity4 asks the app's
// city. The lot developer is the BDS's (slot 36, cached there).
bool AcquireServices(void* u) {
    TRACT_TEST0(acquire_services);
    fc4::TractContext* c = Ctx(u);
    if (c->readOnly) return false;
    TakeIfNull(c->city, AppCity());
    if (c->city == nullptr) {
        fc4::Log("tract: no city; services not acquired");
        return false;
    }
    TakeIfNull(c->bds, c->city->GetBuildingDevelopmentSimulator());
    TakeIfNull(c->landValue, c->city->GetLandValueSimulator());
    if (c->bds != nullptr) TakeIfNull(c->lotDeveloper, fc4::AsGame(c->bds)->GetLotDeveloper());  // BDS slot 36
    TakeIfNull(c->configs, c->city->GetLotConfigurationManager());  // city slot 37
    TakeIfNull(c->lots, c->city->GetLotManager());
    TakeIfNull(c->plumbing, c->city->GetPlumbingSimulator());
    TakeIfNull(c->power, c->city->GetPowerSimulator());
    TakeIfNull(c->traffic, c->city->GetTrafficSimulator());
    TakeIfNull(c->zones, c->city->GetZoneManager());
    const bool ok = c->bds && c->landValue && c->lotDeveloper && c->configs && c->lots && c->plumbing && c->power &&
                    c->traffic && c->zones;
    if (!ok) {
        fc4::Log("tract: a service is missing (bds=%p landValue=%p lotDeveloper=%p configs=%p lots=%p plumbing=%p "
                 "power=%p traffic=%p zones=%p)",
                 static_cast<void*>(c->bds), static_cast<void*>(c->landValue), static_cast<void*>(c->lotDeveloper),
                 static_cast<void*>(c->configs), static_cast<void*>(c->lots), static_cast<void*>(c->plumbing),
                 static_cast<void*>(c->power), static_cast<void*>(c->traffic), static_cast<void*>(c->zones));
    }
    return ok;
}

// §1.3 step 4: simulator slot 19 AddAgent(target, level, "cSC4TractDeveloper", 2).
// [choice] tract_agents off: not registered, reported as done (no Year
// message then reaches NewYear).
bool AddAgent(void* u, uint32_t level, uint32_t message) {
    TRACT_TEST(add_agent, level, message);
    fc4::TractContext* c = Ctx(u);
    if (c->readOnly) return false;
    if (!Gate(u, fc4::Sw::TractAgents, "tract AddAgent (not registered)")) return true;
    cFC4ISC4Simulator* sim = c->city ? fc4::AsGame(c->city->GetSimulator()) : nullptr;  // city slot 50
    if (sim == nullptr || c->target == nullptr) {
        fc4::Log("tract: no simulator; agent level %u not registered", level);
        return false;
    }
    const cRZBaseString name(kAgentName);
    if (!sim->AddAgent(c->target, level, name, kAgentFlags)) {  // slot 19
        fc4::Log("tract: AddAgent(level %u, message 0x%08X) returned false", level, message);
    }
    ++c->agentLevels;
    return true;
}

// §1.3 step 5.
bool AcquireMessageServer(void* u) {
    TRACT_TEST0(acquire_message_server);
    fc4::TractContext* c = Ctx(u);
    if (c->readOnly) return false;
    if (c->msgServer == nullptr) {
        cIGZMessageServer2Ptr ms;
        if (ms) {
            c->msgServer = ms;
            FC4_ADDREF(c->msgServer);
        }
    }
    return c->msgServer != nullptr;
}

void ReleaseMessageServer(void* u) {
    if (Ctx(u)->test != nullptr && Ctx(u)->test->release_message_server != nullptr) {
        Ctx(u)->test->release_message_server(Ctx(u)->test->user);
        return;
    }
    if (Ctx(u)->readOnly) return;
    ReleaseSlot(Ctx(u)->msgServer);
}

// §1.4 step 2: simulator slot 20 RemoveAgent (all levels).
void RemoveAgent(void* u) {
    if (Ctx(u)->test != nullptr && Ctx(u)->test->remove_agent != nullptr) {
        Ctx(u)->test->remove_agent(Ctx(u)->test->user);
        return;
    }
    fc4::TractContext* c = Ctx(u);
    if (c->readOnly || c->agentLevels == 0) return;
    cFC4ISC4Simulator* sim = c->city ? fc4::AsGame(c->city->GetSimulator()) : nullptr;
    if (sim != nullptr && c->target != nullptr) sim->RemoveAgentAllLevels(c->target);  // slot 20
    c->agentLevels = 0;
}

void ReleaseServicesCb(void* u) {
    if (Ctx(u)->test != nullptr && Ctx(u)->test->release_services != nullptr) {
        Ctx(u)->test->release_services(Ctx(u)->test->user);
        return;
    }
    fc4::TractReleaseServices(Ctx(u));
}

// §4.21: the "Building Development" exemplar.
fc4_props* Tuning(void* u) {
    TRACT_TEST0(tuning);
    if (!Gate(u, fc4::Sw::ExemplarLoads, "tract tuning exemplar load (none)")) return nullptr;
    fc4::props::Exemplar ex;
    if (!ex.Load(FC4_TRACT_TUNING_TYPE, FC4_TRACT_TUNING_GROUP, FC4_TRACT_TUNING_INSTANCE)) {
        if (!Ctx(u)->readOnly) fc4::Log("tract: tuning exemplar 0x%08X unavailable", FC4_TRACT_TUNING_INSTANCE);
        return nullptr;
    }
    return fc4::props::ToCoreAll(ex.Holder());
}

// ---- growth developer (growth-developer.md §10.1) ----------------------------------------------------

int32_t DevTargetZone(void* u, uint64_t dev) {
    TRACT_TEST(dev_target_zone, dev);
    return dev ? DevOf(dev)->GetType() : 0;  // slot 5
}
uint32_t DevSubtype(void* u, uint64_t dev) {
    TRACT_TEST(dev_subtype, dev);
    return dev ? DevOf(dev)->GetSubType() : 0;  // slot 6
}
uint32_t DevPurpose(void* u, uint64_t dev) {
    TRACT_TEST(dev_purpose, dev);
    return dev ? DevOf(dev)->GetPurpose() : 0;  // slot 7
}
uint32_t DevWealth(void* u, uint64_t dev) {
    TRACT_TEST(dev_wealth, dev);
    return dev ? DevOf(dev)->GetWealth() : 0;  // slot 8
}

// Slot 9 GetConstructionInfo(profile&) into a zeroed profile (the growth
// developer writes its first 0x18 bytes; growth-developer §6.2).
void DevConstructionInfo(void* u, uint64_t dev, fc4_tract_construction* out) {
    TRACT_TEST(dev_construction_info, dev, out);
    if (out == nullptr) return;
    *out = fc4_tract_construction{};
    if (dev == 0) return;
    FC4ConstructionProfile p{};
    DevOf(dev)->GetConstructionInfo(p);
    out->zone = p.zone;
    out->unknown2 = p.unknown2;
    out->wealth = p.wealth;
    out->purpose = p.purpose;
    out->land_value_min = p.landValueMin;
    out->land_value_max = p.landValueMax;
    out->building_type = p.buildingType;
    out->x = p.x;
    out->z = p.z;
}

cISC4SimGrid<int16_t>* Desirability(uint64_t dev) { return dev ? DevOf(dev)->GetDesirabilityMap() : nullptr; }

int32_t DevDesirabilityTractSize(void* u, uint64_t dev) {
    TRACT_TEST(dev_desirability_tract_size, dev);
    cISC4SimGrid<int16_t>* g = Desirability(dev);  // slot 13
    return g ? g->GetTractSize() : 0;
}
uint32_t DevDesirabilityShift(void* u, uint64_t dev) {
    TRACT_TEST(dev_desirability_shift, dev);
    cISC4SimGrid<int16_t>* g = Desirability(dev);
    return g ? static_cast<uint32_t>(g->GetTractShift()) : 0;
}
int16_t DevDesirabilityValue(void* u, uint64_t dev, int32_t tx, int32_t tz) {
    TRACT_TEST(dev_desirability_value, dev, tx, tz);
    cISC4SimGrid<int16_t>* g = Desirability(dev);
    return g ? g->GetTractValue(tx, tz) : 0;
}
uint32_t DevCapacityValue(void* u, uint64_t dev, int32_t tx, int32_t tz) {
    TRACT_TEST(dev_capacity_value, dev, tx, tz);
    cISC4SimGrid<uint16_t>* g = dev ? DevOf(dev)->GetCapacityMap() : nullptr;  // slot 14
    return g ? g->GetTractValue(tx, tz) : 0;
}

// Slot 18 GetSourceDemand, then cISC4Demand slot 12 (ST0).
bool DevSourceDemand(void* u, uint64_t dev, float* out) {
    TRACT_TEST(dev_source_demand, dev, out);
    if (out != nullptr) *out = 0.0f;
    cISC4Demand* d = dev ? DevOf(dev)->GetSourceDemand() : nullptr;
    if (d == nullptr || out == nullptr) return false;
    *out = fc4::AsGame(d)->QueryActiveDemandValue();
    return true;
}

// Slots 19-22 (AL).
uint8_t DevCap(void* u, uint64_t dev, uint32_t which) {
    TRACT_TEST(dev_cap, dev, which);
    if (dev == 0) return 0;
    cFC4ISC4GrowthDeveloper* d = DevOf(dev);
    switch (which) {
        case FC4_TRACT_CAP_POWER: return d->GetPowerSupplyCap();
        case FC4_TRACT_CAP_WATER: return d->GetWaterSupplyCap();
        case FC4_TRACT_CAP_FIRE: return d->GetFireCoverageCap();
        case FC4_TRACT_CAP_PARK: return d->GetParkCap();
        default: return 0;
    }
}

int32_t DevMonopolyUse(void* u, uint64_t dev) {
    TRACT_TEST(dev_monopoly_use, dev);
    return dev ? DevOf(dev)->GetMonopolyUse() : 0;  // slot 25
}

// Slot 26: the developer's map<uint32, SC4Percentage> (B), read in key order
// (the float at node +0x14). All or nothing: an inconsistent map reads as
// empty.
void DevStylePercentages(void* u, uint64_t dev, fc4_tract_style_callback cb, void* cbCtx) {
    TRACT_TEST(dev_style_percentages, dev, cb, cbCtx);
    if (dev == 0 || cb == nullptr || !Gate(u, fc4::Sw::TractStyleMap, "tract GetStylePercentages (empty)")) return;
    const fc4::GameMapObject* m = DevOf(dev)->GetStylePercentages();
    if (m == nullptr) return;
    std::vector<std::pair<uint32_t, float>> entries;
    const fc4::GameMapStatus st = fc4::WalkGameMap(m, kMaxStyleEntries, [&](const fc4::GameMapNode& n) {
        float f = 0.0f;
        std::memcpy(&f, &n.value, sizeof(f));
        entries.emplace_back(n.key, f);
    });
    if (st != fc4::GameMapStatus::Ok) {
        ++Ctx(u)->mapsRefused;
        NoteOnce(g_noteStyleMap, "tract: a growth developer's style map is inconsistent; read as empty");
        return;
    }
    for (const auto& e : entries) cb(cbCtx, e.first, e.second);
}

// ---- BDS (building-development.md §7.1) ----------------------------------------------------------------

// Slot 39 GetBuildingProfile(type) (inserts a zero profile for an unknown
// type, as the original's call does).
void BuildingProfile(void* u, uint32_t building, fc4_tract_building_profile* out) {
    TRACT_TEST(building_profile, building, out);
    if (out == nullptr) return;
    *out = fc4_tract_building_profile{};
    cFC4ISC4BuildingDevelopmentSimulator* b = Bds(u);
    if (b == nullptr) return;
    const FC4BuildingProfile& p = b->GetBuildingProfile(building);
    out->purpose = p.purpose;
    out->wealth = p.wealth;
    out->capacity = p.capacity;
}

// Slot 19 GetBuildingTypeSetByFamily(set, family) into a 193-bucket STLport
// hash_set (B). The types are handed over bucket by bucket (ascending), each
// chain from its oldest node to its newest, so that inserting them in that
// order into an empty 193-bucket set rebuilds this one (fc4.h). [question]
// Past 193 elements the set the BDS built has rehashed and this order does
// not rebuild it exactly.
void BuildingTypesByFamily(void* u, uint32_t family, fc4_u32_callback cb, void* cbCtx) {
    TRACT_TEST(building_types_by_family, family, cb, cbCtx);
    if (cb == nullptr || !Gate(u, fc4::Sw::TractFamilySets, "tract GetBuildingTypesByFamily (none)")) return;
    cFC4ISC4BuildingDevelopmentSimulator* b = Bds(u);
    if (b == nullptr) return;
    if (!fc4::TractNodeAllocator::Available()) {
        ++Ctx(u)->listsRefused;
        NoteOnce(g_noteNoAllocator, "tract: the game's STL allocator is not available; game sets and lists are not "
                                    "made (no lots, configurations or building types listed)");
        return;
    }
    fc4::GameHashSetU32<fc4::TractNodeAllocator> set(kSetHint);
    if (set.bucket_count() == 0) {
        ++Ctx(u)->listsRefused;
        return;
    }
    b->GetBuildingTypeSetByFamily(reinterpret_cast<fc4::GameHashSetU32<>&>(set), family);
    if (set.bucket_count() != 193) {
        NoteOnce(g_noteFamilySet, "tract: a building family set grew past 193 buckets; its hash order is not "
                                  "rebuilt exactly (question)");
    }
    std::vector<uint32_t> chain;
    for (size_t i = 0; i < set.bucket_count(); ++i) {
        chain.clear();
        for (auto* n = set.bucket(i); n != nullptr; n = n->next) chain.push_back(n->value);
        for (auto it = chain.rbegin(); it != chain.rend(); ++it) cb(cbCtx, *it);
    }
}

// What the tract developer reads of a building exemplar (§4.15.2, §4.16,
// §4.17). [choice] Read once per type and kept for the session: building
// exemplars do not change while the game runs.
struct BuildingInfo {
    bool loaded = false;
    std::vector<uint32_t> groups;
    bool monopoly = false;
    bool fieldsPresent = false;
    std::vector<uint32_t> fields;
};
std::mutex g_buildingsLock;
std::unordered_map<uint32_t, BuildingInfo> g_buildings;

// BDS slot 14 GetBuildingKeyFromType, then the exemplar through the resource
// manager. Null when there is no BDS or exemplar loads are switched off
// (nothing cached); otherwise the cached entry (`loaded` false when the key
// or the exemplar is missing). Entries are never erased, so the pointer
// stays valid (unordered_map keeps its nodes across rehashes); PickBuilding
// asks several times per building type, so nothing is copied.
const BuildingInfo* LoadBuilding(void* u, uint32_t type) {
    {
        std::lock_guard<std::mutex> g(g_buildingsLock);
        auto it = g_buildings.find(type);
        if (it != g_buildings.end()) return &it->second;
    }
    BuildingInfo info;
    cFC4ISC4BuildingDevelopmentSimulator* b = Bds(u);
    if (b == nullptr || !Gate(u, fc4::Sw::ExemplarLoads, "tract building exemplar load (none)")) return nullptr;
    cGZPersistResourceKey key(0, 0, 0);
    if (b->GetBuildingKeyFromType(type, key)) {
        fc4::props::Exemplar ex;
        if (ex.Load(key.type, key.group, key.instance)) {
            info.loaded = true;
            fc4::props::GetU32s(ex.Holder(), kPropOccupantGroups, info.groups);
            std::vector<uint32_t> mono;
            info.monopoly = fc4::props::GetU32s(ex.Holder(), kPropMonopoly, mono) && !mono.empty() && mono[0] != 0;
            info.fieldsPresent = fc4::props::GetU32s(ex.Holder(), kPropFieldIds, info.fields);
        }
    }
    if (!info.loaded) ++Ctx(u)->exemplarMisses;
    std::lock_guard<std::mutex> g(g_buildingsLock);
    return &g_buildings.emplace(type, std::move(info)).first->second;
}

// IsOccupantGroup (0x005E8F70) on the building's exemplar: the group is in
// its occupant-group list (property 0xAA1DD396).
bool BuildingInOccupantGroup(void* u, uint32_t building, uint32_t group) {
    TRACT_TEST(building_in_occupant_group, building, group);
    const BuildingInfo* info = LoadBuilding(u, building);
    if (info == nullptr || !info->loaded) return false;
    return std::find(info->groups.begin(), info->groups.end(), group) != info->groups.end();
}

// §4.17 row 6: the bool property 0x2781283F (absent = false).
bool BuildingIsMonopoly(void* u, uint32_t building) {
    TRACT_TEST(building_is_monopoly, building);
    const BuildingInfo* info = LoadBuilding(u, building);
    return info != nullptr && info->loaded && info->monopoly;
}

// §4.15.2: the uint32 array 0x29B55F73 (false: absent).
bool BuildingFieldIds(void* u, uint32_t building, fc4_u32_callback cb, void* cbCtx) {
    TRACT_TEST(building_field_ids, building, cb, cbCtx);
    const BuildingInfo* info = LoadBuilding(u, building);
    if (info == nullptr || !info->loaded || !info->fieldsPresent) return false;
    if (cb != nullptr) {
        for (uint32_t id : info->fields) cb(cbCtx, id);
    }
    return true;
}

// Slot 33 GetDeveloper(purpose, wealth): no reference.
uint64_t DeveloperFor(void* u, uint32_t purpose, uint32_t wealth) {
    TRACT_TEST(developer_for, purpose, wealth);
    cFC4ISC4BuildingDevelopmentSimulator* b = Bds(u);
    return b ? TokenOf(b->GetDeveloperOf(purpose, wealth)) : 0;
}

// Slots 50 / 51. [choice] tract_resources off: claims refused, nothing released.
bool ClaimConstructionResources(void* u, uint16_t amount) {
    TRACT_TEST(claim_construction_resources, amount);
    if (!Writes(u) || !Gate(u, fc4::Sw::TractResources, "tract ClaimConstructionResources (refused)")) return false;
    cFC4ISC4BuildingDevelopmentSimulator* b = Bds(u);
    return b != nullptr && b->ClaimConstructionResources(amount);
}

void ReleaseConstructionResources(void* u, uint16_t amount) {
    if (Ctx(u)->test != nullptr && Ctx(u)->test->release_construction_resources != nullptr) {
        Ctx(u)->test->release_construction_resources(Ctx(u)->test->user, amount);
        return;
    }
    if (!Writes(u) || !Gate(u, fc4::Sw::TractResources, "tract ReleaseConstructionResources")) return;
    cFC4ISC4BuildingDevelopmentSimulator* b = Bds(u);
    if (b != nullptr) b->ReleaseConstructionResources(amount);
}

// Slots 54-57.
bool Cheat(void* u, uint32_t which) {
    TRACT_TEST(cheat, which);
    cFC4ISC4BuildingDevelopmentSimulator* b = Bds(u);
    if (b == nullptr) return false;
    switch (which) {
        case FC4_TRACT_CHEAT_IGNORE_PLUMBING: return b->CheatIgnorePlumbing();
        case FC4_TRACT_CHEAT_IGNORE_POWER: return b->CheatIgnorePower();
        case FC4_TRACT_CHEAT_IGNORE_CAPS: return b->CheatIgnoreCaps();
        case FC4_TRACT_CHEAT_DEBUG_GROWTH: return b->CheatDebugGrowth();
        default: return false;
    }
}

// ---- lot developer (lot-developer.md; SDK slots match the compiled ones) ---------------------------------

cISC4LotDeveloper* LotDeveloper(void* u) {
    fc4::TractContext* c = Ctx(u);
    if (c->lotDeveloper != nullptr) return c->lotDeveloper;
    cFC4ISC4BuildingDevelopmentSimulator* b = c->readOnly ? Bds(u) : nullptr;
    return b ? b->GetLotDeveloper() : nullptr;
}

SC4Rect<int32_t> RectOf(const fc4_cell_rect& r) { return SC4Rect<int32_t>(r.x1, r.z1, r.x2, r.z2); }

// Slot 36 NeedsAnchorBuilding(rect, purpose) (AL).
bool NeedsAnchorBuilding(void* u, const fc4_cell_rect* rect, uint32_t purpose) {
    TRACT_TEST(needs_anchor_building, rect, purpose);
    cISC4LotDeveloper* ld = LotDeveloper(u);
    if (ld == nullptr || rect == nullptr) return false;
    return ld->NeedsAnchorBuilding(RectOf(*rect), static_cast<int32_t>(purpose));
}

// Slot 6 StartConstruction(lot, cfg, type, flag).
bool StartConstruction(void* u, uint64_t lot, uint64_t cfg, uint32_t building, bool flag) {
    TRACT_TEST(start_construction, lot, cfg, building, flag);
    if (!Writes(u) || lot == 0 || !Gate(u, fc4::Sw::TractConstruction, "tract StartConstruction (refused)")) {
        return false;
    }
    cISC4LotDeveloper* ld = LotDeveloper(u);
    if (ld == nullptr) return false;
    const bool ok = ld->StartConstruction(SdkLotOf(lot), CfgOf(cfg), building, flag);
    FC4_X87_CHECK("tract StartConstruction");
    return ok;
}

// Slot 8 AbandonBuilding(lot, reason).
bool AbandonBuilding(void* u, uint64_t lot, uint32_t reason) {
    TRACT_TEST(abandon_building, lot, reason);
    if (!Writes(u) || lot == 0 || !Gate(u, fc4::Sw::TractAbandon, "tract AbandonBuilding (refused)")) return false;
    cISC4LotDeveloper* ld = LotDeveloper(u);
    // [abi-ret] Lot developer slot 8 returns "AL of SetState" (lot-developer.md §11.1), and SetState leaves it
    // undefined (lot-records.md §13.2); tract-developer.md's table says AL (question LD34). Passed through only:
    // the tract core never reads abandon_building's result.
    return ld != nullptr && ld->AbandonBuilding(SdkLotOf(lot), static_cast<int32_t>(reason));
}

// Slot 9 ReoccupyBuilding(lot, devType).
bool ReoccupyBuilding(void* u, uint64_t lot, uint32_t devType) {
    TRACT_TEST(reoccupy_building, lot, devType);
    if (!Writes(u) || lot == 0 || !Gate(u, fc4::Sw::TractAbandon, "tract ReoccupyBuilding (refused)")) return false;
    cISC4LotDeveloper* ld = LotDeveloper(u);
    return ld != nullptr && ld->ReoccupyBuilding(SdkLotOf(lot), static_cast<int32_t>(devType));
}

// Slot 29 GetAreaSlope(rect, false) (ST0).
float AreaSlope(void* u, const fc4_cell_rect* rect) {
    TRACT_TEST(area_slope, rect);
    cISC4LotDeveloper* ld = LotDeveloper(u);
    if (ld == nullptr || rect == nullptr) return 0.0f;
    return ld->GetAreaSlope(RectOf(*rect), false);
}

// ---- lot manager (zones.md; FC4Interfaces.h cFC4ISC4LotManager) ---------------------------------------------

cFC4ISC4LotManager* LotManager(void* u, const char* what) {
    fc4::TractContext* c = Ctx(u);
    cISC4LotManager* lm = c->lots;
    if (lm == nullptr && c->readOnly && c->city != nullptr) lm = c->city->GetLotManager();
    if (lm == nullptr || !Gate(u, fc4::Sw::TractLotLists, what)) return nullptr;
    return fc4::AsGame(lm);
}

// Slot 6 GetLot(x, z, false): borrowed.
uint64_t GetLot(void* u, int32_t x, int32_t z) {
    TRACT_TEST(get_lot, x, z);
    cFC4ISC4LotManager* lm = LotManager(u, "tract GetLot (none)");
    return lm ? TokenOf(lm->GetLot(x, z, false)) : 0;
}

using LotList = fc4::GameList<cISC4Lot*, fc4::TractNodeAllocator>;

bool ListsAvailable(void* u) {
    if (fc4::TractNodeAllocator::Available()) return true;
    ++Ctx(u)->listsRefused;
    NoteOnce(g_noteNoAllocator, "tract: the game's STL allocator is not available; game sets and lists are not made "
                                "(no lots, configurations or building types listed)");
    return false;
}

// Slot 7 GetLotsInCellRect(list, x1, z1, x2, z2, true) (§4.18): one
// reference per lot, kept until the outermost call returns.
void LotsInCellRect(void* u, const fc4_cell_rect* rect, fc4_token_callback cb, void* cbCtx) {
    TRACT_TEST(lots_in_cell_rect, rect, cb, cbCtx);
    if (rect == nullptr || cb == nullptr) return;
    cFC4ISC4LotManager* lm = LotManager(u, "tract GetLotsInCellRect (none)");
    if (lm == nullptr || !ListsAvailable(u)) return;
    LotList list;
    if (!list.valid()) return;
    lm->GetLotsInCellRect(reinterpret_cast<fc4::GameList<cISC4Lot*>&>(list), rect->x1, rect->z1, rect->x2, rect->z2,
                          true);
    // glue27: the tokens used to be collected into a vector first and handed
    // out in a second pass. The references are already held in `held` as each
    // lot is taken from the list, so the callback can run in the same pass and
    // the vector (a heap allocation per call - this is once per desirability
    // tract of every growth developer's map update) goes away.
    for (cISC4Lot*& lot : list) {
        if (lot == nullptr) continue;
        FC4_ACQUIRED(lot, "GetLotsInCellRect list entry");
        Ctx(u)->held.push_back(lot);
        const uint64_t t = TokenOf(lot);
        lot = nullptr;  // the list's destructor then only frees its nodes
        cb(cbCtx, t);
    }
}

// BuildFarm's list (§4.15.2, §9.2: "one list appended across 4 + 4·k
// calls"). [question] The spec does not say how the calls of one BuildFarm
// are told apart from the next one's: a new list begins at every
// GetLotsSurroundingRect with flags 1 (BuildFarm's first call). The new
// entries (one reference each, held by the list) go to the core in list
// order.
LotList* FarmList(void* u, bool begin) {
    fc4::TractContext* c = Ctx(u);
    if (begin || c->farmLists.empty()) {
        if (!ListsAvailable(u)) return nullptr;
        LotList* l = new LotList();
        if (!l->valid()) {
            delete l;
            return nullptr;
        }
        c->farmLists.push_back(l);
    }
    return c->farmLists.back();
}

template <class Call>
void AppendAndReport(void* u, LotList* list, Call&& call, fc4_token_callback cb, void* cbCtx) {
    auto* head = list->head();
    auto* tail = head->prev;  // the last node before the call (the head when empty)
    call(reinterpret_cast<fc4::GameList<cISC4Lot*>&>(*list));
    std::vector<uint64_t> tokens;
    uint32_t n = 0;
    for (auto* node = tail->next; node != head && n < kMaxListNodes; node = node->next, ++n) {
        if (node->value == nullptr) continue;
        FC4_ACQUIRED(node->value, "GetLotsSurrounding list entry");
        tokens.push_back(TokenOf(node->value));
    }
    for (uint64_t t : tokens) cb(cbCtx, t);
}

// Slot 9 GetLotsSurroundingLot(lot, list, flags): appends.
void LotsSurroundingLot(void* u, uint64_t lot, uint32_t flags, fc4_token_callback cb, void* cbCtx) {
    TRACT_TEST(lots_surrounding_lot, lot, flags, cb, cbCtx);
    if (lot == 0 || cb == nullptr) return;
    cFC4ISC4LotManager* lm = LotManager(u, "tract GetLotsSurroundingLot (none)");
    LotList* list = lm ? FarmList(u, false) : nullptr;
    if (list == nullptr) return;
    AppendAndReport(
        u, list,
        [&](fc4::GameList<cISC4Lot*>& l) { lm->GetLotsSurroundingLot(SdkLotOf(lot), l, static_cast<int32_t>(flags)); },
        cb, cbCtx);
}

// Slot 10 GetLotsSurroundingRect(rect, list, flags): appends (unique against
// the whole list).
void LotsSurroundingRect(void* u, const fc4_cell_rect* rect, uint32_t flags, fc4_token_callback cb, void* cbCtx) {
    TRACT_TEST(lots_surrounding_rect, rect, flags, cb, cbCtx);
    if (rect == nullptr || cb == nullptr) return;
    cFC4ISC4LotManager* lm = LotManager(u, "tract GetLotsSurroundingRect (none)");
    LotList* list = lm ? FarmList(u, flags == 1) : nullptr;
    if (list == nullptr) return;
    const SC4Rect<long> r(rect->x1, rect->z1, rect->x2, rect->z2);
    AppendAndReport(
        u, list, [&](fc4::GameList<cISC4Lot*>& l) { lm->GetLotsSurroundingRect(r, l, static_cast<int32_t>(flags)); },
        cb, cbCtx);
}

// Slot 20 CanCreateLot(x, z, w, h, 0, 0, 0).
int32_t CanCreateLot(void* u, int32_t x, int32_t z, int32_t w, int32_t h) {
    TRACT_TEST(can_create_lot, x, z, w, h);
    cFC4ISC4LotManager* lm = LotManager(u, "tract CanCreateLot (0)");
    return lm ? lm->CanCreateLot(x, z, w, h, false, false, nullptr) : 0;
}

// Slot 22 CreateLotIfPossible(x, z, w, h, facing, &lot, 0, 0, 0): the lot
// comes with one reference for the caller (allocators.md), kept until the
// outermost call returns.
uint64_t CreateLotIfPossible(void* u, int32_t x, int32_t z, int32_t w, int32_t h, int32_t facing) {
    TRACT_TEST(create_lot_if_possible, x, z, w, h, facing);
    if (!Writes(u) || !Gate(u, fc4::Sw::TractLotChanges, "tract CreateLotIfPossible (no lot)")) return 0;
    cFC4ISC4LotManager* lm = LotManager(u, "tract CreateLotIfPossible (no lot)");
    if (lm == nullptr) return 0;
    cISC4Lot* lot = nullptr;
    const bool ok = lm->CreateLotIfPossible(x, z, w, h, facing, lot, false, false, nullptr);
    FC4_X87_CHECK("tract CreateLotIfPossible");
    if (lot == nullptr) return 0;
    FC4_ACQUIRED(lot, "CreateLotIfPossible out-lot");
    Ctx(u)->held.push_back(lot);
    return ok ? TokenOf(lot) : 0;
}

// Slot 24 DeleteLot(lot). The lot stays alive (one reference of ours) until
// the outermost call returns: the core may still hold its token.
bool DeleteLot(void* u, uint64_t lot) {
    TRACT_TEST(delete_lot, lot);
    if (!Writes(u) || lot == 0 || !Gate(u, fc4::Sw::TractLotChanges, "tract DeleteLot (refused)")) return false;
    cFC4ISC4LotManager* lm = LotManager(u, "tract DeleteLot (refused)");
    if (lm == nullptr) return false;
    cISC4Lot* l = SdkLotOf(lot);
    FC4_ADDREF(l);
    Ctx(u)->deleted.push_back(l);
    // [abi-ret] Slot 24 leaves EAX undefined (interfaces.tsv eax-or-void; zones.md §8.6 and the §11.3 row:
    // "no bool result"): the SDK's bool is garbage, so the call is the result.
    lm->DeleteLot(l);
    FC4_X87_CHECK("tract DeleteLot");
    return true;
}

// ---- lots (FC4Interfaces.h cFC4ISC4Lot) ----------------------------------------------------------------

void LotBoundingRect(void* u, uint64_t lot, fc4_cell_rect* out) {
    TRACT_TEST(lot_bounding_rect, lot, out);
    if (out == nullptr) return;
    *out = fc4_cell_rect{-1, -1, -1, -1};
    if (lot == 0) return;
    SC4Rect<int32_t> r;
    if (LotOf(lot)->GetBoundingRect(r)) *out = fc4_cell_rect{r.topLeftX, r.topLeftY, r.bottomRightX, r.bottomRightY};
}
float LotSlope(void* u, uint64_t lot) {
    TRACT_TEST(lot_slope, lot);
    return lot ? LotOf(lot)->GetSlope() : 0.0f;  // slot 18
}
void LotLocation(void* u, uint64_t lot, int32_t* x, int32_t* z) {
    TRACT_TEST(lot_location, lot, x, z);
    int32_t lx = 0, lz = 0;
    if (lot != 0) LotOf(lot)->GetLocation(lx, lz);  // slot 21
    if (x != nullptr) *x = lx;
    if (z != nullptr) *z = lz;
}
void LotSize(void* u, uint64_t lot, uint32_t* w, uint32_t* h) {
    TRACT_TEST(lot_size, lot, w, h);
    uint8_t sw = 0, sh = 0;
    if (lot != 0) LotOf(lot)->GetSize(sw, sh);  // slot 22
    if (w != nullptr) *w = sw;
    if (h != nullptr) *h = sh;
}
int32_t LotFacing(void* u, uint64_t lot) {
    TRACT_TEST(lot_facing, lot);
    return lot ? LotOf(lot)->GetFacing() : 0;  // slot 23
}
void LotSetFacing(void* u, uint64_t lot, int32_t facing) {
    if (Ctx(u)->test != nullptr && Ctx(u)->test->lot_set_facing != nullptr) {
        Ctx(u)->test->lot_set_facing(Ctx(u)->test->user, lot, facing);
        return;
    }
    if (!Writes(u) || lot == 0 || !Gate(u, fc4::Sw::TractLotChanges, "tract lot SetFacing")) return;
    LotOf(lot)->SetFacing(facing);  // slot 24
}
bool LotHasRoadAccess(void* u, uint64_t lot) {
    TRACT_TEST(lot_has_road_access, lot);
    return lot != 0 && LotOf(lot)->HasRoadAccess();  // slot 28
}
bool LotHasOrganicRoadAccess(void* u, uint64_t lot, uint32_t purpose) {
    TRACT_TEST(lot_has_organic_road_access, lot, purpose);
    return lot != 0 && LotOf(lot)->HasOrganicRoadAccess(purpose);  // slot 29
}
int32_t LotZoneType(void* u, uint64_t lot) {
    TRACT_TEST(lot_zone_type, lot);
    return lot ? static_cast<int32_t>(LotOf(lot)->GetZoneType()) : 0;  // slot 31
}
uint32_t LotState(void* u, uint64_t lot) {
    TRACT_TEST(lot_state, lot);
    return lot ? static_cast<uint32_t>(LotOf(lot)->GetState()) : 0;  // slot 33
}
void LotSetState(void* u, uint64_t lot, uint32_t state, uint32_t wealth, uint32_t reason) {
    if (Ctx(u)->test != nullptr && Ctx(u)->test->lot_set_state != nullptr) {
        Ctx(u)->test->lot_set_state(Ctx(u)->test->user, lot, state, wealth, reason);
        return;
    }
    if (!Writes(u) || lot == 0 || !Gate(u, fc4::Sw::TractLotChanges, "tract lot SetState")) return;
    LotOf(lot)->SetState(static_cast<cFC4ISC4Lot::HabitationState>(state), static_cast<cFC4ISC4Lot::WealthType>(wealth),
                         static_cast<cFC4ISC4Lot::VacateReason>(reason));  // slot 34
}
bool LotIsLivable(void* u, uint64_t lot) {
    TRACT_TEST(lot_is_livable, lot);
    return lot != 0 && LotOf(lot)->IsInLivableState();  // slot 37
}
bool LotHasCapacity(void* u, uint64_t lot) {
    TRACT_TEST(lot_has_capacity, lot);
    return lot != 0 && LotOf(lot)->HasCapacity();  // slot 38
}
uint32_t LotDaysSinceStateChange(void* u, uint64_t lot) {
    TRACT_TEST(lot_days_since_state_change, lot);
    return lot ? LotOf(lot)->GetDaysSinceLastStateChange() : 0;  // slot 40
}
uint32_t LotOccupantWealth(void* u, uint64_t lot) {
    TRACT_TEST(lot_occupant_wealth, lot);
    return lot ? LotOf(lot)->GetOccupantWealth() : 0;  // slot 41
}
bool LotIsHistorical(void* u, uint64_t lot) {
    TRACT_TEST(lot_is_historical, lot);
    return lot != 0 && LotOf(lot)->IsHistorical();  // slot 43
}
uint32_t LotBuildingType(void* u, uint64_t lot) {
    TRACT_TEST(lot_building_type, lot);
    return lot ? LotOf(lot)->GetBuildingType(false) : 0;  // slot 46
}
uint16_t LotCapacity(void* u, uint64_t lot, uint32_t devType, bool flag) {
    TRACT_TEST(lot_capacity, lot, devType, flag);
    return lot ? LotOf(lot)->GetCapacityOfDeveloper(static_cast<cFC4ISC4Lot::DeveloperType>(devType), flag)
               : uint16_t{0};  // slot 50
}
uint16_t LotAvailableCapacity(void* u, uint64_t lot, uint32_t devType, bool flag) {
    TRACT_TEST(lot_available_capacity, lot, devType, flag);
    return lot ? LotOf(lot)->GetAvailableCapacity(static_cast<cFC4ISC4Lot::DeveloperType>(devType), flag)
               : uint16_t{0};  // slot 52
}
void LotSetPopulation(void* u, uint64_t lot, uint32_t devType, uint16_t population) {
    if (Ctx(u)->test != nullptr && Ctx(u)->test->lot_set_population != nullptr) {
        Ctx(u)->test->lot_set_population(Ctx(u)->test->user, lot, devType, population);
        return;
    }
    if (!Writes(u) || lot == 0 || !Gate(u, fc4::Sw::TractPopulation, "tract lot SetPopulation")) return;
    LotOf(lot)->SetPopulation(static_cast<cFC4ISC4Lot::DeveloperType>(devType), population);  // slot 55
}
// Lot slot 9 GetLotConfiguration (no reference), then its slot 11.
int32_t LotConfigurationStage(void* u, uint64_t lot) {
    TRACT_TEST(lot_configuration_stage, lot);
    if (lot == 0) return -1;
    cISC4LotConfiguration* cfg = LotOf(lot)->GetLotConfiguration();
    return cfg ? static_cast<int32_t>(cfg->GetGrowthStage()) : -1;
}

// ---- lot configurations (SDK slots match the compiled ones; §8) ------------------------------------------

void CfgSize(void* u, uint64_t cfg, uint32_t* w, uint32_t* h) {
    TRACT_TEST(cfg_size, cfg, w, h);
    uint32_t sw = 0, sh = 0;
    if (cfg != 0) CfgOf(cfg)->GetSize(sw, sh);  // slot 7
    if (w != nullptr) *w = sw;
    if (h != nullptr) *h = sh;
}
float CfgMinSlope(void* u, uint64_t cfg) {
    TRACT_TEST(cfg_min_slope, cfg);
    return cfg ? CfgOf(cfg)->GetMinimumSlope() : 0.0f;  // slot 9
}
float CfgMaxSlope(void* u, uint64_t cfg) {
    TRACT_TEST(cfg_max_slope, cfg);
    return cfg ? CfgOf(cfg)->GetMaximumSlope() : 0.0f;  // slot 8
}
uint8_t CfgGrowthStage(void* u, uint64_t cfg) {
    TRACT_TEST(cfg_growth_stage, cfg);
    return cfg ? CfgOf(cfg)->GetGrowthStage() : uint8_t{0};  // slot 11
}
uint16_t CfgMinCapacity(void* u, uint64_t cfg) {
    TRACT_TEST(cfg_min_capacity, cfg);
    return cfg ? CfgOf(cfg)->GetMinBuildingCapacity() : uint16_t{0};  // slot 12
}
uint16_t CfgMaxCapacity(void* u, uint64_t cfg) {
    TRACT_TEST(cfg_max_capacity, cfg);
    return cfg ? CfgOf(cfg)->GetMaxBuildingCapacity() : uint16_t{0};  // slot 13
}
bool CfgCompatibleZone(void* u, uint64_t cfg, int32_t zone) {
    TRACT_TEST(cfg_compatible_zone, cfg, zone);
    return cfg != 0 && CfgOf(cfg)->IsCompatibleWithZoneType(static_cast<cISC4ZoneManager::ZoneType>(zone));  // 15
}
bool CfgCompatibleWealth(void* u, uint64_t cfg, uint32_t wealth) {
    TRACT_TEST(cfg_compatible_wealth, cfg, wealth);
    return cfg != 0 &&
           CfgOf(cfg)->IsCompatibleWithWealthType(static_cast<cISC4BuildingOccupant::WealthType>(wealth));  // 16
}
bool CfgCompatiblePurpose(void* u, uint64_t cfg, uint32_t purpose) {
    TRACT_TEST(cfg_compatible_purpose, cfg, purpose);
    return cfg != 0 &&
           CfgOf(cfg)->IsCompatibleWithPurposeType(static_cast<cISC4BuildingOccupant::PurposeType>(purpose));  // 17
}
bool CfgCompatibleStyle(void* u, uint64_t cfg, uint32_t style) {
    TRACT_TEST(cfg_compatible_style, cfg, style);
    return cfg != 0 && CfgOf(cfg)->IsCompatibleWithStyleType(style);  // slot 18
}

// STLport's vector<bool> as the game has it: {start {word*, offset}, finish
// {word*, offset}, end_of_storage}, 32-bit words. [question] allocators.md
// gives the allocator (B) but not the layout: this is STLport's
// _Bvector_base. A result that does not look like one is not read (no road
// required) and counted.
struct GameBitVector {
    uint32_t* startWord;
    uint32_t startOffset;
    uint32_t* finishWord;
    uint32_t finishOffset;
    uint32_t* endOfStorage;
    uint32_t spare[3];  // room in case the callee's object is larger (never read)
};

// Slot 21 GetRequiredRoads(vector<bool>&): the callee fills a vector of
// ours; its bit buffer is freed with B.
size_t CfgRequiredRoads(void* u, uint64_t cfg, uint8_t* bits, size_t cap) {
    TRACT_TEST(cfg_required_roads, cfg, bits, cap);
    if (cfg == 0 || !Gate(u, fc4::Sw::TractRoads, "tract GetRequiredRoads (none required)")) return 0;
    if (!ListsAvailable(u)) return 0;
    GameBitVector v{};
    CfgOf(cfg)->GetRequiredRoads(reinterpret_cast<SC4Vector<bool>&>(v));
    size_t count = 0;
    bool ok = true;
    if (v.startWord != nullptr) {
        const ptrdiff_t words = v.finishWord - v.startWord;
        ok = v.startOffset == 0 && v.finishOffset < 32 && words >= 0 && v.endOfStorage >= v.finishWord &&
             words <= 1;
        if (ok) count = static_cast<size_t>(words) * 32 + v.finishOffset;
        ok = ok && count <= FC4_TRACT_MAX_ROAD_BITS;
        if (ok && bits != nullptr) {
            for (size_t i = 0; i < count && i < cap; ++i) bits[i] = (v.startWord[i / 32] >> (i % 32)) & 1u;
        }
    } else {
        ok = v.finishWord == nullptr && v.endOfStorage == nullptr;
    }
    if (v.startWord != nullptr) fc4::TractNodeAllocator::Free(v.startWord);
    if (!ok) {
        ++Ctx(u)->roadsRefused;
        NoteOnce(g_noteRoads, "tract: GetRequiredRoads gave a vector<bool> of an unexpected shape; read as no road "
                              "required (question)");
        return 0;
    }
    return count;
}

// Slot 24 GetPrimaryBuildingConfigurationObject: its payload (family or
// building IDs), in order.
bool CfgPrimaryBuildingIds(void* u, uint64_t cfg, fc4_u32_callback cb, void* cbCtx) {
    TRACT_TEST(cfg_primary_building_ids, cfg, cb, cbCtx);
    if (cfg == 0) return false;
    cSC4LotConfigurationObject* o = CfgOf(cfg)->GetPrimaryBuildingConfigurationObject();
    if (o == nullptr) return false;
    auto& ids = reinterpret_cast<fc4::GameVector<uint32_t>&>(o->data);
    if (cb != nullptr) {
        const size_t n = ids.size();
        for (size_t i = 0; i < n && i < kMaxListNodes; ++i) cb(cbCtx, ids[i]);
    }
    return true;
}

// ---- lot configuration manager (§4.6, §7.4) ----------------------------------------------------------------

cFC4ISC4LotConfigurationManager* Configs(void* u) {
    fc4::TractContext* c = Ctx(u);
    cISC4LotConfigurationManager* m = c->configs != nullptr ? c->configs : c->testConfigs;
    if (m == nullptr && c->readOnly && c->city != nullptr) m = c->city->GetLotConfigurationManager();
    return m ? fc4::AsGame(m) : nullptr;
}

// Slot 8 GetLotConfiguration(id): no reference.
uint64_t GetLotConfiguration(void* u, uint32_t id) {
    TRACT_TEST(get_lot_configuration, id);
    cFC4ISC4LotConfigurationManager* m = Configs(u);
    return m ? TokenOf(m->GetLotConfiguration(id)) : 0;
}

// §4.6 step 1: a filter {wealth, purpose}, registered with slot 18. The
// filter reads configurations through the tract developer's own table.
uint64_t RegisterConfigurationFilter(void* u, uint32_t wealth, uint32_t purpose) {
    TRACT_TEST(register_configuration_filter, wealth, purpose);
    fc4::TractContext* c = Ctx(u);
    if (!Writes(u)) return 0;
    if (!Gate(u, fc4::Sw::TractFilters, "tract configuration filter (none)")) {
        ++c->filtersRefused;
        return 0;
    }
    cFC4ISC4LotConfigurationManager* m = Configs(u);
    if (m == nullptr || c->filterHost == nullptr) {
        ++c->filtersRefused;
        return 0;
    }
    cFC4TractFilter* f = new cFC4TractFilter(c->filterHost, wealth, purpose);
    // [abi-ret] Slot 18 has no defined return (prop-lotconfig.md §B7 "—"; interfaces.tsv eax-or-void): the
    // SDK's bool is a leftover AL, so it is not read. The filter is kept either way, as the original keeps it.
    m->RegisterLotConfigurationFilter(f);
    c->filters.push_back(f);
    ++c->filtersRegistered;
    return TokenOf(static_cast<cISC4LotConfigurationFilter*>(f));
}

// §1.4 step 4: slot 19, then freed.
void UnregisterConfigurationFilter(void* u, uint64_t filter) {
    if (Ctx(u)->test != nullptr && Ctx(u)->test->unregister_configuration_filter != nullptr) {
        Ctx(u)->test->unregister_configuration_filter(Ctx(u)->test->user, filter);
    }
    fc4::TractContext* c = Ctx(u);
    if (!Writes(u) || filter == 0) return;
    auto* f = static_cast<cFC4TractFilter*>(reinterpret_cast<cISC4LotConfigurationFilter*>(FromToken(filter)));
    auto it = std::find(c->filters.begin(), c->filters.end(), f);
    if (it == c->filters.end()) return;  // not ours
    cFC4ISC4LotConfigurationManager* m = Configs(u);
    if (m != nullptr) m->UnregisterLotConfigurationFilter(f);  // slot 19
    c->filters.erase(it);
    ++c->filtersUnregistered;
    delete f;
}

// Slot 9 GetLotConfigurationsByFilter(set, filter) into a 193-bucket STLport
// hash_set (B): one reference per configuration, kept until the outermost
// call returns. Hash order (the core orders them by pointer).
// [choice] These references are kept and released without the reference
// audit's per-reference bookkeeping (TractContext::heldConfigs): a Grow
// lists hundreds of configurations on every pass, and audited they cost
// FreeCity4's Grow several times the original's time, which the BDS's daily
// 3 ms growth budget (building-development.md §4.4) turns into fewer Grow
// calls. Their count is kept instead (configRefs).
void ConfigurationsByFilter(void* u, uint64_t filter, fc4_token_callback cb, void* cbCtx) {
    TRACT_TEST(configurations_by_filter, filter, cb, cbCtx);
    if (filter == 0 || cb == nullptr) return;
    if (!Gate(u, fc4::Sw::TractFilters, "tract GetLotConfigurationsByFilter (none)")) return;
    cFC4ISC4LotConfigurationManager* m = Configs(u);
    if (m == nullptr || !ListsAvailable(u)) return;
    fc4::GameHashSetU32<fc4::TractNodeAllocator> set(kSetHint);
    if (set.bucket_count() == 0) return;
    m->GetLotConfigurationsByFilter(set, reinterpret_cast<cISC4LotConfigurationFilter*>(FromToken(filter)));
    // Sized once: an allocation in game costs about 0.3 us (measured in
    // game), and the vectors would otherwise grow step by step every pass.
    std::vector<uint64_t> tokens;
    tokens.reserve(set.size());
    std::vector<cIGZUnknown*>& held = Ctx(u)->heldConfigs;
    if (held.capacity() - held.size() < set.size()) {
        held.reserve((std::max)(held.capacity() * 2, held.size() + set.size()));
    }
    for (size_t i = 0; i < set.bucket_count(); ++i) {
        for (auto* n = set.bucket(i); n != nullptr; n = n->next) {
            auto* cfg = reinterpret_cast<cISC4LotConfiguration*>(static_cast<uintptr_t>(n->value));
            if (cfg == nullptr) continue;
            Ctx(u)->heldConfigs.push_back(reinterpret_cast<cIGZUnknown*>(cfg));
            ++Ctx(u)->configRefs;
            tokens.push_back(TokenOf(cfg));
        }
    }
    for (uint64_t t : tokens) cb(cbCtx, t);
}

// ---- city grids and services (§4.2, §4.8, §4.10.2) -----------------------------------------------------------

int32_t ZoneAt(void* u, int32_t x, int32_t z) {
    TRACT_TEST(zone_at, x, z);
    fc4::TractContext* c = Ctx(u);
    cISC4ZoneManager* zm = c->zones ? c->zones : (c->city ? c->city->GetZoneManager() : nullptr);
    return zm ? fc4::AsGame(zm)->GetZoneType(x, z) : 0;  // slot 12, the signed byte in EAX
}

uint8_t LandValue(void* u, int32_t x, int32_t z) {
    TRACT_TEST(land_value, x, z);
    fc4::TractContext* c = Ctx(u);
    cISC4LandValueSimulator* lv = c->landValue ? c->landValue : (c->city ? c->city->GetLandValueSimulator() : nullptr);
    return lv ? lv->GetLandValue(x, z) : uint8_t{0};  // slot 6
}

// Power slot 35 GetPoweredGrid() const, then the grid's GetCellValue.
int32_t PoweredValue(void* u, int32_t x, int32_t z) {
    TRACT_TEST(powered_value, x, z);
    fc4::TractContext* c = Ctx(u);
    cISC4PowerSimulator* p = c->power ? c->power : (c->city ? c->city->GetPowerSimulator() : nullptr);
    cISC4SimGrid<uint8_t>* g = p ? fc4::AsGame(p)->GetPoweredGridConst() : nullptr;
    return g ? static_cast<int32_t>(g->GetCellValue(x, z)) : 0;
}

bool IsCellWatered(void* u, int32_t x, int32_t z) {
    TRACT_TEST(is_cell_watered, x, z);
    fc4::TractContext* c = Ctx(u);
    cISC4PlumbingSimulator* p = c->plumbing ? c->plumbing : (c->city ? c->city->GetPlumbingSimulator() : nullptr);
    return p != nullptr && fc4::AsGame(p)->IsCellWatered(x, z);  // slot 11
}

// The fire protection simulator (read from the city when used, §1.3), slot 5's uint8 grid.
uint8_t FireCoverage(void* u, int32_t x, int32_t z) {
    TRACT_TEST(fire_coverage, x, z);
    cISC4City* c = City(u);
    cISC4FireProtectionSimulator* f = c ? c->GetFireProtectionSimulator() : nullptr;
    cISC4SimGrid<uint8_t>* g = f ? fc4::AsGame(f)->GetFireProtectionGrid() : nullptr;
    return g ? g->GetCellValue(x, z) : uint8_t{0};
}

// The aura simulator (read from the city when used), slot 14 GetParkMap (int16).
int16_t ParkValue(void* u, int32_t x, int32_t z) {
    TRACT_TEST(park_value, x, z);
    cISC4City* c = City(u);
    cISC4AuraSimulator* a = c ? c->GetAuraSimulator() : nullptr;
    cISC4SimGrid<int16_t>* g = a ? a->GetParkMap() : nullptr;
    return g ? g->GetCellValue(x, z) : int16_t{0};
}

// §4.8 row 7: traffic simulator slot 13 GetTripLengthMap, GetCellValue.
int32_t RoadAccessValue(void* u, int32_t x, int32_t z) {
    TRACT_TEST(road_access_value, x, z);
    if (!Gate(u, fc4::Sw::TractTraffic, "tract trip length map (0)")) return 0;
    fc4::TractContext* c = Ctx(u);
    cISC4TrafficSimulator* t = c->traffic ? c->traffic : (c->city ? c->city->GetTrafficSimulator() : nullptr);
    cISC4SimGrid<uint8_t>* g = t ? t->GetTripLengthMap() : nullptr;
    return g ? static_cast<int32_t>(g->GetCellValue(x, z)) : 0;
}

// City slots 93, 94.
void CityCellCounts(void* u, int32_t* x, int32_t* z) {
    TRACT_TEST(city_cell_counts, x, z);
    cISC4City* c = City(u);
    if (x != nullptr) *x = c ? static_cast<int32_t>(c->CellCountX()) : 0;
    if (z != nullptr) *z = c ? static_cast<int32_t>(c->CellCountZ()) : 0;
}

// The traffic network map (city slot 40) slot 8 FindNetworkTile(x, z, 0xC4B,
// true): borrowed.
uint64_t FindNetworkTile(void* u, int32_t x, int32_t z) {
    TRACT_TEST(find_network_tile, x, z);
    cISC4City* c = City(u);
    if (c == nullptr || !Gate(u, fc4::Sw::TractTraffic, "tract FindNetworkTile (none)")) return 0;
    auto* map = reinterpret_cast<cFC4ISC4TrafficNetworkMap*>(c->GetTrafficNetwork());
    if (map == nullptr) return 0;
    return static_cast<uint64_t>(reinterpret_cast<uintptr_t>(map->FindNetworkTile(x, z, kNetworkTileMask, true)));
}

// cISC4NetworkOccupant slot 22 IsOfType(type).
bool NetworkIsOfType(void* u, uint64_t tile, uint32_t type) {
    TRACT_TEST(network_is_of_type, tile, type);
    if (tile == 0 || !Gate(u, fc4::Sw::TractTraffic, "tract tile IsOfType (false)")) return false;
    return reinterpret_cast<cISC4NetworkOccupant*>(FromToken(tile))
        ->IsOfType(static_cast<cISC4NetworkOccupant::eNetworkType>(type));
}

// Demolition slot 7 DemolishLots(true, [lot], 0, 0x800, false, 0, 0, 0). The
// list is only walked by the game (allocators.md): it borrows the lot.
bool DemolishLot(void* u, uint64_t lot) {
    TRACT_TEST(demolish_lot, lot);
    fc4::TractContext* c = Ctx(u);
    if (!Writes(u) || lot == 0 || c->city == nullptr) return false;
    if (!Gate(u, fc4::Sw::TractDemolition, "tract DemolishLots (refused)")) return false;
    cISC4Demolition* d = c->city->GetDemolitionUtility();  // city slot 31
    if (d == nullptr) return false;
    bool ok = false;
    auto run = [&](auto& list) {
        cISC4Lot* entry = SdkLotOf(lot);
        list.push_back(entry);
        ok = d->DemolishLots(true, reinterpret_cast<const SC4List<cISC4Lot*>&>(list), 0, kDemolishFlags, false,
                             nullptr, nullptr, 0);
        for (cISC4Lot*& l : list) l = nullptr;  // borrowed: never released by the list
    };
    LotList list;
    if (list.valid()) {
        run(list);
    } else {
        SC4List<cISC4Lot*> sdk;  // the allocator service (A): read only by the game
        run(sdk);
    }
    FC4_X87_CHECK("tract DemolishLots");
    return ok;
}

// Terrain slot 68 RedisplayTerrain(0, 0, &rect, 0) (display only).
void RedisplayTerrain(void* u, const fc4_cell_rect* rect) {
    if (Ctx(u)->test != nullptr && Ctx(u)->test->redisplay_terrain != nullptr) {
        Ctx(u)->test->redisplay_terrain(Ctx(u)->test->user, rect);
        return;
    }
    if (!Writes(u) || rect == nullptr || City(u) == nullptr) return;
    if (!Gate(u, fc4::Sw::TractLotChanges, "tract RedisplayTerrain")) return;
    cISTETerrain* t = City(u)->GetTerrain();
    if (t != nullptr) t->RedisplayTerrain(false, false, RectOf(*rect), 0);
}

// The core calls these void callbacks through the table; the TRACT_TEST
// macro returns their value, so they forward by hand above.

}  // namespace

// ---- the configuration filter (§7.4) ---------------------------------------------------------------------------

namespace {
std::atomic<long> g_liveFilters{0};
}

bool cFC4TractFilter::QueryInterface(uint32_t, void**) { return false; }
uint32_t cFC4TractFilter::AddRef() { return 1; }
uint32_t cFC4TractFilter::Release() { return 1; }

// §4.6: IsCompatibleWithPurposeType(purpose) and IsCompatibleWithWealthType(wealth).
// The manager calls the filter for every configuration it holds, on every
// listing (thousands of calls per Grow). With the game's own table (no test
// seam answering) the two calls are made here, as that table's callbacks
// would make them; fc4_tract_filter_includes (the test seam's path) makes
// the same two calls in the same order.
bool cFC4TractFilter::IsLotConfigurationIncluded(cISC4LotConfiguration* config) {
    const auto* ctx = host_ != nullptr ? static_cast<const fc4::TractContext*>(host_->user) : nullptr;
    if (ctx != nullptr && ctx->test == nullptr) {
        return config != nullptr &&
               config->IsCompatibleWithPurposeType(static_cast<cISC4BuildingOccupant::PurposeType>(purpose_)) &&
               config->IsCompatibleWithWealthType(static_cast<cISC4BuildingOccupant::WealthType>(wealth_));
    }
    return fc4_tract_filter_includes(host_, TokenOf(config), wealth_, purpose_);
}

long cFC4TractFilter::Live() { return g_liveFilters.load(); }

cFC4TractFilter::cFC4TractFilter(const fc4_tract_host* host, uint32_t wealth, uint32_t purpose)
    : wealth_(wealth), purpose_(purpose), host_(host) {
    ++g_liveFilters;
}

cFC4TractFilter::~cFC4TractFilter() { --g_liveFilters; }

namespace fc4 {

fc4_tract_host MakeTractHost(TractContext* ctx) {
    fc4_tract_host h{};
    h.struct_size = sizeof(h);
    h.user = ctx;
    h.acquire_services = &AcquireServices;
    h.add_agent = &AddAgent;
    h.acquire_message_server = &AcquireMessageServer;
    h.release_message_server = &ReleaseMessageServer;
    h.remove_agent = &RemoveAgent;
    h.release_services = &ReleaseServicesCb;
    h.tuning = &Tuning;
    h.dev_target_zone = &DevTargetZone;
    h.dev_subtype = &DevSubtype;
    h.dev_purpose = &DevPurpose;
    h.dev_wealth = &DevWealth;
    h.dev_construction_info = &DevConstructionInfo;
    h.dev_desirability_tract_size = &DevDesirabilityTractSize;
    h.dev_desirability_shift = &DevDesirabilityShift;
    h.dev_desirability_value = &DevDesirabilityValue;
    h.dev_capacity_value = &DevCapacityValue;
    h.dev_source_demand = &DevSourceDemand;
    h.dev_cap = &DevCap;
    h.dev_monopoly_use = &DevMonopolyUse;
    h.dev_style_percentages = &DevStylePercentages;
    h.building_profile = &BuildingProfile;
    h.building_types_by_family = &BuildingTypesByFamily;
    h.building_in_occupant_group = &BuildingInOccupantGroup;
    h.building_is_monopoly = &BuildingIsMonopoly;
    h.building_field_ids = &BuildingFieldIds;
    h.developer_for = &DeveloperFor;
    h.claim_construction_resources = &ClaimConstructionResources;
    h.release_construction_resources = &ReleaseConstructionResources;
    h.cheat = &Cheat;
    h.needs_anchor_building = &NeedsAnchorBuilding;
    h.start_construction = &StartConstruction;
    h.abandon_building = &AbandonBuilding;
    h.reoccupy_building = &ReoccupyBuilding;
    h.area_slope = &AreaSlope;
    h.get_lot = &GetLot;
    h.lots_in_cell_rect = &LotsInCellRect;
    h.lots_surrounding_lot = &LotsSurroundingLot;
    h.lots_surrounding_rect = &LotsSurroundingRect;
    h.can_create_lot = &CanCreateLot;
    h.create_lot_if_possible = &CreateLotIfPossible;
    h.delete_lot = &DeleteLot;
    h.lot_bounding_rect = &LotBoundingRect;
    h.lot_slope = &LotSlope;
    h.lot_location = &LotLocation;
    h.lot_size = &LotSize;
    h.lot_facing = &LotFacing;
    h.lot_set_facing = &LotSetFacing;
    h.lot_has_road_access = &LotHasRoadAccess;
    h.lot_has_organic_road_access = &LotHasOrganicRoadAccess;
    h.lot_zone_type = &LotZoneType;
    h.lot_state = &LotState;
    h.lot_set_state = &LotSetState;
    h.lot_is_livable = &LotIsLivable;
    h.lot_has_capacity = &LotHasCapacity;
    h.lot_days_since_state_change = &LotDaysSinceStateChange;
    h.lot_occupant_wealth = &LotOccupantWealth;
    h.lot_is_historical = &LotIsHistorical;
    h.lot_building_type = &LotBuildingType;
    h.lot_capacity = &LotCapacity;
    h.lot_available_capacity = &LotAvailableCapacity;
    h.lot_set_population = &LotSetPopulation;
    h.lot_configuration_stage = &LotConfigurationStage;
    h.cfg_size = &CfgSize;
    h.cfg_min_slope = &CfgMinSlope;
    h.cfg_max_slope = &CfgMaxSlope;
    h.cfg_growth_stage = &CfgGrowthStage;
    h.cfg_min_capacity = &CfgMinCapacity;
    h.cfg_max_capacity = &CfgMaxCapacity;
    h.cfg_compatible_zone = &CfgCompatibleZone;
    h.cfg_compatible_wealth = &CfgCompatibleWealth;
    h.cfg_compatible_purpose = &CfgCompatiblePurpose;
    h.cfg_compatible_style = &CfgCompatibleStyle;
    h.cfg_required_roads = &CfgRequiredRoads;
    h.cfg_primary_building_ids = &CfgPrimaryBuildingIds;
    h.get_lot_configuration = &GetLotConfiguration;
    h.register_configuration_filter = &RegisterConfigurationFilter;
    h.unregister_configuration_filter = &UnregisterConfigurationFilter;
    h.configurations_by_filter = &ConfigurationsByFilter;
    h.zone_at = &ZoneAt;
    h.land_value = &LandValue;
    h.powered_value = &PoweredValue;
    h.is_cell_watered = &IsCellWatered;
    h.fire_coverage = &FireCoverage;
    h.park_value = &ParkValue;
    h.road_access_value = &RoadAccessValue;
    h.city_cell_counts = &CityCellCounts;
    h.find_network_tile = &FindNetworkTile;
    h.network_is_of_type = &NetworkIsOfType;
    h.demolish_lot = &DemolishLot;
    h.redisplay_terrain = &RedisplayTerrain;
    return h;
}

fc4_tract_host MakeTractReadOnlyHost(TractContext* ctx) {
    ctx->readOnly = true;
    ctx->gated = false;
    fc4_tract_host h = MakeTractHost(ctx);
    h.acquire_services = nullptr;
    h.add_agent = nullptr;
    h.acquire_message_server = nullptr;
    h.release_message_server = nullptr;
    h.remove_agent = nullptr;
    h.release_services = nullptr;
    h.claim_construction_resources = nullptr;
    h.release_construction_resources = nullptr;
    h.start_construction = nullptr;
    h.abandon_building = nullptr;
    h.reoccupy_building = nullptr;
    h.create_lot_if_possible = nullptr;
    h.delete_lot = nullptr;
    h.lot_set_facing = nullptr;
    h.lot_set_state = nullptr;
    h.lot_set_population = nullptr;
    h.register_configuration_filter = nullptr;
    h.unregister_configuration_filter = nullptr;
    h.demolish_lot = nullptr;
    h.redisplay_terrain = nullptr;
    return h;
}

void TractReleaseHeld(TractContext* ctx) {
    std::vector<cIGZUnknown*> held;
    std::vector<cIGZUnknown*> deleted;
    std::vector<fc4::GameList<cISC4Lot*, fc4::TractNodeAllocator>*> lists;
    held.swap(ctx->held);
    deleted.swap(ctx->deleted);
    // The configurations' references (unaudited, see ConfigurationsByFilter).
    std::vector<cIGZUnknown*> configs;
    configs.swap(ctx->heldConfigs);
    for (cIGZUnknown* o : configs) {
        if (o != nullptr) o->Release();
    }
    configs.clear();
    if (ctx->heldConfigs.empty()) configs.swap(ctx->heldConfigs);  // the storage, for the next Grow
    lists.swap(ctx->farmLists);
    auto isDeleted = [&](cIGZUnknown* o) { return std::find(deleted.begin(), deleted.end(), o) != deleted.end(); };
    // A lot the core deleted may lose its last reference here: that free is
    // expected (the lot manager dropped its own reference in DeleteLot).
    auto drop = [&](cIGZUnknown* o) {
        if (o == nullptr) return;
        if (isDeleted(o)) {
            FC4_RELEASE_CREATED(o);
        } else {
            FC4_RELEASE(o);
        }
    };
    for (cIGZUnknown* o : held) drop(o);
    for (auto* l : lists) {
        for (cISC4Lot*& lot : *l) {
            drop(reinterpret_cast<cIGZUnknown*>(lot));
            lot = nullptr;
        }
        delete l;
    }
    for (cIGZUnknown* o : deleted) FC4_RELEASE_CREATED(o);
}

void TractReleaseServices(TractContext* ctx) {
    if (ctx->readOnly) return;
    // §1.4 steps 1, 3 and 5 (the filters and the manager, step 4, are the
    // core's unregister calls and this release).
    ReleaseSlot(ctx->msgServer);
    ReleaseSlot(ctx->city);
    ReleaseSlot(ctx->bds);
    ReleaseSlot(ctx->landValue);
    ReleaseSlot(ctx->lotDeveloper);
    ReleaseSlot(ctx->configs);
    ReleaseSlot(ctx->lots);
    ReleaseSlot(ctx->plumbing);
    ReleaseSlot(ctx->power);
    ReleaseSlot(ctx->traffic);
    ReleaseSlot(ctx->zones);
}

void TractFreeFilters(TractContext* ctx) {
    std::vector<cFC4TractFilter*> filters;
    filters.swap(ctx->filters);
    cISC4LotConfigurationManager* m = ctx->configs != nullptr ? ctx->configs : ctx->testConfigs;
    for (cFC4TractFilter* f : filters) {
        if (m != nullptr) fc4::AsGame(m)->UnregisterLotConfigurationFilter(f);  // slot 19
        ++ctx->filtersUnregistered;
        delete f;
    }
}

}  // namespace fc4
