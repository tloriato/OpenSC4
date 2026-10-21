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

#include "FC4NetMgrShadow.h"

#include "FC4Checked.h"
#include "FC4Config.h"
#include "FC4CoreProps.h"
#include "FC4Diag.h"
#include "FC4Fpu.h"
#include "FC4Interfaces.h"
#include "FC4Log.h"
#include "FC4NetMgrHost.h"
#include "FC4Properties.h"
#include "FC4RefAudit.h"
#include "FC4Seed.h"
#include "FC4Streams.h"
#include "FC4WriteCapture.h"

#include "GZServPtrs.h"
#include "cGZPersistResourceKey.h"
#include "cIGZApp.h"
#include "cIGZFrameWork.h"
#include "cIGZMessage2.h"
#include "cIGZMessage2Standard.h"
#include "cIGZMessageServer2.h"
#include "cIGZMessageTarget2.h"
#include "cIGZPersistResourceKeyFilter.h"
#include "cIGZPersistResourceKeyList.h"
#include "cIGZPersistResourceManager.h"
#include "cISC4App.h"
#include "cISC4City.h"
#include "cISC4NetworkLot.h"
#include "cISC4NetworkLotManager.h"
#include "cISC4NetworkManager.h"
#include "cISC4NetworkOccupant.h"
#include "cISC4Occupant.h"
#include "cISCPropertyHolder.h"
#include "cISCResExemplar.h"
#include "cISC4Simulator.h"
#include "cRZCOMDllDirector.h"

#include <algorithm>
#include <cstdarg>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <map>
#include <set>
#include <string>
#include <vector>

namespace fc4 {
namespace {

// §3.1 step 3 and §3.2 step 2: the managers' subscriptions.
constexpr uint32_t kMgrMessages[] = {0xA6B79602, 0x278E6003, 0xEA3B0AF1, 0x6A3B0AFD, 0x6AB97795, 0x26D31EC2,
                                     0x26D31EC1, 0x99EF1142, 0x99EF1143, 0xC9DA96E8, 0xC9DA96E9};
constexpr uint32_t kLotMessages[] = {0x89D3E9F3, 0x99EF1143, 0xA9B6C2D6, 0x07699455, 0xE8BDA2E7, 0x69247DC8};
constexpr uint32_t kMsgOccupantInserted = 0x99EF1142;
constexpr uint32_t kMsgOccupantRemoved = 0x99EF1143;
constexpr uint32_t kMsgCreateLotObjects = 0xA9B6C2D6;
constexpr uint32_t kMsgUsableChanged = 0x89D3E9F3;
constexpr uint32_t kMsgZoningChanged = 0x07699455;
constexpr uint32_t kMsgLotStateChanged = 0xE8BDA2E7;
constexpr uint32_t kMsgViewChanged = 0xA6B79602;
constexpr uint32_t kIID_cISC4NetworkOccupant = 0xA821EF94;
constexpr uint32_t kIID_cIGZMessageTarget2 = 0x452294AA;
// §7.6: the terrain rule queries every network type.
constexpr uint32_t kAllTypes = 0x1FFF;
// §8.7 step 6.
constexpr uint32_t kUnderTextureFlags = 0x6000000;
// §8.8 step 1: the pothole effect prop's class.
constexpr uint32_t kClsidEffectProp = 0x2977AA49;
// [choice] The shadow's stand-in tools answer this class (never saved).
constexpr uint32_t kClsidStandInTool = 0x4E455430;
// §2.4: lot configuration exemplars.
constexpr uint32_t kExemplarType = 0x6534284A;
constexpr uint32_t kPropExemplarType = 0x10;
constexpr uint32_t kNetworkLotExemplarType = 0x21;
// Differences listed per line.
constexpr size_t kMaxListed = 12;

bool g_enabled = false;
bool g_settingsRead = false;
bool g_listening = false;
// netmgr_shadow_order: where the lot core hears its 6 messages. late (the
// default): a second target subscribed at PostCityInit, after the original
// network lot manager (which subscribes at its Init, §3.2), so the core reads
// the game as the original did, after every listener before it; early: the
// PostAppInit listener, before every city listener.
bool g_late = true;
bool g_lateListening = false;
bool g_loggedOurs = false;

// The cores and their host (main thread).
NetMgrContext g_ctx;
fc4_netmgr_host g_mhost{};
fc4_netlot_host g_lhost{};
fc4_netmgr* g_mgr = nullptr;
fc4_netlot* g_lot = nullptr;
cISC4City* g_city = nullptr;
bool g_inMessage = false;

// The network lot configurations of the exemplars (§3.2 step 4), read once
// per process (the exemplars do not change), in the key list's order.
bool g_configsRead = false;
std::vector<fc4_props*> g_configs;
uint32_t g_exemplarKeys = 0;

// Counters since the last line.
struct Counters {
    uint32_t messages = 0, mgrMessages = 0, lotMessages = 0;
    uint32_t terrainMessages = 0, toolCalls = 0, decided = 0, confirmed = 0, keptByOrig = 0, origOnly = 0,
             rebuilt = 0, candidatesChecked = 0, refits = 0;
    uint32_t styleWrites = 0, styleChecked = 0, styleGone = 0, styleMismatches = 0;
    uint32_t orderBefore = 0, orderProbes = 0;
    uint32_t potholesCreated = 0;
};
Counters g_n;
uint32_t g_lines = 0, g_exactLines = 0, g_draws = 0, g_resyncs = 0;
// NM10 (open question, network-manager.md §6.2 0xE8BDA2E7): in game the
// original never changes the road style of the first row's inner cells of a
// lot state message's grown rect (x0 < x < x1, z = z0), where the spec walks
// the whole border (and, rarely, restyles such a cell later, during another
// message, as if its key had stayed pending). The cells our lot core updated
// there since the city loaded; a style or lot difference at one of them is
// counted under NM10, not as a mismatch, until the question is answered.
std::set<std::pair<int32_t, int32_t>> g_nm10Cells;
uint32_t g_nm10Lines = 0, g_nm10Total = 0;
uint32_t g_nm10Pending = 0;  // tick-time NM10 differences since the last line
std::string g_nm10PendingDiffs;
std::string g_pendingDiffs;  // tick-time differences, reported on the next line
uint32_t g_pendingMismatches = 0;
size_t g_pendingListed = 0;

// Recorded writes, checked at the next tick (each occupant held, NetHold).
struct StyleWrite {
    int32_t variation = -1;
    bool flags = false;
    bool texture = false;
};
std::map<uint64_t, StyleWrite> g_style;
// Per message (order late only): the variations of the message's cells
// before the original handled it (taken by the early target), and the
// variations the lot core wrote in it; compared when the late target has run
// the core, so the original's and our road style writes are compared message
// by message (diagnostics: `per message`).
struct SnapCell {
    int32_t x = 0, z = 0;
    uint32_t before = 0;
    int32_t zone = -1, wealth = -1;  // the original's slots 9, 10 before the original handled it
};
std::map<uint64_t, SnapCell> g_snap;
bool g_snapValid = false;
uint32_t g_snapType = 0;
fc4_cell_rect g_snapRect{};  // the message's cells (grown rect)
std::string g_geom;          // the first message per line where ours changed a cell the original kept
size_t g_geomListed = 0;
// 0xE8BDA2E7 border cells by place in the grown rect (first row: its first
// cell, the inner cells, its last cell; the middle rows at x0, at x1; the
// last row): cells, the original changed, ours changed where the original
// kept. Whole city session (reported at city shutdown).
constexpr int kPlaces = 7;
const char* const kPlaceNames[kPlaces] = {"first row x0", "first row inner", "first row x1", "middle x0", "middle x1",
                                          "last row", "not on the border"};
uint32_t g_place[kPlaces][3] = {};
// netmgr_shadow_wide = N: the first N lot state messages are snapshotted over
// the whole city (not only the grown rect), to see what the original changes
// off the border; the first changes there are listed at city shutdown.
int32_t g_wideLeft = 0;
std::string g_wideNotes;
size_t g_wideListed = 0;
bool g_snapWide = false;

int PlaceOf(int32_t x, int32_t z, const fc4_cell_rect& r) {
    if (x < r.x1 || x > r.x2 || z < r.z1 || z > r.z2) return 6;
    if (z == r.z1) return x == r.x1 ? 0 : (x == r.x2 ? 2 : 1);
    if (z == r.z2) return 5;
    if (x == r.x1) return 3;
    if (x == r.x2) return 4;
    return 6;
}
std::map<uint64_t, uint32_t> g_msgWrites;
struct PerMessage {
    uint32_t messages = 0, agree = 0, oursOnly = 0, origOnly = 0, differ = 0;
};
// Per lot message type (usable, zoning, lot state): messages compared, cells
// looked at, cells whose variation the original changed, cells ours changed.
struct PerType {
    uint32_t messages = 0, cells = 0, origChanged = 0, oursChanged = 0;
};
std::map<uint32_t, PerType> g_pt;
PerMessage g_pm;
std::string g_pmFirst;
size_t g_pmListed = 0;
// The terrain rule: the candidates of each 0x278E6003 query (held) and the
// occupants the core destroyed, with their cells.
std::set<uint64_t> g_candidates;
std::map<uint64_t, std::pair<int32_t, int32_t>> g_decided;
std::vector<std::pair<int32_t, int32_t>> g_decidedCells;

// The captures (reused).
WriteCapture g_origMgr, g_origLot, g_ourMgr, g_ourLot;

cISC4City* CurrentCity() {
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

void ReadSettings() {
    if (g_settingsRead) return;
    g_settingsRead = true;
    g_enabled = SettingInt("netmgr_shadow", "FC4_NETMGR_SHADOW", 0) != 0;
    g_wideLeft = static_cast<int32_t>(SettingInt("netmgr_shadow_wide", "FC4_NETMGR_SHADOW_WIDE", 0));
    std::string order = Setting("netmgr_shadow_order", "FC4_NETMGR_SHADOW_ORDER");
    for (char& c : order) c = static_cast<char>(c >= 'A' && c <= 'Z' ? c - 'A' + 'a' : c);
    g_late = order != "early";
}

cISC4NetworkOccupant* NetOf(uint64_t t) { return reinterpret_cast<cISC4NetworkOccupant*>(FromToken(t)); }

// In the city: cISC4Occupant GetFlags() & 1 (§5, §7.4).
bool Placed(uint64_t n) {
    cISC4Occupant* o = n ? NetOf(n)->AsOccupant() : nullptr;  // slot 48
    return o != nullptr && (o->GetFlags() & 1) != 0;
}

void AppendF(std::string& s, const char* fmt, ...) {
    char buf[512];
    va_list ap;
    va_start(ap, fmt);
    _vsnprintf_s(buf, sizeof(buf), _TRUNCATE, fmt, ap);
    va_end(ap);
    s += buf;
}

struct Diff {
    uint32_t mismatches = 0;
    std::string listed;
    size_t n = 0;
    void Add(const char* fmt, ...) {
        ++mismatches;
        if (n++ >= kMaxListed) return;
        char b[200];
        va_list ap;
        va_start(ap, fmt);
        _vsnprintf_s(b, sizeof(b), _TRUNCATE, fmt, ap);
        va_end(ap);
        if (!listed.empty()) listed += ", ";
        listed += b;
    }
};

bool Nm10Cell(int32_t x, int32_t z) { return g_nm10Cells.count(std::make_pair(x, z)) != 0; }

// A tick-time difference at an NM10 cell (counted, listed, not a mismatch).
void PendingNm10(const char* fmt, ...) {
    if (g_nm10Pending++ >= kMaxListed) return;
    char b[200];
    va_list ap;
    va_start(ap, fmt);
    _vsnprintf_s(b, sizeof(b), _TRUNCATE, fmt, ap);
    va_end(ap);
    if (!g_nm10PendingDiffs.empty()) g_nm10PendingDiffs += ", ";
    g_nm10PendingDiffs += b;
}

void PendingDiff(const char* fmt, ...) {
    ++g_pendingMismatches;
    if (g_pendingListed++ >= kMaxListed) return;
    char b[200];
    va_list ap;
    va_start(ap, fmt);
    _vsnprintf_s(b, sizeof(b), _TRUNCATE, fmt, ap);
    va_end(ap);
    if (!g_pendingDiffs.empty()) g_pendingDiffs += ", ";
    g_pendingDiffs += b;
}

std::string DateText(int32_t julian) {
    if (julian <= 0) return "0000-00-00";
    fc4_date d{};
    fc4_date_from_julian(julian, &d);
    char b[24];
    _snprintf_s(b, sizeof(b), _TRUNCATE, "%04d-%02d-%02d", d.year, d.month, d.day_of_month);
    return b;
}

// ---- the configurations (§3.2 step 4) -------------------------------------------------------------------

class ExemplarKeyFilter final : public cIGZPersistResourceKeyFilter {
public:
    bool QueryInterface(uint32_t riid, void** ppvObj) override {
        if (ppvObj == nullptr) return false;
        if (riid == GZIID_cIGZPersistResourceKeyFilter || riid == GZIID_cIGZUnknown) {
            *ppvObj = this;
            AddRef();
            return true;
        }
        *ppvObj = nullptr;
        return false;
    }
    uint32_t AddRef() override { return ++refs_; }
    uint32_t Release() override { return refs_ > 0 ? --refs_ : 0; }
    bool IsKeyIncluded(cGZPersistResourceKey const& key) override { return key.type == kExemplarType; }

private:
    uint32_t refs_ = 1;
};

// Every exemplar of exemplar type 0x21, in the key list's order (the
// resource manager's slot 17 with a type filter: the list of slot 18, as
// cFC4Flora.cpp FloraForEachExemplar), its effective properties.
void ReadConfigurations() {
    if (g_configsRead) return;
    g_configsRead = true;
    cIGZPersistResourceManagerPtr rm;
    if (!rm) {
        Log("netmgr shadow: no resource manager; no network lot configurations");
        return;
    }
    ExemplarKeyFilter filter;
    FC4_OUT(cIGZPersistResourceKeyList*, list, "cIGZPersistResourceKeyList* (GetAvailableResourceList out pointer)");
    rm->GetAvailableResourceList(list.ptr(), &filter);
    list.Check();
    cIGZPersistResourceKeyList* l = list.get();
    if (l == nullptr) {
        Log("netmgr shadow: WARNING the resource manager returned no key list; no network lot configurations");
        return;
    }
    FC4_ACQUIRED(l, "GetAvailableResourceList");
    std::vector<cGZPersistResourceKey> keys;
    const uint32_t n = l->Size();
    keys.reserve(n);
    for (uint32_t i = 0; i < n; ++i) keys.push_back(l->GetKey(i));
    FC4_RELEASE_CREATED(l);
    g_exemplarKeys = n;
    for (const cGZPersistResourceKey& k : keys) {
        if (k.type != kExemplarType) continue;
        FC4_OUT(cISCResExemplar*, out, "cISCResExemplar* (GetResource out pointer)");
        const bool ok = rm->GetResource(k, GZIID_cISCResExemplar, reinterpret_cast<void**>(out.ptr()), 0, nullptr);
        out.Check();
        cISCResExemplar* ex = out.get();
        if (!ok || ex == nullptr) continue;
        FC4_ACQUIRED(ex, "GetResource");
        const cISCPropertyHolder* h = ex->AsISCPropertyHolder();
        std::vector<uint32_t> t;
        if (h != nullptr && props::GetU32s(h, kPropExemplarType, t) && !t.empty() && t[0] == kNetworkLotExemplarType) {
            if (fc4_props* p = props::ToCoreAll(h, ex)) g_configs.push_back(p);
        }
        FC4_RELEASE(ex);
    }
    Log("netmgr shadow: %u exemplar keys enumerated, %u network lot configuration exemplars (exemplar type 0x21)", n,
        static_cast<unsigned>(g_configs.size()));
}

// ---- the shadow host's recording callbacks ----------------------------------------------------------------

uint64_t CreateTool(void*, uint32_t) { return NewNetStandIn(kClsidStandInTool); }

void ToolDestroyOccupants(void* u, uint64_t, const uint64_t* list, size_t count) {
    ++g_n.toolCalls;
    for (size_t i = 0; i < count; ++i) {
        const uint64_t n = list[i];
        if (n == 0 || g_decided.count(n) != 0) continue;
        int32_t x = 0, z = 0;
        if (n != 0) {
            uint32_t cx = 0, cz = 0;
            NetOf(n)->GetOccupiedCell(cx, cz);  // slot 59
            x = static_cast<int32_t>(cx);
            z = static_cast<int32_t>(cz);
        }
        // The core keeps its reference (§7.2 step 2.3, the quirk): the
        // object stays alive for the check.
        g_decided.emplace(n, std::make_pair(x, z));
        g_decidedCells.emplace_back(x, z);
        ++g_n.decided;
    }
    (void)u;
}

void ToolUpdateUnderground(void*, uint64_t, const int32_t*, const uint64_t*, size_t, uint32_t) { ++g_n.refits; }

fc4_netmgr_host g_readMgr{};

// §7.6: the candidates of the terrain query, held until the check.
void OccupantsInRect(void* u, const fc4_cell_rect* r, uint32_t mask, fc4_token_callback cb, void* ctx) {
    struct Tap {
        fc4_token_callback cb;
        void* ctx;
        bool record;
    } tap{cb, ctx, mask == kAllTypes};
    g_readMgr.occupants_in_rect(
        u, r, mask,
        [](void* c, uint64_t n) {
            Tap* t = static_cast<Tap*>(c);
            if (t->record && g_candidates.insert(n).second) NetHold(n);
            if (t->cb != nullptr) t->cb(t->ctx, n);
        },
        &tap);
}

StyleWrite& StyleOf(uint64_t n) {
    auto it = g_style.find(n);
    if (it == g_style.end()) {
        NetHold(n);
        it = g_style.emplace(n, StyleWrite{}).first;
    }
    return it->second;
}

void SetNetworkFlag(void*, uint64_t n, uint32_t bits) {
    if (n == 0) return;
    ++g_n.styleWrites;
    if ((bits & kUnderTextureFlags) == kUnderTextureFlags) StyleOf(n).flags = true;
}

void SetUnderTexture(void*, uint64_t n, uint32_t) {
    if (n == 0) return;
    ++g_n.styleWrites;
    StyleOf(n).texture = true;  // no getter: counted, not compared
}

void SetVariation(void*, uint64_t n, uint32_t v) {
    if (n == 0) return;
    ++g_n.styleWrites;
    StyleOf(n).variation = static_cast<int32_t>(v & 0xFF);
    g_msgWrites[n] = v & 0xFF;
}

uint64_t CreateEffectProp(void*) {
    ++g_n.potholesCreated;
    return NewNetStandIn(kClsidEffectProp);
}

// ---- the checks of the recorded writes (the next tick) --------------------------------------------------

// §8.7 table V, V[zone][max(wealth - 1, 0)] (diagnostics only); -1 for a
// zone or wealth out of range.
int32_t Variation(int32_t zone, int32_t wealth) {
    if (zone < 0 || zone > 15 || wealth < 0 || wealth > 3) return -1;
    const int32_t w = wealth > 0 ? wealth - 1 : 0;
    switch (zone) {
        case 0: return 0;
        case 1:
        case 4: return 1 + w;
        case 2:
        case 3:
        case 5:
        case 6: return 4 + w;
        case 7:
        case 14: return 7;
        case 8:
        case 9: return w < 2 ? 1 : 2;
        default: return 1;
    }
}

bool NearDecided(int32_t x, int32_t z) {
    for (const auto& c : g_decidedCells) {
        if (std::abs(c.first - x) <= 1 && std::abs(c.second - z) <= 1) return true;
    }
    return false;
}

void SettleStep(void*) {
    // The terrain rule (§7.6).
    for (const auto& d : g_decided) {
        if (Placed(d.first)) {
            ++g_n.keptByOrig;
            PendingDiff("terrain: ours destroyed %p at %d,%d, the original kept it", static_cast<void*>(FromToken(d.first)),
                        d.second.first, d.second.second);
        } else {
            ++g_n.confirmed;
        }
    }
    for (uint64_t c : g_candidates) {
        ++g_n.candidatesChecked;
        if (g_decided.count(c) != 0) {
            if (!Placed(c)) continue;  // leaked with the core's reference: no release here
            NetDrop(c);
            continue;
        }
        if (Placed(c)) {
            NetDrop(c);
            continue;
        }
        // Gone though ours kept it: a neighbour the tool rebuilt, or a
        // difference. Our reference is kept (it may be the last one).
        uint32_t cx = 0, cz = 0;
        NetOf(c)->GetOccupiedCell(cx, cz);
        if (NearDecided(static_cast<int32_t>(cx), static_cast<int32_t>(cz))) {
            ++g_n.rebuilt;
        } else {
            ++g_n.origOnly;
            PendingDiff("terrain: the original destroyed %p at %u,%u, ours kept it", static_cast<void*>(FromToken(c)), cx,
                        cz);
        }
    }
    g_candidates.clear();
    g_decided.clear();
    g_decidedCells.clear();
    // Road style (§8.7): the original made the same writes after ours.
    for (const auto& w : g_style) {
        const uint64_t n = w.first;
        if (!Placed(n)) {
            ++g_n.styleGone;  // removed since: not compared, reference kept
            continue;
        }
        ++g_n.styleChecked;
        cISC4NetworkOccupant* o = NetOf(n);
        bool bad = false;
        uint32_t got = 0;
        if (w.second.variation >= 0) {
            got = o->GetVariation();  // slot 28
            if (static_cast<int32_t>(got) != w.second.variation) bad = true;
        }
        const bool flags = o->HasNetworkFlag(kUnderTextureFlags);  // slot 8
        if (w.second.flags && !flags) bad = true;
        uint32_t cx = 0, cz = 0;
        if (bad) o->GetOccupiedCell(cx, cz);
        if (bad && Nm10Cell(static_cast<int32_t>(cx), static_cast<int32_t>(cz))) {
            PendingNm10("style at %u,%u: variation %u/%d (orig/ours)", cx, cz, got, w.second.variation);
        } else if (bad) {
            ++g_n.styleMismatches;
            // Now (after both): the original's and our zone / wealth for the
            // cell (slots 9, 10) and the variation they give (§8.7 step 7).
            int32_t zo = -1, wo = -1, zu = -1, wu = -1;
            if (g_city != nullptr) {
                if (cISC4NetworkLotManager* nlm = g_city->GetNetworkLotManager()) {
                    zo = static_cast<int32_t>(nlm->GetZoneTypeForNetwork(cx, cz));  // slot 9
                    wo = static_cast<int32_t>(nlm->GetWealthForNetwork(cx, cz));    // slot 10
                }
            }
            if (g_lot != nullptr) {
                zu = fc4_netlot_zone_type_for_network(g_lot, static_cast<int32_t>(cx), static_cast<int32_t>(cz));
                wu = fc4_netlot_wealth_for_network(g_lot, static_cast<int32_t>(cx), static_cast<int32_t>(cz));
                NetMgrReleaseHeld(&g_ctx);
            }
            PendingDiff("style at %u,%u%s: variation %u/%d flags 0x6000000 %d/%d (orig/ours); now zone/wealth %d/%d "
                        "(orig) %d/%d (ours) -> variation %d",
                        cx, cz, o->HasNetworkFlag(0x40) ? " (avenue)" : "", got, w.second.variation, flags ? 1 : 0,
                        w.second.flags ? 1 : 0, zo, wo, zu, wu, Variation(zo, wo));
        }
        NetDrop(n);
    }
    g_style.clear();
}

void Settle() {
    if (g_candidates.empty() && g_decided.empty() && g_style.empty()) return;
    if (!SehGuard("netmgr shadow (checking the recorded writes)", &SettleStep, nullptr)) {
        Log("netmgr shadow: WARNING structured exception while checking the recorded writes; they are dropped");
        g_candidates.clear();
        g_decided.clear();
        g_decidedCells.clear();
        g_style.clear();
    }
}

// ---- the cores ------------------------------------------------------------------------------------------

void DropCores() {
    if (g_mgr != nullptr) {
        fc4_netmgr_shutdown(g_mgr);
        fc4_netmgr_destroy(g_mgr);
        g_mgr = nullptr;
    }
    if (g_lot != nullptr) {
        fc4_netlot_shutdown(g_lot);
        fc4_netlot_destroy(g_lot);
        g_lot = nullptr;
    }
    NetMgrReleaseHeld(&g_ctx);
}

void MakeHosts(cISC4City* city) {
    g_ctx.city = city;
    g_readMgr = MakeNetMgrReadOnlyHost(&g_ctx);
    g_mhost = g_readMgr;
    g_mhost.create_tool = &CreateTool;
    g_mhost.tool_destroy_occupants = &ToolDestroyOccupants;
    g_mhost.tool_update_underground_heights = &ToolUpdateUnderground;
    g_mhost.occupants_in_rect = &OccupantsInRect;
    g_lhost = MakeNetLotReadOnlyHost(&g_ctx);
    g_lhost.set_network_flag = &SetNetworkFlag;
    g_lhost.set_under_texture = &SetUnderTexture;
    g_lhost.set_variation = &SetVariation;
    g_lhost.create_effect_prop = &CreateEffectProp;
}

// A fresh network manager core on the original's record (Read, then Init).
bool StartMgr(cISC4NetworkManager* orig) {
    if (g_mgr != nullptr) {
        fc4_netmgr_shutdown(g_mgr);
        fc4_netmgr_destroy(g_mgr);
        g_mgr = nullptr;
    }
    g_mgr = fc4_netmgr_create(&g_mhost, TokenOf(orig));
    if (g_mgr == nullptr) return false;
    CaptureIStream in(g_origMgr);
    const bool ok = fc4_netmgr_read(g_mgr, in.Get());
    fc4_netmgr_init(g_mgr);
    NetMgrReleaseHeld(&g_ctx);
    if (!ok) {
        fc4_netmgr_shutdown(g_mgr);
        fc4_netmgr_destroy(g_mgr);
        g_mgr = nullptr;
    }
    return ok;
}

bool StartLot(cISC4NetworkLotManager* orig) {
    if (g_lot != nullptr) {
        fc4_netlot_shutdown(g_lot);
        fc4_netlot_destroy(g_lot);
        g_lot = nullptr;
    }
    g_lot = fc4_netlot_create(&g_lhost, TokenOf(orig), SessionSeed());
    if (g_lot == nullptr) return false;
    for (fc4_props* p : g_configs) fc4_netlot_add_configuration_props(g_lot, 0, p, nullptr, 0);
    CaptureIStream in(g_origLot);
    const bool ok = fc4_netlot_read(g_lot, in.Get());
    fc4_netlot_init(g_lot);
    NetMgrReleaseHeld(&g_ctx);
    if (!ok) {
        fc4_netlot_shutdown(g_lot);
        fc4_netlot_destroy(g_lot);
        g_lot = nullptr;
    }
    return ok;
}

// ---- the records --------------------------------------------------------------------------------------

struct MgrRecord {
    bool ok = false;
    uint16_t version = 0;
    std::vector<const void*> managed;  // each entry's object (nullptr: a null reference)
};

MgrRecord ParseMgr(const WriteCapture& cap) {
    MgrRecord r;
    CaptureReader b(cap.Bytes());
    uint32_t n = 0;
    if (!b.U16(r.version) || !b.U32(n)) return r;
    for (uint32_t i = 0; i < n; ++i) {
        uint32_t id = 0;
        if (!b.Ref(id)) return r;
        r.managed.push_back(id ? cap.Object(id) : nullptr);
    }
    uint8_t extra = 0;
    r.ok = !b.U8(extra);
    return r;
}

struct LotRecord {
    bool ok = false;
    uint16_t version = 0;
    std::vector<uint16_t> order;
    std::map<uint16_t, std::pair<const void*, uint32_t>> lots;  // key -> (occupant object, configuration ID)
    std::map<uint16_t, uint32_t> potholes;                      // key -> class ID (0: null)
    uint32_t duplicates = 0;
};

LotRecord ParseLot(const WriteCapture& cap) {
    LotRecord r;
    CaptureReader b(cap.Bytes());
    uint32_t n = 0;
    if (!b.U16(r.version) || !b.U32(n)) return r;
    for (uint32_t i = 0; i < n; ++i) {
        uint16_t key = 0;
        uint32_t id = 0, cfg = 0;
        if (!b.U16(key) || !b.Ref(id) || !b.U32(cfg)) return r;
        r.order.push_back(key);
        if (!r.lots.emplace(key, std::make_pair(id ? cap.Object(id) : nullptr, cfg)).second) ++r.duplicates;
    }
    uint32_t m = 0;
    if (!b.U32(m)) return r;
    for (uint32_t i = 0; i < m; ++i) {
        uint16_t key = 0;
        uint32_t id = 0;
        if (!b.U16(key) || !b.Ref(id)) return r;
        if (!r.potholes.emplace(key, id ? cap.ClassOf(id) : 0).second) ++r.duplicates;
    }
    uint8_t extra = 0;
    r.ok = !b.U8(extra);
    return r;
}

bool WriteOurs(WriteCapture& cap, bool mgr) {
    cap.Capture(nullptr);  // empties it
    OStreamAdapter a(cap);
    const bool ok = mgr ? fc4_netmgr_write(g_mgr, a.Get()) : fc4_netlot_write(g_lot, a.Get());
    NetMgrReleaseHeld(&g_ctx);
    return ok && cap.GetError() == 0;
}

// The occupant behind a record entry as its network occupant (borrowed: the
// original's lot keeps it alive).
uint64_t NetworkOccupantOf(const void* ser) {
    if (ser == nullptr) return 0;
    cIGZUnknown* u = static_cast<cIGZUnknown*>(const_cast<void*>(ser));
    void* p = nullptr;
    if (!u->QueryInterface(kIID_cISC4NetworkOccupant, &p) || p == nullptr) return 0;
    cIGZUnknown* q = static_cast<cIGZUnknown*>(p);
    if (IsOurObject(q)) {
        q->Release();
    } else {
        FC4_ACQUIRED(q, "QueryInterface");
        FC4_RELEASE(q);
    }
    return TokenOf(q);
}

// NM11 diagnostics (verify-regress4): for a lot held by one side only, the
// network occupant's style inputs (variation, rotation, flip, IsUsable) and
// whether the entry's configuration is compatible now under our zone and
// wealth (§8.4, §8.5), so a later run shows which input differs.
struct LotDetail {
    char text[128] = "";
};
LotDetail DetailOf(const void* ser, uint32_t cfg, int32_t x, int32_t z, int32_t zone, int32_t wealth) {
    LotDetail d;
    const uint64_t n = NetworkOccupantOf(ser);
    if (n == 0) {
        std::snprintf(d.text, sizeof(d.text), "no network occupant;");
        return d;
    }
    cISC4NetworkOccupant* net = NetOf(n);
    const uint32_t var = net->GetVariation();
    const int32_t rot = static_cast<int32_t>(net->GetRotation());
    const int32_t flip = static_cast<int32_t>(net->GetFlip());
    const bool usable = net->IsUsable();
    const bool compat = fc4_netlot_is_lot_compatible(g_lot, cfg, x, z, zone, wealth, rot, flip);
    NetMgrReleaseHeld(&g_ctx);
    std::snprintf(d.text, sizeof(d.text), "variation 0x%X rotation %d flip %d usable %d, compatible now %d;", var,
                  rot, flip, usable ? 1 : 0, compat ? 1 : 0);
    return d;
}

// ---- one comparison -------------------------------------------------------------------------------------

struct Run {
    cISC4City* city = nullptr;
    int32_t julian = 0;
    const char* event = "month";
    bool start = false;  // PostCityInit: the cores start from the records first
    // results
    bool done = false;
    uint32_t mismatches = 0;
    std::string line;
};

void RunStep(void* p) {
    Run& r = *static_cast<Run*>(p);
    cISC4NetworkManager* nm = r.city->GetNetworkManager();          // slot 38, borrowed
    cISC4NetworkLotManager* nlm = r.city->GetNetworkLotManager();  // slot 42, borrowed
    if (nm == nullptr || nlm == nullptr) {
        r.line = "the city has no network manager or network lot manager; nothing compared";
        return;
    }
    if (IsOurObject(nm) || IsOurObject(nlm)) {
        if (!g_loggedOurs) {
            g_loggedOurs = true;
            Log("netmgr shadow: the city's network manager or network lot manager is FreeCity4's; the shadow compares "
                "the ORIGINALS: nothing compared");
        }
        DropCores();
        return;
    }
    Diff d;
    Diff nm10;  // differences at NM10 cells
    bool answered = false;
    const bool origMgrOk = g_origMgr.Capture(reinterpret_cast<cIGZUnknown*>(nm), &answered);
    const bool origLotOk = g_origLot.Capture(reinterpret_cast<cIGZUnknown*>(nlm), &answered);
    if (!origMgrOk || !origLotOk) {
        r.line = std::string("INCOMPLETE (the original's ") + (!origMgrOk ? "component 53" : "component 11") +
                 " Write failed); not compared";
        return;
    }
    std::string followed = r.start ? "start" : "followed";
    if (r.start || g_city != r.city) {
        DropCores();
        g_city = r.city;
        MakeHosts(r.city);
        ReadConfigurations();
        const bool m = StartMgr(nm);
        const bool l = StartLot(nlm);
        if (!m) d.Add("our core refused the original's component 53");
        if (!l) d.Add("our core refused the original's component 11");
        followed = "start";
    }
    if (g_mgr == nullptr || g_lot == nullptr) {
        // Restarted after a refusal or a fault.
        if (g_mgr == nullptr && !StartMgr(nm)) d.Add("our core refused the original's component 53");
        if (g_lot == nullptr && !StartLot(nlm)) d.Add("our core refused the original's component 11");
        followed = "restarted";
    }

    // 1. Component 53: byte for byte and object for object.
    const MgrRecord om = ParseMgr(g_origMgr);
    MgrRecord um;
    bool mgrSame = false;
    if (g_mgr != nullptr && WriteOurs(g_ourMgr, true)) {
        um = ParseMgr(g_ourMgr);
        mgrSame = g_origMgr.Bytes() == g_ourMgr.Bytes() && om.managed == um.managed && om.ok && um.ok;
        if (!mgrSame) {
            if (om.managed.size() != um.managed.size()) {
                d.Add("component 53: %u/%u managed (orig/ours)", static_cast<unsigned>(om.managed.size()),
                      static_cast<unsigned>(um.managed.size()));
            }
            const size_t n = std::min(om.managed.size(), um.managed.size());
            for (size_t i = 0; i < n; ++i) {
                if (om.managed[i] != um.managed[i]) {
                    d.Add("component 53 entry %u: %p/%p", static_cast<unsigned>(i), om.managed[i], um.managed[i]);
                    break;
                }
            }
            if (om.managed.size() == um.managed.size() && om.managed == um.managed) {
                d.Add("component 53: bytes %u/%u differ (version %u/%u, ok %d/%d)",
                      static_cast<unsigned>(g_origMgr.Bytes().size()), static_cast<unsigned>(g_ourMgr.Bytes().size()),
                      om.version, um.version, om.ok ? 1 : 0, um.ok ? 1 : 0);
            }
        }
    } else if (g_mgr != nullptr) {
        d.Add("our component 53 Write failed");
    }

    // 2. Component 11: entry by entry (the original writes its hash maps'
    //    order, ours keeps insertion order); the zone and wealth of every
    //    lot cell; a differing configuration: both compatible = a draw.
    const LotRecord ol = ParseLot(g_origLot);
    LotRecord ul;
    uint32_t cells = 0, zoneDiffs = 0, wealthDiffs = 0, cfgSame = 0, draws = 0, cfgBad = 0;
    bool lotBytes = false;
    if (g_lot != nullptr && WriteOurs(g_ourLot, false)) {
        ul = ParseLot(g_ourLot);
        lotBytes = g_origLot.Bytes() == g_ourLot.Bytes();
        if (!ol.ok || !ul.ok || ol.version != ul.version || ol.duplicates != 0 || ul.duplicates != 0) {
            d.Add("component 11: ok %d/%d version %u/%u duplicate keys %u/%u", ol.ok ? 1 : 0, ul.ok ? 1 : 0,
                  ol.version, ul.version, ol.duplicates, ul.duplicates);
        }
        std::set<uint16_t> keys;
        for (const auto& e : ol.lots) keys.insert(e.first);
        for (const auto& e : ul.lots) keys.insert(e.first);
        for (uint16_t k : keys) {
            const int32_t x = k >> 8, z = k & 0xFF;
            const auto o = ol.lots.find(k);
            const auto u = ul.lots.find(k);
            const int32_t zo = static_cast<int32_t>(nlm->GetZoneTypeForNetwork(x, z));    // slot 9
            const int32_t wo = static_cast<int32_t>(nlm->GetWealthForNetwork(x, z));      // slot 10
            const int32_t zu = fc4_netlot_zone_type_for_network(g_lot, x, z);
            const int32_t wu = fc4_netlot_wealth_for_network(g_lot, x, z);
            NetMgrReleaseHeld(&g_ctx);
            ++cells;
            if (zo != zu) {
                ++zoneDiffs;
                d.Add("zone for network at %d,%d: %d/%d", x, z, zo, zu);
            }
            if (wo != wu) {
                ++wealthDiffs;
                d.Add("wealth for network at %d,%d: %d/%d", x, z, wo, wu);
            }
            Diff& dl = Nm10Cell(x, z) ? nm10 : d;
            if (o == ol.lots.end()) {
                const LotDetail ld = DetailOf(u->second.first, u->second.second, x, z, zu, wu);
                dl.Add("lot at %d,%d: ours only (config 0x%08X; %s zone/wealth orig %d/%d ours %d/%d)", x, z,
                       u->second.second, ld.text, zo, wo, zu, wu);
                continue;
            }
            if (u == ul.lots.end()) {
                const LotDetail ld = DetailOf(o->second.first, o->second.second, x, z, zu, wu);
                dl.Add("lot at %d,%d: the original's only (config 0x%08X; %s zone/wealth orig %d/%d ours %d/%d)", x,
                       z, o->second.second, ld.text, zo, wo, zu, wu);
                continue;
            }
            if (o->second.first != u->second.first) {
                d.Add("lot at %d,%d: occupant %p/%p", x, z, o->second.first, u->second.first);
                continue;
            }
            if (o->second.second == u->second.second) {
                ++cfgSame;
                continue;
            }
            const uint64_t n = NetworkOccupantOf(o->second.first);
            const int32_t rot = n ? static_cast<int32_t>(NetOf(n)->GetRotation()) : 0;
            const int32_t flip = n ? static_cast<int32_t>(NetOf(n)->GetFlip()) : 0;
            const bool co = fc4_netlot_is_lot_compatible(g_lot, o->second.second, x, z, zu, wu, rot, flip);
            const bool cu = fc4_netlot_is_lot_compatible(g_lot, u->second.second, x, z, zu, wu, rot, flip);
            NetMgrReleaseHeld(&g_ctx);
            if (co && cu) {
                ++draws;  // two compatible configurations: the clock-seeded draw
            } else {
                ++cfgBad;
                dl.Add("lot at %d,%d: config 0x%08X/0x%08X compatible now %d/%d", x, z, o->second.second,
                       u->second.second, co ? 1 : 0, cu ? 1 : 0);
            }
        }
        std::set<uint16_t> pk;
        for (const auto& e : ol.potholes) pk.insert(e.first);
        for (const auto& e : ul.potholes) pk.insert(e.first);
        for (uint16_t k : pk) {
            const auto o = ol.potholes.find(k);
            const auto u = ul.potholes.find(k);
            if (o == ol.potholes.end() || u == ul.potholes.end()) {
                d.Add("pothole at %d,%d: %s only", k >> 8, k & 0xFF, o == ol.potholes.end() ? "ours" : "the original's");
            } else if (o->second != u->second) {
                d.Add("pothole at %d,%d: class 0x%08X/0x%08X", k >> 8, k & 0xFF, o->second, u->second);
            }
        }
    } else if (g_lot != nullptr) {
        d.Add("our component 11 Write failed");
    }

    // The checks made at the ticks since the last line.
    const uint32_t tickMismatches = g_pendingMismatches;
    if (tickMismatches != 0) {
        d.mismatches += tickMismatches;
        if (!d.listed.empty() && !g_pendingDiffs.empty()) d.listed += ", ";
        d.listed += g_pendingDiffs;
    }

    // A core whose record differs restarts from the original's (a draw too:
    // later choices depend on the configuration kept).
    const bool lotDiffers = !(ol.lots.size() == ul.lots.size() && cfgSame == ol.lots.size() &&
                              ol.potholes.size() == ul.potholes.size()) ||
                            zoneDiffs != 0 || wealthDiffs != 0;
    std::string resync;
    if (!mgrSame && !r.start) {
        resync += StartMgr(nm) ? " mgr" : " mgr(refused)";
    }
    if (lotDiffers && !r.start) {
        resync += StartLot(nlm) ? " lot" : " lot(refused)";
    }
    if (!resync.empty()) ++g_resyncs;
    g_draws += draws;

    fc4_netmgr_state ms{};
    ms.struct_size = sizeof(ms);
    if (g_mgr != nullptr) fc4_netmgr_get_state(g_mgr, &ms);
    fc4_netlot_state ls{};
    ls.struct_size = sizeof(ls);
    if (g_lot != nullptr) fc4_netlot_get_state(g_lot, &ls);

    std::string& s = r.line;
    AppendF(s, "mismatches=%u %s; managed=%u/%u record53=%s (%u bytes); lots=%u/%u potholes=%u/%u record11=%s",
            d.mismatches, followed.c_str(), static_cast<unsigned>(om.managed.size()),
            static_cast<unsigned>(um.managed.size()), mgrSame ? "equal" : "DIFFERS",
            static_cast<unsigned>(g_origMgr.Bytes().size()), static_cast<unsigned>(ol.lots.size()),
            static_cast<unsigned>(ul.lots.size()), static_cast<unsigned>(ol.potholes.size()),
            static_cast<unsigned>(ul.potholes.size()),
            lotBytes ? "equal" : (lotDiffers ? "DIFFERS" : "same entries (order differs)"));
    AppendF(s, "; lot cells=%u zone diffs=%u wealth diffs=%u configs same=%u draws=%u (not compared: clock-seeded) "
               "incompatible=%u; configs loaded=%u",
            cells, zoneDiffs, wealthDiffs, cfgSame, draws, cfgBad, ls.configs);
    AppendF(s, "; messages=%u (mgr %u, lot %u); terrain msgs=%u tool calls=%u destroyed=%u confirmed=%u kept-by-orig=%u "
               "orig-only=%u rebuilt=%u candidates=%u refits=%u",
            g_n.messages, g_n.mgrMessages, g_n.lotMessages, g_n.terrainMessages, g_n.toolCalls, g_n.decided,
            g_n.confirmed, g_n.keptByOrig, g_n.origOnly, g_n.rebuilt, g_n.candidatesChecked, g_n.refits);
    AppendF(s, "; style writes=%u checked=%u gone=%u mismatches=%u; potholes created=%u; order %s: probe before=%u/%u",
            g_n.styleWrites, g_n.styleChecked, g_n.styleGone, g_n.styleMismatches, g_n.potholesCreated,
            g_late ? "late" : "early", g_n.orderBefore, g_n.orderProbes);
    if (g_late) {
        AppendF(s, "; per message: %u compared, variations agree=%u ours-only=%u orig-only=%u differ=%u", g_pm.messages,
                g_pm.agree, g_pm.oursOnly, g_pm.origOnly, g_pm.differ);
        for (const auto& t : g_pt) {
            AppendF(s, "; 0x%08X: %u msgs %u cells, the original changed %u, ours changed %u", t.first, t.second.messages,
                    t.second.cells, t.second.origChanged, t.second.oursChanged);
        }
        if (!g_pmFirst.empty()) s += " (" + g_pmFirst + ")";
        if (!g_geom.empty()) s += "; cells before/orig/ours " + g_geom;
        g_geom.clear();
        g_geomListed = 0;
        g_pt.clear();
        g_pm = PerMessage{};
        g_pmFirst.clear();
        g_pmListed = 0;
    }
    const uint32_t nm10Count = nm10.mismatches + g_nm10Pending;
    AppendF(s, "; NM10 (lot state border's first row, open)=%u", nm10Count);
    if (!resync.empty()) AppendF(s, "; resynced:%s", resync.c_str());
    if (!d.listed.empty()) s += "; differences: " + d.listed;
    if (nm10Count != 0) {
        std::string l = nm10.listed;
        if (!l.empty() && !g_nm10PendingDiffs.empty()) l += ", ";
        l += g_nm10PendingDiffs;
        s += "; NM10 differences: " + l;
        ++g_nm10Lines;
        g_nm10Total += nm10Count;
    }
    g_nm10Pending = 0;
    g_nm10PendingDiffs.clear();
    r.mismatches = d.mismatches;
    r.done = true;
    g_n = Counters{};
    g_pendingDiffs.clear();
    g_pendingMismatches = 0;
    g_pendingListed = 0;
}

void RunOnce(cISC4City* city, int32_t julian, const char* event, bool start) {
    if (!g_enabled || city == nullptr) return;
    Settle();
    Run r;
    r.city = city;
    r.julian = julian;
    r.event = event;
    r.start = start;
    if (!SehGuard("netmgr shadow (comparison)", &RunStep, &r)) {
        Log("netmgr shadow %s %s: WARNING structured exception; the cores are abandoned (their references leak)",
            event, DateText(julian).c_str());
        g_mgr = nullptr;
        g_lot = nullptr;
        g_ctx.held.clear();
        return;
    }
    if (r.line.empty()) return;
    std::string line = std::string("netmgr shadow (") + event + ") " + DateText(julian) + ": " + r.line;
    LogShadowLine("netmgr shadow (cont.)", reinterpret_cast<const uint8_t*>(line.data()), line.size());
    if (r.done) {
        ++g_lines;
        if (r.mismatches == 0) ++g_exactLines;
    }
}

// ---- the followed messages (§6) ---------------------------------------------------------------------------

bool IsMgrMessage(uint32_t t) {
    for (uint32_t m : kMgrMessages) {
        if (m == t) return true;
    }
    return false;
}

bool IsLotMessage(uint32_t t) {
    for (uint32_t m : kLotMessages) {
        if (m == t) return true;
    }
    return false;
}

struct Msg {
    uint32_t type = 0;
    uint32_t d[4] = {};
    uint64_t void1 = 0, unknown = 0;
    bool hasRect = false;
    fc4_cell_rect rect{};
    bool mgr = false, lot = false;
};

// The order probe: at an occupant removal, the original network lot manager
// still has the cell's lot for this occupant only if it has not handled the
// message yet (§6.2).
void OrderProbe(const Msg& m) {
    if (g_city == nullptr || m.void1 == 0) return;
    cISC4NetworkLotManager* nlm = g_city->GetNetworkLotManager();
    if (nlm == nullptr) return;
    const uint64_t n = g_lhost.query_network_occupant(g_lhost.user, m.void1);
    if (n == 0) return;
    int32_t x = 0, z = 0;
    g_lhost.position_cell(g_lhost.user, n, &x, &z);
    cISC4NetworkLot* lot = nlm->GetNetworkLot(x, z);  // slot 5, borrowed
    if (lot != nullptr) {
        ++g_n.orderProbes;
        if (TokenOf(lot->GetNetworkOccupant()) == n) ++g_n.orderBefore;  // lot slot 3, borrowed
    }
    NetDrop(n);
}

// The cells a lot message updates (§6.2), for the per-message comparison:
// the occupant's cell (usable), the zoning rect grown by one, the lot's
// bounding rect grown by one (its border; the whole rect is taken).
bool MessageCells(const Msg& m, fc4_cell_rect* r) {
    if (m.type == kMsgUsableChanged && m.unknown != 0) {
        const uint64_t n = g_lhost.query_network_occupant(g_lhost.user, m.unknown);
        if (n == 0) return false;
        int32_t x = 0, z = 0;
        g_lhost.position_cell(g_lhost.user, n, &x, &z);
        NetDrop(n);
        *r = fc4_cell_rect{x, z, x, z};
        return true;
    }
    if (m.type == kMsgZoningChanged && m.hasRect) {
        *r = fc4_cell_rect{m.rect.x1 - 1, m.rect.z1 - 1, m.rect.x2 + 1, m.rect.z2 + 1};
        return true;
    }
    if (m.type == kMsgLotStateChanged && m.unknown != 0) {
        const uint64_t lot = g_lhost.query_lot(g_lhost.user, m.unknown);
        if (lot == 0) return false;
        fc4_cell_rect b{};
        g_lhost.lot_bounding_rect(g_lhost.user, lot, &b);
        NetDrop(lot);
        *r = fc4_cell_rect{b.x1 - 1, b.z1 - 1, b.x2 + 1, b.z2 + 1};
        return true;
    }
    return false;
}

constexpr int32_t kMaxSnapCells = 4096;

void TakeSnapshot(void* p) {
    const Msg& m = *static_cast<const Msg*>(p);
    g_snap.clear();
    g_snapValid = false;
    fc4_cell_rect r{};
    if (!MessageCells(m, &r)) return;
    if ((r.x2 - r.x1 + 1) * (r.z2 - r.z1 + 1) > kMaxSnapCells) return;
    int32_t cx = 0, cz = 0;
    g_lhost.city_size(g_lhost.user, &cx, &cz);
    g_snapWide = m.type == kMsgLotStateChanged && g_wideLeft > 0;
    if (g_snapWide) --g_wideLeft;
    const fc4_cell_rect area = g_snapWide ? fc4_cell_rect{0, 0, cx - 1, cz - 1} : r;
    for (int32_t z = area.z1; z <= area.z2; ++z) {
        for (int32_t x = area.x1; x <= area.x2; ++x) {
            if (x < 0 || z < 0 || x >= cx || z >= cz) continue;
            const uint64_t n = g_lhost.find_network_tile(g_lhost.user, x, z, 0x1F4F);  // borrowed
            if (n == 0) continue;
            SnapCell c{x, z, NetOf(n)->GetVariation() & 0xFFu};
            const bool inRect = x >= r.x1 && x <= r.x2 && z >= r.z1 && z <= r.z2;
            if (cISC4NetworkLotManager* nlm = inRect && g_city ? g_city->GetNetworkLotManager() : nullptr) {
                c.zone = static_cast<int32_t>(nlm->GetZoneTypeForNetwork(x, z));  // slot 9
                c.wealth = static_cast<int32_t>(nlm->GetWealthForNetwork(x, z));  // slot 10
            }
            g_snap[n] = c;
        }
    }
    g_snapValid = true;
    g_snapType = m.type;
    g_snapRect = r;
}

void PmNote(const char* fmt, ...) {
    if (g_pmListed >= 6) return;
    ++g_pmListed;
    char b[200];
    va_list ap;
    va_start(ap, fmt);
    std::vsnprintf(b, sizeof(b), fmt, ap);
    va_end(ap);
    if (!g_pmFirst.empty()) g_pmFirst += ", ";
    g_pmFirst += b;
}

// After the original (snapshot -> now) and ours (g_msgWrites) handled the
// same message: whose style writes changed what.
bool InSnapRect(const SnapCell& c) {
    return c.x >= g_snapRect.x1 && c.x <= g_snapRect.x2 && c.z >= g_snapRect.z1 && c.z <= g_snapRect.z2;
}

void CompareMessageStyle(const Msg& m, const std::map<uint64_t, uint32_t>& afterOrig) {
    // The geometry of a message where ours changed a cell the original kept:
    // the rect, then every cell with a network tile as x,z:before/orig/ours.
    bool oursOnly = false;
    for (const auto& c : g_snap) {
        auto w = g_msgWrites.find(c.first);
        const uint32_t orig = afterOrig.count(c.first) != 0 ? afterOrig.at(c.first) : c.second.before;
        if (w != g_msgWrites.end() && w->second != orig && orig == c.second.before) oursOnly = true;
    }
    if (oursOnly && g_geomListed < 1) {
        ++g_geomListed;
        char b[96];
        std::snprintf(b, sizeof(b), "%s[0x%08X %u->%u rect %d,%d..%d,%d:", g_geom.empty() ? "" : " ", m.type, m.d[0],
                      m.d[1], g_snapRect.x1, g_snapRect.z1, g_snapRect.x2, g_snapRect.z2);
        g_geom += b;
        for (const auto& c : g_snap) {
            if (!InSnapRect(c.second)) continue;
            auto w = g_msgWrites.find(c.first);
            const uint32_t orig = afterOrig.count(c.first) != 0 ? afterOrig.at(c.first) : c.second.before;
            char v[8] = "-";
            if (w != g_msgWrites.end()) std::snprintf(v, sizeof(v), "%u", w->second);
            std::snprintf(b, sizeof(b), " %d,%d:%u/%u/%s", c.second.x, c.second.z, c.second.before, orig, v);
            g_geom += b;
        }
        g_geom += "]";
    }
    ++g_pm.messages;
    PerType& pt = g_pt[m.type];
    ++pt.messages;
    for (const auto& c : g_snap) {
        const uint32_t orig = afterOrig.count(c.first) != 0 ? afterOrig.at(c.first) : c.second.before;
        auto w = g_msgWrites.find(c.first);
        const bool origChanged = orig != c.second.before;
        if (!InSnapRect(c.second)) {
            // netmgr_shadow_wide: a cell off the message's rect.
            ++g_place[6][0];
            if (origChanged) ++g_place[6][1];
            if (!origChanged && w != g_msgWrites.end() && w->second != orig) ++g_place[6][2];
            if (origChanged && g_wideListed < 16) {
                ++g_wideListed;
                char b[160];
                std::snprintf(b, sizeof(b), "%srect %d,%d..%d,%d: %d,%d %u->%u (ours %s)", g_wideNotes.empty() ? "" : ", ",
                              g_snapRect.x1, g_snapRect.z1, g_snapRect.x2, g_snapRect.z2, c.second.x, c.second.z,
                              c.second.before, orig, w != g_msgWrites.end() ? "wrote" : "no write");
                g_wideNotes += b;
            }
            continue;
        }
        ++pt.cells;
        if (origChanged) ++pt.origChanged;
        if (w != g_msgWrites.end() && w->second != orig) ++pt.oursChanged;
        if (m.type == kMsgLotStateChanged) {
            const int place = PlaceOf(c.second.x, c.second.z, g_snapRect);
            ++g_place[place][0];
            if (origChanged) ++g_place[place][1];
            if (!origChanged && w != g_msgWrites.end() && w->second != orig) ++g_place[place][2];
            if (place == 6 && origChanged && g_wideListed < 16) {
                ++g_wideListed;
                char b[160];
                std::snprintf(b, sizeof(b), "%srect %d,%d..%d,%d: %d,%d %u->%u (ours %s)", g_wideNotes.empty() ? "" : ", ",
                              g_snapRect.x1, g_snapRect.z1, g_snapRect.x2, g_snapRect.z2, c.second.x, c.second.z,
                              c.second.before, orig, w != g_msgWrites.end() ? "wrote" : "no write");
                g_wideNotes += b;
            }
        }
        if (w == g_msgWrites.end()) {
            if (origChanged) {
                ++g_pm.origOnly;
                PmNote("0x%08X d1=%u at %d,%d: the original %u->%u, ours no write", m.type, m.d[0], c.second.x,
                       c.second.z, c.second.before, orig);
            }
            continue;
        }
        if (w->second == orig) {
            ++g_pm.agree;
        } else if (!origChanged) {
            ++g_pm.oursOnly;
            int32_t zl = -1, wl = -1;
            if (cISC4NetworkLotManager* nlm = g_city ? g_city->GetNetworkLotManager() : nullptr) {
                zl = static_cast<int32_t>(nlm->GetZoneTypeForNetwork(c.second.x, c.second.z));
                wl = static_cast<int32_t>(nlm->GetWealthForNetwork(c.second.x, c.second.z));
            }
            PmNote("0x%08X d1=%u d2=%u at %d,%d: ours %u->%u, the original kept %u; its zone/wealth before %d/%d, after "
                   "it ran %d/%d",
                   m.type, m.d[0], m.d[1], c.second.x, c.second.z, c.second.before, w->second, orig, c.second.zone,
                   c.second.wealth, zl, wl);
        } else {
            ++g_pm.differ;
            PmNote("0x%08X d1=%u at %d,%d: the original %u->%u, ours ->%u", m.type, m.d[0], c.second.x, c.second.z,
                   c.second.before, orig, w->second);
        }
    }
}

void CallMessage(void* p) {
    const Msg& m = *static_cast<const Msg*>(p);
    if (m.type == kMsgOccupantRemoved && m.lot) OrderProbe(m);
    if (m.mgr && g_mgr != nullptr) {
        ++g_n.mgrMessages;
        if (m.type == 0x278E6003) ++g_n.terrainMessages;
        fc4_netmgr_do_message(g_mgr, m.type, m.d[0], m.d[1], m.d[2], m.d[3], m.void1, 0, 0);
        NetMgrReleaseHeld(&g_ctx);
    }
    if (m.lot && g_lot != nullptr) {
        ++g_n.lotMessages;
        // The original has handled it (order late): its variations now.
        const bool compare = g_late && g_snapValid && g_snapType == m.type;
        std::map<uint64_t, uint32_t> afterOrig;
        if (compare) {
            for (const auto& c : g_snap) afterOrig[c.first] = NetOf(c.first)->GetVariation() & 0xFFu;
        }
        g_msgWrites.clear();
        fc4_cell_rect nr{};
        if (m.type == kMsgLotStateChanged && MessageCells(m, &nr)) {
            for (int32_t x = nr.x1 + 1; x < nr.x2; ++x) g_nm10Cells.insert(std::make_pair(x, nr.z1));
        }
        fc4_netlot_do_message(g_lot, m.type, m.d[0], m.d[1], m.d[2], m.void1, m.unknown, m.hasRect ? &m.rect : nullptr);
        NetMgrReleaseHeld(&g_ctx);
        if (compare) CompareMessageStyle(m, afterOrig);
        g_snapValid = false;
        g_snap.clear();
    }
}

// `late`: the call comes from the target subscribed after the original
// network lot manager (netmgr_shadow_order = late).
void OnMessage(cIGZMessage2* msg, bool late) {
    if (msg == nullptr || (g_mgr == nullptr && g_lot == nullptr) || g_inMessage) return;
    Msg m;
    m.type = msg->GetType();
    m.mgr = !late && IsMgrMessage(m.type);
    m.lot = late == g_late && IsLotMessage(m.type);
    const bool snapshot = !late && g_late && IsLotMessage(m.type);
    if (!m.mgr && !m.lot && !snapshot) return;
    if (m.type == kMsgViewChanged) return;  // presentation only (§6.1)
    void* p = nullptr;
    if (!msg->QueryInterface(GZIID_cIGZMessage2Standard, &p) || p == nullptr) return;
    cIGZMessage2Standard* s = static_cast<cIGZMessage2Standard*>(p);
    FC4_ACQUIRED(s, "QueryInterface");
    m.d[0] = static_cast<uint32_t>(s->GetData1());
    m.d[1] = static_cast<uint32_t>(s->GetData2());
    m.d[2] = static_cast<uint32_t>(s->GetData3());
    m.d[3] = static_cast<uint32_t>(s->GetData4());
    if (m.type == kMsgOccupantInserted || m.type == kMsgOccupantRemoved || m.type == kMsgCreateLotObjects) {
        m.void1 = TokenOf(s->GetVoid1());
    }
    if (m.type == kMsgUsableChanged || m.type == kMsgLotStateChanged) m.unknown = TokenOf(s->GetIGZUnknown());
    if (m.type == kMsgZoningChanged) {
        // §6.2: *data2 is the cell rect {x1, z1, x2, z2}.
        if (const int32_t* r = static_cast<const int32_t*>(s->GetVoid2())) {
            m.rect = fc4_cell_rect{r[0], r[1], r[2], r[3]};
            m.hasRect = true;
        }
    }
    // Order late: the early target sees the lot messages first (subscribed
    // for them only to take the snapshot).
    if (snapshot && g_lot != nullptr) {
        g_inMessage = true;
        if (!SehGuard("netmgr shadow (style snapshot)", &TakeSnapshot, &m)) {
            g_snap.clear();
            g_snapValid = false;
        }
        g_inMessage = false;
    }
    if (!m.mgr && !m.lot) {
        FC4_RELEASE(s);
        return;
    }
    ++g_n.messages;
    g_inMessage = true;
    if (!SehGuard("netmgr shadow (followed message)", &CallMessage, &m)) {
        Log("netmgr shadow: WARNING structured exception in a followed message 0x%08X; the cores are abandoned "
            "(their references leak) until the next comparison restarts them",
            m.type);
        g_mgr = nullptr;
        g_lot = nullptr;
        g_ctx.held.clear();
    }
    g_inMessage = false;
    FC4_RELEASE(s);
}

class EventTarget final : public cIGZMessageTarget2 {
public:
    explicit EventTarget(bool late) : late_(late) {}
    bool QueryInterface(uint32_t riid, void** ppvObj) override {
        if (ppvObj == nullptr) return false;
        if (riid != kIID_cIGZMessageTarget2 && riid != GZIID_cIGZUnknown) return false;
        *ppvObj = static_cast<cIGZMessageTarget2*>(this);
        AddRef();
        return true;
    }
    uint32_t AddRef() override { return ++refs_; }
    uint32_t Release() override { return refs_ > 0 ? --refs_ : 0u; }
    bool DoMessage(cIGZMessage2* msg) override {
        if (msg == nullptr) return true;
        FC4_X87_SCOPE("netmgr shadow DoMessage", msg->GetType());  // x87 leftover guard (FC4Fpu.h)
        OnMessage(msg, late_);
        return true;
    }

private:
    bool late_;
    uint32_t refs_ = 0;
};

EventTarget& Events() {
    static EventTarget t(false);
    return t;
}

EventTarget& LateEvents() {
    static EventTarget t(true);
    return t;
}

// The late target: subscribed at PostCityInit (after the city's managers
// subscribed at their Init), removed at PreCityShutdown.
void LateSubscribe(bool on) {
    if (!g_late || on == g_lateListening) return;
    cIGZMessageServer2Ptr ms;
    if (!ms) return;
    g_lateListening = on;
    if (on) LateEvents().AddRef();  // our own reference (static object)
    bool ok = true;
    for (uint32_t id : kLotMessages) {
        if (on) {
            ok = ms->AddNotification(static_cast<cIGZMessageTarget2*>(&LateEvents()), id) && ok;
        } else {
            ms->RemoveNotification(static_cast<cIGZMessageTarget2*>(&LateEvents()), id);
        }
    }
    if (on) {
        Log("netmgr shadow: the lot core's listener (late: after the original network lot manager) registered for %u "
            "message types%s",
            static_cast<unsigned>(sizeof(kLotMessages) / sizeof(kLotMessages[0])),
            ok ? "" : " (a notification was refused; the lot core can miss messages)");
    }
}

}  // namespace

void NetMgrShadowStartup() {
    g_settingsRead = false;
    ReadSettings();
    if (!g_enabled) return;  // off by default: not a word in the log
    const char* src = "";
    Setting("netmgr_shadow", "FC4_NETMGR_SHADOW", &src);
    Log("netmgr shadow: ON (netmgr_shadow from %s): two shadow cores (read-only hosts) start from the ORIGINAL network "
        "manager's and network lot manager's records at PostCityInit and follow their 16 messages (the manager's through "
        "a listener registered before any city's, the lot manager's, with netmgr_shadow_order = late, after the "
        "original); the terrain rule's destructions and the road style writes are recorded and "
        "checked against the game at the next tick; at PostCityInit, each SimNewMonth and before a scripted save the "
        "component 53 and 11 records (our Write against the original's), the zone and wealth for network at every lot "
        "cell (slots 9, 10) and the compatibility of differing configurations are compared. Not compared: the lot "
        "configuration draws (clock-seeded generator, network-manager.md §17), the under-texture (no getter), the "
        "presentation (lighting, view rotation, retrofit, water display), props, flora and pothole effects",
        src);
}

void NetMgrShadowStart() {
    ReadSettings();
    if (!g_enabled || g_listening) return;
    cIGZMessageServer2Ptr ms;
    if (!ms) {
        Log("netmgr shadow: no message server; the shadow follows nothing");
        return;
    }
    g_listening = true;
    Events().AddRef();  // our own reference (static object)
    bool ok = true;
    uint32_t n = 0;
    for (uint32_t id : kMgrMessages) {
        ok = ms->AddNotification(static_cast<cIGZMessageTarget2*>(&Events()), id) && ok;
        ++n;
    }
    for (uint32_t id : kLotMessages) {
        if (id == kMsgOccupantRemoved) continue;  // already subscribed
        // late: only for the per-message style snapshot (the lot core hears
        // them through its own target)
        ok = ms->AddNotification(static_cast<cIGZMessageTarget2*>(&Events()), id) && ok;
        ++n;
    }
    Log("netmgr shadow: listener registered for %u message types (order %s: the lot core's messages %s)%s", n,
        g_late ? "late" : "early",
        g_late ? "come through a second target subscribed at PostCityInit" : "come through this one",
        ok ? "" : " (a notification was refused; the cores can miss messages)");
}

void NetMgrShadowStop() {
    if (!g_listening) return;
    g_listening = false;
    cIGZMessageServer2Ptr ms;
    if (!ms) return;
    for (uint32_t id : kMgrMessages) ms->RemoveNotification(static_cast<cIGZMessageTarget2*>(&Events()), id);
    for (uint32_t id : kLotMessages) {
        if (id != kMsgOccupantRemoved) ms->RemoveNotification(static_cast<cIGZMessageTarget2*>(&Events()), id);
    }
    LateSubscribe(false);
}

void NetMgrShadowCityLoaded(cISC4City* city) {
    if (!g_enabled || city == nullptr) return;
    int32_t julian = 0;
    if (cFC4ISC4Simulator* sim = AsGame(city->GetSimulator())) julian = sim->GetSimDateNumber();
    RunOnce(city, julian, "load", true);
    if (g_listening) LateSubscribe(true);
}

void NetMgrShadowMonth(cISC4City* city, int32_t julian) {
    if (!g_enabled || city == nullptr || g_city != city) return;
    RunOnce(city, julian, "month", false);
}

void NetMgrShadowBeforeSave(cISC4City* city, int32_t julian) {
    if (!g_enabled || city == nullptr || g_city != city) return;
    RunOnce(city, julian, "save", false);
}

void NetMgrShadowTick() {
    if (!g_enabled || g_inMessage) return;
    Settle();
}

void NetMgrShadowCityShutdown() {
    if (!g_enabled) return;
    LateSubscribe(false);
    Settle();
    if (g_city != nullptr || g_mgr != nullptr || g_lot != nullptr) {
        Log("netmgr shadow: city shutdown: %u lines, %u with mismatches=0 (exact %u/%u); %u configuration draws not "
            "compared; %u resyncs; NM10 (open question: the lot state message's border, first row) differences %u on %u "
            "lines",
            g_lines, g_exactLines, g_exactLines, g_lines, g_draws, g_resyncs, g_nm10Total, g_nm10Lines);
        if (g_late) {
            std::string t;
            for (int i = 0; i < kPlaces; ++i) {
                AppendF(t, "%s%s %u/%u/%u", i ? ", " : "", kPlaceNames[i], g_place[i][0], g_place[i][1],
                        g_place[i][2]);
            }
            Log("netmgr shadow: lot state changed (0xE8BDA2E7) border cells with a network tile, by place in the grown "
                "rect (cells / the original changed the variation / ours changed it where the original kept it): %s",
                t.c_str());
            if (!g_wideNotes.empty()) {
                Log("netmgr shadow: the original's changes off the grown rect's border (netmgr_shadow_wide): %s",
                    g_wideNotes.c_str());
            }
        }
    }
    std::memset(g_place, 0, sizeof(g_place));
    g_wideNotes.clear();
    g_wideListed = 0;
    DropCores();
    g_city = nullptr;
    g_ctx.city = nullptr;
    g_lines = g_exactLines = g_draws = g_resyncs = 0;
    g_nm10Lines = g_nm10Total = g_nm10Pending = 0;
    g_nm10PendingDiffs.clear();
    g_nm10Cells.clear();
    g_n = Counters{};
    g_pendingDiffs.clear();
    g_pendingMismatches = 0;
    g_pendingListed = 0;
}

}  // namespace fc4
