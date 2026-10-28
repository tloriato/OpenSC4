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

// The demolition utility's host table: see FC4DemolitionHost.h.
//
// Slot numbers cited below are docs/abi/interfaces.tsv; behaviour is
// docs/specs/demolition.md (§n) and docs/specs/occupant-manager.md (OM §n).

#include "FC4DemolitionHost.h"

#include "FC4CoreProps.h"
#include "FC4DevelopmentInterfaces.h"
#include "FC4GameList.h"
#include "FC4GameSet.h"
#include "FC4Interfaces.h"
#include "FC4Log.h"
#include "FC4Properties.h"
#include "FC4RefAudit.h"
#include "FC4Streams.h"
#include "FC4LotDevInterfaces.h"
#include "cFC4Message2Standard.h"
#include "cIGZMessageTarget2.h"

#include "GZServPtrs.h"
#include "SC4List.h"
#include "SC4Point.h"
#include "SC4CellRegion.h"
#include "SC4Rect.h"
#include "cIGZMessageServer2.h"
#include "cISC4City.h"
#include "cISC4Lot.h"
#include "cISC4LotConfiguration.h"
#include "cISC4LotManager.h"
#include "cISC4NetworkOccupant.h"
#include "cISC4BuildingOccupant.h"
#include "cISC4Occupant.h"
#include "cISC4OccupantFilter.h"
#include "cISC4OccupantManager.h"
#include "cISCPropertyHolder.h"
#include "cS3DVector2.h"
#include "cS3DVector3.h"

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>
// windows.h maps SendMessage and RemoveProp to their A variants; the game
// interfaces have methods of those names.
#undef SendMessage
#undef RemoveProp

#include <algorithm>
#include <cfloat>
#include <cmath>
#include <cstring>
#include <map>
#include <set>

namespace fc4 {
namespace {

// demolition.md §1, §4.2, §4.3, §4.6, §6.
constexpr uint32_t kTypeFoundation = 0x68FD0C69;
constexpr uint32_t kTypeBuilding = 0x278128A0;
constexpr uint32_t kTypePowerPole = 0x2890D4DE;
constexpr uint32_t kTypeConstruction = 0x096E763C;
constexpr uint32_t kTypeProp = 0xA823821E;
constexpr uint32_t kTypeNetwork = 0xC772BF98;
constexpr uint32_t kTypeFlora = 0x74758926;
constexpr uint32_t kIID_cISC4FoundationOccupant = 0xC8FCE101;
constexpr uint32_t kIID_cISC4PowerPoleOccupant = 0xA8C86142;
constexpr uint32_t kIID_cISC4NetworkOccupant = 0xA821EF94;
constexpr uint32_t kIID_cISC4Occupant = 0x26BB767E;
constexpr uint32_t kIID_cISC4BuildingOccupant = 0x87DDA3A9;
constexpr uint32_t kMsgPreDemolishNetworkOccupant = 0xAA7A014D;
// §4.2 step 7: every network type bit except water pipe, power line, subway.
constexpr uint32_t kUndergroundHiddenFlags = 0x1F4F;
// §4.2 step 1: the two shared filters' group and interface tests.
constexpr uint32_t kRealismIfaceA = 0x4A10AFC2;
constexpr uint32_t kRealismIfaceB = 0x8A139184;
constexpr uint32_t kDefaultFilterGroups[] = {0x4217u, 0x4208u, 0x421Du};
constexpr uint32_t kRealisticExtraGroup = 0x5001;
// §6 tuning exemplars.
constexpr uint32_t kTuningType = 0x6534284A;
constexpr uint32_t kTuningThresholdsGroup = 0x690F693F;
constexpr uint32_t kTuningThresholdsInst = 0x2A8A09CA;
constexpr uint32_t kTuningBuildingGroup = 0xE7E2C2DB;
constexpr uint32_t kTuningBuildingInst = 0xE8DA7677;
// §4.7: the terrain tolerance and the per-vertex box half-width.
constexpr float kTerrainTolerance = 0.05f;
constexpr float kImmovableBoxHalf = 14.5f;

// A full rectangle as the game's SC4CellRegion<long> (zones.md, 0x24 bytes:
// x1, z1, x2, z2, then the cell map; bit (dx, dz) is bit dz % 32 of
// rows[dx][dz / 32]). The memory is ours; PlaceZone only clears bits. The
// same layout cFC4Development.cpp builds for its zoning calls.
class RegionBuffer {
public:
    bool Build(const fc4_cell_rect& r) {
        const int64_t w = int64_t{r.x2} - r.x1 + 1;
        const int64_t h = int64_t{r.z2} - r.z1 + 1;
        if (w <= 0 || h <= 0 || w * h > 4096) return false;
        layout_.x1 = r.x1;
        layout_.z1 = r.z1;
        layout_.x2 = r.x2;
        layout_.z2 = r.z2;
        const uint32_t words = static_cast<uint32_t>((h + 31) / 32);
        bits_.assign(static_cast<size_t>(w) * words, 0);
        rows_.resize(static_cast<size_t>(w));
        for (int64_t dx = 0; dx < w; ++dx) {
            uint32_t* row = &bits_[static_cast<size_t>(dx) * words];
            rows_[static_cast<size_t>(dx)] = row;
            for (int64_t dz = 0; dz < h; ++dz) row[dz / 32] |= 1u << (dz % 32);
        }
        layout_.map.width = static_cast<uint32_t>(w);
        layout_.map.height = static_cast<uint32_t>(h);
        layout_.map.words = words;
        layout_.map.rows = rows_.data();
        return true;
    }
    SC4CellRegion<long>& Zoning() { return *reinterpret_cast<SC4CellRegion<long>*>(&layout_); }

private:
    struct Layout {
        int32_t x1, z1, x2, z2;
        struct {
            uint32_t width, height, words;
            uint32_t** rows;
        } map;
    };
    Layout layout_{};
    std::vector<uint32_t> bits_;
    std::vector<uint32_t*> rows_;
};

DemolitionHostContext* C(void* u) { return static_cast<DemolitionHostContext*>(u); }
cISC4City* City(void* u) { return C(u)->city; }

cISC4Occupant* Occ(uint64_t t) {
    return t == 0 ? nullptr : reinterpret_cast<cISC4Occupant*>(static_cast<uintptr_t>(t));
}
cISC4Lot* Lot(uint64_t t) {
    return t == 0 ? nullptr : reinterpret_cast<cISC4Lot*>(static_cast<uintptr_t>(t));
}
cFC4OccupantFilter* Filt(uint64_t t) {
    return t == 0 ? nullptr : reinterpret_cast<cFC4OccupantFilter*>(static_cast<uintptr_t>(t));
}

// A callback whose game call the spec leaves open (§4.8 [choice] and the
// effect slots): recorded once each so the log names every one.
void NotWired(void* u, const char* what) {
    auto& v = C(u)->notWired;
    if (std::find(v.begin(), v.end(), what) == v.end()) {
        v.push_back(what);
        Log("demolition host: not wired (recorded, no game call): %s", what);
    }
}

cISC4OccupantManager* OMgr(cISC4City* city) {
    return city ? city->GetOccupantManager() : nullptr;  // city slot 34, borrowed
}

bool InManager(cISC4Occupant* o) { return o != nullptr && o->IsFlagSet(1); }  // occupant slot 32, DM4

// The shared occupant filters (§4.2 step 1). The game's own
// cSC4DefaultOccupantFilter / cSC4RealisticOccupantFilter have no getter, so
// the verdicts are the spec's literal reading here.
bool ExposesInterface(cISC4Occupant* o, uint32_t iid) {
    if (o == nullptr) return false;
    void* p = nullptr;
    if (!o->QueryInterface(iid, &p) || p == nullptr) return false;
    auto* queried = static_cast<cIGZUnknown*>(p);
    FC4_ACQUIRED(queried, "QueryInterface");
    FC4_RELEASE(queried);
    return true;
}

bool IsDefaultFilterGroup(cISC4Occupant* o) {
    for (uint32_t g : kDefaultFilterGroups) {
        if (o != nullptr && o->IsOccupantGroup(g)) return true;
    }
    return false;
}

// occupant-manager.md §13.2.3: all three realistic-filter flags start clear.
bool RealisticRejects(cISC4Occupant* o, cISC4City* city) {
    if (o->IsOccupantGroup(kRealisticExtraGroup)) return true;
    if (o->GetType() == 0x497F6D9D && ExposesInterface(o, 0x097F6D98) && city != nullptr) {
        cS3DVector3 position;
        o->GetPosition(&position);
        int x = 0, z = 0;
        city->PositionToCell(position.fX, position.fZ, x, z);
        cISC4LotManager* manager = city->GetLotManager();
        cISC4Lot* lot = manager ? AsGame(manager)->GetLot(x, z, false) : nullptr;
        if (lot != nullptr && static_cast<uint32_t>(AsGame(lot)->GetState()) == 1) return true;
    }
    return o->GetType() == kTypeProp &&
           (ExposesInterface(o, 0x4B44FBE2) || ExposesInterface(o, 0x2B3B7D86));
}

}  // namespace

// ---- the two shared filters (§4.2 step 1) ---------------------------------------------------------

bool DemolitionDefaultFilterIncludes(cISC4Occupant* o) {
    if (o == nullptr) return false;
    if (ExposesInterface(o, kRealismIfaceA) || ExposesInterface(o, kRealismIfaceB) || IsDefaultFilterGroup(o)) {
        return false;
    }
    // occupant-manager.md §13.2.3: these are holder properties, not IIDs.
    const cISCPropertyHolder* holder = o->AsPropertyHolder();
    if (props::Has(holder, 0x49512A09) || props::Has(holder, 0x8A15F1BD)) return false;
    std::vector<uint32_t> hidden;
    return !props::GetU32s(holder, 0xCAA45670, hidden) || hidden.empty() || hidden[0] == 0;
}

bool DemolitionRealisticFilterIncludes(cISC4Occupant* o, cISC4City* city) {
    return DemolitionDefaultFilterIncludes(o) && !RealisticRejects(o, city);
}

void DemolitionVisitOccupants(SC4List<cISC4Occupant*>& list, fc4_token_callback cb, void* ctx) {
    for (cISC4Occupant*& o : list) {
        if (o != nullptr) {
            FC4_ACQUIRED(o, "GetLotOccupants");
            cb(ctx, TokenOf(o));
            FC4_RELEASE_CREATED(o);
            o = nullptr; // SC4List's destructor releases non-null pointer elements.
        }
    }
}

// ---- the occupant manager's box and cell-rect queries (OM §8) -------------------------------------

std::shared_ptr<void> DemolitionOccupantsInBox(cISC4City* city, float x0, float z0, float x1, float z1,
                                              DemolitionHostContext* holdIn) {
    cISC4OccupantManager* om = OMgr(city);
    if (om == nullptr) return nullptr;
    auto list = std::make_shared<SC4List<cISC4Occupant*>>();
    // Slot 36 GetOccupantsByBBox(list, min3, max3, filter, maxCount): y spans
    // everything, maxCount -1 is "all" (OM §8.1). A null filter: the core
    // applies Φ itself, which gives the same set (OM §8.2 step 5).
    const float min3[3] = {x0, -FLT_MAX, z0};
    const float max3[3] = {x1, FLT_MAX, z1};
    om->GetOccupantsByBBox(*list, min3, max3, static_cast<cISC4OccupantFilter*>(nullptr), 0xFFFFFFFFu);
    if (holdIn != nullptr) holdIn->held.push_back(list);
    return list;
}

std::shared_ptr<void> DemolitionOccupantsInRect(cISC4City* city, const fc4_cell_rect* r,
                                               DemolitionHostContext* holdIn) {
    cISC4OccupantManager* om = OMgr(city);
    if (om == nullptr || r == nullptr) return nullptr;
    auto list = std::make_shared<SC4List<cISC4Occupant*>>();
    // Slot 40 GetOccupantsByStandardCityCells (OM §8.4): inclusive corners.
    const SC4Point<int> mn(r->x1, r->z1);
    const SC4Point<int> mx(r->x2, r->z2);
    om->GetOccupantsByStandardCityCells(*list, &mn, &mx, static_cast<cISC4OccupantFilter*>(nullptr),
                                       0xFFFFFFFFu);
    if (holdIn != nullptr) holdIn->held.push_back(list);
    return list;
}

cISC4OccupantManager* DemolitionOccupantManager(cISC4City* city) { return OMgr(city); }

cFC4Terrain* DemolitionTerrain(cISC4City* city) {
    // city slot 84 GetTerrain, borrowed.
    return city ? reinterpret_cast<cFC4Terrain*>(city->GetTerrain()) : nullptr;
}

void DemolitionSendPreDemolish(cIGZMessageServer2* server, cISC4Occupant* occupant) {
    if (server == nullptr || occupant == nullptr) return;
    cFC4Message2Standard* msg = FC4MakeMessage(kMsgPreDemolishNetworkOccupant, 0, 0, 0);
    if (msg == nullptr) return;
    msg->SetVoid1(occupant);
    msg->SetIGZUnknown(occupant);
    server->MessageSend(msg);
    msg->Release();
}

void DemolitionHostReleaseHeld(DemolitionHostContext* ctx) {
    if (ctx == nullptr) return;
    for (auto& l : ctx->held) {
        // Each held list's occupants carry one reference from the query
        // (OM §8.1 step 7); ReleaseOccupantList (slot 48) drops them.
        auto* list = static_cast<SC4List<cISC4Occupant*>*>(l.get());
        if (list == nullptr || ctx->city == nullptr) continue;
        cISC4OccupantManager* om = OMgr(ctx->city);
        if (om != nullptr) om->ReleaseOccupantList(*list);  // slot 48 [abi-ret] eax-or-void
    }
    ctx->held.clear();
}

void DemolitionHostReleaseAll(DemolitionHostContext* ctx) {
    if (ctx == nullptr) return;
    DemolitionHostReleaseHeld(ctx);
    ctx->defaultFilter = 0;
    ctx->realisticFilter = 0;
    ctx->city = nullptr;
}

// ---- host callbacks (fc4_demolition_host) ---------------------------------------------------------

namespace {

// -- lifecycle (demolition §4.1) --

void Subscribe(void* u, uint32_t type) {
    cIGZMessageServer2Ptr ms;
    if (ms && C(u)->messageTarget != nullptr) {
        ms->AddNotification(static_cast<cIGZMessageTarget2*>(C(u)->messageTarget), type);
    }
}

void Unsubscribe(void* u, uint32_t type) {
    cIGZMessageServer2Ptr ms;
    if (ms && C(u)->messageTarget != nullptr) {
        ms->RemoveNotification(static_cast<cIGZMessageTarget2*>(C(u)->messageTarget), type);
    }
}

bool AcquireCity(void* u) { return City(u) != nullptr; }
void ReleaseCity(void*) {}  // borrowed for the city session; never released here

void CityCellCounts(void* u, int32_t* x, int32_t* z) {
    // [question DQ2] §2 names these "city slots 93, 94"; docs/abi lists those
    // as the lighting manager and the draw context. Every other glue lane
    // reads CellCountX / CellCountZ here (FC4DisasterHost.cpp, "slots 93, 94"),
    // so this host does the same rather than guess at the terrain's counters.
    cISC4City* c = City(u);
    if (x) *x = c ? static_cast<int32_t>(c->CellCountX()) : 0;
    if (z) *z = c ? static_cast<int32_t>(c->CellCountZ()) : 0;
}

bool AcquireOccupantManager(void* u) { return OMgr(City(u)) != nullptr; }
void ReleaseOccupantManager(void*) {}

bool AcquireLotDeveloper(void* u) {
    cISC4City* c = City(u);
    return c != nullptr && c->GetLotDeveloper() != nullptr;  // city slot 79
}
void ReleaseLotDeveloper(void*) {}

bool AcquireTerrain(void* u) { return DemolitionTerrain(City(u)) != nullptr; }
void ReleaseTerrain(void*) {}

// -- tuning (§6) --
// Each returns a NEW fc4_props the core takes ownership of (capi take_props);
// without the exemplar the core keeps its shipped values.

fc4_props* TuningExemplar(uint32_t group, uint32_t inst) {
    props::Exemplar ex;
    if (!ex.Load(kTuningType, group, inst)) return nullptr;
    return props::ToCoreAll(ex.Holder());
}

fc4_props* ThresholdsExemplar(void*) { return TuningExemplar(kTuningThresholdsGroup, kTuningThresholdsInst); }
fc4_props* BuildingDevelopmentExemplar(void*) { return TuningExemplar(kTuningBuildingGroup, kTuningBuildingInst); }

// -- references --
void Release(void* u, uint64_t t) {
    // Only the view filter and a deferred lot's lot reach here (mod.rs
    // "Tokens and references"); both are references the core took.
    cIGZUnknown* p = FromToken(t);
    if (p != nullptr) {
        FC4_RELEASE(p);
    }
    (void)u;
}

// -- the view and the filters (§4.2, §4.4 step 2) --

uint64_t AcquireViewFilter(void* u) {
    // [question DQ3] GetCurrentViewFilterInfo is view slot 18 and returns a
    // struct by reference, not a filter object; §4.4 step 2 describes the
    // result as an AddRef'd filter that may be null. With no reachable view
    // the spec's [choice] applies: no view filter.
    (void)u;
    return 0;
}

bool ViewLevel(void* u, uint32_t* out) {
    // §4.4 step 2 [choice]: with no 3D view the level is 1.
    if (out) *out = 1;
    (void)u;
    return true;
}

bool FilterIncludes(void*, uint64_t filter, uint64_t occupant) {
    cFC4OccupantFilter* f = Filt(filter);
    return f != nullptr && f->IsOccupantIncluded(Occ(occupant));  // filter slot 3 [abi-ret] al
}

bool FilterIncludesType(void*, uint64_t filter, uint32_t type) {
    cFC4OccupantFilter* f = Filt(filter);
    return f != nullptr && f->IsOccupantTypeIncluded(type);  // filter slot 4 [abi-ret] al
}

bool DefaultFilterIncludes(void*, uint64_t occupant) {
    return DemolitionDefaultFilterIncludes(Occ(occupant));
}

bool RealisticFilterIncludes(void* u, uint64_t occupant) {
    return DemolitionRealisticFilterIncludes(Occ(occupant), City(u));
}

// -- occupants --

uint32_t OccupantType(void*, uint64_t t) {
    cISC4Occupant* o = Occ(t);
    return o != nullptr ? o->GetType() : 0u;  // occupant slot 7
}

void OccupantPosition(void*, uint64_t t, float* out3) {
    if (out3 == nullptr) return;
    out3[0] = out3[1] = out3[2] = 0.0f;
    cISC4Occupant* o = Occ(t);
    if (o == nullptr) return;
    cS3DVector3 v;
    o->GetPosition(&v);  // occupant slot 8
    out3[0] = v.fX;
    out3[1] = v.fY;
    out3[2] = v.fZ;
}

bool CanRemove(void*, uint64_t t, uint32_t level) {
    cISC4Occupant* o = Occ(t);
    return o != nullptr && o->CanRemove(level);  // occupant slot 14
}

bool OccupantFlagSet(void*, uint64_t t, uint32_t flag) {
    cISC4Occupant* o = Occ(t);
    return o != nullptr && o->IsFlagSet(flag);  // occupant slot 32
}

uint32_t OccupantFlags(void*, uint64_t t) {
    cISC4Occupant* o = Occ(t);
    return o != nullptr ? o->GetFlags() : 0u;  // occupant slot 33
}

bool IsOccupantGroup(void*, uint64_t t, uint32_t group) {
    cISC4Occupant* o = Occ(t);
    return o != nullptr && o->IsOccupantGroup(group);  // occupant slot 22
}

bool OccupantBoolProperty(void*, uint64_t t, uint32_t id, bool* out) {
    if (out) *out = false;
    cISC4Occupant* o = Occ(t);
    if (o == nullptr) return false;
    const cISCPropertyHolder* h = o->AsPropertyHolder();  // occupant slot 6
    if (h == nullptr) return false;
    // [question DQ6] The property is a bool; props has no GetBool, so the
    // numeric read is used and a non-zero value is true (a bool variant
    // converts through GetU32s).
    std::vector<uint32_t> v;
    if (!props::GetU32s(h, id, v) || v.empty()) return false;
    if (out) *out = v[0] != 0;
    return true;
}

void OccupantBounds(void*, uint64_t t, float* out4) {
    if (out4 == nullptr) return;
    out4[0] = out4[1] = out4[2] = out4[3] = 0.0f;
    cISC4Occupant* o = Occ(t);
    if (o == nullptr) return;
    cS3DVector3 mn, mx;
    o->GetBoundingBox(&mn, &mx);  // occupant slot 10
    out4[0] = mn.fX;
    out4[1] = mn.fZ;
    out4[2] = mx.fX;
    out4[3] = mx.fZ;
}

bool FoundationSlot9(void*, uint64_t t, uint32_t* out) {
    // §4.2 step 5: cISC4FoundationOccupant slot 9 (GetKind). The original
    // does not test the QueryInterface result (DM5); FreeCity4 rejects an
    // occupant without the interface [choice].
    if (out) *out = 0;
    cISC4Occupant* o = Occ(t);
    if (o == nullptr) return false;
    void* p = nullptr;
    if (!o->QueryInterface(kIID_cISC4FoundationOccupant, &p) || p == nullptr) return false;
    cFC4FoundationOccupant* f = static_cast<cFC4FoundationOccupant*>(p);
    FC4_ACQUIRED(f, "QueryInterface");
    if (out) *out = f->GetKind();  // slot 9
    FC4_RELEASE(f);
    return true;
}

bool NetworkHasAnyFlag(void*, uint64_t t, uint32_t mask, bool* out) {
    // §4.2 step 7: cISC4NetworkOccupant slot 9.
    if (out) *out = false;
    cISC4Occupant* o = Occ(t);
    if (o == nullptr) return false;
    void* p = nullptr;
    if (!o->QueryInterface(kIID_cISC4NetworkOccupant, &p) || p == nullptr) return false;
    cFC4NetworkOccupant* n = static_cast<cFC4NetworkOccupant*>(p);
    FC4_ACQUIRED(n, "QueryInterface");
    const bool any = n->HasAnyNetworkFlag(mask) != 0;  // slot 9 [abi-ret] eax-or-void
    FC4_RELEASE(n);
    if (out) *out = any;
    return true;
}

bool NetworkHasType(void*, uint64_t t, uint32_t type) {
    cISC4Occupant* o = Occ(t);
    return o != nullptr && o->GetType() == type;
}

void OccupantShutdown(void*, uint64_t t) {
    cISC4Occupant* o = Occ(t);
    if (o != nullptr) o->Shutdown();  // occupant slot 4
}

bool RemoveOccupantAt(void* u, uint64_t t, bool shutdown) {
    cISC4Occupant* o = Occ(t);
    cISC4OccupantManager* om = OMgr(City(u));
    if (o == nullptr || om == nullptr) return false;
    return om->RemoveOccupant(o, shutdown, 0);  // slot 21 [abi-ret] al
}

void OccupantsInBox(void* u, float x0, float z0, float x1, float z1, fc4_token_callback cb, void* ctx) {
    auto list = DemolitionOccupantsInBox(City(u), x0, z0, x1, z1, C(u));
    if (!list || cb == nullptr) return;
    for (cISC4Occupant* o : *static_cast<SC4List<cISC4Occupant*>*>(list.get())) {
        if (o != nullptr) cb(ctx, TokenOf(o));
    }
}

void OccupantsInCellRect(void* u, const fc4_cell_rect* r, fc4_token_callback cb, void* ctx) {
    auto list = DemolitionOccupantsInRect(City(u), r, C(u));
    if (!list || cb == nullptr) return;
    for (cISC4Occupant* o : *static_cast<SC4List<cISC4Occupant*>*>(list.get())) {
        if (o != nullptr) cb(ctx, TokenOf(o));
    }
}

// -- prices and removals (§4.3) --

bool BuildingDemolitionCost(void* u, uint64_t t, int64_t* out) {
    // The building development cost getter (building, 1): city slot 53, then
    // slot 10 (int64 property 0x099AFACD).
    cISC4City* c = City(u);
    cISC4Occupant* o = Occ(t);
    if (c == nullptr || o == nullptr) return false;
    cISC4BuildingDevelopmentSimulator* bds = c->GetBuildingDevelopmentSimulator();  // slot 53
    if (bds == nullptr) return false;
    // The slot takes the cISC4BuildingOccupant* interface, which is not
    // necessarily the cISC4Occupant* at offset 0: QueryInterface for it and
    // release the reference that gives (HANDOFF §5.1b).
    void* p = nullptr;
    if (!o->QueryInterface(kIID_cISC4BuildingOccupant, &p) || p == nullptr) return false;
    cISC4BuildingOccupant* b = static_cast<cISC4BuildingOccupant*>(p);
    FC4_ACQUIRED(b, "QueryInterface");
    if (out) {
        // abi: via cFC4ISC4BuildingDevelopmentSimulator (slot 10; the SDK's
        // slot 9 is the foundation/flora-body overload)
        *out = fc4::AsGame(bds)->GetCostOfOccupantDemolition(b, 1);  // abi: via cFC4I, slot 10
    }
    FC4_RELEASE(b);
    return true;
}

bool FloraDemolitionCost(void* u, uint64_t t, int64_t* out) {
    // The flora cost getter (o, 0): city slot 59, flora slot 22.
    cISC4City* c = City(u);
    cISC4Occupant* o = Occ(t);
    if (c == nullptr || o == nullptr) return false;
    cISC4FloraSimulator* f = c->GetFloraSimulator();  // slot 59
    if (f == nullptr) return false;
    // abi: via cFC4FloraSim (slot 22)
    if (out) *out = reinterpret_cast<cFC4FloraSim*>(f)->GetCostOfOccupantDemolition(o, 0);  // abi: via cFC4I, slot 22
    return true;
}

bool IsNetworkOccupant(void*, uint64_t t) {
    cISC4Occupant* o = Occ(t);
    return o != nullptr && o->GetType() == kTypeNetwork;
}

int64_t NetworkDemolitionCost(void*, uint64_t t) {
    // cISC4NetworkOccupant slot 63 (int64).
    cISC4Occupant* o = Occ(t);
    if (o == nullptr) return 0;
    void* p = nullptr;
    if (!o->QueryInterface(kIID_cISC4NetworkOccupant, &p) || p == nullptr) return 0;
    cFC4NetworkOccupant* n = static_cast<cFC4NetworkOccupant*>(p);
    FC4_ACQUIRED(n, "QueryInterface");
    const int64_t cost = n->NetSlot63();  // slot 63
    FC4_RELEASE(n);
    return cost;
}

bool IsPowerPole(void*, uint64_t t) {
    cISC4Occupant* o = Occ(t);
    return o != nullptr && o->GetType() == kTypePowerPole;
}

int64_t PoleDemolitionCost(void*, uint64_t t) {
    // cISC4PowerPoleOccupant slot 10 (int64).
    cISC4Occupant* o = Occ(t);
    if (o == nullptr) return 0;
    void* p = nullptr;
    if (!o->QueryInterface(kIID_cISC4PowerPoleOccupant, &p) || p == nullptr) return 0;
    cFC4ISC4PowerPoleOccupant* pole = static_cast<cFC4ISC4PowerPoleOccupant*>(p);
    FC4_ACQUIRED(pole, "QueryInterface");
    const int64_t cost = pole->PoleSlot10();  // slot 10
    FC4_RELEASE(pole);
    return cost;
}

void PoleConnections(void*, uint64_t t, fc4_token_callback cb, void* ctx) {
    // §4.3 "Pole references" (DM11): the neighbours are remembered with one
    // reference each; here they are handed out borrowed and the caller (the
    // core) only reads them for the call, so no reference is taken.
    cISC4Occupant* o = Occ(t);
    if (o == nullptr || cb == nullptr) return;
    void* p = nullptr;
    if (!o->QueryInterface(kIID_cISC4PowerPoleOccupant, &p) || p == nullptr) return;
    cFC4ISC4PowerPoleOccupant* pole = static_cast<cFC4ISC4PowerPoleOccupant*>(p);
    FC4_ACQUIRED(pole, "QueryInterface");
    const uint32_t n = pole->PoleSlot04();  // slot 4, the count
    for (uint32_t i = 0; i < n; ++i) {
        cISC4Occupant* other = pole->PoleSlot07(i) ? reinterpret_cast<cISC4Occupant*>(static_cast<uintptr_t>(
                                                        static_cast<uint64_t>(pole->PoleSlot07(i))))
                                                   : nullptr;
        if (other != nullptr) cb(ctx, TokenOf(other));
    }
    FC4_RELEASE(pole);
}

uint32_t PoleConnectionCount(void*, uint64_t t) {
    cISC4Occupant* o = Occ(t);
    if (o == nullptr) return 0;
    void* p = nullptr;
    if (!o->QueryInterface(kIID_cISC4PowerPoleOccupant, &p) || p == nullptr) return 0;
    cFC4ISC4PowerPoleOccupant* pole = static_cast<cFC4ISC4PowerPoleOccupant*>(p);
    FC4_ACQUIRED(pole, "QueryInterface");
    const uint32_t n = pole->PoleSlot04();  // slot 4
    FC4_RELEASE(pole);
    return n;
}

void PoleBreakAllConnections(void*, uint64_t t) {
    cISC4Occupant* o = Occ(t);
    if (o == nullptr) return;
    void* p = nullptr;
    if (!o->QueryInterface(kIID_cISC4PowerPoleOccupant, &p) || p == nullptr) return;
    cFC4ISC4PowerPoleOccupant* pole = static_cast<cFC4ISC4PowerPoleOccupant*>(p);
    FC4_ACQUIRED(pole, "QueryInterface");
    pole->BreakAllConnections(true);  // slot 6
    FC4_RELEASE(pole);
}

void SendPreDemolish(void* u, uint64_t t) {
    // §3: 0xAA7A014D with void1 = the pole occupant; the neighbours simulator
    // answers with DeleteOccupantConnection.
    cISC4Occupant* o = Occ(t);
    if (o == nullptr) return;
    cIGZMessageServer2Ptr ms;
    if (!ms) return;
    DemolitionSendPreDemolish(ms, o);
    (void)u;
}

bool HasPropManager(void* u) {
    cISC4City* c = City(u);
    return c != nullptr && c->GetPropManager() != nullptr;  // city slot 35
}

bool RemoveProp(void* u, uint64_t t) {
    cISC4City* c = City(u);
    cISC4Occupant* o = Occ(t);
    if (c == nullptr || o == nullptr) return false;
    cISC4PropManager* pm = c->GetPropManager();  // slot 35
    if (pm == nullptr) return false;
    return reinterpret_cast<cFC4PropManager2*>(pm)->RemoveProp(o);  // slot 29 [abi-ret] al
}

// -- caller-owned containers (§4.3, §4.4 step 8) --

void ListAppend(void*, uint64_t list, uint64_t occupant) {
    // DM11: the orphan carries one reference that the list's owner releases.
    auto* l = reinterpret_cast<SC4List<cISC4Occupant*>*>(FromToken(list));
    cISC4Occupant* o = Occ(occupant);
    if (l == nullptr || o == nullptr) return;
    o->AddRef();
    l->push_back(o);
}

void SetInsert(void*, uint64_t set, uint64_t occupant) {
    // The `demolished` set is std::set<cRZAutoRefCount<cISC4Occupant>>: a
    // unique insert AddRefs only a new one (FC4GameSet.h).
    auto* s = reinterpret_cast<GameSetObject*>(FromToken(set));
    cISC4Occupant* o = Occ(occupant);
    if (s == nullptr || o == nullptr) return;
    const uint32_t key = static_cast<uint32_t>(reinterpret_cast<uintptr_t>(o));
    if (GameSetInsertU32<StlNodeAllocator>(s, key, 0x7FFFFFFFu) == GameSetResult::Inserted) o->AddRef();
}

// -- networks (§4.6) --

void QueueRemovals(void* u, bool on) {
    cISC4City* c = City(u);
    if (c == nullptr) return;
    cISC4NetworkManager* nm = c->GetNetworkManager();  // slot 38
    if (nm != nullptr) reinterpret_cast<cFC4NetworkManager*>(nm)->QueueRemovals(on);  // slot 7 void
}

void DeleteOccupantConnection(void* u, uint64_t t) {
    // §4.6: neighbors.DeleteOccupantConnection(o) (neighbours simulator slot 8).
    cISC4Occupant* o = Occ(t);
    cISC4City* c = City(u);
    if (o == nullptr || c == nullptr) return;
    cISC4NeighborsSimulator* ns = c->GetNeighborsSimulator();  // slot 62
    if (ns != nullptr) fc4::AsGame(ns)->DeleteOccupantConnection(o);  // slot 8
}

void DestroyNetworkOccupants(void* u, const uint64_t* tokens, size_t count) {
    // §4.6: networkManager.DestroyNetworkOccupants(list), then the list goes.
    cISC4City* c = City(u);
    if (c == nullptr || tokens == nullptr || count == 0) return;
    cISC4NetworkManager* nm = c->GetNetworkManager();  // slot 38
    if (nm == nullptr) return;
    SC4List<cISC4Occupant*> list;
    for (size_t i = 0; i < count; ++i) {
        cISC4Occupant* o = Occ(tokens[i]);
        if (o != nullptr) {
            o->AddRef();
            list.push_back(o);
        }
    }
    reinterpret_cast<cFC4NetworkManager*>(nm)->DestroyNetworkOccupants(list);  // slot 6 void
    for (cISC4Occupant*& o : list) {
        if (o != nullptr) o->Release();
        o = nullptr; // the SDK list destructor must not release the same reference again
    }
}

// -- city geometry (§4.4 step 4, §4.7) --

void CellCornerToPosition(void* u, int32_t x, int32_t z, float* fx, float* fz) {
    cISC4City* c = City(u);
    if (c == nullptr) {
        if (fx) *fx = 0;
        if (fz) *fz = 0;
        return;
    }
    float a = 0, b = 0;
    c->CellCornerToPosition(x, z, a, b);  // city slot 96
    if (fx) *fx = a;
    if (fz) *fz = b;
}

bool PositionToCell(void* u, float x, float z, int32_t* cx, int32_t* cz) {
    cISC4City* c = City(u);
    if (c == nullptr) return false;
    int a = 0, b = 0;
    c->PositionToCell(x, z, a, b);  // city slot 95
    if (cx) *cx = a;
    if (cz) *cz = b;
    return true;
}

// -- lots --

uint64_t LotAt(void* u, int32_t x, int32_t z) {
    cISC4City* c = City(u);
    if (c == nullptr) return 0;
    cISC4LotManager* lm = c->GetLotManager();  // city slot 33
    if (lm == nullptr) return 0;
    // [question DQ4] §4.4 step 4 names this "lot manager slot 5"; slot 5 is
    // GetLotAtPoint and slot 6 is GetLot(x, z, bool), which every other glue
    // lane calls (cFC4Aura.cpp "slot 6, borrowed"). Slot 6 is used here.
    return TokenOf(fc4::AsGame(lm)->GetLot(x, z, false));  // slot 6, borrowed
}

void LotsInCellRect(void* u, const fc4_cell_rect* r, fc4_token_callback cb, void* ctx) {
    cISC4City* c = City(u);
    if (c == nullptr || r == nullptr || cb == nullptr) return;
    cISC4LotManager* lm = c->GetLotManager();  // slot 33
    if (lm == nullptr) return;
    fc4::GameList<cISC4Lot*> lots;
    fc4::AsGame(lm)->GetLotsInCellRect(lots, r->x1, r->z1, r->x2, r->z2, false);  // slot 7
    for (cISC4Lot*& l : lots) {
        if (l != nullptr) {
            FC4_ACQUIRED(l, "GetLotsInCellRect");
            cb(ctx, TokenOf(l));
            FC4_RELEASE_CREATED(l);  // one reference each (OM/lot manager contract)
            l = nullptr; // GameList's destructor also releases non-null pointer elements
        }
    }
}

uint64_t LotBuilding(void*, uint64_t t) {
    cISC4Lot* l = Lot(t);
    // Lot slot 48 returns the building interface, not its occupant subobject.
    // Occupant tokens must use building slot 3 (SDK AsOccupant); both are borrowed.
    cISC4BuildingOccupant* b = l != nullptr ? fc4::AsGame(l)->GetBuilding() : nullptr;
    return b != nullptr ? TokenOf(b->AsOccupant()) : 0;
}

uint32_t LotState(void*, uint64_t t) {
    cISC4Lot* l = Lot(t);
    return l != nullptr ? static_cast<uint32_t>(fc4::AsGame(l)->GetState()) : 0u;  // slot 33
}

bool LotDemolishable(void*, uint64_t t) {
    cISC4Lot* l = Lot(t);
    return l != nullptr && fc4::AsGame(l)->IsInDemolishableState();  // slot 39
}

void LotOccupants(void*, uint64_t t, fc4_token_callback cb, void* ctx) {
    cISC4Lot* l = Lot(t);
    if (l == nullptr || cb == nullptr) return;
    // Slot 49 GetLotOccupants [abi-ret] via-call: one reference each.
    SC4List<cISC4Occupant*> list;
    fc4::AsGame(l)->GetLotOccupants(list);
    DemolitionVisitOccupants(list, cb, ctx);
}

void LotSize(void*, uint64_t t, uint8_t* sx, uint8_t* sz) {
    if (sx) *sx = 0;
    if (sz) *sz = 0;
    cISC4Lot* l = Lot(t);
    if (l == nullptr) return;
    uint8_t a = 0, b = 0;
    fc4::AsGame(l)->GetSize(a, b);  // slot 22
    if (sx) *sx = a;
    if (sz) *sz = b;
}

void LotBoundingRect(void*, uint64_t t, fc4_cell_rect* out) {
    if (out == nullptr) return;
    out->x1 = out->z1 = out->x2 = out->z2 = 0;
    cISC4Lot* l = Lot(t);
    if (l == nullptr) return;
    SC4Rect<int32_t> r;
    if (!fc4::AsGame(l)->GetBoundingRect(r)) return;  // slot 14
    out->x1 = r.topLeftX;
    out->z1 = r.topLeftY;
    out->x2 = r.bottomRightX;
    out->z2 = r.bottomRightY;
}

uint32_t LotFacing(void*, uint64_t t) {
    cISC4Lot* l = Lot(t);
    return l != nullptr ? static_cast<uint32_t>(fc4::AsGame(l)->GetFacing()) : 0u;  // slot 23
}

uint32_t LotZoneType(void*, uint64_t t) {
    cISC4Lot* l = Lot(t);
    return l != nullptr ? static_cast<uint32_t>(fc4::AsGame(l)->GetZoneType()) : 0u;  // slot 31
}

bool LotWaterCompatible(void*, uint64_t t, bool* out) {
    if (out) *out = false;
    cISC4Lot* l = Lot(t);
    if (l == nullptr) return false;
    cISC4LotConfiguration* cfg = fc4::AsGame(l)->GetLotConfiguration();  // slot 9
    if (cfg == nullptr) return false;
    const bool ok = cfg->IsCompatibleWithWater();  // lot configuration slot 14
    if (out) *out = ok;
    return true;
}

bool LotInitialized(void*, uint64_t t) {
    cISC4Lot* l = Lot(t);
    return l != nullptr && fc4::AsGame(l)->IsInitialized();  // slot 6
}

bool LotVerticesLocked(void*, uint64_t t) {
    cISC4Lot* l = Lot(t);
    // [abi-ret] slot 68 GetVerticesAreLocked: TSV ret eax-or-void (a bool in
    // AL); demolition.md §4.4 step 7 and §4.7 read it as a bool.
    return l != nullptr && fc4::AsGame(l)->GetVerticesAreLocked();  // [abi-ret] slot 68
}

uint32_t LotCondition(void*, uint64_t t) {
    cISC4Lot* l = Lot(t);
    return l != nullptr ? fc4::AsGame(l)->GetCondition() : 0u;  // slot 35
}

void ExplodeLot(void* u, uint64_t t, bool b) {
    cISC4City* c = City(u);
    cISC4Lot* l = Lot(t);
    if (c == nullptr || l == nullptr) return;
    cISC4LotDeveloper* ld = c->GetLotDeveloper();  // slot 79
    if (ld != nullptr) fc4::AsGame(ld)->ExplodeLot(l, b);  // slot 14 [abi-ret] via-call
}

void StartDemolishLot(void* u, uint64_t t) {
    cISC4City* c = City(u);
    cISC4Lot* l = Lot(t);
    if (c == nullptr || l == nullptr) return;
    cISC4LotDeveloper* ld = c->GetLotDeveloper();  // slot 79
    if (ld != nullptr) fc4::AsGame(ld)->StartDemolishLot(l);  // slot 12 [abi-ret] unknown
}

void EndDemolishLot(void* u, uint64_t t) {
    cISC4City* c = City(u);
    cISC4Lot* l = Lot(t);
    if (c == nullptr || l == nullptr) return;
    cISC4LotDeveloper* ld = c->GetLotDeveloper();  // slot 79
    if (ld != nullptr) fc4::AsGame(ld)->EndDemolishLot(l);  // slot 13 [abi-ret] via-call
}

void DeleteLot(void* u, uint64_t t) {
    cISC4City* c = City(u);
    cISC4Lot* l = Lot(t);
    if (c == nullptr || l == nullptr) return;
    cISC4LotManager* lm = c->GetLotManager();  // slot 33
    if (lm != nullptr) fc4::AsGame(lm)->DeleteLot(l);  // slot 24
}

bool CreateLotIfPossible(void* u, int32_t x, int32_t z, uint32_t facing) {
    // §4.4 step 7 re-parcel: CreateLotIfPossible(x, z, 1, 1, facing, &new, 0, 0, 0).
    cISC4City* c = City(u);
    if (c == nullptr) return false;
    cISC4LotManager* lm = c->GetLotManager();  // slot 33
    if (lm == nullptr) return false;
    cISC4Lot* created = nullptr;
    const bool ok = fc4::AsGame(lm)->CreateLotIfPossible(x, z, 1, 1, static_cast<int32_t>(facing), created, false,
                                                        false, nullptr);  // slot 22
    if (created != nullptr) {
        FC4_ACQUIRED(created, "CreateLotIfPossible");
        FC4_RELEASE_CREATED(created);
    }
    return ok;
}

uint32_t ZoneAt(void* u, int32_t x, int32_t z) {
    cISC4City* c = City(u);
    if (c == nullptr) return 0;
    cISC4ZoneManager* zm = c->GetZoneManager();  // slot 36
    return zm != nullptr ? static_cast<uint32_t>(fc4::AsGame(zm)->GetZoneType(x, z)) : 0u;  // slot 12
}

void Dezone(void* u, const fc4_cell_rect* r) {
    // §4.4 step 7: PlaceZone(rect, 0, 1, 1, 0, 1, 1, 0, 0, 0) (zone manager slot 11).
    cISC4City* c = City(u);
    if (c == nullptr || r == nullptr) return;
    cISC4ZoneManager* zm = c->GetZoneManager();  // slot 36
    if (zm == nullptr) return;
    RegionBuffer rb;
    if (!rb.Build(*r)) return;
    fc4::AsGame(zm)->PlaceZone(rb.Zoning(), 0, 1, 1, 0, 1, 1, nullptr, nullptr, nullptr);  // slot 11
}

// -- terrain (§4.7) --

float AltitudeAtVertex(void* u, int32_t x, int32_t z) {
    cFC4Terrain* t = DemolitionTerrain(City(u));
    return t != nullptr ? t->GetAltitudeAtVertex(x, z) : 0.0f;  // terrain slot 14, x87
}

bool IsWaterVertex(void* u, int32_t x, int32_t z) {
    cFC4Terrain* t = DemolitionTerrain(City(u));
    return t != nullptr && t->IsWaterVertex(x, z);  // terrain slot 37
}

void SetAltitudeRect(void* u, const fc4_cell_rect* r, float h, uint32_t reason) {
    cFC4Terrain* t = DemolitionTerrain(City(u));
    if (t == nullptr || r == nullptr) return;
    fc4_cell_rect rr{*r};
    t->SetAltitudeRect(rr, h, false, reason);  // terrain slot 49
}

void SetAltitudeVertex(void* u, int32_t x, int32_t z, float h) {
    cFC4Terrain* t = DemolitionTerrain(City(u));
    if (t == nullptr) return;
    t->SetAltitudeXYZ(x, z, h);  // terrain slot 51, void (§4.7 per-vertex step 1)
}

void RedisplayTerrain(void* u, const fc4_cell_rect* r, uint32_t reason) {
    cFC4Terrain* t = DemolitionTerrain(City(u));
    if (t == nullptr || r == nullptr) return;
    fc4_cell_rect rr{*r};
    t->RedisplayTerrain(true, true, rr, reason);  // terrain slot 68, void
}

// -- effects (§4.8) --

bool EffectExtent(void*, uint64_t, float* out) {
    // [question DQ5] §4.8's model extent has no slot in docs/abi; the effect
    // sizes are presentation only and the spec lets a replacement do nothing
    // here [choice].
    if (out) *out = 0.0f;
    return false;
}

void PlayOccupantSound(void* u, uint64_t) { NotWired(u, "play_occupant_sound"); }

void StartOccupantEffect(void* u, uint64_t, const uint32_t*, size_t, bool, int32_t, int32_t, bool) {
    NotWired(u, "start_occupant_effect");
}

void StartGroupEffect(void* u, uint32_t, int32_t, int32_t) { NotWired(u, "start_group_effect"); }

}  // namespace

// ---- the table ------------------------------------------------------------------------------------

fc4_demolition_host MakeDemolitionHost(DemolitionHostContext* ctx) {
    fc4_demolition_host h{};
    h.struct_size = sizeof(h);
    h.user = ctx;
    // lifecycle
    h.subscribe = &Subscribe;
    h.unsubscribe = &Unsubscribe;
    h.acquire_city = &AcquireCity;
    h.release_city = &ReleaseCity;
    h.city_cell_counts = &CityCellCounts;
    h.acquire_occupant_manager = &AcquireOccupantManager;
    h.release_occupant_manager = &ReleaseOccupantManager;
    h.acquire_lot_developer = &AcquireLotDeveloper;
    h.release_lot_developer = &ReleaseLotDeveloper;
    h.acquire_terrain = &AcquireTerrain;
    h.release_terrain = &ReleaseTerrain;
    // tuning
    h.thresholds_exemplar = &ThresholdsExemplar;
    h.building_development_exemplar = &BuildingDevelopmentExemplar;
    // references
    h.release = &Release;
    // the view and the filters
    h.acquire_view_filter = &AcquireViewFilter;
    h.view_level = &ViewLevel;
    h.filter_includes = &FilterIncludes;
    h.filter_includes_type = &FilterIncludesType;
    h.default_filter_includes = &DefaultFilterIncludes;
    h.realistic_filter_includes = &RealisticFilterIncludes;
    // occupants
    h.occupant_type = &OccupantType;
    h.occupant_position = &OccupantPosition;
    h.can_remove = &CanRemove;
    h.occupant_flag_set = &OccupantFlagSet;
    h.occupant_flags = &OccupantFlags;
    h.is_occupant_group = &IsOccupantGroup;
    h.occupant_bool_property = &OccupantBoolProperty;
    h.occupant_bounds = &OccupantBounds;
    h.foundation_slot9 = &FoundationSlot9;
    h.network_has_any_flag = &NetworkHasAnyFlag;
    h.network_has_type = &NetworkHasType;
    h.occupant_shutdown = &OccupantShutdown;
    h.remove_occupant = &RemoveOccupantAt;
    h.occupants_in_box = &OccupantsInBox;
    h.occupants_in_cell_rect = &OccupantsInCellRect;
    // prices and removals
    h.building_demolition_cost = &BuildingDemolitionCost;
    h.flora_demolition_cost = &FloraDemolitionCost;
    h.is_network_occupant = &IsNetworkOccupant;
    h.network_demolition_cost = &NetworkDemolitionCost;
    h.is_power_pole = &IsPowerPole;
    h.pole_demolition_cost = &PoleDemolitionCost;
    h.pole_connections = &PoleConnections;
    h.pole_connection_count = &PoleConnectionCount;
    h.pole_break_all_connections = &PoleBreakAllConnections;
    h.send_pre_demolish = &SendPreDemolish;
    h.has_prop_manager = &HasPropManager;
    h.remove_prop = &RemoveProp;
    // caller-owned containers
    h.list_append = &ListAppend;
    h.set_insert = &SetInsert;
    // networks
    h.queue_removals = &QueueRemovals;
    h.delete_occupant_connection = &DeleteOccupantConnection;
    h.destroy_network_occupants = &DestroyNetworkOccupants;
    // city geometry
    h.cell_corner_to_position = &CellCornerToPosition;
    h.position_to_cell = &PositionToCell;
    // lots
    h.lot_at = &LotAt;
    h.lots_in_cell_rect = &LotsInCellRect;
    h.lot_building = &LotBuilding;
    h.lot_state = &LotState;
    h.lot_demolishable = &LotDemolishable;
    h.lot_occupants = &LotOccupants;
    h.lot_size = &LotSize;
    h.lot_bounding_rect = &LotBoundingRect;
    h.lot_facing = &LotFacing;
    h.lot_zone_type = &LotZoneType;
    h.lot_water_compatible = &LotWaterCompatible;
    h.lot_initialized = &LotInitialized;
    h.lot_vertices_locked = &LotVerticesLocked;
    h.lot_condition = &LotCondition;
    h.explode_lot = &ExplodeLot;
    h.start_demolish_lot = &StartDemolishLot;
    h.end_demolish_lot = &EndDemolishLot;
    h.delete_lot = &DeleteLot;
    h.create_lot_if_possible = &CreateLotIfPossible;
    h.zone_at = &ZoneAt;
    h.dezone = &Dezone;
    // terrain
    h.altitude_at_vertex = &AltitudeAtVertex;
    h.is_water_vertex = &IsWaterVertex;
    h.set_altitude_rect = &SetAltitudeRect;
    h.set_altitude_vertex = &SetAltitudeVertex;
    h.redisplay_terrain = &RedisplayTerrain;
    // effects
    h.effect_extent = &EffectExtent;
    h.play_occupant_sound = &PlayOccupantSound;
    h.start_occupant_effect = &StartOccupantEffect;
    h.start_group_effect = &StartGroupEffect;
    return h;
}

}  // namespace fc4
