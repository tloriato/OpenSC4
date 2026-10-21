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

#include "cFC4CityDetail.h"

#include "FC4Config.h"
#include "FC4Diag.h"
#include "FC4GameList.h"
#include "FC4Interfaces.h"
#include "FC4LotDevInterfaces.h"
#include "FC4Log.h"
#include "FC4RefAudit.h"
#include "FC4Seed.h"
#include "FC4Streams.h"
#include "FC4TractInterfaces.h"

#include "GZServPtrs.h"
#include "SC4List.h"
#include "SC4Point.h"
#include "SC4Preferences.h"
#include "SC4Rect.h"
#include "cIGZApp.h"
#include "cIGZFrameWork.h"
#include "cISC4App.h"
#include "cISC4City.h"
#include "cISC4FloraSimulator.h"
#include "cISC4Lot.h"
#include "cISC4LotConfiguration.h"
#include "cISC4LotConfigurationManager.h"
#include "cISC4LotDeveloper.h"
#include "cISC4LotManager.h"
#include "cISC4NetworkLot.h"
#include "cISC4NetworkLotManager.h"
#include "cISC4Occupant.h"
#include "cISC4OccupantManager.h"
#include "cISC4PropManager.h"
#include "cS3DVector3.h"
#include "cSC4LotConfigurationObject.h"

#include <cstring>
#include <vector>

using fc4::FromToken;
using fc4::TokenOf;

namespace {

constexpr uint32_t kIID_cIGZSerializable = 0xE4FDA3D4;

fc4::CityDetailContext* Ctx(void* u) { return static_cast<fc4::CityDetailContext*>(u); }

cISC4Lot* SdkLot(uint64_t t) { return reinterpret_cast<cISC4Lot*>(FromToken(t)); }
cFC4ISC4Lot* LotOf(uint64_t t) { return t ? fc4::AsGame(SdkLot(t)) : nullptr; }
cISC4Occupant* OccOf(uint64_t t) { return reinterpret_cast<cISC4Occupant*>(FromToken(t)); }

cISC4City* City() { return fc4::propcity::AppCity(); }

bool Writes(void* u, const char* what) {
    if (!Ctx(u)->readOnly) return true;
    (void)what;
    return false;
}

void SinkTokens(const std::vector<uint64_t>& v, fc4_u64s_sink sink, void* ctx) {
    if (sink != nullptr && !v.empty()) sink(ctx, v.data(), v.size());
}

// The entries of a game occupant list, one reference each kept in the pool.
std::vector<uint64_t> Take(void* u, SC4List<cISC4Occupant*>& list, const char* how) {
    std::vector<uint64_t> out;
    for (cISC4Occupant*& o : list) {
        if (o == nullptr) continue;
        FC4_ACQUIRED(o, how);
        Ctx(u)->pool.Adopt(o);
        out.push_back(TokenOf(o));
        o = nullptr;
    }
    return out;
}

// ---- host callbacks (fc4_city_detail_host) ---------------------------------------------------------

uint32_t DetailSetting(void* u) {
    const uint32_t s = fc4::CityDetailSetting();
    Ctx(u)->settingRead = s;
    return s;
}

void Lots(void* u, fc4_u64s_sink sink, void* ctx) {
    cISC4City* c = City();
    if (c == nullptr) return;
    bool ordered = false;
    const std::vector<cISC4Lot*> lots = fc4::propcity::AllLots(c, c->GetLotManager(), Ctx(u)->pool, &ordered);
    Ctx(u)->lotsOrdered = ordered;
    std::vector<uint64_t> tokens;
    tokens.reserve(lots.size());
    for (cISC4Lot* l : lots) tokens.push_back(TokenOf(l));
    SinkTokens(tokens, sink, ctx);
}

cISC4LotConfiguration* CfgOfLot(uint64_t lot) { return lot ? LotOf(lot)->GetLotConfiguration() : nullptr; }  // slot 9

bool LotConfigObjects(void*, uint64_t lot, fc4_lot_cfg_objects_sink sink, void* ctx) {
    cISC4LotConfiguration* cfg = CfgOfLot(lot);
    if (cfg == nullptr) return false;
    fc4::propcity::SinkCfgObjects(cfg->GetLotConfigurationObjectArray(), sink, ctx);  // cfg slot 23
    return true;
}

void LotOccupants(void* u, uint64_t lot, fc4_u64s_sink sink, void* ctx) {
    if (lot == 0) return;
    SC4List<cISC4Occupant*> list;
    LotOf(lot)->GetLotOccupants(list);  // slot 49
    SinkTokens(Take(u, list, "GetLotOccupants list entry"), sink, ctx);
}

// §3.3 step 3: every occupant in the lot's cell rectangle (slot 14). The
// original visits them through the occupant manager's slot 46 with no type;
// here the typed slot 41 with type 0, which forwards with no filter
// (allocators.md: the same traversal, list A, one reference each).
void OccupantsInLotRect(void* u, uint64_t lot, fc4_u64s_sink sink, void* ctx) {
    cISC4City* c = City();
    cISC4OccupantManager* om = c ? c->GetOccupantManager() : nullptr;
    if (lot == 0 || om == nullptr) return;
    SC4Rect<int32_t> r;
    LotOf(lot)->GetBoundingRect(r);  // slot 14
    const SC4Point<int> a(r.topLeftX, r.topLeftY);
    const SC4Point<int> b(r.bottomRightX, r.bottomRightY);
    SC4List<cISC4Occupant*> list;
    om->GetOccupantsByStandardCityCells(list, &a, &b, 0u, 0xFFFFFFFFu);
    SinkTokens(Take(u, list, "GetOccupantsByStandardCityCells list entry"), sink, ctx);
}

uint32_t OccupantType(void*, uint64_t occ) { return occ ? static_cast<uint32_t>(OccOf(occ)->GetType()) : 0; }

bool OccupantLotTag(void*, uint64_t occ, uint32_t* tag) {
    uint32_t t = 0;
    const bool has = occ != 0 && OccOf(occ)->GetLotTag(t);  // slot 27
    if (tag != nullptr) *tag = has ? t : 0;
    return has;
}

void DemolishFlora(void* u, uint64_t occ) {
    ++Ctx(u)->removes;
    if (!Writes(u, "DemolishFloraOccupant") || occ == 0) return;
    cISC4City* c = City();
    cISC4FloraSimulator* f = c ? c->GetFloraSimulator() : nullptr;
    if (f != nullptr) f->DemolishFloraOccupant(OccOf(occ), 0);  // flora slot 19
}

void RemoveProp(void* u, uint64_t occ) {
    ++Ctx(u)->removes;
    if (!Writes(u, "RemovePropA") || occ == 0) return;
    cISC4City* c = City();
    cISC4PropManager* pm = c ? c->GetPropManager() : nullptr;
    if (pm != nullptr) pm->RemovePropA(OccOf(occ));  // slot 29
}

cSC4LotConfigurationObject* ObjectAt(cISC4LotConfiguration* cfg, uint32_t index) {
    if (cfg == nullptr) return nullptr;
    SC4Vector<cSC4LotConfigurationObject>* v = cfg->GetLotConfigurationObjectArray();  // slot 23
    if (v == nullptr || index >= v->size()) return nullptr;
    return v->begin() + index;
}

void ObjectPlacement(void*, uint64_t lot, uint32_t index, fc4_object_placement* out) {
    if (out == nullptr) return;
    *out = fc4_object_placement{};
    cISC4City* c = City();
    cISC4LotDeveloper* ld = c ? c->GetLotDeveloper() : nullptr;
    cISC4LotConfiguration* cfg = CfgOfLot(lot);
    cSC4LotConfigurationObject* obj = ObjectAt(cfg, index);
    if (ld == nullptr || obj == nullptr) return;
    float pos[3] = {0.0f, 0.0f, 0.0f};
    int32_t orient = 0;
    fc4::AsGame(ld)->GetObjectPosition(SdkLot(lot), cfg, obj, pos, orient);  // lot developer slot 39
    out->x = pos[0];
    out->y = pos[1];
    out->z = pos[2];
    out->orientation = static_cast<uint32_t>(orient);
}

void ObjectIdArray(void*, uint64_t lot, uint32_t index, fc4_u32s_sink sink, void* ctx) {
    if (sink == nullptr) return;
    cISC4City* c = City();
    cISC4LotConfigurationManager* m = c ? c->GetLotConfigurationManager() : nullptr;
    cSC4LotConfigurationObject* obj = ObjectAt(CfgOfLot(lot), index);
    if (m == nullptr || obj == nullptr || !fc4::TractNodeAllocator::Available()) return;
    fc4::GameVector<uint32_t, fc4::TractNodeAllocator> ids;
    fc4::AsGame(m)->GetObjectIDArray(obj, &ids);  // slot 27
    std::vector<uint32_t> copy(ids.begin(), ids.end());
    if (!copy.empty()) sink(ctx, copy.data(), copy.size());
}

uint64_t AddLotProp(void* u, uint32_t id, const fc4_object_placement* p) {
    ++Ctx(u)->creates;
    if (!Writes(u, "AddLotProp") || p == nullptr) return 0;
    cISC4City* c = City();
    cISC4PropManager* pm = c ? c->GetPropManager() : nullptr;
    if (pm == nullptr) return 0;
    const cS3DVector3 pos(p->x, p->y, p->z);
    return TokenOf(pm->AddLotProp(id, pos, static_cast<int32_t>(p->orientation)));  // slot 26, borrowed
}

uint64_t AddFlora(void* u, uint32_t id, const fc4_object_placement* p) {
    ++Ctx(u)->creates;
    if (!Writes(u, "AddNewFloraOccupant") || p == nullptr) return 0;
    cISC4City* c = City();
    cISC4FloraSimulator* f = c ? c->GetFloraSimulator() : nullptr;
    if (f == nullptr) return 0;
    float pos[3] = {p->x, p->y, p->z};
    return TokenOf(f->AddNewFloraOccupant(id, pos, false));  // flora slot 17, borrowed
}

void SetLotTag(void* u, uint64_t occ, uint32_t tag) {
    ++Ctx(u)->tags;
    if (!Writes(u, "SetLotTag") || occ == 0) return;
    OccOf(occ)->SetLotTag(tag);  // slot 28
}

struct NetLots {
    std::vector<int32_t> x, z;
    std::vector<uint64_t> lots;
};

bool CollectNetworkLot(uint32_t x, uint32_t z, cISC4NetworkLot* lot, void* ctx) {
    auto* n = static_cast<NetLots*>(ctx);
    n->x.push_back(static_cast<int32_t>(x));
    n->z.push_back(static_cast<int32_t>(z));
    n->lots.push_back(TokenOf(lot));
    // true: go on with the next network lot. city-detail.md §3.4 redevelops
    // every network lot, and IterateAllNetworkLots stops at the first false
    // (network-manager.md, slot 11). (An earlier comment named a question CD6
    // that was never filed; the two specs settle it.)
    return true;
}

void NetworkLots(void* u, fc4_network_lot_sink sink, void* ctx) {
    if (sink == nullptr) return;
    cISC4City* c = City();
    cISC4NetworkLotManager* nlm = c ? c->GetNetworkLotManager() : nullptr;  // city slot 42
    if (nlm == nullptr) return;
    NetLots n;
    nlm->IterateAllNetworkLots(&CollectNetworkLot, &n);  // slot 11
    for (size_t i = 0; i < n.lots.size(); ++i) {
        fc4::propcity::Hold(FromToken(n.lots[i]));  // kept until the end of the pass
        Ctx(u)->pool.Adopt(FromToken(n.lots[i]));
    }
    for (size_t i = 0; i < n.lots.size(); ++i) sink(ctx, n.x[i], n.z[i], n.lots[i]);
}

void RedevelopNetworkLot(void* u, int32_t x, int32_t z, uint64_t lot) {
    ++Ctx(u)->redevelops;
    if (!Writes(u, "RedevelopLot")) return;
    cISC4City* c = City();
    cISC4NetworkLotManager* nlm = c ? c->GetNetworkLotManager() : nullptr;
    if (nlm != nullptr) {
        nlm->RedevelopLot(static_cast<uint32_t>(x), static_cast<uint32_t>(z),
                          reinterpret_cast<cISC4NetworkLot*>(FromToken(lot)));  // slot 12
    }
}

}  // namespace

uint32_t fc4::CityDetailSetting() {
    cIGZFrameWork* fw = RZGetFrameWork();
    cIGZApp* app = fw ? fw->Application() : nullptr;
    cISC4App* sc4 = nullptr;
    if (app == nullptr || !app->QueryInterface(GZIID_cISC4App, reinterpret_cast<void**>(&sc4)) || sc4 == nullptr) {
        return 3;
    }
    FC4_ACQUIRED(sc4, "QueryInterface");
    SC4Preferences* p = sc4->GetPreferences();  // slot 38, borrowed
    const uint32_t s = p ? p->videoPreferences.cityDetail : 3u;  // +0xF0C + 0xA = +0xF16
    FC4_RELEASE(sc4);
    return s;
}

fc4_city_detail_host fc4::MakeCityDetailHost(CityDetailContext* ctx) {
    fc4_city_detail_host h{};
    h.struct_size = sizeof(h);
    h.user = ctx;
    h.detail_setting = &DetailSetting;
    h.lots = &Lots;
    h.lot_config_objects = &LotConfigObjects;
    h.lot_occupants = &LotOccupants;
    h.occupants_in_lot_rect = &OccupantsInLotRect;
    h.occupant_type = &OccupantType;
    h.occupant_lot_tag = &OccupantLotTag;
    h.demolish_flora = &DemolishFlora;
    h.remove_prop = &RemoveProp;
    h.object_placement = &ObjectPlacement;
    h.object_id_array = &ObjectIdArray;
    h.add_lot_prop = &AddLotProp;
    h.add_flora = &AddFlora;
    h.set_lot_tag = &SetLotTag;
    h.network_lots = &NetworkLots;
    h.redevelop_network_lot = &RedevelopNetworkLot;
    return h;
}

// ---- the manager ---------------------------------------------------------------------------------

cIGZUnknown* CreateFC4CityDetailManager() {
    return static_cast<cIGZUnknown*>(static_cast<cFC4ISC4CityDetailManager*>(new cFC4CityDetailManager()));
}

cFC4CityDetailManager::cFC4CityDetailManager() {
    host_ = fc4::MakeCityDetailHost(&ctx_);
    bool fixed = false;
    const char* source = nullptr;
    seed_ = fc4::SubsystemSeed("citydetail_seed", "FC4_CITYDETAIL_SEED", &fixed, &source);
    core_ = fc4_city_detail_create(&host_, seed_, TokenOf(static_cast<cFC4ISC4CityDetailManager*>(this)));
    if (core_ == nullptr) fc4::Log("cFC4CityDetailManager: core creation failed");
    fc4::Log("cFC4CityDetailManager(%p) created, core %p, session seed %u (%s)", static_cast<void*>(this),
             static_cast<void*>(core_), seed_, source ? source : (fixed ? "citydetail_seed" : "session seed"));
}

cFC4CityDetailManager::~cFC4CityDetailManager() {
    fc4_city_detail_destroy(core_);
    core_ = nullptr;
    ctx_.pool.Drain();
}

// §1: 1 and 0x6AD4B9F1 give the interface (+0), 0xE4FDA3D4 the
// serializable (+4); anything else is false.
bool cFC4CityDetailManager::QueryInterface(uint32_t riid, void** ppvObj) {
    if (ppvObj == nullptr) return false;
    switch (riid) {
        case GZIID_cIGZUnknown:
        case kFC4_IID_cISC4CityDetailManager:
            *ppvObj = static_cast<cFC4ISC4CityDetailManager*>(this);
            ++qiOwn_;
            break;
        case kIID_cIGZSerializable:
            *ppvObj = static_cast<cIGZSerializable*>(this);
            ++qiSer_;
            break;
        default:
            ++qiOther_;
            return false;
    }
    AddRef();
    return true;
}

uint32_t cFC4CityDetailManager::AddRef() { return core_ ? fc4_city_detail_add_ref(core_) : 1; }

// §1: the last Release runs the deleting destructor.
uint32_t cFC4CityDetailManager::Release() {
    if (destroying_) return 0;
    const uint32_t left = core_ ? fc4_city_detail_release(core_) : 0;
    if (left != 0) return left;
    destroying_ = true;
    delete this;
    return 0;
}

bool cFC4CityDetailManager::Init() {
    FC4_TRACE("CityDetailManager::Init");
    ++inits_;
    if (core_ == nullptr) return true;
    const uint32_t before = fc4_city_detail_get_level(core_);
    ctx_.removes = ctx_.creates = ctx_.tags = ctx_.redevelops = 0;
    fc4_city_detail_init(core_);
    ctx_.pool.Drain();
    fc4_city_detail_state st{};
    st.struct_size = sizeof(st);
    fc4_city_detail_get_state(core_, &st);
    static const char* const kPass[] = {"none", "reduce", "increase"};
    fc4::Log("citydetail Init: setting=%u level %d->%d pass=%s removed=%u created=%u draws=%u network-lots=%u "
             "(lot order: %s)",
             ctx_.settingRead, static_cast<int>(before), static_cast<int>(st.level),
             st.last_pass < 3 ? kPass[st.last_pass] : "?", st.last_removed, st.last_created, st.last_draws,
             st.last_network_lots, ctx_.lotsOrdered ? "the lot list" : "GetLotsInCellRect, not the spec's list order");
    return true;  // §3.1
}

bool cFC4CityDetailManager::Shutdown() {
    FC4_TRACE("CityDetailManager::Shutdown");
    const bool ok = core_ == nullptr || fc4_city_detail_shutdown(core_);  // §3.1: true, nothing else
    // The counters exclude this line's own GetCityDetail read (the core's getter).
    fc4::Log("citydetail Shutdown -> %d (level %d, inits=%u, QueryInterface own=%u serializable=%u refused=%u, "
             "GetCityDetail=%u)",
             ok ? 1 : 0, core_ ? static_cast<int>(fc4_city_detail_get_level(core_)) : -1, inits_.load(),
             qiOwn_.load(), qiSer_.load(), qiOther_.load(), getDetail_.load());
    return ok;
}

uint32_t cFC4CityDetailManager::GetCityDetail() {
    ++getDetail_;
    return core_ ? fc4_city_detail_get_level(core_) : 0xFFFFFFFFu;
}

bool cFC4CityDetailManager::Write(cIGZOStream& s) {
    FC4_TRACE("CityDetailManager::Write");
    fc4::OStreamAdapter a(s);
    const bool ok = core_ != nullptr && fc4_city_detail_write(core_, a.Get());
    fc4::Log("citydetail Write -> %d (level %d)", ok ? 1 : 0, static_cast<int>(core_ ? fc4_city_detail_get_level(core_) : 0xFFFFFFFFu));
    return ok;
}

bool cFC4CityDetailManager::Read(cIGZIStream& s) {
    FC4_TRACE("CityDetailManager::Read");
    fc4::IStreamAdapter a(s);
    const bool ok = core_ != nullptr && fc4_city_detail_read(core_, a.Get());
    fc4::Log("citydetail Read -> %d (level %d)", ok ? 1 : 0, static_cast<int>(core_ ? fc4_city_detail_get_level(core_) : 0xFFFFFFFFu));
    return ok;
}

uint32_t cFC4CityDetailManager::GetGZCLSID() { return kFC4_CLSID_cSC4CityDetailManager; }
