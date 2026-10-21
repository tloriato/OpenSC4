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

// cFC4PropManager: the prop manager's GZCOM class over the fc4_prop_manager
// core (docs/specs/prop-lotconfig.md Part A). See cFC4PropManager.h.

#include "cFC4PropManager.h"

#include "FC4Config.h"
#include "FC4CoreProps.h"
#include "FC4Properties.h"
#include "FC4GameList.h"
#include "FC4Log.h"
#include "FC4RefAudit.h"
#include "FC4Interfaces.h"
#include "FC4Streams.h"
#include "cFC4Message2Standard.h"

#include "GZServPtrs.h"
#include "SC4List.h"
#include "SC4Point.h"
#include "SC4Vector.h"
#include "cGZPersistResourceKey.h"
#include "cIGZApp.h"
#include "cIGZCOM.h"
#include "cIGZFrameWork.h"
#include "cIGZMessageServer2.h"
#include "cIGZPersistResourceKeyFilter.h"
#include "cIGZPersistResourceKeyList.h"
#include "cIGZPersistResourceManager.h"
#include "cIGZString.h"
#include "cISC4App.h"
#include "cISC4City.h"
#include "cISC4Lot.h"
#include "cISC4LotManager.h"
#include "cISC4Occupant.h"
#include "cISC4OccupantFilter.h"
#include "cISC4OccupantManager.h"
#include "cISC4PropManager.h"
#include "cISC4PropOccupant.h"
#include "cISTETerrain.h"
#include "cRZCOMDllDirector.h"

#include <cstring>
#include <vector>

namespace {

// Prop usage values (prop-lotconfig.md §A2): 1 city, 2 lot, 3 building, 4 street.
constexpr uint32_t kUsageCity = 1;
constexpr uint32_t kUsageLot = 2;
constexpr uint32_t kUsageBuilding = 3;
constexpr uint32_t kUsageStreet = 4;
// The exemplar key type the catalogue walks (fc4.h: the resource manager's
// keys of type 0x6534284A).
constexpr uint32_t kExemplarKeyType = 0x6534284A;
// §A3.4: kSC4MessagePropExemplarDataUpdated.
constexpr uint32_t kFC4_MsgPropExemplarDataUpdated = 0x896FDC70;
// The prop occupant interface the game creates props through (FC4PropLotShadow.cpp).
constexpr uint32_t kIID_cISC4PropOccupant = 0xE9793A65;
constexpr uint32_t kOccupantTypeProp = 0xA823821E;      // NG25
constexpr uint32_t kOccupantTypeBuilding = 0x278128A0;  // §A7.2 slot 34

// The city through the application (borrowed; the application owns it).
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

cFC4PropManager* Self(void* u) { return static_cast<cFC4PropManager*>(u); }

cISC4City* City(void* u) { return AppCity(); }

cISC4OccupantManager* Occupants(void* u) {
    cISC4City* c = City(u);
    return c ? c->GetOccupantManager() : nullptr;
}

cISC4LotManager* Lots(void* u) {
    cISC4City* c = City(u);
    return c ? c->GetLotManager() : nullptr;
}

// Resolves the cISC4Occupant sub-object off any prop token (the prop-occupant
// primary at +0, the occupant base at +8). QI hands the caller one reference,
// released by the RAII wrapper below, so the token pointer itself stays
// borrowed. Tokens are created as primaries (CreatePropInstance) or as
// occupant sub-objects (occupant-manager queries), so every occupant-side
// call must go through this instead of a blind cast: on the primary vtable
// occupant slot 9 is GetOrientation (pop 0), and calling SetPosition (pop 4)
// through it desyncs the stack (RTC #0 -> int3, the glue21fix2 game fault).
cISC4Occupant* OccupantOf(uint64_t t) {
    cIGZUnknown* u = reinterpret_cast<cIGZUnknown*>(static_cast<uintptr_t>(t));
    if (u == nullptr) return nullptr;
    void* q = nullptr;
    if (!u->QueryInterface(GZIID_cISC4Occupant, &q) || q == nullptr) return nullptr;
    return static_cast<cISC4Occupant*>(q);
}

// RAII: releases the reference a QI in this scope took (the glue §5.1b rule:
// never Release a pointer you did not create/QI).
struct ScopedOcc {
    cISC4Occupant* o;
    explicit ScopedOcc(uint64_t t) : o(OccupantOf(t)) {}
    ~ScopedOcc() {
        if (o != nullptr) o->Release();
    }
    cISC4Occupant* get() const { return o; }
};

// Resolves the cISC4PropOccupant primary off any prop token, with a temporary
// reference the scope releases (the prop-side mirror of OccupantOf).
cISC4PropOccupant* PropOfScoped(uint64_t t, cISC4PropOccupant** out) {
    cIGZUnknown* u = reinterpret_cast<cIGZUnknown*>(static_cast<uintptr_t>(t));
    if (u == nullptr || out == nullptr) return nullptr;
    void* q = nullptr;
    if (!u->QueryInterface(kIID_cISC4PropOccupant, &q) || q == nullptr) {
        *out = nullptr;
        return nullptr;
    }
    *out = static_cast<cISC4PropOccupant*>(q);
    return *out;
}

struct ScopedProp {
    cISC4PropOccupant* p;
    explicit ScopedProp(uint64_t t) : p(nullptr) { PropOfScoped(t, &p); }
    ~ScopedProp() {
        if (p != nullptr) p->Release();
    }
    cISC4PropOccupant* get() const { return p; }
};

uint64_t Tok(const void* p) { return static_cast<uint64_t>(reinterpret_cast<uintptr_t>(p)); }

// ---- the read-only half (the same shapes as FC4PropLotShadow.cpp's PropHost) --------------------------

// The 0x14-byte occupant filter whose usage the queries match (§A2 +0x1C;
// FC4PropLotShadow.cpp's PropUsageFilter).
class PropUsageFilter final : public cISC4OccupantFilter {
public:
    uint32_t usage = 0;
    bool QueryInterface(uint32_t riid, void** ppvObj) override {
        if (ppvObj == nullptr) return false;
        if (riid == GZIID_cISC4OccupantFilter || riid == GZIID_cIGZUnknown) {
            *ppvObj = static_cast<cISC4OccupantFilter*>(this);
            return true;
        }
        *ppvObj = nullptr;
        return false;
    }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    bool IsOccupantIncluded(cISC4Occupant* o) override {
        if (o == nullptr || static_cast<uint32_t>(o->GetType()) != kOccupantTypeProp) return false;
        void* q = nullptr;
        if (!o->QueryInterface(kIID_cISC4PropOccupant, &q) || q == nullptr) return false;
        cISC4PropOccupant* p = static_cast<cISC4PropOccupant*>(q);
        FC4_ACQUIRED(p, "QueryInterface");
        const bool pass = p->GetPropUsage() == usage;  // prop slot 22
        FC4_RELEASE(p);
        return pass;
    }
    bool IsOccupantTypeIncluded(uint32_t type) override { return type == kOccupantTypeProp; }
    bool IsPropertyHolderIncluded(cISCPropertyHolder*) override { return true; }
};

// The list's entries as tokens: each carries one reference, which goes to the
// core (FC4PropLotShadow.cpp's HandOver).
void HandOver(SC4List<cISC4Occupant*>& list, fc4_token_callback found, void* ctx) {
    for (cISC4Occupant*& o : list) {
        if (o == nullptr) continue;
        FC4_ACQUIRED(o, "prop list entry");
        if (found != nullptr) {
            found(ctx, Tok(o));
        } else {
            FC4_RELEASE(o);
        }
        o = nullptr;
    }
}


// A key filter that keeps only the exemplar type (the same shape as
// FC4PropLotShadow.cpp's ExemplarKeyFilter).
class ExemplarKeyFilter final : public cIGZPersistResourceKeyFilter {
public:
    bool QueryInterface(uint32_t riid, void** ppvObj) override {
        if (ppvObj == nullptr) return false;
        if (riid == GZIID_cIGZUnknown || riid == GZIID_cIGZPersistResourceKeyFilter) {
            *ppvObj = this;
            return true;
        }
        *ppvObj = nullptr;
        return false;
    }
    uint32_t AddRef() override { return ++refs_; }
    uint32_t Release() override { return refs_ > 0 ? --refs_ : 0; }
    bool IsKeyIncluded(cGZPersistResourceKey const& key) override { return key.type == kExemplarKeyType; }

private:
    uint32_t refs_ = 1;
};

bool ForEachExemplarKey(void*, fc4_key_visitor visit, void* ctx) {
    cIGZPersistResourceManagerPtr rm;
    if (!rm) return false;
    ExemplarKeyFilter filter;
    cIGZPersistResourceKeyList* l = nullptr;
    rm->GetAvailableResourceList(&l, &filter);
    if (l == nullptr) return false;
    FC4_ACQUIRED(l, "GetAvailableResourceList");
    const uint32_t n = l->Size();  // cIGZPersistResourceKeyList slot 9
    for (uint32_t i = 0; i < n; ++i) {
        const cGZPersistResourceKey k = l->GetKey(i);
        if (k.type == kExemplarKeyType) visit(ctx, k.type, k.group, k.instance);
    }
    FC4_RELEASE_CREATED(l);
    return true;
}

fc4_props* LoadExemplar(void*, uint32_t t, uint32_t g, uint32_t i) {
    fc4::props::Exemplar ex;
    if (!ex.Load(t, g, i)) return nullptr;
    return fc4::props::ToCoreAll(ex);
}

void AddRefToken(void*, uint64_t token) {
    cIGZUnknown* o = reinterpret_cast<cIGZUnknown*>(static_cast<uintptr_t>(token));
    if (o != nullptr) FC4_ADDREF(o);
}

void ReleaseToken(void*, uint64_t token) {
    cIGZUnknown* o = reinterpret_cast<cIGZUnknown*>(static_cast<uintptr_t>(token));
    if (o != nullptr) FC4_RELEASE(o);
}

// §A6.1 step 1: a prop occupant of `cls` through GZCOM. The object the class
// factory returns already carries the caller's reference (the cFC4Development.cpp
// DG5 convention), so no extra AddRef. The token handed to the core is
// NORMALIZED to the cISC4Occupant sub-object (QI 0x26BB767E): every
// occupant-side host callback (Init, SetPosition, IsFlagSet, InsertOccupant,
// ...) then runs on the right vtable. The factory's primary reference is
// released right after the QI adds the core's own.
uint64_t CreatePropInstance(void*, uint32_t cls) {
    cIGZCOM* com = RZGetCOMDllDirector() ? GZCOM() : nullptr;
    if (com == nullptr) return 0;
    void* p = nullptr;
    if (!com->GetClassObject(cls, kIID_cISC4PropOccupant, &p) || p == nullptr) return 0;
    auto* prop = static_cast<cISC4PropOccupant*>(p);
    FC4_ACQUIRED(prop, "GetClassObject");  // DG5: the factory's caller reference
    void* q = nullptr;
    if (!prop->QueryInterface(GZIID_cISC4Occupant, &q) || q == nullptr) {
        FC4_RELEASE_CREATED(prop);  // the factory's reference; nothing else holds it
        return 0;
    }
    auto* occ = static_cast<cISC4Occupant*>(q);
    FC4_ACQUIRED(occ, "QueryInterface");  // the core's reference, via the token
    FC4_RELEASE_CREATED(prop);  // the factory's reference (the QI's ref takes over)
    return Tok(occ);
}

// §A6.1 step 3: a builtin prop of `kind` (fc4.h: plain / terrain decal /
// visual effect, the two latter with their own GZCOM classes).
uint64_t CreateBuiltinProp(void*, uint32_t kind) {
    static const uint32_t kTerrainDecal = 0x2977AA48;
    static const uint32_t kVisualEffect = 0x2977AA49;
    const uint32_t cls = kind == FC4_PROP_BUILTIN_TERRAIN_DECAL
                             ? kTerrainDecal
                             : kind == FC4_PROP_BUILTIN_VISUAL_EFFECT ? kVisualEffect : 0;
    if (cls == 0) return 0;  // plain: the caller's own class (CreatePropInstance)
    return CreatePropInstance(nullptr, cls);
}

// §A3.4: kSC4MessagePropExemplarDataUpdated (0x896FDC70), void1 and the
// IGZUnknown = the manager, data2 = the type map's count (PL2).
void PostExemplarDataUpdated(void* u, uint64_t manager, uint32_t data2) {
    cIGZMessageServer2Ptr ms;
    if (!ms) return;
    cFC4Message2Standard* msg = FC4MakeMessage(kFC4_MsgPropExemplarDataUpdated, 0,
                                               static_cast<intptr_t>(data2), 0);
    if (msg == nullptr) return;
    ms->MessagePost(msg, false);
    msg->Release();
    (void)u;
    (void)manager;
}

void SetPropType(void*, uint64_t prop, uint32_t id) {
    ScopedProp p(prop);
    if (p.get() != nullptr) p.get()->SetPropType(id);
}

fc4_props* AttachExemplar(void*, uint64_t prop, uint32_t t, uint32_t g, uint32_t i, bool) {
    fc4::props::Exemplar ex;
    if (!ex.Load(t, g, i)) return nullptr;
    (void)prop;
    return fc4::props::ToCoreAll(ex);
}

void SetBoundingBox(void*, uint64_t prop, const float* min3, const float* max3) {
    ScopedProp p(prop);
    if (p.get() != nullptr) p.get()->SetBoundingBox(min3, max3);
}

void Position(void*, uint64_t prop, float* pos3) {
    if (pos3 == nullptr) return;
    pos3[0] = pos3[1] = pos3[2] = 0.0f;
    // Tokens arrive as either the prop-occupant primary (CreatePropInstance)
    // or the occupant sub-object (occupant-manager queries); resolving the
    // occupant interface through QI (OccupantOf) covers both. Blind casts
    // here were the glue21 gate-9/gate-10 AV at +0xA843DB reading 1.
    ScopedOcc o(prop);
    if (o.get() == nullptr) return;
    cS3DVector3 v;
    o.get()->GetPosition(&v);  // occupant slot 8
    pos3[0] = v.fX;
    pos3[1] = v.fY;
    pos3[2] = v.fZ;
}

void CellToWorld(void*, int32_t x, int32_t z, float* wx, float* wz) {
    cISC4City* c = City(nullptr);
    if (c == nullptr || wx == nullptr || wz == nullptr) return;
    float fx = 0.0f, fz = 0.0f;
    c->CellCornerToPosition(x, z, fx, fz);  // city slot 96
    *wx = fx;
    *wz = fz;
}

void WorldToCell(void*, float x, float z, int32_t* cx, int32_t* cz) {
    cISC4City* c = City(nullptr);
    if (c == nullptr || cx == nullptr || cz == nullptr) return;
    int ix = 0, iz = 0;
    c->PositionToCell(x, z, ix, iz);  // city slot 95
    *cx = ix;
    *cz = iz;
}

uint64_t BuildingAtCell(void*, int32_t x, int32_t z) {
    cISC4OccupantManager* om = Occupants(nullptr);
    if (om == nullptr) return 0;
    cISC4Occupant* found = nullptr;
    om->GetFirstOccupantByStandardCityCell(found, x, z, kOccupantTypeBuilding);  // slot 33
    if (found == nullptr) return 0;
    FC4_ACQUIRED(found, "GetFirstOccupantByStandardCityCell");  // one reference for the core
    return Tok(found);
}

uint64_t LotAtCell(void*, int32_t x, int32_t z) {
    cISC4LotManager* lm = Lots(nullptr);
    if (lm == nullptr) return 0;
    return Tok(reinterpret_cast<cFC4ISC4LotManager*>(lm)->GetLot(x, z, false));  // lot manager slot 6, no reference
}

void LotCellRect(void*, uint64_t lot, fc4_cell_rect* out) {
    if (out == nullptr) return;
    *out = fc4_cell_rect{0, 0, 0, 0};
    cISC4Lot* l = reinterpret_cast<cISC4Lot*>(static_cast<uintptr_t>(lot));
    if (l == nullptr) return;
    SC4Rect<int32_t> r;
    if (!l->GetBoundingRect(r)) return;  // lot slot 14
    out->x1 = r.topLeftX;
    out->z1 = r.topLeftY;
    out->x2 = r.bottomRightX;
    out->z2 = r.bottomRightY;
}

// ---- the city-mutating half (what FC4PropLotShadow.cpp deliberately leaves NULL) ----------------------

void RegisterShutdownCallback(void* u) {
    // prop-lotconfig.md §A4: the core asks to be told at application
    // shutdown. app slot 69 is not in docs/abi (the cFC4Development.cpp DG7
    // choice), so the callback is not registered with the application: the
    // director's PreAppShutdown runs fc4::PropManagerAppShutdown instead.
    fc4::Log("propmgr: the core registered an application shutdown callback (run at PreAppShutdown)");
}

void RegisterCommand(void* u, uint32_t id) {
    fc4::Log("propmgr: command %08X registered with the core (the prop manager keeps no game command)", id);
    (void)u;
}

void UnregisterCommand(void* u, uint32_t id) {
    fc4::Log("propmgr: command %08X unregistered", id);
    (void)u;
}

void Subscribe(void* u, uint32_t msgType) {
    cFC4PropManager* s = Self(u);
    cIGZMessageServer2Ptr ms;
    fc4::Log("propmgr: subscribe %08X (server %d)", msgType, ms ? 1 : 0);
    if (!ms || s == nullptr) return;
    ms->AddNotification(s->Target(), msgType);
}

void Unsubscribe(void* u, uint32_t msgType) {
    cFC4PropManager* s = Self(u);
    cIGZMessageServer2Ptr ms;
    if (!ms || s == nullptr) return;
    ms->RemoveNotification(s->Target(), msgType);
}

uint64_t AcquireService(void*, uint32_t service) {
    // fc4.h: the three services the core asks for, as the game's objects.
    cISC4City* c = City(nullptr);
    cIGZUnknown* p = nullptr;
    if (service == FC4_PROP_SERVICE_OCCUPANT_MANAGER && c != nullptr) {
        p = reinterpret_cast<cIGZUnknown*>(c->GetOccupantManager());
    } else if (service == FC4_PROP_SERVICE_LOT_MANAGER && c != nullptr) {
        p = reinterpret_cast<cIGZUnknown*>(c->GetLotManager());
    } else if (service == FC4_PROP_SERVICE_RESOURCE_MANAGER) {
        cIGZPersistResourceManagerPtr rm;
        p = static_cast<cIGZUnknown*>(static_cast<cIGZPersistResourceManager*>(rm));
    }
    fc4::Log("propmgr: acquire_service %u -> %p", service, p);
    if (p == nullptr) return 0;
    FC4_ADDREF(p);  // one reference for the core, given back through `release`
    return Tok(p);
}

uint64_t ViewFilter(void*) {
    // §A2 +0x24: the 3D view's view filter (message 0x06CDB65B). The view is
    // not in docs/abi, so there is no filter to answer (the core treats 0 as
    // "no filter").
    return 0;
}

bool OccupantFlag(void*, uint64_t occ, uint32_t flag) {
    ScopedOcc o(occ);
    return o.get() != nullptr && o.get()->IsFlagSet(flag);
}

bool OccupantInit(void*, uint64_t occ) {
    ScopedOcc o(occ);
    // [abi-ret] cISC4Occupant slot 3 (interfaces.tsv cSC4Occupant::Init) is
    // eax-or-void: the SDK declares bool and the game leaves whatever the last
    // inner call left. The core treats a missing Init as failure.
    return o.get() != nullptr && o.get()->Init();
}

void OccupantShutdown(void*, uint64_t occ) {
    ScopedOcc o(occ);
    if (o.get() != nullptr) o.get()->Shutdown();
}

void SetPropUsage(void*, uint64_t occ, uint32_t usage) {
    ScopedProp p(occ);
    if (p.get() != nullptr) p.get()->SetPropUsage(usage);
}

void SetPosition(void*, uint64_t occ, const float* pos3) {
    ScopedOcc o(occ);
    if (o.get() == nullptr || pos3 == nullptr) return;
    const cS3DVector3 v(pos3[0], pos3[1], pos3[2]);
    o.get()->SetPosition(&v);
}

void SetOrientation(void*, uint64_t occ, int32_t orientation) {
    ScopedProp p(occ);
    if (p.get() != nullptr) p.get()->SetOrientation(orientation);
}

void Hide(void*, uint64_t occ) {
    ScopedOcc o(occ);
    if (o.get() != nullptr) o.get()->SetFlag(0x00000001u);  // the hidden bit
}

bool InsertOccupant(void*, uint64_t occ) {
    cISC4OccupantManager* om = Occupants(nullptr);
    ScopedOcc o(occ);
    return om != nullptr && o.get() != nullptr && om->InsertOccupant(o.get(), 0);
}

void RemoveOccupant(void*, uint64_t occ) {
    cISC4OccupantManager* om = Occupants(nullptr);
    ScopedOcc o(occ);
    if (om != nullptr && o.get() != nullptr) om->RemoveOccupant(o.get(), true, 0);
}

bool OccupantsInCells(void*, const fc4_cell_rect* rect, uint32_t usage, fc4_token_callback found, void* ctx) {
    if (rect == nullptr || found == nullptr) return false;
    cISC4OccupantManager* om = Occupants(nullptr);
    if (om == nullptr) return false;
    PropUsageFilter filter;
    filter.usage = usage;
    const SC4Point<int> mn(rect->x1, rect->z1);
    const SC4Point<int> mx(rect->x2, rect->z2);
    SC4List<cISC4Occupant*> list;
    const bool ok = om->GetOccupantsByStandardCityCells(
        list, &mn, &mx, static_cast<cISC4OccupantFilter*>(&filter), 0xFFFFFFFFu);  // slot 40
    HandOver(list, found, ctx);
    return ok;
}

bool OccupantsInBox(void* u, const float* min3, const float* max3, uint32_t usage, fc4_token_callback found,
                    void* ctx) {
    if (min3 == nullptr || max3 == nullptr || found == nullptr) return false;
    // §A7.2: the box query is by usage; walk the cell rectangle the box covers
    // and keep the matching usage.
    const float x1 = min3[0] < max3[0] ? min3[0] : max3[0];
    const float z1 = min3[2] < max3[2] ? min3[2] : max3[2];
    const float x2 = min3[0] < max3[0] ? max3[0] : min3[0];
    const float z2 = min3[2] < max3[2] ? max3[2] : min3[2];
    int32_t cx1 = 0, cz1 = 0, cx2 = 0, cz2 = 0;
    WorldToCell(u, x1, z1, &cx1, &cz1);
    WorldToCell(u, x2, z2, &cx2, &cz2);
    const fc4_cell_rect r{cx1, cz1, cx2, cz2};
    return OccupantsInCells(u, &r, usage, found, ctx);
}

uint32_t BuildingProps(void*, uint64_t building, fc4_token_callback visit, void* ctx) {
    // §A7.1 step 3: the 0xA8322B7F property object holds a building's props.
    // Its layout is not in docs/abi (the shadow says the same), so the list is
    // not read: 0 props.
    (void)building;
    (void)visit;
    (void)ctx;
    return 0;
}

void SetBuildingProps(void*, uint64_t building, const uint64_t* props, size_t n) {
    fc4::Log("propmgr: SetBuildingProps %08X %u (the 0xA8322B7F property is not in docs/abi)",
             static_cast<unsigned>(building), static_cast<unsigned>(n));
    (void)props;
}

void AddBuildingPropsProperty(void*, uint64_t building, const uint64_t* props, size_t n) {
    SetBuildingProps(nullptr, building, props, n);
}

void RemoveBuildingPropsProperty(void*, uint64_t building) {
    fc4::Log("propmgr: RemoveBuildingPropsProperty %08X (the 0xA8322B7F property is not in docs/abi)",
             static_cast<unsigned>(building));
}

uint64_t CreateTextDecal(void*) {
    // §A3.2 / the text decal slots: PL1 is still with an RE lane and the
    // decal class is not in docs/abi. No decal is created.
    return 0;
}

float TerrainAltitude(void*, float x, float z) {
    cISC4City* c = City(nullptr);
    cISTETerrain* t = c ? c->GetTerrain() : nullptr;
    return t != nullptr ? t->GetAltitude(x, z) : 0.0f;  // terrain slot 13, ST0
}

void DecalSetText(void*, uint64_t decal, const char* text, size_t len) {
    fc4::Log("propmgr: decal text %.*s (PL1 is still with an RE lane)",
             static_cast<int>(len > 64 ? 64 : len), text != nullptr ? text : "");
    (void)decal;
}

void DecalSlot8(void*, uint64_t decal, const float* values3) {
    (void)decal;
    (void)values3;
}

void DecalSlot10(void*, uint64_t decal, float value) {
    (void)decal;
    (void)value;
}

void DecalSlot6(void*, uint64_t decal, float value) {
    (void)decal;
    (void)value;
}

uint64_t ResolveRef(void*, uint64_t token, uint32_t iid) {
    cIGZUnknown* o = reinterpret_cast<cIGZUnknown*>(static_cast<uintptr_t>(token));
    if (o == nullptr) return 0;
    void* q = nullptr;
    if (!o->QueryInterface(iid, &q) || q == nullptr) return 0;
    return Tok(q);  // one reference for the caller (QueryInterface AddRef'd it)
}

fc4_prop_manager_host PropHost(cFC4PropManager* s) {
    fc4_prop_manager_host h{};
    h.struct_size = sizeof(h);
    h.user = s;
    h.for_each_exemplar_key = &ForEachExemplarKey;
    h.load_exemplar = &LoadExemplar;
    h.register_shutdown_callback = &RegisterShutdownCallback;
    h.post_exemplar_data_updated = nullptr;  // the message the core posts is
                                             // built in fc4_prop_manager_do_message
    h.register_command = &RegisterCommand;
    h.unregister_command = &UnregisterCommand;
    h.subscribe = &Subscribe;
    h.unsubscribe = &Unsubscribe;
    h.acquire_service = &AcquireService;
    h.view_filter = &ViewFilter;
    h.add_ref = &AddRefToken;
    h.release = &ReleaseToken;
    h.create_prop_instance = &CreatePropInstance;
    h.create_builtin_prop = &CreateBuiltinProp;
    h.set_prop_type = &SetPropType;
    h.attach_exemplar = &AttachExemplar;
    h.set_bounding_box = &SetBoundingBox;
    h.occupant_flag = &OccupantFlag;
    h.occupant_init = &OccupantInit;
    h.occupant_shutdown = &OccupantShutdown;
    h.set_prop_usage = &SetPropUsage;
    h.set_position = &SetPosition;
    h.set_orientation = &SetOrientation;
    h.position = &Position;
    h.hide = &Hide;
    h.insert_occupant = &InsertOccupant;
    h.remove_occupant = &RemoveOccupant;
    h.occupants_in_box = &OccupantsInBox;
    h.occupants_in_cell_rect = &OccupantsInCells;
    h.building_at_cell = &BuildingAtCell;
    h.lot_at_cell = &LotAtCell;
    h.lot_cell_rect = &LotCellRect;
    h.cell_to_world = &CellToWorld;
    h.world_to_cell = &WorldToCell;
    h.building_props = &BuildingProps;
    h.set_building_props = &SetBuildingProps;
    h.add_building_props_property = &AddBuildingPropsProperty;
    h.remove_building_props_property = &RemoveBuildingPropsProperty;
    h.create_text_decal = &CreateTextDecal;
    h.terrain_altitude = &TerrainAltitude;
    h.decal_set_text = &DecalSetText;
    h.decal_slot8 = &DecalSlot8;
    h.decal_slot10 = &DecalSlot10;
    h.decal_slot6 = &DecalSlot6;
    h.resolve_ref = &ResolveRef;
    return h;
}

}  // namespace

cFC4PropManager::~cFC4PropManager() = default;

bool cFC4PropManager::QueryInterface(uint32_t riid, void** ppvObj) {
    if (ppvObj == nullptr) return false;
    // §A1: 0x28326F3E and 1 -> +0; the message target (+4) and the
    // serializable (+8). A successful QueryInterface adds a reference (the
    // COM contract; cFC4Simulator::QueryInterfaceImpl) - as for the lot
    // configuration manager, the class store's loader takes the factory
    // result as the caller's reference and releases it after Read; without
    // the AddRef that release can drop the table's reference and destroy the
    // object while the city still holds the pointer.
    switch (riid) {
        case kFC4_IID_cISC4PropManager:
        case GZIID_cIGZUnknown:
            *ppvObj = Primary();
            break;
        case kFC4_IID_cIGZMessageTarget2Prop:
            *ppvObj = Target();
            break;
        case kFC4_IID_cIGZSerializableProp:
            *ppvObj = static_cast<cIGZSerializable*>(this);
            break;
        default:
            *ppvObj = nullptr;
            return false;
    }
    AddRef();
    return true;
}

uint32_t cFC4PropManager::AddRef() { return ++refCount_; }

uint32_t cFC4PropManager::Release() {
    if (refCount_ == 0 || --refCount_ != 0) return refCount_;
    if (destroying_) return 0;
    destroying_ = true;
    Shutdown();  // §A1: the last Release calls Shutdown then the deleting dtor
    delete this;
    return 0;
}

// `Read` runs before `Init` (HANDOFF 5.1): the core is created in the
// constructor so a saved record can be read before Init is called. Nothing
// here sizes saved state from the city.
cFC4PropManager::cFC4PropManager() {
    host_ = PropHost(this);
    core_ = fc4_prop_manager_create(&host_, fc4::PropCatalogueProcess(), 0);
}

bool cFC4PropManager::Init() {
    if (initialised_) return true;
    if (core_ == nullptr) return false;
    const bool ok = fc4_prop_manager_init(core_);
    initialised_ = ok;
    return ok;
}

bool cFC4PropManager::Shutdown() {
    if (!initialised_ && core_ == nullptr) return true;
    const bool ok = core_ != nullptr ? fc4_prop_manager_shutdown(core_) : true;
    if (core_ != nullptr) {
        fc4_prop_manager_destroy(core_);
        core_ = nullptr;
    }
    initialised_ = false;
    return ok;
}

void cFC4PropManager::GetAllPropTypes(SC4Vector<uint32_t>& types) {
    if (core_ == nullptr) return;
    const size_t n = fc4_prop_manager_all_prop_types(core_, nullptr, 0);
    if (n == 0) {
        types.clear();
        return;
    }
    std::vector<uint32_t> v(n);
    const size_t got = fc4_prop_manager_all_prop_types(core_, v.data(), n);
    types.clear();
    types.push_back(v.data(), got);
}

void cFC4PropManager::GetAllPropFamilyTypes(SC4Vector<uint32_t>& families) {
    if (core_ == nullptr) return;
    const size_t n = fc4_prop_manager_all_prop_family_types(core_, nullptr, 0);
    if (n == 0) {
        families.clear();
        return;
    }
    std::vector<uint32_t> v(n);
    const size_t got = fc4_prop_manager_all_prop_family_types(core_, v.data(), n);
    families.clear();
    families.push_back(v.data(), got);
}

bool cFC4PropManager::GetPropKeyFromType(uint32_t propID, cGZPersistResourceKey& key) {
    if (core_ == nullptr) return false;
    fc4_res_key rk{};
    if (!fc4_prop_manager_prop_key_from_type(core_, propID, &rk)) return false;
    key.type = rk.type_id;
    key.group = rk.group;
    key.instance = rk.instance;
    return true;
}

bool cFC4PropManager::IsPropTypeValid(uint32_t propID) {
    return core_ != nullptr && fc4_prop_manager_is_prop_type_valid(core_, propID);
}

uint32_t cFC4PropManager::GetPropTypesByFamily(SC4Vector<uint32_t>& types, uint32_t family) {
    if (core_ == nullptr) return 0;
    std::vector<uint32_t> v;
    const uint32_t n = fc4_prop_manager_prop_types_by_family(
        core_, family, nullptr, 0,
        [](void* ctx, const uint32_t* p, size_t count) {
            static_cast<std::vector<uint32_t>*>(ctx)->assign(p, p + count);
        },
        &v);
    types.clear();
    if (!v.empty()) types.push_back(v.data(), v.size());
    return n;
}

bool cFC4PropManager::IsPropInFamily(uint32_t propID, uint32_t family) {
    return core_ != nullptr && fc4_prop_manager_is_prop_in_family(core_, propID, family);
}

bool cFC4PropManager::IsPropFamilyTypeValid(uint32_t family) {
    return core_ != nullptr && fc4_prop_manager_is_prop_family_type_valid(core_, family);
}

cIGZString& cFC4PropManager::GetPropName(uint32_t propID) {
    // §A5 slot 12: a reference to a string that is always empty (the name map
    // is never filled). The reference is a game cRZString (GameString: the
    // game's own vtable and a B character buffer) so a caller that reads it
    // lands on the game's string code; it is empty and never written.
    (void)propID;
    static fc4::GameString empty = [] {
        fc4::GameString s{};
        fc4::GameStringInit(s, "", 0);
        return s;
    }();
    return *reinterpret_cast<cIGZString*>(&empty);
}

cISC4PropOccupant::PropProfile& cFC4PropManager::GetPropProfile(uint32_t propID) {
    static cISC4PropOccupant::PropProfile zero{};
    if (core_ == nullptr) return zero;
    fc4_prop_profile pp{};
    fc4_prop_manager_prop_profile(core_, propID, &pp);
    static thread_local cISC4PropOccupant::PropProfile out{};
    std::memcpy(&out, &pp, sizeof(out) < sizeof(pp) ? sizeof(out) : sizeof(pp));
    return out;
}

const cISC4PropOccupant::PropProfile* cFC4PropManager::GetPropProfilePtr(uint32_t propID) {
    if (core_ == nullptr) return nullptr;
    static thread_local cISC4PropOccupant::PropProfile out{};
    fc4_prop_profile pp{};
    if (!fc4_prop_manager_prop_profile_ptr(core_, propID, &pp)) return nullptr;
    std::memcpy(&out, &pp, sizeof(out) < sizeof(pp) ? sizeof(out) : sizeof(pp));
    return &out;
}

// Converts the core's normalized token (the cISC4Occupant sub-object carrying
// the creation reference) into the cISC4PropOccupant* the SDK callers own:
// QI to the primary takes one reference, the token's creation reference is
// released, and the primary's reference goes to the game caller (one
// reference owned, §A6.1 step 6).
bool TokenToPropPrimary(uint64_t t, cISC4PropOccupant*& prop) {
    cIGZUnknown* u = reinterpret_cast<cIGZUnknown*>(static_cast<uintptr_t>(t));
    if (u == nullptr) return false;
    void* q = nullptr;
    if (!u->QueryInterface(kIID_cISC4PropOccupant, &q) || q == nullptr) return false;
    prop = static_cast<cISC4PropOccupant*>(q);
    FC4_RELEASE_CREATED(u);  // the creation reference (the QI ref hands over)
    return true;
}

bool cFC4PropManager::CreateProp(uint32_t propID, cISC4PropOccupant*& prop) {
    // GLUE21-2 (prop-developer.md 13.2): the out parameter is left UNTOUCHED
    // until the core answers, never zeroed first - the game's callers take
    // the occupant without a null check, so a cleared slot is read either
    // way. When the core cannot produce an occupant (GZCOM does not answer
    // and no built-in route exists yet) the call fails and the caller's
    // pointer keeps whatever it held, as the original's only clean exit
    // (13.2 step 1) leaves it.
    if (core_ == nullptr) return false;
    const uint64_t t = fc4_prop_manager_create_prop(core_, propID);
    if (t == 0) return false;
    return TokenToPropPrimary(t, prop);
}

bool cFC4PropManager::CreateGenericProp(cGZPersistResourceKey const& key, cISC4PropOccupant*& prop) {
    // GLUE21-2, as CreateProp: the out parameter is not pre-cleared.
    if (core_ == nullptr) return false;
    const uint64_t t = fc4_prop_manager_create_generic_prop(core_, key.type, key.group, key.instance);
    if (t == 0) return false;
    return TokenToPropPrimary(t, prop);
}

namespace {

// Fills the game's SC4List from the core's token callback (one entry each).
void Collect(void* ctx, uint64_t token) {
    cISC4Occupant* o = reinterpret_cast<cISC4Occupant*>(static_cast<uintptr_t>(token));
    static_cast<SC4List<cISC4Occupant*>*>(ctx)->push_back(o);
}

}  // namespace

bool cFC4PropManager::GetCityPropsWorld(SC4List<cISC4Occupant*>& list, SC4Rect<float> const& rect) {
    if (core_ == nullptr) return false;
    return fc4_prop_manager_get_city_props_world(core_, static_cast<float>(rect.topLeftX),
                                                 static_cast<float>(rect.topLeftY), static_cast<float>(rect.bottomRightX),
                                                 static_cast<float>(rect.bottomRightY), nullptr, 0, &Collect, &list);
}

bool cFC4PropManager::GetCityPropsCells(SC4List<cISC4Occupant*>& list, SC4Rect<int32_t> const& rect) {
    if (core_ == nullptr) return false;
    const fc4_cell_rect r{rect.topLeftX, rect.topLeftY, rect.bottomRightX, rect.bottomRightY};
    return fc4_prop_manager_get_props_in_cells(core_, kUsageCity, &r, nullptr, 0, &Collect, &list);
}

bool cFC4PropManager::AddCityPropOcc(cISC4Occupant* prop) {
    return core_ != nullptr &&
           fc4_prop_manager_add_city_prop(core_, Tok(prop));
}

cISC4Occupant* cFC4PropManager::AddCityPropNew(uint32_t propID, cS3DVector3 const& position,
                                               int32_t orientation) {
    if (core_ == nullptr) return nullptr;
    const float p[3] = {position.fX, position.fY, position.fZ};
    return reinterpret_cast<cISC4Occupant*>(
        static_cast<uintptr_t>(fc4_prop_manager_add_prop_new(core_, kUsageCity, propID, p, orientation)));
}

bool cFC4PropManager::RemoveCityPropsWorld(SC4Rect<float> const& rect) {
    return core_ != nullptr &&
           fc4_prop_manager_remove_city_props_world(core_, static_cast<float>(rect.topLeftX),
                                                    static_cast<float>(rect.topLeftY), static_cast<float>(rect.bottomRightX),
                                                    static_cast<float>(rect.bottomRightY));
}

bool cFC4PropManager::RemoveCityPropsCells(SC4Rect<int32_t> const& rect) {
    if (core_ == nullptr) return false;
    const fc4_cell_rect r{rect.topLeftX, rect.topLeftY, rect.bottomRightX, rect.bottomRightY};
    return fc4_prop_manager_remove_city_props_cells(core_, &r);
}

bool cFC4PropManager::GetLotPropsAt(SC4List<cISC4Occupant*>& list, int32_t x, int32_t z) {
    if (core_ == nullptr) return false;
    return fc4_prop_manager_get_props_at(core_, kUsageLot, x, z, nullptr, 0, &Collect, &list);
}

bool cFC4PropManager::GetLotPropsOfLot(SC4List<cISC4Occupant*>& list, cISC4Lot* lot) {
    if (core_ == nullptr) return false;
    return fc4_prop_manager_get_props_of(core_, kUsageLot, Tok(lot), nullptr, 0, &Collect, &list);
}

bool cFC4PropManager::AddLotPropToLot(cISC4Lot* lot, cISC4Occupant* prop) {
    return core_ != nullptr && fc4_prop_manager_add_lot_prop(core_, Tok(lot), Tok(prop));
}

cISC4Occupant* cFC4PropManager::AddLotPropNew(uint32_t propID, cS3DVector3 const& position,
                                              int32_t orientation) {
    if (core_ == nullptr) return nullptr;
    const float p[3] = {position.fX, position.fY, position.fZ};
    return reinterpret_cast<cISC4Occupant*>(
        static_cast<uintptr_t>(fc4_prop_manager_add_prop_new(core_, kUsageLot, propID, p, orientation)));
}

bool cFC4PropManager::RemoveLotPropsAt(int32_t x, int32_t z) {
    return core_ != nullptr && fc4_prop_manager_remove_props_at(core_, kUsageLot, x, z);
}

bool cFC4PropManager::RemoveLotPropsOfLot(cISC4Lot* lot) {
    return core_ != nullptr && fc4_prop_manager_remove_props_of(core_, kUsageLot, Tok(lot));
}

bool cFC4PropManager::RemovePropOcc(cISC4Occupant* prop) {
    return core_ != nullptr && fc4_prop_manager_remove_prop(core_, Tok(prop));
}

bool cFC4PropManager::GetStreetPropsCells(SC4List<cISC4Occupant*>& list, SC4Rect<int32_t> const& rect) {
    if (core_ == nullptr) return false;
    const fc4_cell_rect r{rect.topLeftX, rect.topLeftY, rect.bottomRightX, rect.bottomRightY};
    return fc4_prop_manager_get_props_in_cells(core_, kUsageStreet, &r, nullptr, 0, &Collect, &list);
}

// §A9 slot 31: returns AL = 0 (the game's own implementation does too).
bool cFC4PropManager::GetStreetPropsAt(SC4List<cISC4Occupant*>& list, int32_t x, int32_t z) {
    if (core_ == nullptr) return false;
    fc4_prop_manager_get_props_at(core_, kUsageStreet, x, z, nullptr, 0, &Collect, &list);
    return false;
}

cISC4Occupant* cFC4PropManager::AddStreetPropNew(uint32_t propID, cS3DVector3 const& position,
                                                 int32_t orientation) {
    if (core_ == nullptr) return nullptr;
    const float p[3] = {position.fX, position.fY, position.fZ};
    return reinterpret_cast<cISC4Occupant*>(
        static_cast<uintptr_t>(fc4_prop_manager_add_prop_new(core_, kUsageStreet, propID, p, orientation)));
}

void cFC4PropManager::RemoveStreetPropsFromTile(int32_t x, int32_t z) {
    if (core_ != nullptr) fc4_prop_manager_remove_props_at(core_, kUsageStreet, x, z);
}

bool cFC4PropManager::GetBuildingPropsAt(SC4List<cISC4Occupant*>& list, int32_t x, int32_t z) {
    if (core_ == nullptr) return false;
    return fc4_prop_manager_get_props_at(core_, kUsageBuilding, x, z, nullptr, 0, &Collect, &list);
}

bool cFC4PropManager::GetBuildingPropsOfBuilding(SC4List<cISC4Occupant*>& list, cISC4Occupant* building) {
    if (core_ == nullptr) return false;
    return fc4_prop_manager_get_props_of(core_, kUsageBuilding, Tok(building), nullptr, 0, &Collect, &list);
}

bool cFC4PropManager::AddBuildingPropToBuilding(cISC4Occupant* building, cISC4Occupant* prop) {
    return core_ != nullptr && fc4_prop_manager_add_building_prop(core_, Tok(building), Tok(prop));
}

bool cFC4PropManager::RemoveBuildingPropsAt(int32_t x, int32_t z) {
    return core_ != nullptr && fc4_prop_manager_remove_props_at(core_, kUsageBuilding, x, z);
}

bool cFC4PropManager::RemoveBuildingPropsOfBuilding(cISC4Occupant* building) {
    return core_ != nullptr && fc4_prop_manager_remove_props_of(core_, kUsageBuilding, Tok(building));
}

bool cFC4PropManager::RemoveBuildingPropOne(cISC4Occupant* building, cISC4Occupant* prop) {
    return core_ != nullptr && fc4_prop_manager_remove_building_prop(core_, Tok(building), Tok(prop));
}

bool cFC4PropManager::DoMessage(cIGZMessage2* pMessage) {
    if (core_ == nullptr || pMessage == nullptr) return true;
    fc4_prop_manager_do_message(core_, pMessage->GetType(), 0, nullptr, 0);
    return true;  // §A4.3: every handler returns true
}

bool cFC4PropManager::Write(cIGZOStream& gzOut) {
    if (core_ == nullptr) return false;
    fc4::OStreamAdapter a(gzOut);
    return fc4_prop_manager_write(core_, a.Get());
}

bool cFC4PropManager::Read(cIGZIStream& gzIn) {
    if (core_ == nullptr) return false;
    fc4::IStreamAdapter a(gzIn);
    return fc4_prop_manager_read(core_, a.Get());
}

uint32_t cFC4PropManager::GetGZCLSID() { return kFC4_CLSID_cSC4PropManager; }

cIGZUnknown* CreateFC4PropManager() {
    return static_cast<cIGZUnknown*>(static_cast<cFC4ISC4PropManager*>(new cFC4PropManager()));
}

namespace fc4 {

fc4_prop_catalogue* PropCatalogueProcess() {
    static fc4_prop_catalogue* cat = nullptr;
    if (cat == nullptr) {
        cat = fc4_prop_catalogue_create();
        Log("propmgr: process prop catalogue %p", static_cast<void*>(cat));
    }
    return cat;
}

void PropManagerAppShutdown() {
    fc4_prop_catalogue* cat = PropCatalogueProcess();
    if (cat != nullptr) {
        fc4_prop_catalogue_app_shutdown(cat);
        fc4_prop_catalogue_destroy(cat);
    }
}

cFC4PropManager* OurPropManager(cISC4PropManager* p) {
    if (p == nullptr) return nullptr;
    // Ours answers GetGZCLSID 0xA990BD10 through the class we registered; an
    // original answers the same value, so the test is on the class pointer
    // the subsystem table created: `CreateFC4PropManager` is the only maker
    // of a cFC4PropManager, and QueryInterface(1) gives its primary pointer.
    void* primary = nullptr;
    if (!p->QueryInterface(GZIID_cIGZUnknown, &primary) || primary == nullptr) return nullptr;
    return static_cast<cFC4PropManager*>(static_cast<cFC4ISC4PropManager*>(AsGame(p)));
}

}  // namespace fc4
