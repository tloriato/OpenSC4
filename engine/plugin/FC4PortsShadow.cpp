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

#include "FC4PortsShadow.h"

#include "FC4Config.h"
#include "FC4Diag.h"
#include "FC4Fpu.h"
#include "FC4GameList.h"
#include "FC4Interfaces.h"
#include "FC4Log.h"
#include "FC4PortInterfaces.h"
#include "FC4RefAudit.h"
#include "FC4Streams.h"
#include "FC4WriteCapture.h"
#include "cFC4PortDeveloper.h"

#include "GZServPtrs.h"
#include "cIGZApp.h"
#include "cIGZFrameWork.h"
#include "cIGZMessage2.h"
#include "cIGZMessage2Standard.h"
#include "cIGZMessageServer2.h"
#include "cIGZMessageTarget2.h"
#include "cISC4AirportDeveloper.h"
#include "cISC4App.h"
#include "cISC4City.h"
#include "cISC4Lot.h"
#include "cISC4SeaportDeveloper.h"
#include "cISTETerrain.h"
#include "cRZCOMDllDirector.h"

#include <cstdarg>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

namespace fc4 {
namespace {

constexpr uint32_t kMsgSimNewMonth = 0x66956816;
constexpr uint32_t kMsgOccupantInserted = 0x99EF1142;
constexpr uint32_t kMsgLotStateChanged = 0xE8BDA2E7;
constexpr uint32_t kMsgPostCityInit = 0x26D31EC1;
constexpr uint32_t kMsgPreCityShutdown = 0x26D31EC2;
constexpr uint32_t kMsgTerrainChanged = 0x6A3B0AFD;
constexpr uint32_t kMsgHistoryRecord = 0x89EFA536;
constexpr uint32_t kGrowth[] = {0xEA394B74, 0xCB3BE815, 0xEB3BE415, 0x2B3BE44A, 0x6B3BE6D8, 0x0B3BE81A, 0x8A394DC4};
constexpr uint32_t kIID_cIGZMessageTarget2 = 0x452294AA;
constexpr uint32_t kIID_cISC4Occupant = 0x26BB767E;
constexpr uint32_t kIID_cISC4Lot = 0xE818FCED;
constexpr const char* kAgentName = "cSC4PortDeveloper";  // §3 (AO1): both developers
constexpr uint32_t kHistoryKey[2] = {0x6A3E9032, 0xEA3E904A};
constexpr uint32_t kTuning[2] = {0xCA270E9D, 0xEA29AD92};
constexpr uint16_t kZone[2] = {11, 12};
constexpr size_t kMaxListed = 12;

bool g_enabled = false;
bool g_simulatorReplaced = false;
bool g_portsReplaced = false;
bool g_loggedOurs = false;
bool g_listening = false;

// What the shadow host recorded during our month.
struct Recorded {
    struct Cond {
        uint64_t lot;
        uint32_t condition;
    };
    std::vector<Cond> conditions;
    std::vector<int32_t> histories;
    uint32_t effects = 0, events = 0, lastEvent = 0, capacities = 0, otherWrites = 0;
    uint32_t props = 0;  // §4.4 step 3 prop switches (a month write, not growth or removal; glue-portdev4)
    void Clear() { *this = Recorded{}; }
};

// One followed developer.
struct Follow {
    uint32_t kind = 0;
    cIGZUnknown* sim = nullptr;  // the original's primary interface (identity only)
    cIGZMessageTarget2* agent = nullptr;
    PortContext ctx;
    fc4_port_host host{};
    fc4_port_developer* core = nullptr;
    const void* coreSim = nullptr;
    bool regrow = false;          // a growth message arrived since our last start
    bool mapPending = true;       // seaport: compare the map at the next month
    uint32_t months = 0, cleanMonths = 0, followed = 0;
    Recorded rec;
    WriteCapture capture;
    std::vector<int32_t> origHistories;  // the original's history values this month
};

Follow g_f[2];
int g_inOriginalMonth = -1;  // which original's month is running (history capture)

Follow& Of(void* u) { return u == &g_f[1].ctx ? g_f[1] : g_f[0]; }

// ---- the recording host (writes recorded, not made) ------------------------------------------------

void RecCondition(void* u, uint64_t lot, uint32_t c) { Of(u).rec.conditions.push_back(Recorded::Cond{lot, c}); }
void RecEffects(void* u, uint64_t, float, float) { ++Of(u).rec.effects; }
void RecHistory(void* u, uint32_t, int32_t v) { Of(u).rec.histories.push_back(v); }
void RecEvent(void* u, uint32_t e) {
    ++Of(u).rec.events;
    Of(u).rec.lastEvent = e;
}
void RecCapacity(void* u, uint64_t, uint32_t, uint32_t) { ++Of(u).rec.capacities; }
void RecFull(void* u, uint64_t, uint32_t, int64_t) { ++Of(u).rec.otherWrites; }
bool RecWithdraw(void* u, int64_t) {
    ++Of(u).rec.otherWrites;
    return true;
}
void RecLot(void* u, uint64_t) { ++Of(u).rec.otherWrites; }
uint64_t RecCreateLot(void* u, int32_t, int32_t, int32_t, int32_t, uint32_t) {
    ++Of(u).rec.otherWrites;
    return 0;
}
void RecDezone(void* u, const fc4_cell_rect*) { ++Of(u).rec.otherWrites; }
uint32_t RecProps(void* u, uint64_t, uint32_t, uint32_t) {
    ++Of(u).rec.props;
    return 0;
}

void ReadSettings() { g_enabled = SettingInt("ports_shadow", "FC4_PORTS_SHADOW", 0) != 0; }

bool Active() { return g_enabled && g_simulatorReplaced && !g_portsReplaced; }

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

// City slot 77 (airport) / 76 (seaport), borrowed; null when it is ours.
cIGZUnknown* OriginalOf(cISC4City* city, uint32_t kind) {
    if (city == nullptr) return nullptr;
    cIGZUnknown* raw = kind == 0 ? static_cast<cIGZUnknown*>(city->GetAirportDeveloper())
                                 : static_cast<cIGZUnknown*>(city->GetSeaportDeveloper());
    if (raw == nullptr) return nullptr;
    if (IsOurObject(raw)) {
        if (!g_loggedOurs) {
            g_loggedOurs = true;
            Log("ports shadow: inactive: the city's port developers are FreeCity4's own (do not enable ports)");
        }
        return nullptr;
    }
    return raw;
}

cIGZMessageTarget2* AgentOf(cIGZUnknown* sim) {
    void* agent = nullptr;
    if (!sim->QueryInterface(kIID_cIGZMessageTarget2, &agent) || agent == nullptr) return nullptr;
    cIGZUnknown* a = static_cast<cIGZUnknown*>(agent);
    FC4_ACQUIRED(a, "QueryInterface");
    FC4_RELEASE(a);  // only its identity is kept
    return static_cast<cIGZMessageTarget2*>(agent);
}

void DropCore(Follow& f) {
    if (f.core != nullptr) {
        fc4_port_developer_shutdown(f.core);
        fc4_port_developer_destroy(f.core);
    }
    f.core = nullptr;
    f.coreSim = nullptr;
}

void Track(Follow& f, cIGZUnknown* sim) {
    DropCore(f);
    f.sim = sim;
    f.agent = AgentOf(sim);
    f.mapPending = true;
    Log("ports shadow: following the original %s developer %p (agent %p)", PortKindName(f.kind),
        static_cast<void*>(sim), static_cast<void*>(f.agent));
}

// ---- the original's record (§7) -------------------------------------------------------------------

struct RecordPort {
    uint32_t stage;
    uint16_t typeIndex;
    int16_t x, z;
};

struct Record {
    bool ok = false;
    uint32_t instance = 0;
    uint16_t zone = 0;
    std::vector<RecordPort> ports;
};

Record Parse(const WriteCapture& cap) {
    Record r;
    CaptureReader b(cap.Bytes());
    uint16_t v1 = 0, v2 = 0, n = 0;
    if (!b.U16(v1) || !b.U16(v2) || v1 != 2 || v2 != 1) return r;
    if (!b.U32(r.instance) || !b.U16(r.zone) || !b.U16(n)) return r;
    for (uint16_t i = 0; i < n; ++i) {
        RecordPort p{};
        uint16_t x = 0, z = 0;
        if (!b.U32(p.stage) || !b.U16(p.typeIndex) || !b.U16(x) || !b.U16(z)) return r;
        p.x = static_cast<int16_t>(x);
        p.z = static_cast<int16_t>(z);
        r.ports.push_back(p);
    }
    uint8_t extra = 0;
    r.ok = !b.U8(extra);  // the whole record was read
    return r;
}

// A fresh shadow core from the original's capture: Read, Init (the read-only
// host creates and registers nothing), PostCityInit (the lots looked up).
bool Restart(Follow& f) {
    DropCore(f);
    f.ctx = PortContext{};
    f.ctx.readOnly = true;
    f.ctx.kind = f.kind;
    f.host = MakePortHost(&f.ctx);
    f.host.set_lot_condition = &RecCondition;
    f.host.update_occupant_effects = &RecEffects;
    f.host.record_history = &RecHistory;
    f.host.push_event = &RecEvent;
    f.host.set_max_trip_capacity = &RecCapacity;
    f.host.set_full_expenses = &RecFull;
    f.host.withdraw_funds = &RecWithdraw;
    f.host.start_demolish_lot = &RecLot;
    f.host.end_demolish_lot = &RecLot;
    f.host.create_lot = &RecCreateLot;
    f.host.dezone = &RecDezone;
    f.host.set_access_props_state = &RecProps;
    f.core = fc4_port_developer_create(&f.host, f.kind, TokenOf(f.sim));
    if (f.core == nullptr) return false;
    CaptureIStream in(f.capture);
    const bool ok = fc4_port_developer_read(f.core, in.Get());
    fc4_port_developer_init(f.core);
    fc4_port_developer_do_message(f.core, kMsgPostCityInit, 0, 0, 0);
    if (!ok) {
        DropCore(f);
        return false;
    }
    f.coreSim = f.sim;
    f.regrow = false;
    return true;
}

std::vector<RecordPort> OurPorts(const Follow& f) {
    std::vector<RecordPort> v;
    fc4_port_lot l{};
    l.struct_size = sizeof(l);
    for (uint32_t i = 0; fc4_port_developer_get_port(f.core, i, &l); ++i) {
        v.push_back(RecordPort{l.stage, static_cast<uint16_t>(l.type_index), l.x, l.z});
    }
    return v;
}

bool SamePorts(const std::vector<RecordPort>& a, const std::vector<RecordPort>& b) {
    if (a.size() != b.size()) return false;
    for (size_t i = 0; i < a.size(); ++i) {
        if (a[i].stage != b[i].stage || a[i].typeIndex != b[i].typeIndex || a[i].x != b[i].x || a[i].z != b[i].z) {
            return false;
        }
    }
    return true;
}

std::string DateText(int32_t julian) {
    if (julian <= 0) return "0000-00-00";
    fc4_date d{};
    fc4_date_from_julian(julian, &d);
    char b[24];
    snprintf(b, sizeof(b), "%04d-%02d-%02d", d.year, d.month, d.day_of_month);
    return b;
}

uint32_t Bits(float f) {
    uint32_t b;
    std::memcpy(&b, &f, sizeof(b));
    return b;
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
        vsnprintf(b, sizeof(b), fmt, ap);
        va_end(ap);
        if (!listed.empty()) listed += ", ";
        listed += b;
    }
};

struct PortLotState {
    uint64_t lot;
    uint32_t before, predicted;
};

// One month of one developer: Before (guarded) -> the original's DoMessage
// (unguarded, exactly once) -> After (guarded).
struct Month {
    Follow* f = nullptr;
    int32_t julian = 0;
    bool compared = false;
    const char* incomplete = nullptr;
    const char* followed = "yes";
    Record rec;
    Diff d;
    std::vector<PortLotState> lots;
    fc4_port_state ours{};
    std::string sea = "-", map = "-";
    float avgOrig = 0.0f, avgOurs = 0.0f;
    uint32_t tripsOrig = 0, tripsOurs = 0, portsOrig = 0;
};

cFC4ISC4AirportDeveloper* AsAirport(cIGZUnknown* u) { return reinterpret_cast<cFC4ISC4AirportDeveloper*>(u); }
cFC4ISC4SeaportDeveloper* AsSeaport(cIGZUnknown* u) { return reinterpret_cast<cFC4ISC4SeaportDeveloper*>(u); }
cFC4ISC4PortDeveloper* AsPort(cIGZUnknown* u) { return reinterpret_cast<cFC4ISC4PortDeveloper*>(u); }

// §4.8: CityHasSea and every cell's access byte (the terrain's cell counts).
void CompareMap(Month& mo) {
    Follow& f = *mo.f;
    cFC4ISC4SeaportDeveloper* o = AsSeaport(f.sim);
    const bool seaOrig = o->CityHasSea();
    const bool seaOurs = fc4_port_developer_city_has_sea(f.core);
    mo.sea = std::string(seaOrig ? "1" : "0") + "/" + (seaOurs ? "1" : "0");
    if (seaOrig != seaOurs) mo.d.Add("CityHasSea %d/%d", seaOrig ? 1 : 0, seaOurs ? 1 : 0);
    cISC4City* city = CurrentCity();
    cISTETerrain* t = city ? city->GetTerrain() : nullptr;
    const int32_t w = t ? static_cast<int32_t>(t->CellCountX()) : 0;
    const int32_t h = t ? static_cast<int32_t>(t->CellCountZ()) : 0;
    uint32_t differ = 0, water = 0, edged = 0;
    for (int32_t z = 0; z < h; ++z) {
        for (int32_t x = 0; x < w; ++x) {
            const uint8_t a = o->GetCellSeaAccess(x, z);
            const uint8_t b = fc4_port_developer_cell_sea_access(f.core, x, z);
            if (a & 0x80) ++water;
            if (a & 0x0F) ++edged;
            if (a != b) {
                if (differ == 0) mo.d.Add("sea access (%d,%d) 0x%02X/0x%02X", x, z, unsigned{a}, unsigned{b});
                ++differ;
            }
        }
    }
    if (differ > 1) mo.d.Add("sea access: %u cells differ", differ);
    char b[96];
    snprintf(b, sizeof(b), "%dx%d water=%u edge-reaching=%u differ=%u", w, h, water, edged, differ);
    mo.map = b;
    f.mapPending = false;
}

void MonthBefore(void* ctx) {
    Month& mo = *static_cast<Month*>(ctx);
    Follow& f = *mo.f;
    const bool captured = f.capture.Capture(f.sim);
    if (captured) mo.rec = Parse(f.capture);
    if (!mo.rec.ok) {
        mo.incomplete =
            captured ? "the original's record could not be parsed" : "the original's record could not be captured";
        DropCore(f);
        return;
    }
    if (mo.rec.instance != kTuning[f.kind] || mo.rec.zone != kZone[f.kind]) {
        mo.d.Add("record instance 0x%08X zone %u (shipped 0x%08X, %u)", mo.rec.instance, unsigned{mo.rec.zone},
                 kTuning[f.kind], unsigned{kZone[f.kind]});
    }
    if (f.core == nullptr || f.coreSim != f.sim) {
        mo.followed = "first";
        if (!Restart(f)) {
            mo.incomplete = "the shadow core could not read the original's record";
            return;
        }
    } else if (f.regrow) {
        mo.followed = "regrown";
        if (!Restart(f)) {
            mo.incomplete = "the shadow core could not read the original's record";
            return;
        }
    } else if (!SamePorts(OurPorts(f), mo.rec.ports)) {
        const size_t ours = OurPorts(f).size();
        mo.d.Add("ports %u/%u (orig/ours: stage, type, location in order)", static_cast<unsigned>(mo.rec.ports.size()),
                 static_cast<unsigned>(ours));
        mo.followed = "resynced";
        if (!Restart(f)) {
            mo.incomplete = "the shadow core could not read the original's record";
            return;
        }
    }
    if (f.kind == 1 && f.mapPending) CompareMap(mo);
    fc4_port_lot l{};
    l.struct_size = sizeof(l);
    for (uint32_t i = 0; fc4_port_developer_get_port(f.core, i, &l); ++i) {
        if (l.lot == 0) continue;
        const uint32_t c = f.host.lot_condition(&f.ctx, l.lot);
        mo.lots.push_back(PortLotState{l.lot, c, c});
    }
    f.rec.Clear();
    fc4_port_developer_do_message(f.core, kMsgSimNewMonth, 0, 0, 0);
    for (const Recorded::Cond& w : f.rec.conditions) {
        for (PortLotState& s : mo.lots) {
            if (s.lot == w.lot) s.predicted = w.condition;
        }
    }
    mo.ours.struct_size = sizeof(mo.ours);
    fc4_port_developer_get_state(f.core, &mo.ours);
    f.origHistories.clear();
    mo.compared = true;
}

void MonthAfter(void* ctx) {
    Month& mo = *static_cast<Month*>(ctx);
    Follow& f = *mo.f;
    if (!mo.compared) {
        Log("ports shadow %s month %s: INCOMPLETE (%s); not compared", PortKindName(f.kind),
            DateText(mo.julian).c_str(), mo.incomplete ? mo.incomplete : "a fault before the original's month");
        return;
    }
    cFC4ISC4PortDeveloper* o = AsPort(f.sim);
    // History (§4.3): the original's values with its key, seen during its month.
    if (f.origHistories != f.rec.histories) {
        mo.d.Add("history %u values (first %d) / %u (first %d)", static_cast<unsigned>(f.origHistories.size()),
                 f.origHistories.empty() ? 0 : f.origHistories[0], static_cast<unsigned>(f.rec.histories.size()),
                 f.rec.histories.empty() ? 0 : f.rec.histories[0]);
    }
    SC4Percentage pct;
    pct.percentage = -1.0f;
    o->GetAvgEfficiency(&pct);
    mo.avgOrig = pct.percentage;
    mo.avgOurs = fc4_port_developer_avg_efficiency(f.core);
    if (Bits(mo.avgOrig) != Bits(mo.avgOurs)) {
        mo.d.Add("GetAvgEfficiency %.9g/%.9g", static_cast<double>(mo.avgOrig), static_cast<double>(mo.avgOurs));
    }
    mo.tripsOrig = f.kind == 0 ? AsAirport(f.sim)->GetNumAirportTrips() : AsSeaport(f.sim)->GetNumSeaportTrips();
    mo.tripsOurs = fc4_port_developer_num_trips(f.core);
    if (mo.tripsOrig != mo.tripsOurs) mo.d.Add("trips %u/%u", mo.tripsOrig, mo.tripsOurs);
    mo.portsOrig = o->GetNumPorts(-1, -1);
    const uint32_t portsOurs = fc4_port_developer_num_ports(f.core, 0xFFFFFFFFu, 0xFFFFFFFFu);
    if (mo.portsOrig != portsOurs) mo.d.Add("GetNumPorts(all) %u/%u", mo.portsOrig, portsOurs);
    const uint32_t countOrig = f.kind == 0 ? AsAirport(f.sim)->GetNumAirports(0) : AsSeaport(f.sim)->GetNumSeaports();
    if (countOrig != mo.portsOrig) mo.d.Add("GetNum%sports %u, GetNumPorts %u", f.kind == 0 ? "Air" : "Sea", countOrig,
                                            mo.portsOrig);
    // GetAllPortLots: the original's lots in order (one reference each, released).
    if (StlNodeAllocator::Available()) {
        GameVector<cISC4Lot*> lots;
        o->GetAllPortLots(lots);
        std::vector<uint64_t> orig;
        for (cISC4Lot* lot : lots) {
            orig.push_back(TokenOf(lot));
            cIGZUnknown* u = reinterpret_cast<cIGZUnknown*>(lot);
            if (u == nullptr) continue;
            if (IsOurObject(u)) {
                u->Release();
            } else {
                FC4_ACQUIRED(u, "GetAllPortLots");
                FC4_RELEASE(u);
            }
        }
        std::vector<uint64_t> ours(fc4_port_developer_all_port_lots(f.core, nullptr, 0));
        if (!ours.empty()) fc4_port_developer_all_port_lots(f.core, ours.data(), ours.size());
        if (orig != ours) {
            mo.d.Add("GetAllPortLots %u/%u lots", static_cast<unsigned>(orig.size()), static_cast<unsigned>(ours.size()));
        }
    }
    // Per port lot: usage values, maintenance, the condition after the month.
    for (const PortLotState& s : mo.lots) {
        cISC4Lot* lot = reinterpret_cast<cISC4Lot*>(FromToken(s.lot));
        uint32_t ou = 0, oc = 0, wu = 0, wc = 0;
        const bool of = f.kind == 0 ? AsAirport(f.sim)->GetAirportUsageValues(lot, ou, oc)
                                    : AsSeaport(f.sim)->GetSeaportUsageValues(lot, ou, oc);
        const bool wf = fc4_port_developer_usage_values(f.core, s.lot, &wu, &wc);
        if (of != wf || ou != wu || oc != wc) {
            mo.d.Add("lot %p usage %d %u/%u, ours %d %u/%u", static_cast<void*>(lot), of ? 1 : 0, ou, oc, wf ? 1 : 0,
                     wu, wc);
        }
        const int64_t om = f.kind == 0 ? AsAirport(f.sim)->GetMaintenanceCost(lot) : AsSeaport(f.sim)->GetMaintenanceCost(lot);
        const int64_t wm = fc4_port_developer_maintenance_cost(f.core, s.lot);
        if (om != wm) mo.d.Add("lot %p maintenance %lld/%lld", static_cast<void*>(lot), static_cast<long long>(om),
                               static_cast<long long>(wm));
        const uint32_t after = f.host.lot_condition(&f.ctx, s.lot);
        if (after != s.predicted) {
            mo.d.Add("lot %p condition %u -> orig %u, ours %u", static_cast<void*>(lot), s.before, after, s.predicted);
        }
    }
    // Line 1 (§4.2): the live full expenses against our RequiredBudget (set at
    // adoption, removal and growth only; with ports).
    if (portsOurs > 0 && mo.ours.department != 0) {
        int64_t full = 0, cur = 0;
        if (f.host.line_expenses(&f.ctx, mo.ours.department, 1, &full, &cur)) {
            const int64_t req = fc4_port_developer_required_budget(f.core);
            if (full != req) mo.d.Add("line 1 full %lld / required %lld", static_cast<long long>(full),
                                      static_cast<long long>(req));
        }
    }
    if (f.rec.otherWrites != 0) mo.d.Add("ours made %u growth or removal writes in a month", f.rec.otherWrites);
    ++f.months;
    if (mo.d.mismatches == 0) ++f.cleanMonths;
    char num[400];
    snprintf(num, sizeof(num),
             "ports=%u avg %.9g/%.9g trips %u/%u history %u/%u values sea=%s map=%s; ours: ran=%u skipped=%u "
             "event=0x%08X events=%u effects=%u conditions=%u capacities=%u props=%u",
             mo.portsOrig, static_cast<double>(mo.avgOrig), static_cast<double>(mo.avgOurs), mo.tripsOrig, mo.tripsOurs,
             static_cast<unsigned>(f.origHistories.size()), static_cast<unsigned>(f.rec.histories.size()),
             mo.sea.c_str(), mo.map.c_str(), mo.ours.month_ran, mo.ours.month_skipped, mo.ours.month_event,
             f.rec.events, f.rec.effects, static_cast<unsigned>(f.rec.conditions.size()), f.rec.capacities, f.rec.props);
    const std::string line = std::string("ports shadow ") + PortKindName(f.kind) + " month " + DateText(mo.julian) +
                             ": followed=" + mo.followed + " " + num +
                             "; messages followed=" + std::to_string(f.followed) +
                             "; mismatches=" + std::to_string(mo.d.mismatches) +
                             (mo.d.listed.empty() ? std::string() : "; differences: " + mo.d.listed);
    LogShadowLine("ports shadow (cont.)", reinterpret_cast<const uint8_t*>(line.data()), line.size());
    f.followed = 0;
}

void CityShutdown() {
    for (Follow& f : g_f) {
        if (f.core == nullptr && f.sim == nullptr) continue;
        if (f.months > 0) {
            Log("ports shadow: city shutdown: %s %u month lines, %u with mismatches=0", PortKindName(f.kind), f.months,
                f.cleanMonths);
        }
        DropCore(f);
        f.sim = nullptr;
        f.agent = nullptr;
        f.months = f.cleanMonths = f.followed = 0;
    }
}

// ---- the followed messages (§3) -------------------------------------------------------------------

struct Msg {
    uint32_t type;
    uint64_t data;
    uint32_t oldState, newState;
};

Follow* g_msgFollow = nullptr;

void CallMessage(void* ctx) {
    const Msg* m = static_cast<const Msg*>(ctx);
    fc4_port_developer_do_message(g_msgFollow->core, m->type, m->data, m->oldState, m->newState);
}

void OnMessage(cIGZMessage2* msg) {
    if (msg == nullptr) return;
    const uint32_t type = msg->GetType();
    if (type == kMsgPreCityShutdown) {
        CityShutdown();
        return;
    }
    bool growth = false;
    for (uint32_t g : kGrowth) growth = growth || g == type;
    if (growth) {
        // The original rebuilds its ready ports (§4.5); ours restarts from its
        // record at the next month.
        for (Follow& f : g_f) {
            if (f.core != nullptr) f.regrow = true;
        }
        Log("ports shadow: growth message 0x%08X: the shadow cores restart from the records at the next month", type);
        return;
    }
    if (type != kMsgOccupantInserted && type != kMsgLotStateChanged && type != kMsgPostCityInit &&
        type != kMsgTerrainChanged && type != kMsgHistoryRecord) {
        return;
    }
    void* p = nullptr;
    if (!msg->QueryInterface(GZIID_cIGZMessage2Standard, &p) || p == nullptr) return;
    cIGZMessage2Standard* s = static_cast<cIGZMessage2Standard*>(p);
    FC4_ACQUIRED(s, "QueryInterface");
    if (type == kMsgHistoryRecord) {
        if (g_inOriginalMonth >= 0 && static_cast<uint32_t>(s->GetData1()) == kHistoryKey[g_inOriginalMonth]) {
            g_f[g_inOriginalMonth].origHistories.push_back(static_cast<int32_t>(s->GetData2()));
        }
        FC4_RELEASE(s);
        return;
    }
    Msg m{};
    m.type = type;
    cIGZUnknown* held = nullptr;
    if (type == kMsgOccupantInserted) {
        void* q = nullptr;
        cIGZUnknown* v = static_cast<cIGZUnknown*>(s->GetVoid1());
        if (v != nullptr && v->QueryInterface(kIID_cISC4Occupant, &q) && q != nullptr) held = static_cast<cIGZUnknown*>(q);
    } else if (type == kMsgLotStateChanged) {
        void* q = nullptr;
        cIGZUnknown* v = s->GetIGZUnknown();
        if (v != nullptr && v->QueryInterface(kIID_cISC4Lot, &q) && q != nullptr) held = static_cast<cIGZUnknown*>(q);
        m.oldState = static_cast<uint32_t>(s->GetData1());
        m.newState = static_cast<uint32_t>(s->GetData2());
    }
    if (held != nullptr && !IsOurObject(held)) FC4_ACQUIRED(held, "QueryInterface");
    m.data = TokenOf(held);
    for (Follow& f : g_f) {
        if (f.core == nullptr) continue;
        if (type == kMsgTerrainChanged && f.kind == 1) f.mapPending = true;
        ++f.followed;
        g_msgFollow = &f;
        SehGuard("ports shadow (followed message)", &CallMessage, &m);
    }
    g_msgFollow = nullptr;
    if (held != nullptr) {
        if (IsOurObject(held)) {
            held->Release();
        } else {
            FC4_RELEASE(held);
        }
    }
    FC4_RELEASE(s);
}

class EventTarget final : public cIGZMessageTarget2 {
public:
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
        FC4_X87_SCOPE("ports shadow DoMessage", msg->GetType());  // x87 leftover guard (FC4Fpu.h)
        OnMessage(msg);
        return true;
    }

private:
    uint32_t refs_ = 0;
};

EventTarget& Events() {
    static EventTarget t;
    return t;
}

// Registered once, at the first delivery the shadow follows, and left
// registered for the process.
void Listen() {
    if (g_listening) return;
    g_listening = true;
    cIGZMessageServer2Ptr ms;
    if (!ms) {
        Log("ports shadow: no message server; the messages of §3 do not reach ours");
        return;
    }
    Events().AddRef();  // our own reference (static object)
    bool ok = true;
    std::vector<uint32_t> ids = {kMsgOccupantInserted, kMsgLotStateChanged, kMsgPostCityInit, kMsgPreCityShutdown,
                                 kMsgTerrainChanged, kMsgHistoryRecord};
    ids.insert(ids.end(), std::begin(kGrowth), std::end(kGrowth));
    for (uint32_t id : ids) ok = ms->AddNotification(static_cast<cIGZMessageTarget2*>(&Events()), id) && ok;
    if (!ok) Log("ports shadow: a notification was refused; ours can miss messages");
}

}  // namespace

void PortsShadowStartup(bool simulatorReplaced, bool portsReplaced) {
    g_simulatorReplaced = simulatorReplaced;
    g_portsReplaced = portsReplaced;
    g_f[0].kind = 0;
    g_f[1].kind = 1;
    ReadSettings();
    if (!g_enabled) return;
    const char* src = "";
    Setting("ports_shadow", "FC4_PORTS_SHADOW", &src);
    if (portsReplaced) {
        Log("ports shadow: requested (ports_shadow from %s) but INACTIVE: it compares against the ORIGINAL airport "
            "and seaport developers, and ports is enabled (ours)",
            src);
    } else if (!simulatorReplaced) {
        Log("ports shadow: requested (ports_shadow from %s) but INACTIVE: it follows the originals' months, which "
            "pass through FreeCity4's simulator, and simulator is not enabled",
            src);
    } else {
        Log("ports shadow: ON (ports_shadow from %s): a shadow core per developer (read-only host) starts from the "
            "ORIGINAL's record at its first month and follows the messages of §3; at each SimNewMonth the record's "
            "ports are compared with ours, then after the original's month its history value, GetAvgEfficiency "
            "(bitwise), trips, GetNumPorts, GetAllPortLots, each port lot's usage values, maintenance and "
            "condition, and line 1's full expenses with our RequiredBudget; the seaport's sea map at the first "
            "month and after a terrain message. Not compared: the advisor event and the demand effects (ours "
            "logged)",
            src);
    }
}

bool PortsShadowDeliver(cIGZMessageTarget2* target, const char* agentName, cIGZMessage2* msg, uint32_t type,
                        int64_t data1, bool* finished) {
    if (type != kMsgSimNewMonth || target == nullptr || msg == nullptr || finished == nullptr || !Active()) {
        return false;
    }
    if (target != g_f[0].agent && target != g_f[1].agent &&
        (agentName == nullptr || strcmp(agentName, kAgentName) != 0)) {
        return false;
    }
    cISC4City* city = CurrentCity();
    Follow* which = nullptr;
    for (Follow& f : g_f) {
        cIGZUnknown* sim = OriginalOf(city, f.kind);
        if (sim == nullptr) continue;
        if (sim != f.sim) Track(f, sim);
        if (target == f.agent) which = &f;
    }
    if (which == nullptr) return false;
    Listen();
    Month mo;
    mo.f = which;
    mo.julian = static_cast<int32_t>(data1);
    if (!SehGuard("ports shadow (before the month)", &MonthBefore, &mo)) {
        Log("ports shadow: a fault before the original's month; the shadow core is abandoned");
        which->core = nullptr;
        which->coreSim = nullptr;
        mo.compared = false;
    }
    g_inOriginalMonth = static_cast<int>(which->kind);
    *finished = target->DoMessage(msg);  // the original's month, exactly once
    g_inOriginalMonth = -1;
    if (!SehGuard("ports shadow (after the month)", &MonthAfter, &mo)) {
        Log("ports shadow: a fault after the original's month; the shadow core is abandoned");
        which->core = nullptr;
        which->coreSim = nullptr;
    }
    return true;
}

}  // namespace fc4
