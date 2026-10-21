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

// cFC4LotConfigManager: the lot configuration manager's GZCOM class over the
// fc4_lot_config_manager core (docs/specs/prop-lotconfig.md Part B). See
// cFC4LotConfigManager.h.

#include "cFC4LotConfigManager.h"

#include "FC4Config.h"
#include "FC4GameList.h"
#include "FC4Log.h"
#include "FC4RefAudit.h"
#include "FC4Streams.h"
#include "cFC4LotConfiguration.h"
#include "cFC4LotDeveloper.h"

#include "GZServPtrs.h"
#include "cGZPersistResourceKey.h"
#include "cIGZApp.h"
#include "cIGZCOM.h"
#include "cIGZFrameWork.h"
#include "cIGZMessage2.h"
#include "cIGZMessageServer2.h"
#include "cIGZPersistResourceKeyList.h"
#include "cIGZPersistResourceManager.h"
#include "cISC4App.h"
#include "SC4Vector.h"
#include "cISC4BuildingDevelopmentSimulator.h"
#include "cISC4City.h"
#include "cISC4FloraSimulator.h"
#include "cISC4LotConfiguration.h"
#include "cISC4LotConfigurationFilter.h"
#include "cSC4LotConfigurationObject.h"
#include "cISC4LotConfigurationManager.h"
#include "cISC4PropManager.h"
#include "cRZCOMDllDirector.h"

#include <new>
#include <vector>

namespace {

cISC4City* AppCity() {
    cIGZFrameWork* fw = RZGetFrameWork();
    cIGZApp* app = fw ? fw->Application() : nullptr;
    cISC4App* sc4 = nullptr;
    if (app == nullptr || !app->QueryInterface(GZIID_cISC4App, reinterpret_cast<void**>(&sc4)) || sc4 == nullptr) {
        return nullptr;
    }
    FC4_ACQUIRED(sc4, "QueryInterface");
    FC4_RELEASE(sc4);  // the application lives for the process
    return sc4->GetCity();
}

cFC4LotConfigManager* Self(void* u) { return static_cast<cFC4LotConfigManager*>(u); }

uint64_t Tok(const void* p) { return static_cast<uint64_t>(reinterpret_cast<uintptr_t>(p)); }

cISC4LotConfiguration* Cfg(uint64_t t) {
    return reinterpret_cast<cISC4LotConfiguration*>(static_cast<uintptr_t>(t));
}

// The three query kinds of fc4_lot_config_ids / _configurations.
// The game's hash_set<cISC4LotConfiguration*> (allocators.md; hasher PL20).
using LotConfigSet = fc4::GameHashSetU32<fc4::StlNodeAllocator, fc4::PointerHashU32>;
constexpr uint32_t kByFilter = 0;
constexpr uint32_t kByFamily = 1;
constexpr uint32_t kBySize = 2;

// ---- the host callbacks (the read-only half: the shapes of FC4PropLotShadow.cpp's LcmHost) ------------

uint32_t DetailLevel(void*) {
    // §B3.1: the catalogue is built at the city's detail level.
    return 3;
}

bool ForEachExemplarKey(void*, fc4_key_visitor visit, void* ctx) {
    // The exemplar keys of type 0x6534284A (fc4.h), the same list the prop
    // manager's catalogue walks.
    cIGZPersistResourceManagerPtr rm;
    if (!rm) return false;
    cIGZPersistResourceKeyList* l = nullptr;
    rm->GetAvailableResourceList(&l, nullptr);
    if (l == nullptr) return false;
    FC4_ACQUIRED(l, "GetAvailableResourceList");
    const uint32_t n = l->Size();  // cIGZPersistResourceKeyList slot 9
    for (uint32_t i = 0; i < n; ++i) {
        const cGZPersistResourceKey k = l->GetKey(i);
        if (k.type == 0x6534284A) visit(ctx, k.type, k.group, k.instance);
    }
    FC4_RELEASE_CREATED(l);
    return true;
}

void RegisterShutdownCallback(void*) {
    // B4.2: the core asks to be told at application shutdown so its unload can
    // run. app slot 69 is not in docs/abi (the cFC4Development.cpp DG7 choice),
    // so the callback is not registered with the application: the director's
    // PreAppShutdown runs fc4::LotConfigManagerAppShutdown, which calls
    // fc4_lot_config_unload ONCE over the process catalogue (glue15).
    fc4::Log("lotcfg: the core registered an application shutdown callback (unload at PreAppShutdown)");
}

void UnregisterShutdownCallback(void*) {
    fc4::Log("lotcfg: the core unregistered its application shutdown callback");
}

uint64_t CreateConfiguration(void*) {
    // GLUE21-1, answered (lot-configuration.md 1.1): cSC4LotConfiguration is
    // NOT GZCOM-creatable - the class ID 0xA99062B4 exists only for
    // serialization refs - and the named route to the allocate-and-construct
    // pair is the manager's own helper, which is exactly what a replacement
    // manager replaces. So the replacement allocates its own
    // cISC4LotConfiguration-shaped object (option 1 of 1.1): cFC4LotConfiguration,
    // the 34-slot table of 8 over the layout of 2, filled by the Load of 3.
    // The creation reference is ours to hand over (B3.2): the original helper
    // AddRefs right after the construct, the manager's slot 6 takes its own
    // reference (B3.4) and slot 5 releases the creation one - the catalogue
    // keeps the object at count 1. Without it the catalogue's release after
    // the add reaches 0 and deletes the object out from under the catalogue.
    cFC4LotConfiguration* c = new cFC4LotConfiguration();
    c->AddRef();
    return Tok(c);
}

bool LoadConfiguration(void*, uint64_t cfg, uint32_t t, uint32_t g, uint32_t i, uint32_t detail) {
    cISC4LotConfiguration* c = Cfg(cfg);
    if (c == nullptr) return false;
    cGZPersistResourceKey key;
    key.type = t;
    key.group = g;
    key.instance = i;
    // lot-configuration.md 3: Load(key, detail), slot 31.
    return c->Load(key, static_cast<uint8_t>(detail));
}

uint32_t ConfigId(void*, uint64_t cfg) {
    cISC4LotConfiguration* c = Cfg(cfg);
    return c != nullptr ? c->GetID() : 0;  // configuration slot 3
}

bool ConfigBelongsToFamily(void*, uint64_t cfg, uint32_t family) {
    cISC4LotConfiguration* c = Cfg(cfg);
    return c != nullptr && c->BelongsToFamily(family);  // slot 5
}

void ConfigFamilies(void*, uint64_t cfg, fc4_u32s_sink out, void* ctx) {
    cISC4LotConfiguration* c = Cfg(cfg);
    std::vector<uint32_t> v;
    if (c != nullptr) {
        if (SC4Vector<uint32_t>* f = c->GetFamilyIDArray()) {  // slot 6
            for (uint32_t id : *f) v.push_back(id);
        }
    }
    if (out != nullptr) out(ctx, v.empty() ? nullptr : v.data(), v.size());
}

void ConfigSize(void*, uint64_t cfg, uint32_t* x, uint32_t* z) {
    uint32_t w = 0, d = 0;
    if (cISC4LotConfiguration* c = Cfg(cfg)) c->GetSize(w, d);  // slot 7
    if (x != nullptr) *x = w;
    if (z != nullptr) *z = d;
}

void ConfigObjects(void*, uint64_t cfg, fc4_lotdev_cfg_object_callback visit, void* ctx) {
    cISC4LotConfiguration* c = Cfg(cfg);
    if (c == nullptr || visit == nullptr) return;
    SC4Vector<cSC4LotConfigurationObject>* v = c->GetLotConfigurationObjectArray();  // slot 23
    if (v == nullptr) return;
    std::vector<uint32_t> ids;
    for (cSC4LotConfigurationObject* o = v->begin(); o != v->end(); ++o) {
        const fc4_lotdev_cfg_object co = fc4::LotDevObjectOf(o, ids);
        visit(ctx, &co);
    }
}

void SetConfigObjects(void*, uint64_t cfg, const fc4_lotdev_cfg_object* objects, size_t n) {
    // The editing helpers (B5.7) write the object array back. The
    // configuration class's setter is not a documented slot (lot-configuration.md
    // 5.4-5.7 cover the manager's helpers only), so the array is left alone
    // and the call is logged: the editor path is not this lane's.
    fc4::Log("lotcfg: SetConfigObjects %u objects (the configuration class's setter is not a documented slot)",
             static_cast<unsigned>(n));
    (void)cfg;
    (void)objects;
}

bool ConfigPrimaryBuilding(void*, uint64_t cfg, fc4_lotdev_cfg_object_callback visit, void* ctx) {
    cISC4LotConfiguration* c = Cfg(cfg);
    if (c == nullptr) return false;
    cSC4LotConfigurationObject* o = c->GetPrimaryBuildingConfigurationObject();  // slot 25
    if (o == nullptr) return false;
    if (visit != nullptr) {
        std::vector<uint32_t> ids;
        const fc4_lotdev_cfg_object co = fc4::LotDevObjectOf(o, ids);
        visit(ctx, &co);
    }
    return true;
}

void SetConfigBuildingInfo(void*, uint64_t cfg, const fc4_lot_config_building_info* info) {
    // B4.3 PostCityInit fills the building information: +0x3E, +0x40, +0x80
    // and +0x8C, written directly (lot-configuration.md 4 - no slot). On our
    // own configurations (GLUE21-1) the fields are ordinary members of
    // cFC4LotConfiguration; on the original's (the shadow's read-only walk)
    // the object is not ours and the write is skipped, as before.
    if (info == nullptr || cfg == 0) return;
    auto* c = reinterpret_cast<cFC4LotConfiguration*>(static_cast<uintptr_t>(cfg));
    if (!fc4::IsOurObject(c)) {
        static bool noted = false;
        if (!noted) {
            noted = true;
            fc4::Log("lotcfg: SetConfigBuildingInfo on a non-FreeCity4 configuration (skipped)");
        }
        return;
    }
    std::vector<uint32_t> styles(info->groups, info->groups + info->n_groups);
    c->SetBuildingInfo(info->min_capacity, info->max_capacity, std::move(styles), info->non_monopoly != 0);
}

void ConfigRetainingWalls(void*, uint64_t cfg, fc4_u32s_sink out, void* ctx) {
    cISC4LotConfiguration* c = Cfg(cfg);
    std::vector<uint32_t> v;
    if (c != nullptr) {
        if (SC4Vector<uint32_t>* w = c->GetRetainingWallIDArray()) {  // slot 31
            for (uint32_t id : *w) v.push_back(id);
        }
    }
    if (out != nullptr) out(ctx, v.empty() ? nullptr : v.data(), v.size());
}

void SetConfigRetainingWalls(void*, uint64_t cfg, const uint32_t* ids, size_t n) {
    fc4::Log("lotcfg: SetConfigRetainingWalls %u (the configuration class's setter is not a documented slot)",
             static_cast<unsigned>(n));
    (void)cfg;
    (void)ids;
}

bool IsRetainingWallValid(void*, uint32_t id) { return id != 0; }

uint64_t CreateGenericConfiguration(void*, uint32_t type, uint32_t, uint32_t, const fc4_lotdev_cfg_object*,
                                    size_t) {
    // Same route as CreateConfiguration (GLUE21-1, answered): our own
    // cISC4LotConfiguration-shaped object. The editor's generic
    // configuration (B5.7) fills itself afterwards through the editing
    // helpers.
    return CreateConfiguration(nullptr);
}

uint64_t AcquireService(void*, uint32_t service) {
    // fc4.h: the three services the core asks for.
    cISC4City* c = AppCity();
    cIGZUnknown* p = nullptr;
    if (service == FC4_LCM_SERVICE_BUILDING_DEVELOPMENT && c != nullptr) {
        p = reinterpret_cast<cIGZUnknown*>(c->GetBuildingDevelopmentSimulator());
    } else if (service == FC4_LCM_SERVICE_FLORA && c != nullptr) {
        p = reinterpret_cast<cIGZUnknown*>(c->GetFloraSimulator());
    } else if (service == FC4_LCM_SERVICE_PROP_MANAGER && c != nullptr) {
        p = reinterpret_cast<cIGZUnknown*>(c->GetPropManager());
    }
    return Tok(p);
}

void Subscribe(void* u, uint32_t msgType) {
    cFC4LotConfigManager* s = Self(u);
    cIGZMessageServer2Ptr ms;
    if (!ms || s == nullptr) return;
    ms->AddNotification(s->Target(), msgType);
}

void Unsubscribe(void* u, uint32_t msgType) {
    cFC4LotConfigManager* s = Self(u);
    cIGZMessageServer2Ptr ms;
    if (!ms || s == nullptr) return;
    ms->RemoveNotification(s->Target(), msgType);
}

void AddRefToken(void*, uint64_t token) {
    cIGZUnknown* o = reinterpret_cast<cIGZUnknown*>(static_cast<uintptr_t>(token));
    if (o != nullptr) FC4_ADDREF(o);
}

void ReleaseToken(void*, uint64_t token) {
    cIGZUnknown* o = reinterpret_cast<cIGZUnknown*>(static_cast<uintptr_t>(token));
    if (o != nullptr) FC4_RELEASE(o);
}

bool FilterMatches(void*, uint64_t filter, uint64_t cfg) {
    cISC4LotConfigurationFilter* f =
        reinterpret_cast<cISC4LotConfigurationFilter*>(static_cast<uintptr_t>(filter));
    cISC4LotConfiguration* c = Cfg(cfg);
    // [abi-ret] cISC4LotConfigurationFilter slot 3 (interfaces.tsv
    // cSC4LotConfigurationFilter::IsLotConfigurationIncluded) is eax-or-void:
    // the SDK declares bool. The core treats "no filter" as "no match".
    return f != nullptr && c != nullptr && f->IsLotConfigurationIncluded(c);
}

fc4_props* BuildingExemplar(void*, uint32_t type) {
    // §B5: the building development simulator's exemplar for a building type.
    cISC4City* c = AppCity();
    cISC4BuildingDevelopmentSimulator* b =
        c ? reinterpret_cast<cISC4BuildingDevelopmentSimulator*>(c->GetBuildingDevelopmentSimulator()) : nullptr;
    // The BDS's exemplar lookup is not a documented slot (the shadow reads the
    // same way: no exemplar). [choice] nothing is answered.
    (void)b;
    (void)type;
    return nullptr;
}

bool IsBuildingTypeValid(void*, uint32_t id) {
    cISC4City* c = AppCity();
    cISC4BuildingDevelopmentSimulator* b =
        c ? reinterpret_cast<cISC4BuildingDevelopmentSimulator*>(c->GetBuildingDevelopmentSimulator()) : nullptr;
    // slot 15; [abi-ret] eax-or-void (interfaces.tsv cISC4BuildingDevelopmentSimulator 15)
    return b != nullptr && b->IsBuildingTypeValid(id);
}

bool IsBuildingFamilyTypeValid(void*, uint32_t id) {
    cISC4City* c = AppCity();
    cISC4BuildingDevelopmentSimulator* b =
        c ? reinterpret_cast<cISC4BuildingDevelopmentSimulator*>(c->GetBuildingDevelopmentSimulator()) : nullptr;
    // slot 16; [abi-ret] eax-or-void (interfaces.tsv cISC4BuildingDevelopmentSimulator 16)
    return b != nullptr && b->IsBuildingFamilyTypeValid(id);
}

uint32_t BuildingTypesByFamily(void*, uint32_t id, fc4_u32s_sink out, void* ctx) {
    cISC4City* c = AppCity();
    cISC4BuildingDevelopmentSimulator* b =
        c ? reinterpret_cast<cISC4BuildingDevelopmentSimulator*>(c->GetBuildingDevelopmentSimulator()) : nullptr;
    if (b == nullptr) return 0;
    fc4::GameVector<uint32_t> v;
    // slot 18; [abi-ret] eax-or-void (interfaces.tsv cISC4BuildingDevelopmentSimulator 18): the count
    const uint32_t n = b->GetBuildingTypesByFamily(reinterpret_cast<SC4Vector<uint32_t>&>(v), id);
    std::vector<uint32_t> s;
    for (uint32_t* p = v.begin(); p != v.end(); ++p) s.push_back(*p);
    if (out != nullptr) out(ctx, s.empty() ? nullptr : s.data(), s.size());
    return n;
}

uint32_t BuildingTypesIntoSet(void* u, uint32_t id, fc4_u32s_sink out, void* ctx) {
    return BuildingTypesByFamily(u, id, out, ctx);
}

bool IsBuildingInFamily(void*, uint32_t type, uint32_t family) {
    cISC4City* c = AppCity();
    cISC4BuildingDevelopmentSimulator* b =
        c ? reinterpret_cast<cISC4BuildingDevelopmentSimulator*>(c->GetBuildingDevelopmentSimulator()) : nullptr;
    // slot 20; [abi-ret] eax-or-void (interfaces.tsv cISC4BuildingDevelopmentSimulator 20)
    return b != nullptr && b->IsBuildingInFamily(type, family);
}

uint16_t BuildingCapacity(void*, uint32_t type) {
    cISC4City* c = AppCity();
    cISC4BuildingDevelopmentSimulator* b =
        c ? reinterpret_cast<cISC4BuildingDevelopmentSimulator*>(c->GetBuildingDevelopmentSimulator()) : nullptr;
    // The building capacity table is not a documented BDS slot (B5.7).
    (void)b;
    (void)type;
    return 0;
}

bool BuildingFootprint(void*, uint32_t type, float* w, float* d) {
    cISC4City* c = AppCity();
    cISC4BuildingDevelopmentSimulator* b =
        c ? reinterpret_cast<cISC4BuildingDevelopmentSimulator*>(c->GetBuildingDevelopmentSimulator()) : nullptr;
    // The building footprint is not a documented BDS slot (B5.5).
    (void)b;
    (void)type;
    if (w != nullptr) *w = 0.0f;
    if (d != nullptr) *d = 0.0f;
    return false;
}

bool FloraRadius(void*, uint32_t id, float* radius) {
    cISC4City* c = AppCity();
    cISC4FloraSimulator* f = c ? c->GetFloraSimulator() : nullptr;
    // The flora radius is not a documented slot (B5.5).
    (void)f;
    (void)id;
    if (radius != nullptr) *radius = 0.0f;
    return false;
}

bool IsFloraTypeValid(void*, uint32_t id) {
    cISC4City* c = AppCity();
    cISC4FloraSimulator* f = c ? c->GetFloraSimulator() : nullptr;
    // slot 11; [abi-ret] eax-or-void (interfaces.tsv cISC4FloraSimulator 11)
    return f != nullptr && f->IsFloraTypeValid(id);
}

uint32_t FloraTypesIntoSet(void*, uint32_t id, fc4_u32s_sink out, void* ctx) {
    cISC4City* c = AppCity();
    cISC4FloraSimulator* f = c ? c->GetFloraSimulator() : nullptr;
    if (f == nullptr) return 0;
    fc4::GameHashSetU32<> set(100);
    // slot 14; [abi-ret] eax-or-void (interfaces.tsv cISC4FloraSimulator 14): the count
    const uint32_t n = f->GetFloraTypesByFamily(reinterpret_cast<intptr_t&>(set), id);
    std::vector<uint32_t> v;
    for (size_t b = 0; b < set.bucket_count(); ++b) {
        for (auto* p = set.bucket(b); p != nullptr; p = p->next) v.push_back(p->value);
    }
    if (out != nullptr) out(ctx, v.empty() ? nullptr : v.data(), v.size());
    return n;
}

bool IsPropTypeValid(void*, uint32_t id) {
    cISC4City* c = AppCity();
    cISC4PropManager* p = c ? c->GetPropManager() : nullptr;
    return p != nullptr && p->IsPropTypeValid(id);  // slot 8
}

uint32_t PropTypesByFamily(void*, uint32_t id, fc4_u32s_sink out, void* ctx) {
    cISC4City* c = AppCity();
    cISC4PropManager* p = c ? c->GetPropManager() : nullptr;
    if (p == nullptr) return 0;
    fc4::GameVector<uint32_t> v;
    const uint32_t n = p->GetPropTypesByFamily(reinterpret_cast<SC4Vector<uint32_t>&>(v), id);  // slot 9
    std::vector<uint32_t> s;
    for (uint32_t* q = v.begin(); q != v.end(); ++q) s.push_back(*q);
    if (out != nullptr) out(ctx, s.empty() ? nullptr : s.data(), s.size());
    return n;
}

bool IsPropFamilyTypeValid(void*, uint32_t id) {
    cISC4City* c = AppCity();
    cISC4PropManager* p = c ? c->GetPropManager() : nullptr;
    return p != nullptr && p->IsPropFamilyTypeValid(id);  // slot 11
}

bool PropSize(void*, uint32_t id, float* x, float* z) {
    if (x == nullptr || z == nullptr) return false;
    cISC4City* c = AppCity();
    cISC4PropManager* p = c ? c->GetPropManager() : nullptr;
    if (p == nullptr) return false;
    const cISC4PropOccupant::PropProfile* prof = p->GetPropProfilePtr(id);  // slot 14
    if (prof == nullptr) return false;
    *x = 1.0f;
    *z = 1.0f;
    return true;
}

uint64_t ResolveRef(void*, uint64_t token, uint32_t iid) {
    cIGZUnknown* o = reinterpret_cast<cIGZUnknown*>(static_cast<uintptr_t>(token));
    if (o == nullptr) return 0;
    void* q = nullptr;
    if (!o->QueryInterface(iid, &q) || q == nullptr) return 0;
    return Tok(q);  // one reference for the caller
}

fc4_lot_config_host LcmHost(cFC4LotConfigManager* s) {
    fc4_lot_config_host h{};
    h.struct_size = sizeof(h);
    h.user = s;
    h.detail_level = &DetailLevel;
    h.for_each_exemplar_key = &ForEachExemplarKey;
    h.register_shutdown_callback = &RegisterShutdownCallback;
    h.unregister_shutdown_callback = &UnregisterShutdownCallback;
    h.create_configuration = &CreateConfiguration;
    h.load_configuration = &LoadConfiguration;
    h.config_id = &ConfigId;
    h.config_belongs_to_family = &ConfigBelongsToFamily;
    h.config_families = &ConfigFamilies;
    h.config_size = &ConfigSize;
    h.config_objects = &ConfigObjects;
    h.set_config_objects = &SetConfigObjects;
    h.config_primary_building = &ConfigPrimaryBuilding;
    h.set_config_building_info = &SetConfigBuildingInfo;
    h.config_retaining_walls = &ConfigRetainingWalls;
    h.set_config_retaining_walls = &SetConfigRetainingWalls;
    h.is_retaining_wall_valid = &IsRetainingWallValid;
    h.create_generic_configuration = &CreateGenericConfiguration;
    h.acquire_service = &AcquireService;
    h.subscribe = &Subscribe;
    h.unsubscribe = &Unsubscribe;
    h.add_ref = &AddRefToken;
    h.release = &ReleaseToken;
    h.filter_matches = &FilterMatches;
    h.building_exemplar = &BuildingExemplar;
    h.is_building_type_valid = &IsBuildingTypeValid;
    h.is_building_family_type_valid = &IsBuildingFamilyTypeValid;
    h.building_types_by_family = &BuildingTypesByFamily;
    h.building_types_into_set = &BuildingTypesIntoSet;
    h.is_building_in_family = &IsBuildingInFamily;
    h.building_capacity = &BuildingCapacity;
    h.building_footprint = &BuildingFootprint;
    h.flora_radius = &FloraRadius;
    h.is_flora_type_valid = &IsFloraTypeValid;
    h.flora_types_into_set = &FloraTypesIntoSet;
    h.is_prop_type_valid = &IsPropTypeValid;
    h.prop_types_by_family = &PropTypesByFamily;
    h.is_prop_family_type_valid = &IsPropFamilyTypeValid;
    h.prop_size = &PropSize;
    h.resolve_ref = &ResolveRef;
    return h;
}

// Fills the game's set (STLport hash_set<uint32_t>, B) from the core's ID list.
void FillSet(void* set, const std::vector<uint32_t>& ids) {
    auto* s = reinterpret_cast<fc4::GameHashSetU32<>*>(set);
    if (s == nullptr) return;
    for (uint32_t id : ids) s->insert(id);
}

}  // namespace

cFC4LotConfigManager::~cFC4LotConfigManager() = default;

bool cFC4LotConfigManager::QueryInterface(uint32_t riid, void** ppvObj) {
    if (ppvObj == nullptr) return false;
    // B1: `1` and 0xC8EDBEC0 -> +0; 0xE4FDA3D4 -> +8; the message target is
    // not offered. A successful QueryInterface adds a reference (the COM
    // contract; cFC4Simulator::QueryInterfaceImpl): the class store counts on
    // it - its loader takes the factory result as the caller's reference,
    // registers the object in the segment's table with its own AddRef and
    // releases its reference after Read. Without the AddRef the two
    // cancel and the store's release destroys the object, leaving the city's
    // component-9 pointer dangling into city Init (the 0x45BF49 fault).
    switch (riid) {
        case kFC4_IID_cISC4LotConfigManager:
        case GZIID_cIGZUnknown:
            *ppvObj = Primary();
            break;
        case kFC4_IID_cIGZSerializableLcm:
            *ppvObj = static_cast<cIGZSerializable*>(this);
            break;
        default:
            *ppvObj = nullptr;
            return false;
    }
    AddRef();
    return true;
}

uint32_t cFC4LotConfigManager::AddRef() { return ++refCount_; }

uint32_t cFC4LotConfigManager::Release() {
    if (refCount_ == 0 || --refCount_ != 0) return refCount_;
    if (destroying_) return 0;
    destroying_ = true;
    Shutdown();  // B1: the last Release calls Shutdown then the deleting dtor
    delete this;
    return 0;
}

// `Read` runs before `Init` (HANDOFF 5.1): the core is created in the
// constructor so a saved record can be read before Init is called. Nothing
// here sizes saved state from the city.
cFC4LotConfigManager::cFC4LotConfigManager() {
    host_ = LcmHost(this);
    core_ = fc4_lot_config_create(&host_, fc4::LotConfigCatalogueProcess(), 0);
}

bool cFC4LotConfigManager::Init() {
    if (initialised_) return true;
    if (core_ == nullptr) return false;
    const bool ok = fc4_lot_config_init(core_);
    initialised_ = ok;
    return ok;
}

bool cFC4LotConfigManager::Shutdown() {
    if (!initialised_ && core_ == nullptr) return true;
    const bool ok = core_ != nullptr ? fc4_lot_config_shutdown(core_) : true;
    if (core_ != nullptr) {
        fc4_lot_config_destroy(core_);
        core_ = nullptr;
    }
    initialised_ = false;
    return ok;
}

bool cFC4LotConfigManager::AddLotConfigurationByKey(cGZPersistResourceKey const& key) {
    return core_ != nullptr &&
           fc4_lot_config_add_by_key(core_, key.type, key.group, key.instance);
}

bool cFC4LotConfigManager::AddLotConfiguration(cISC4LotConfiguration* config, bool add) {
    return core_ != nullptr && fc4_lot_config_add_configuration(core_, Tok(config), add);
}

bool cFC4LotConfigManager::RemoveLotConfiguration(uint32_t id) {
    return core_ != nullptr && fc4_lot_config_remove_configuration(core_, id);
}

cISC4LotConfiguration* cFC4LotConfigManager::GetLotConfiguration(uint32_t id) {
    if (core_ == nullptr) return nullptr;
    // B7 slot 8: no reference.
    return Cfg(fc4_lot_config_configuration(core_, id));
}

uint32_t cFC4LotConfigManager::GetLotConfigurationsByFilter(
    fc4::GameHashSetU32<fc4::TractNodeAllocator>& configs, cISC4LotConfigurationFilter* filter) {
    if (core_ == nullptr) return 0;
    std::vector<uint64_t> v;
    fc4_lot_config_configurations(core_, kByFilter, Tok(filter), 0,
                                  [](void* ctx, uint64_t t) {
                                      static_cast<std::vector<uint64_t>*>(ctx)->push_back(t);
                                  },
                                  &v);
    // §B5.2 slot 9 reference rule (PL19): the core took exactly one
    // reference per returned configuration on this call, cache hit or not
    // (with_refs); the game's growth pass releases each set element once.
    // No second add here. The set is the game's hash_set<cISC4LotConfiguration*>,
    // so values go to the buckets of its pointer hasher (PointerHashU32, PL20).
    auto* s = reinterpret_cast<LotConfigSet*>(&configs);
    if (s != nullptr) {
        for (uint64_t t : v) s->insert(static_cast<uint32_t>(t & 0xFFFFFFFFu));
    }
    return static_cast<uint32_t>(v.size());
}

uint32_t cFC4LotConfigManager::GetLotConfigurationsByFamily(void* configs, uint32_t family) {
    if (core_ == nullptr) return 0;
    std::vector<uint64_t> v;
    fc4_lot_config_configurations(core_, kByFamily, family, 0,
                                  [](void* ctx, uint64_t t) {
                                      static_cast<std::vector<uint64_t>*>(ctx)->push_back(t);
                                  },
                                  &v);
    // §B5.2: one reference per match, taken by the core (see slot 9).
    auto* s = reinterpret_cast<LotConfigSet*>(configs);
    if (s != nullptr) {
        for (uint64_t t : v) s->insert(static_cast<uint32_t>(t & 0xFFFFFFFFu));
    }
    return static_cast<uint32_t>(v.size());
}

uint32_t cFC4LotConfigManager::GetLotConfigurationsBySize(void* configs, uint32_t x, uint32_t z) {
    if (core_ == nullptr) return 0;
    std::vector<uint64_t> v;
    fc4_lot_config_configurations(core_, kBySize, x, z,
                                  [](void* ctx, uint64_t t) {
                                      static_cast<std::vector<uint64_t>*>(ctx)->push_back(t);
                                  },
                                  &v);
    // §B5.2: one reference per match, taken by the core (see slot 9).
    auto* s = reinterpret_cast<LotConfigSet*>(configs);
    if (s != nullptr) {
        for (uint64_t t : v) s->insert(static_cast<uint32_t>(t & 0xFFFFFFFFu));
    }
    return static_cast<uint32_t>(v.size());
}

void cFC4LotConfigurationReleaseWalk(uint32_t v) {
    reinterpret_cast<cIGZUnknown*>(static_cast<uintptr_t>(v))->Release();
}

void cFC4LotConfigManager::ClearLotConfigurationSet(void* configs) {
    // §B7 slot 12: release every element once and empty the set. The nodes
    // are freed here (to the same allocator the game's set uses) and every
    // bucket head is nulled, so the game's own destructor afterwards finds
    // empty chains and only frees the bucket buffer - no double free.
    auto* s = reinterpret_cast<LotConfigSet*>(configs);
    if (s == nullptr) return;
    s->ForEach(&cFC4LotConfigurationReleaseWalk);
    s->ClearNodes();
}

namespace {

uint32_t IdsBy(void* u, uint32_t which, uint64_t a, uint32_t b, void* out) {
    auto* self = static_cast<cFC4LotConfigManager*>(u);
    if (self == nullptr || self->Core() == nullptr) return 0;
    std::vector<uint32_t> v;
    const uint32_t n = fc4_lot_config_ids(self->Core(), which, a, b, nullptr, 0);
    if (n != 0) {
        v.resize(n);
        fc4_lot_config_ids(self->Core(), which, a, b, v.data(), v.size());
    }
    FillSet(out, v);
    return n;
}

}  // namespace

uint32_t cFC4LotConfigManager::GetLotConfigurationIDsByFilter(void* ids,
                                                             cISC4LotConfigurationFilter* filter) {
    return IdsBy(this, kByFilter, Tok(filter), 0, ids);
}

uint32_t cFC4LotConfigManager::GetLotConfigurationIDsByFamily(void* ids, uint32_t family) {
    return IdsBy(this, kByFamily, family, 0, ids);
}

uint32_t cFC4LotConfigManager::GetLotConfigurationIDsBySize(void* ids, uint32_t x, uint32_t z) {
    return IdsBy(this, kBySize, x, z, ids);
}

bool cFC4LotConfigManager::LotConfigurationHasBuildingFamily(cISC4LotConfiguration* config, uint32_t family,
                                                             cSC4LotConfigurationObject** object) {
    if (object != nullptr) *object = nullptr;
    if (core_ == nullptr) return false;
    uint32_t index = 0;
    const bool ok = fc4_lot_config_has_building(core_, Tok(config), family, false, &index);
    if (ok && object != nullptr) *object = fc4::LotDevObjectAt(config, index);
    return ok;
}

bool cFC4LotConfigManager::LotConfigurationHasBuildingType(cISC4LotConfiguration* config, uint32_t type,
                                                           cSC4LotConfigurationObject** object) {
    if (object != nullptr) *object = nullptr;
    if (core_ == nullptr) return false;
    uint32_t index = 0;
    const bool ok = fc4_lot_config_has_building(core_, Tok(config), type, true, &index);
    if (ok && object != nullptr) *object = fc4::LotDevObjectAt(config, index);
    return ok;
}

void cFC4LotConfigManager::RegisterLotConfigurationFilter(cISC4LotConfigurationFilter* filter) {
    if (core_ != nullptr) fc4_lot_config_register_filter(core_, Tok(filter));
}

void cFC4LotConfigManager::UnregisterLotConfigurationFilter(cISC4LotConfigurationFilter* filter) {
    if (core_ != nullptr) fc4_lot_config_unregister_filter(core_, Tok(filter));
}

// The process-wide game-shaped hash containers of namespace fc4 (below).
namespace fc4 {
void RebuildConfigurationMap(fc4_lot_config_manager* core);
void RebuildFamilyIDSet(fc4_lot_config_manager* core);
void* ConfigurationMapTable();
void* FamilyIDSetTable();
void FreeGameHashTables();
}  // namespace fc4

void* cFC4LotConfigManager::GetLotConfigurationMap() {
    // §B2: the address of the process-wide configuration map. The game's
    // callers (the building development simulator's maximum density table,
    // building-development.md §4.7.4; the PlopAllLotTypes cheat, zones.md
    // §5.4; the shadow's map comparison) walk it read-only with no null
    // check, so a null answer faults (glue21 gate-5: SimCity 4.exe+0x4B0390,
    // the map's begin iterator on a null table). The map is rebuilt from the
    // core's catalogue on every call: the game-shaped STLport hash_map
    // (allocators.md hash_set shape, 12-byte nodes {next, key, value}),
    // one reference per configuration held here. The core's own map stays
    // the authority; the callers only walk.
    fc4::RebuildConfigurationMap(core_);
    return fc4::ConfigurationMapTable();
}

void* cFC4LotConfigManager::GetLotConfigurationFamilyIDSet() {
    // §B2: the address of the process-wide family set (B2's 0x00B216DC), as
    // the map above: rebuilt from the core, walked read-only.
    fc4::RebuildFamilyIDSet(core_);
    return fc4::FamilyIDSetTable();
}

bool cFC4LotConfigManager::CreateGenericLotConfiguration(cISC4LotConfiguration*& config, uint32_t id) {
    config = nullptr;
    if (core_ == nullptr) return false;
    const uint64_t t = fc4_lot_config_create_generic(core_, id);
    config = Cfg(t);
    return config != nullptr;
}

uint32_t cFC4LotConfigManager::FixLotConfiguration(cISC4LotConfiguration* config) {
    return core_ != nullptr ? fc4_lot_config_fix(core_, Tok(config)) : 0;
}

void cFC4LotConfigManager::GetMaxFootprintForBuildingFamilies(void const* families, cS3DRect2D& rect) {
    if (core_ == nullptr) return;
    const auto* ids = static_cast<const uint32_t*>(families);
    float out4[4] = {};
    if (!fc4_lot_config_max_footprint(core_, 0, ids, ids != nullptr ? 4 : 0, out4)) return;
    // cS3DRect2D is only forward-declared in the plugin (its layout is not in
    // docs/abi), and the shadow does not compare the footprints (B5.5). [choice]
    // the rectangle is left alone.
    (void)out4;
    (void)rect;
}

void cFC4LotConfigManager::GetMaxFootprintForFloraFamilies(void const* families, cS3DRect2D& rect) {
    if (core_ == nullptr) return;
    const auto* ids = static_cast<const uint32_t*>(families);
    float out4[4] = {};
    if (!fc4_lot_config_max_footprint(core_, 1, ids, ids != nullptr ? 4 : 0, out4)) return;
    // cS3DRect2D is only forward-declared in the plugin (its layout is not in
    // docs/abi), and the shadow does not compare the footprints (B5.5). [choice]
    // the rectangle is left alone.
    (void)out4;
    (void)rect;
}

void cFC4LotConfigManager::GetMaxFootprintForPropFamilies(void const* families, cS3DRect2D& rect) {
    if (core_ == nullptr) return;
    const auto* ids = static_cast<const uint32_t*>(families);
    float out4[4] = {};
    if (!fc4_lot_config_max_footprint(core_, 2, ids, ids != nullptr ? 4 : 0, out4)) return;
    // cS3DRect2D is only forward-declared in the plugin (its layout is not in
    // docs/abi), and the shadow does not compare the footprints (B5.5). [choice]
    // the rectangle is left alone.
    (void)out4;
    (void)rect;
}

void cFC4LotConfigManager::GetObjectIDArray(cSC4LotConfigurationObject* object, void* ids) {
    if (core_ == nullptr) return;
    // §B5.6: the caller's `vec` is an SC4Vector<uint32_t>& (the SDK header and
    // the shadow's GameU32s both say vector, NOT a hash set). The game reads
    // it back as a vector in CreateLotObjects (network-manager.md §8.6:
    // `ids[pick]` after RandomUint32Uniform(|ids|)) — filling it as a hash set
    // left begin=0 / garbage end and faulted the game's own CreateLotObjects
    // (PL17). Clear + push_back through the game allocator, like the shadow.
    auto* v = reinterpret_cast<fc4::GameVector<uint32_t>*>(ids);
    if (v == nullptr) return;
    v->clear();
    if (object == nullptr) return;
    // The id buffer must outlive `co`: co.ids points into it.
    std::vector<uint32_t> buf;
    const fc4_lotdev_cfg_object co = fc4::LotDevObjectOf(object, buf);
    std::vector<uint32_t> out;
    const uint32_t n = fc4_lot_config_object_id_array(core_, &co, nullptr, 0);
    if (n != 0) {
        out.resize(n);
        fc4_lot_config_object_id_array(core_, &co, out.data(), out.size());
    }
    for (uint32_t id : out) v->push_back(id);
}

bool cFC4LotConfigManager::DoMessage(cIGZMessage2* pMessage) {
    if (core_ == nullptr || pMessage == nullptr) return true;
    // B4.3: kSC4MessagePostCityInit (0x26D31EC1) -> true.
    fc4_lot_config_do_message(core_, pMessage->GetType());
    return true;
}

bool cFC4LotConfigManager::Write(cIGZOStream& gzOut) {
    if (core_ == nullptr) return false;
    fc4::OStreamAdapter a(gzOut);
    return fc4_lot_config_write(core_, a.Get());
}

bool cFC4LotConfigManager::Read(cIGZIStream& gzIn) {
    if (core_ == nullptr) return false;
    fc4::IStreamAdapter a(gzIn);
    return fc4_lot_config_read(core_, a.Get());
}

uint32_t cFC4LotConfigManager::GetGZCLSID() { return kFC4_CLSID_cSC4LotConfigManager; }

cIGZUnknown* CreateFC4LotConfigManager() {
    return static_cast<cIGZUnknown*>(
        static_cast<cFC4ISC4LotConfigurationManager*>(new cFC4LotConfigManager()));
}

namespace fc4 {

fc4_lot_config_catalogue* LotConfigCatalogueProcess() {
    static fc4_lot_config_catalogue* cat = nullptr;
    if (cat == nullptr) {
        cat = fc4_lot_config_catalogue_create();
        Log("lotcfg: process configuration catalogue %p", static_cast<void*>(cat));
    }
    return cat;
}

void LotConfigManagerAppShutdown() {
    // glue15: the unload callback runs ONCE per process, never per run. It
    // empties the map; the catalogue is destroyed right after.
    fc4_lot_config_catalogue* cat = LotConfigCatalogueProcess();
    if (cat == nullptr) return;
    fc4_lot_config_host h{};
    h.struct_size = sizeof(h);
    h.user = nullptr;
    if (fc4_lot_config_manager* m = fc4_lot_config_create(&h, cat, 0)) {
        fc4_lot_config_unload(m);
        fc4_lot_config_destroy(m);
    }
    fc4_lot_config_catalogue_destroy(cat);
    FreeGameHashTables();
}

// ---- the game-shaped process map and family set (§B2 0x00B216C8 / 0x00B216DC) ----
//
// Slot 20 / 21 hand the game the address of an STLport hash container
// (allocators.md hash_set shape: a 0x14-byte object, an empty functor member
// first, the bucket vector {begin, end, capacity_end} of Node*, the count
// last). The game's callers walk them read-only with no null check
// (building-development.md §4.7.4's maximum density table, zones.md §5.4's
// PlopAllLotTypes, the shadow's map comparison), so a null answer faults
// (glue21 gate-5: SimCity 4.exe+0x4B0390, the map's begin iterator on a null
// table). The map's nodes are 12 bytes {next, key, cISC4LotConfiguration*}
// (building-development.md §4.7.4 Inputs); the family set's are the 8-byte
// hash_set nodes {next, value}. The identity hash: a key lives in the chain
// of bucket key mod n, n = (end - begin) / 4, a prime from the table.
//
// Both are rebuilt from the core's catalogue on each slot 20 / 21 call: the
// core stays the authority and the game only walks. The tables live for the
// process so the addresses handed out stay valid; a rebuild empties and
// refills them in place. Each configuration in the map carries one reference
// (B2: the map holds one reference per configuration), taken on insert and
// released on erase or at shutdown.

struct GameHashTable {
    uint32_t functors;  // hasher/key-equal/extractor: empty, never read
    void** begin;       // bucket vector
    void** end;         // bucket count = (end - begin) / 4
    void** capacityEnd;
    uint32_t count;
};
static_assert(sizeof(GameHashTable) == 0x14, "the game's hash container is 0x14 bytes");

// The map's node: {next, key, value} (12 bytes). The set's: {next, value}
// (8 bytes) - the same layout with no key, read as node[1] = the value.
struct LcmHashNode {
    LcmHashNode* next;
    uint32_t key;
    cISC4LotConfiguration* value;
};
static_assert(sizeof(LcmHashNode) == 12, "the map node is 12 bytes");

struct GameSetNode {
    GameSetNode* next;
    uint32_t value;
};
static_assert(sizeof(GameSetNode) == 8, "the set node is 8 bytes");

// The prime table of allocators.md hash_set; a game bucket count is always
// one of these. 193 holds 1909 configurations at load factor ~10 - the
// game's own map uses the same table growth, so this is in range.
constexpr uint32_t kGameHashPrimes[] = {53u,  97u,       193u,      389u,      769u,       1543u, 3079u,
                                        6151u, 12289u,    24593u,    49157u,    98317u,     196613u,
                                        393241u, 786433u, 1572869u,  3145739u,  6291469u,   12582917u,
                                        25165843u, 50331653u, 100663319u, 201326611u, 402653189u,
                                        805306457u, 1610612741u, 3221225473u, 4294967291u};

uint32_t NextGameHashPrime(uint32_t hint) {
    for (uint32_t p : kGameHashPrimes) {
        if (p >= hint) return p;
    }
    return kGameHashPrimes[27];
}

// One process hash table with its own bucket buffer and node list. Rebuild
// clears every chain and re-grows the buckets for the new count. The two
// tables differ only in the node payload (the map's 12-byte node holds a
// key and a configuration reference; the set's 8-byte node a family ID), so
// both keep LcmHashNode-shaped nodes and a flag says whether a Release is
// owed per node. The nodes are walked (and freed) through the buckets, so
// the state keeps no separate list.
struct GameHashState {
    GameHashTable table{};
    void** buckets = nullptr;      // the bucket buffer (used slots)
    uint32_t used = 0;             // the table's bucket count (end - begin) / 4
    uint32_t nodeCount = 0;
    bool ownsValues = false;       // true: each map node's value is a reference

    void Clear() {
        if (buckets != nullptr && used != 0) {
            for (uint32_t i = 0; i < used; ++i) {
                for (LcmHashNode* n = static_cast<LcmHashNode*>(buckets[i]); n != nullptr;) {
                    LcmHashNode* next = n->next;
                    if (ownsValues && n->value != nullptr) n->value->Release();  // the map's reference
                    delete n;
                    n = next;
                }
                buckets[i] = nullptr;
            }
        }
        nodeCount = 0;
        table.count = 0;
    }

    ~GameHashState() { Clear(); }
};

GameHashState* ConfigMapState() {
    static GameHashState s;
    s.ownsValues = true;
    return &s;
}

GameHashState* FamilySetState() {
    static GameHashState s;
    s.ownsValues = false;
    return &s;
}

// Grows the bucket buffer to the first prime >= n and relinks the nodes.
bool Grow(GameHashState& s, uint32_t n) {
    const uint32_t prime = NextGameHashPrime(n);
    if (prime <= s.used) return true;
    auto** fresh = new (std::nothrow) void*[prime];
    if (fresh == nullptr) return false;
    for (uint32_t i = 0; i < prime; ++i) fresh[i] = nullptr;
    if (s.buckets != nullptr) {
        for (uint32_t i = 0; i < s.used; ++i) {
            for (LcmHashNode* node = static_cast<LcmHashNode*>(s.buckets[i]); node != nullptr;) {
                LcmHashNode* next = node->next;
                LcmHashNode*& head = *reinterpret_cast<LcmHashNode**>(&fresh[node->key % prime]);
                node->next = head;
                head = node;
                node = next;
            }
        }
        delete[] reinterpret_cast<uint8_t*>(s.buckets);
    }
    s.buckets = fresh;
    s.used = prime;
    s.table.begin = fresh;
    s.table.end = fresh + prime;
    s.table.capacityEnd = fresh + prime;
    return true;
}

// Inserts {key, value} at the head of bucket key mod n, as the game's insert
// does. A key already present keeps its first entry (the game's map::insert
// does not overwrite).
void InsertConfig(GameHashState& s, uint32_t key, cISC4LotConfiguration* value) {
    if (s.used == 0 && !Grow(s, 1)) return;
    for (LcmHashNode* n = static_cast<LcmHashNode*>(s.buckets[key % s.used]); n != nullptr; n = n->next) {
        if (n->key == key) return;
    }
    if (s.table.count + 1 > s.used && !Grow(s, s.table.count + 1)) return;
    auto* node = new (std::nothrow) LcmHashNode{};
    if (node == nullptr) return;
    node->key = key;
    node->value = value;
    if (value != nullptr) value->AddRef();  // the map holds one reference per configuration
    LcmHashNode*& head = *reinterpret_cast<LcmHashNode**>(&s.buckets[key % s.used]);
    node->next = head;
    head = node;
    ++s.nodeCount;
    ++s.table.count;
}

// Inserts the family ID at the head of its bucket, skipping a present one
// (a set insert). The node is the 8-byte shape {next, value}; it is
// allocated as an LcmHashNode (12 bytes) so Clear and Grow can walk either
// table with one node type. The set node's +4 holds the ID as a u32 (the
// node's value field is read as an integer by the set walkers), so no
// configuration Release is owed - GameHashState::ownsValues is false here.
void InsertFamily(GameHashState& s, uint32_t id) {
    if (s.used == 0 && !Grow(s, 1)) return;
    for (GameSetNode* n = static_cast<GameSetNode*>(s.buckets[id % s.used]); n != nullptr; n = n->next) {
        if (n->value == id) return;
    }
    if (s.table.count + 1 > s.used && !Grow(s, s.table.count + 1)) return;
    auto* node = new (std::nothrow) LcmHashNode{};
    if (node == nullptr) return;
    node->key = id;  // the set node's +4: the family ID
    node->value = nullptr;
    LcmHashNode*& head = *reinterpret_cast<LcmHashNode**>(&s.buckets[id % s.used]);
    node->next = head;
    head = node;
    ++s.nodeCount;
    ++s.table.count;
}

void RebuildConfigurationMap(fc4_lot_config_manager* core) {
    GameHashState& s = *ConfigMapState();
    s.Clear();
    if (core == nullptr) return;
    const uint32_t n = fc4_lot_config_configuration_map(core, nullptr, nullptr, 0);
    if (n == 0) return;
    std::vector<uint32_t> ids(n);
    std::vector<uint64_t> toks(n);
    fc4_lot_config_configuration_map(core, ids.data(), toks.data(), n);
    if (!Grow(s, n)) return;
    for (uint32_t i = 0; i < n; ++i) {
        InsertConfig(s, ids[i], Cfg(toks[i]));
    }
    Log("lotcfg: process configuration map rebuilt, %u entries", static_cast<unsigned>(s.table.count));
}

void RebuildFamilyIDSet(fc4_lot_config_manager* core) {
    GameHashState& s = *FamilySetState();
    s.Clear();
    if (core == nullptr) return;
    const uint32_t n = fc4_lot_config_family_id_set(core, nullptr, 0);
    if (n == 0) return;
    std::vector<uint32_t> ids(n);
    fc4_lot_config_family_id_set(core, ids.data(), n);
    if (!Grow(s, n)) return;
    for (uint32_t i = 0; i < n; ++i) InsertFamily(s, ids[i]);
    Log("lotcfg: process family set rebuilt, %u entries", static_cast<unsigned>(s.table.count));
}

void FreeGameHashTables() {
    delete ConfigMapState();
    delete FamilySetState();
}

void* ConfigurationMapTable() { return &ConfigMapState()->table; }

void* FamilyIDSetTable() { return &FamilySetState()->table; }

}  // namespace fc4
