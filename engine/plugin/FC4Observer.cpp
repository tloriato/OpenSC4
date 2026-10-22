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

#include "FC4Observer.h"

#include "FC4AuraShadow.h"
#include "FC4Autoload.h"
#include "FC4Autotest.h"
#include "FC4Checked.h"
#include "FC4Config.h"
#include "FC4Diag.h"
#include "FC4Dialogs.h"
#include "FC4Fpu.h"
#include "FC4Interfaces.h"
#include "FC4Log.h"
#include "FC4Streams.h"
#include "FC4Popups.h"
#include "FC4ProtectionShadow.h"
#include "FC4Script.h"
#include "FC4Subsystems.h"
#include "FC4Turbo.h"
#include "FC4Watchdog.h"
#include "FC4ZonesShadow.h"
#include "FC4ZoneDevShadow.h"
#include "cFC4ZoneDeveloper.h"
#include "cFC4NetworkTool.h"  // the glue29 lane: NetToolRegistry city shutdown
#include "cFC4ZoneManager.h"
#include "FC4TractShadow.h"
#include "FC4CommercialShadow.h"
#include "FC4CivicShadow.h"
#include "FC4ParksShadow.h"
#include "FC4PropDevShadow.h"     // the glue12b1 lane
#include "FC4CityDetailShadow.h"  // the glue12b1 lane
#include "cFC4PropDeveloper.h"    // the glue12b1 lane: the propdev month line
#include "FC4AutomataShadow.h"  // the glue19 lane
#include "FC4SimGridProbe.h"    // the glue-simgrids4 lane
#include "FC4MySimsShadow.h"    // the mysims1 lane: mysims_shadow
#include "FC4AutomataDriver.h"  // the glue19 lane: the riot / fire driver
#include "FC4DisasterShadow.h"
#include "FC4EffectsShadow.h"
#include "FC4Clock24Shadow.h"
#include "FC4HistoryShadow.h"
#include "FC4NetMgrShadow.h"
#include "FC4TerrainShadow.h"  // the glue16 lane
#include "FC4CitySitShadow.h"     // the glue17 lane: citysit_shadow
#include "FC4DemolitionPriceShadow.h"  // the glue23 lane: demolition_price_shadow
#include "FC4DemolitionShadow.h"  // the glue17 lane: demolition_shadow
#include "FC4DispatchShadow.h"    // the glue17 lane: dispatch_shadow
#include "cFC4TractDeveloper.h"
#include "FC4LotDevShadow.h"
#include "FC4OccMgrShadow.h"
#include "FC4PropLotShadow.h"  // the glue15 lane
#include "cFC4LotDeveloper.h"
#include "cFC4Message2Standard.h"
#include "fc4.h"

#include "GZServPtrs.h"
#include "cIGZApp.h"
#include "cIGZFrameWork.h"
#include "cIGZMessage2Standard.h"
#include "cIGZMessageServer2.h"
#include "cIGZMessageTarget2.h"
#include "cIGZSystemService.h"
#include "cISC4App.h"
#include "cISC4City.h"
#include "cISC4DepartmentBudget.h"
#include "cISC4Demand.h"
#include "cISC4DemandSimulator.h"
#include "cISC4ResidentialSimulator.h"
#include "cRZCOMDllDirector.h"
#include "FC4FormatsShadow.h"  // the glue-formats4 lane
#include "FC4Span1555Shadow.h"  // the glue-span1555-impl4 lane
#include "FC4SpanContext.h"  // the glue-span-context-final4 lane
#include "FC4WeatherShadow.h"

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>

#include <algorithm>
#include <cstdarg>
#include <cstdio>
#include <cstring>

namespace fc4 {
namespace {

constexpr uint32_t kIID_cIGZMessageTarget2 = 0x452294AA;
constexpr uint32_t kMsgPostCityInit = 0x26D31EC1;
constexpr uint32_t kMsgSimNewDay = 0x66956814;
constexpr uint32_t kMsgSimNewMonth = 0x66956816;
constexpr uint32_t kMsgConnectionsReady = 0x6AC284F3;  // foundations.md 11.6; popup_dialogs = 0 waits for it
constexpr uint32_t kMsgPreCityShutdown = 0x26D31EC2;   // the autotest's "ended" verdict
constexpr uint32_t kMsgPostCityShutdown = 0x26D31EC3;
// The aura simulator's cycle-completed message (aura.md §11.1). The aura
// shadow (FC4AuraShadow.h) no longer uses it: it is posted and arrives
// later than the original's state 17 (§5.3).
constexpr uint32_t kMsgAuraCycleCompleted = 0x2A09B21A;
constexpr uint32_t kMsgPreSave = 0x26C63343;  // foundations.md §3.7: the prop / lot configuration / tutorial shadows
// The city situation manager's activation message (city-situation.md 3): the
// automata layer's U-Drive-It pause / resume rides on it (automata.md 9).
constexpr uint32_t kMsgSituationActive = 0xABBBCA64;
// foundations.md 3.7 load steps 3 and 5: the class-object store load (every
// saved object's Read) ends before PostLoad and the synchronous PreCityInit, so
// references read in a Read are readable from here on (FC4Streams.h
// NoteUnreadRef). glue-exemplars4.
constexpr uint32_t kMsgPostLoad = 0x26C63342;
constexpr uint32_t kMsgPreCityInit = 0x26D31EC0;
constexpr uint32_t kObserverMessages[] = {kMsgPostCityInit,     kMsgSimNewDay,       kMsgSimNewMonth,
                                         kMsgConnectionsReady, kMsgPreCityShutdown, kMsgPostCityShutdown,
                                         kMsgAuraCycleCompleted, kMsgPreSave, kMsgSituationActive,
                                         kMsgPostLoad, kMsgPreCityInit};
constexpr uint32_t kObserverServiceID = 0x5C7A43E2;  // random, FreeCity4's
constexpr int32_t kObserverPriority = -1000000;      // [choice] tooling; any value works

// Demand IDs (public SC4 modding documentation: the occupant-group / demand
// IDs of the RCI groups), demand index 0 = this city. [choice]
struct DemandId {
    uint32_t id;
    const char* name;
    char group;  // 'R', 'C', 'I'
};
const DemandId kDemands[] = {
    {0x1010, "R$", 'R'},    {0x1020, "R$$", 'R'},   {0x1030, "R$$$", 'R'},  {0x3110, "Cs$", 'C'},
    {0x3120, "Cs$$", 'C'},  {0x3130, "Cs$$$", 'C'}, {0x3320, "Co$$", 'C'},  {0x3330, "Co$$$", 'C'},
    {0x4100, "I-R", 'I'},   {0x4200, "I-D", 'I'},   {0x4300, "I-M", 'I'},   {0x4400, "I-HT", 'I'},
};
constexpr size_t kDemandCount = sizeof(kDemands) / sizeof(kDemands[0]);

cISC4City* g_testCity = nullptr;  // test seam

cISC4City* CurrentCity() {
    if (g_testCity != nullptr) return g_testCity;
    cIGZFrameWork* fw = RZGetFrameWork();
    cIGZApp* app = fw ? fw->Application() : nullptr;
    cISC4App* sc4 = nullptr;
    if (app == nullptr || !app->QueryInterface(GZIID_cISC4App, reinterpret_cast<void**>(&sc4)) || sc4 == nullptr) {
        return nullptr;
    }
    cISC4City* city = sc4->GetCity();
    sc4->Release();
    return city;
}

int64_t DataOf(cIGZMessage2* msg, bool* ok) {
    *ok = false;
    cIGZMessage2Standard* m = nullptr;
    if (!msg->QueryInterface(GZIID_cIGZMessage2Standard, reinterpret_cast<void**>(&m)) || m == nullptr) return 0;
    const int64_t d1 = static_cast<int64_t>(m->GetData1());
    m->Release();
    *ok = true;
    return d1;
}

// ---- the digest ------------------------------------------------------------------------------------

struct DigestCall {
    int32_t julian;
    char line[1024];
};

void AppendF(char* out, size_t cap, const char* fmt, ...) {
    const size_t len = strlen(out);
    if (len + 1 >= cap) return;
    va_list ap;
    va_start(ap, fmt);
    _vsnprintf_s(out + len, cap - len, _TRUNCATE, fmt, ap);
    va_end(ap);
}

// Reads the digest through public interfaces only. Runs under SehGuard.
void DigestStep(void* p) {
    auto* c = static_cast<DigestCall*>(p);
    char* out = c->line;
    const size_t cap = sizeof(c->line);
    fc4_date d{};
    fc4_date_from_julian(c->julian, &d);
    _snprintf_s(out, cap, _TRUNCATE, "DIGEST julian=%d (%04d-%02d-%02d)", c->julian, d.year, d.month, d.day_of_month);
    cISC4City* city = CurrentCity();
    if (city == nullptr) {
        AppendF(out, cap, " no city");
        return;
    }
    cFC4ISC4BudgetSimulator* budget = AsGame(city->GetBudgetSimulator());
    if (budget != nullptr) {
        AppendF(out, cap, " funds=%lld", static_cast<long long>(budget->GetTotalFunds()));
    } else {
        AppendF(out, cap, " funds=-");
    }

    cISC4ResidentialSimulator* res = city->GetResidentialSimulator();
    cISC4DemandSimulator* demandSim = city->GetDemandSimulator();
    float value[kDemandCount] = {};
    bool have[kDemandCount] = {};
    for (size_t i = 0; i < kDemandCount && demandSim != nullptr; ++i) {
        cISC4Demand* dm = demandSim->GetDemand(kDemands[i].id, 0);  // [choice] borrowed pointer (no AddRef)
        if (dm == nullptr) continue;
        have[i] = true;
        value[i] = dm->QueryDemandValue();
    }
    // R = the residential simulator's population; C and I = the supply of
    // the census sum nodes 0x3001 / 0x4001 (demand §10, "population versus
    // jobs"), as the original's advisors read them.
    if (res != nullptr) {
        AppendF(out, cap, " population R/C/I=%ld", static_cast<long>(res->GetPopulation()));
    } else {
        AppendF(out, cap, " population R/C/I=-");
    }
    for (uint32_t census : {0x3001u, 0x4001u}) {
        cISC4Demand* dm = demandSim ? demandSim->GetDemand(census, 0) : nullptr;  // borrowed
        if (dm != nullptr) {
            AppendF(out, cap, "/%.0f", static_cast<double>(dm->QuerySupplyValue()));
        } else {
            AppendF(out, cap, "/-");
        }
    }
    AppendF(out, cap, " demand ");
    for (size_t i = 0; i < kDemandCount; ++i) AppendF(out, cap, "%s%s", i ? "/" : "", kDemands[i].name);
    AppendF(out, cap, "=");
    for (size_t i = 0; i < kDemandCount; ++i) {
        if (have[i]) {
            AppendF(out, cap, "%s%.1f", i ? "/" : "", static_cast<double>(value[i]));
        } else {
            AppendF(out, cap, "%s-", i ? "/" : "");
        }
    }
    if (budget != nullptr) {
        AppendF(out, cap, " budget income/expense=%lld/%lld", static_cast<long long>(budget->GetTotalMonthlyIncome()),
                static_cast<long long>(budget->GetTotalMonthlyExpense()));
    } else {
        AppendF(out, cap, " budget income/expense=-/-");
    }
}

void WriteDigest(int32_t julian) {
    DigestCall c{};
    c.julian = julian;
    const bool clean = SehGuard("monthly digest", &DigestStep, &c);
    Log("%s%s", c.line, clean ? "" : " [INCOMPLETE: structured exception while reading]");
}

// ---- observe: the monthly line and the save/load snapshot ------------------------------------------
// Read only through FC4's interface declarations, at slots checked against
// docs/abi/interfaces.tsv (cISC4App: docs/abi/vtables.tsv, cSC4App::vftable_3;
// it is not in interfaces.tsv):
//   budget 6 GetTotalFunds, 14 GetTotalMonthlyExpense, 16 GetTotalMonthlyIncome,
//     18 GetTaxIncome(int32) (return channel "edx:eax?", low confidence),
//     29 GetAllGroups, 31 GetDepartmentBudgetsInGroup;
//   department budget 3 GetDepartmentID, 10 GetFundingPercentage, 15 GetTotalExpenses;
//   demand simulator 6 GetDemand; demand 8 QuerySupplyValue, 9 QueryDemandValue;
//   residential simulator 16 GetPopulation; simulator 8 GetSimDateNumber;
//   app 16 SaveCity(bool) (pop 4; slot 15 is SaveCity(path, bool)).
// Nothing unverified is called. Not read: the names of the tax types (the
// line gives GetTaxIncome(0..11) by index), and a per-department "monthly"
// expense other than GetTotalExpenses. [choice]

constexpr int32_t kTaxTypes = 12;  // GetTaxIncome(type), types 0..11

cISC4App* g_testApp = nullptr;  // test seam

// The application: a counted reference (released at scope end), or the
// test's fake (borrowed).
struct AppRef {
    cISC4App* app = nullptr;
    bool counted = false;
    AppRef() {
        if (g_testApp != nullptr) {
            app = g_testApp;
            return;
        }
        cIGZFrameWork* fw = RZGetFrameWork();
        cIGZApp* a = fw ? fw->Application() : nullptr;
        cISC4App* sc4 = nullptr;
        if (a != nullptr && a->QueryInterface(GZIID_cISC4App, reinterpret_cast<void**>(&sc4)) && sc4 != nullptr) {
            app = sc4;
            counted = true;
        }
    }
    ~AppRef() {
        if (counted) app->Release();
    }
    AppRef(const AppRef&) = delete;
    AppRef& operator=(const AppRef&) = delete;
};

struct DeptRow {
    uint32_t id;
    int64_t expense;
    float funding;
    bool haveFunding;
};
constexpr size_t kMaxDepts = 96;
struct Depts {
    DeptRow rows[kMaxDepts];
    size_t count;
    bool listed;     // false: no budget, no game allocator, or GetAllGroups failed
    bool truncated;  // more than kMaxDepts
};

// Every department of every budget group, deduplicated by ID and sorted.
// [abi] GetAllGroups / GetDepartmentBudgetsInGroup fill B vectors
// (allocators.md table (2)); the departments come with one reference each,
// released here. Without the game's allocator (unknown executable, tests)
// nothing is listed.
void ReadDepartments(cFC4ISC4BudgetSimulator* budget, Depts& out) {
    out.count = 0;
    out.listed = false;
    out.truncated = false;
    if (budget == nullptr || !StlNodeAllocator::Available()) return;
    GameVector<GameBudgetGroupInfo> groups;
    // [abi-ret] Slots 29 and 31 (interfaces.tsv eax-or-void) return no meaningful value (budget.md §6, the
    // closing rule): the SDK's bool is a leftover AL. Judge by the vectors only.
    budget->GetAllGroups(groups);
    out.listed = true;
    for (GameBudgetGroupInfo& g : groups) {
        GameVector<cISC4DepartmentBudget*> depts;
        budget->GetDepartmentBudgetsInGroup(g.groupID, depts);  // [abi-ret] slot 31: no result (above)
        for (cISC4DepartmentBudget* raw : depts) {
            if (raw == nullptr) continue;
            cFC4ISC4DepartmentBudget* d = AsGame(raw);
            const uint32_t id = d->GetDepartmentID();
            bool seen = false;
            for (size_t i = 0; i < out.count && !seen; ++i) seen = out.rows[i].id == id;
            if (!seen && out.count < kMaxDepts) {
                DeptRow& r = out.rows[out.count++];
                r.id = id;
                r.expense = d->GetTotalExpenses();
                const SC4Percentage* pct = d->GetFundingPercentage();
                r.haveFunding = pct != nullptr;
                r.funding = pct != nullptr ? pct->percentage : 0.0f;
            } else if (!seen) {
                out.truncated = true;
            }
            raw->Release();
        }
    }
    std::sort(out.rows, out.rows + out.count, [](const DeptRow& a, const DeptRow& b) { return a.id < b.id; });
}

void AppendDepts(char* out, size_t cap, const char* key, const Depts& depts, bool funding) {
    AppendF(out, cap, " %s=", key);
    if (!depts.listed) {
        AppendF(out, cap, "-");
    } else if (depts.count == 0) {
        AppendF(out, cap, "none");
    }
    for (size_t i = 0; i < depts.count; ++i) {
        const DeptRow& r = depts.rows[i];
        if (!funding) {
            AppendF(out, cap, "%s0x%08X:%lld", i ? "," : "", r.id, static_cast<long long>(r.expense));
        } else if (r.haveFunding) {
            AppendF(out, cap, "%s0x%08X:%.4f", i ? "," : "", r.id, static_cast<double>(r.funding));
        } else {
            AppendF(out, cap, "%s0x%08X:-", i ? "," : "", r.id);
        }
    }
    if (depts.truncated) AppendF(out, cap, ",...");
}

struct ObserveCall {
    int32_t julian;
    const char* when;  // snapshot only
    char line[3000];
    Depts depts;
};

// observe month YYYY-MM: date=… julian=… funds=… income=… expense=…
// tax[0..11]=… dept_expense=0xID:…,… demand R$/…/I-HT=… population R/C/I=…
// power req/prod/cons/demand=… consumers powered/unpowered=… plants=… aged=…
// distressed=… (the active power simulator's getters, power §10.1 slots 11,
// 12, 17, 15, 22, 21, 24, 28, 29: the same line for the original and ours)
// residential EQ/HQ/LE=… (the active residential simulator's GetGlobalEQ /
// GetGlobalHQ / GetGlobalLE, residential §10.1 slots 32 and 33, x87, and 34,
// AL only; `-` without one). Appended after the power fields, so parsers of
// the earlier fields keep working.
void ObserveMonthStep(void* p) {
    auto* c = static_cast<ObserveCall*>(p);
    char* out = c->line;
    const size_t cap = sizeof(c->line);
    fc4_date d{};
    fc4_date_from_julian(c->julian, &d);
    _snprintf_s(out, cap, _TRUNCATE, "observe month %04d-%02d: date=%04d-%02d-%02d julian=%d", d.year, d.month, d.year,
                d.month, d.day_of_month, c->julian);
    cISC4City* city = CurrentCity();
    if (city == nullptr) {
        AppendF(out, cap, " no city");
        return;
    }
    cFC4ISC4BudgetSimulator* budget = AsGame(city->GetBudgetSimulator());
    if (budget != nullptr) {
        AppendF(out, cap, " funds=%lld income=%lld expense=%lld tax[0..%d]=",
                static_cast<long long>(budget->GetTotalFunds()), static_cast<long long>(budget->GetTotalMonthlyIncome()),
                static_cast<long long>(budget->GetTotalMonthlyExpense()), kTaxTypes - 1);
        for (int32_t t = 0; t < kTaxTypes; ++t) {
            AppendF(out, cap, "%s%lld", t ? "/" : "", static_cast<long long>(budget->GetTaxIncomeOfType(t)));
        }
    } else {
        AppendF(out, cap, " funds=- income=- expense=- tax[0..%d]=-", kTaxTypes - 1);
    }
    ReadDepartments(budget, c->depts);
    AppendDepts(out, cap, "dept_expense", c->depts, false);

    cFC4ISC4DemandSimulator* demandSim = AsGame(city->GetDemandSimulator());
    AppendF(out, cap, " demand ");
    for (size_t i = 0; i < kDemandCount; ++i) AppendF(out, cap, "%s%s", i ? "/" : "", kDemands[i].name);
    AppendF(out, cap, "=");
    for (size_t i = 0; i < kDemandCount; ++i) {
        cFC4ISC4Demand* dm = demandSim ? AsGame(demandSim->GetDemand(kDemands[i].id, 0)) : nullptr;  // borrowed
        if (dm != nullptr) {
            AppendF(out, cap, "%s%.1f", i ? "/" : "", static_cast<double>(dm->QueryDemandValue()));
        } else {
            AppendF(out, cap, "%s-", i ? "/" : "");
        }
    }
    cFC4ISC4ResidentialSimulator* res = AsGame(city->GetResidentialSimulator());
    if (res != nullptr) {
        AppendF(out, cap, " population R/C/I=%ld", static_cast<long>(res->GetPopulation()));
    } else {
        AppendF(out, cap, " population R/C/I=-");
    }
    for (uint32_t census : {0x3001u, 0x4001u}) {
        cFC4ISC4Demand* dm = demandSim ? AsGame(demandSim->GetDemand(census, 0)) : nullptr;  // borrowed
        if (dm != nullptr) {
            AppendF(out, cap, "/%.0f", static_cast<double>(dm->QuerySupplyValue()));
        } else {
            AppendF(out, cap, "/-");
        }
    }
    cFC4ISC4PowerSimulator* power = AsGame(city->GetPowerSimulator());  // borrowed
    if (power != nullptr) {
        AppendF(out, cap,
                " power req/prod/cons/demand=%ld/%ld/%ld/%ld consumers powered/unpowered=%ld/%ld plants=%ld aged=%ld "
                "distressed=%ld",
                static_cast<long>(power->GetMonthlyRequirement()), static_cast<long>(power->GetMonthlyProduction()),
                static_cast<long>(power->GetMonthlyConsumptionTotal()), static_cast<long>(power->GetMonthlyDemand()),
                static_cast<long>(power->GetPoweredConsumerCount()),
                static_cast<long>(power->GetUnpoweredConsumerCount()), static_cast<long>(power->GetPowerPlantCountAll()),
                static_cast<long>(power->GetAgedPowerPlantCount()),
                static_cast<long>(power->GetDistressedPowerPlantCount()));
    } else {
        AppendF(out, cap, " power=-");
    }
    if (res != nullptr) {
        const float eq = res->GetGlobalEQ();     // slot 32 (ST0)
        const float hq = res->GetGlobalHQ();     // slot 33 (ST0)
        const uint32_t le = res->GetGlobalLE();  // slot 34 (AL only; widened here)
        AppendF(out, cap, " residential EQ/HQ/LE=%.4f/%.4f/%u", static_cast<double>(eq), static_cast<double>(hq), le);
    } else {
        AppendF(out, cap, " residential EQ/HQ/LE=-");
    }
}

// observe snapshot (<when>): date=… julian=… funds=… fundings=0xID:pct,…
// (pct = SC4Percentage::percentage as stored; the date from the simulator).
void SnapshotStep(void* p) {
    auto* c = static_cast<ObserveCall*>(p);
    char* out = c->line;
    const size_t cap = sizeof(c->line);
    _snprintf_s(out, cap, _TRUNCATE, "observe snapshot (%s):", c->when);
    cISC4City* city = CurrentCity();
    if (city == nullptr) {
        AppendF(out, cap, " no city");
        return;
    }
    cFC4ISC4Simulator* sim = AsGame(city->GetSimulator());
    if (sim != nullptr) {
        const int32_t j = sim->GetSimDateNumber();
        fc4_date d{};
        fc4_date_from_julian(j, &d);
        AppendF(out, cap, " date=%04d-%02d-%02d julian=%d", d.year, d.month, d.day_of_month, j);
    } else {
        AppendF(out, cap, " date=- julian=-");
    }
    cFC4ISC4BudgetSimulator* budget = AsGame(city->GetBudgetSimulator());
    if (budget != nullptr) {
        AppendF(out, cap, " funds=%lld", static_cast<long long>(budget->GetTotalFunds()));
    } else {
        AppendF(out, cap, " funds=-");
    }
    ReadDepartments(budget, c->depts);
    AppendDepts(out, cap, "fundings", c->depts, true);
}

// Main thread only (the record is static: the department table is large).
void WriteObserve(void (*step)(void*), const char* what, int32_t julian, const char* when) {
    static ObserveCall c;
    c.julian = julian;
    c.when = when;
    c.line[0] = '\0';
    const bool clean = SehGuard(what, step, &c);
    Log("%s%s", c.line, clean ? "" : " [INCOMPLETE: structured exception while reading]");
}

struct SaveCall {
    cISC4App* app;
    bool fast;
    bool result;
};

void SaveStep(void* p) {
    auto* c = static_cast<SaveCall*>(p);
    // [abi-ret] App slots 15/16 (vtables.tsv eax-or-void): no spec gives the result (question HG2); it is only
    // reported (ok=%d in the script log), never acted on.
    c->result = c->app->SaveCity(c->fast);  // the SDK's SaveCity(bool) compiles to slot 16 (pop 4), as the game's
}

// ---- the observer object ----------------------------------------------------------------------------

class cFC4Observer final : public cIGZSystemService, public cIGZMessageTarget2 {
public:
    bool QueryInterface(uint32_t riid, void** ppvObj) override {
        if (ppvObj == nullptr) return false;
        switch (riid) {
            case GZIID_cIGZSystemService: *ppvObj = static_cast<cIGZSystemService*>(this); break;
            case kIID_cIGZMessageTarget2: *ppvObj = static_cast<cIGZMessageTarget2*>(this); break;
            case GZIID_cIGZUnknown: *ppvObj = static_cast<cIGZUnknown*>(static_cast<cIGZSystemService*>(this)); break;
            default: *ppvObj = nullptr; return false;
        }
        AddRef();
        return true;
    }
    // A static object: counted, never deleted.
    uint32_t AddRef() override { return static_cast<uint32_t>(InterlockedIncrement(&refs_)); }
    uint32_t Release() override {
        const LONG r = InterlockedDecrement(&refs_);
        return r > 0 ? static_cast<uint32_t>(r) : 0u;
    }

    // cIGZSystemService
    uint32_t GetServiceID() override { return serviceId_; }
    cIGZSystemService* SetServiceID(uint32_t id) override {
        serviceId_ = id;
        return this;
    }
    int32_t GetServicePriority() override { return kObserverPriority; }
    bool IsServiceRunning() override { return running_; }
    cIGZSystemService* SetServiceRunning(bool running) override {
        running_ = running;
        return this;
    }
    bool Init() override { return true; }
    bool Shutdown() override { return true; }
    bool OnTick(uint32_t) override {
        WatchdogStart();  // once; this is the main thread
        NoteFrameworkTick();
        FpuTick();
        // close_dialogs (FC4Dialogs.h): a modal loop ticks us once per pass
        // (modal-dialogs.md §2.3); before the script and the autotest.
        DialogsOnTick();
        // netmgr_shadow (FC4NetMgrShadow.h): the recorded writes against the game.
        NetMgrShadowTick();
        if (loadSnapshotDue_) {
            loadSnapshotDue_ = false;
            WriteObserve(&SnapshotStep, "observe snapshot", 0, "after load");
        }
        // script = <path>: a `save` and the actions behind it (FC4Script.h);
        // before the save autotest and before AutotestOnTick's quit or end.
        ScriptOnTick();
        DisastersShadowOnTick();  // disasters_shadow: the quiet-point comparison
        fc4::AutomataDriverSituationTick();  // U-Drive-It (automata.md 9): resume when the countdown ends
        if (saveDue_) {
            saveDue_ = false;
            SaveAndEnd();  // does not return
        }
        AutoloadOnTick();
        const int speed = AutotestOnTick();
        if (speed != 0) Unpause(speed);
        return true;
    }
    bool OnIdle(uint32_t) override { return true; }
    int32_t GetServiceTickPriority() override { return kObserverPriority; }

    // cIGZMessageTarget2
    bool DoMessage(cIGZMessage2* msg) override {
        if (msg == nullptr) return true;
        FC4_X87_SCOPE("observer DoMessage", msg->GetType());  // x87 leftover guard (FC4Fpu.h)
        switch (msg->GetType()) {
            case kMsgPostCityInit:
                Log("observer: PostCityInit (%s)", SimulatorReplaced() ? "replacement" : "passthrough");
                FpuLog("city load (PostCityInit)");
                if (!SimulatorReplaced()) SetCityRunning(true);  // ours does it itself
                AutotestOnCityLoaded();
                AutoloadOnCityLoaded();
                PopupDialogsOnCityLoad();
                SimGridProbeAtCityLoaded();  // glue-simgrids4 tooling (simgrids_probe)
                AuraShadowCityLoaded(CurrentCity());  // aura_shadow = 1 only
                ZonesShadowCityLoaded(CurrentCity());  // zones_shadow = 1 only
                TractShadowCityLoaded(CurrentCity());  // tract_shadow = 1 only
                LotDevShadowCityLoaded(CurrentCity());  // lotdev_shadow = 1 only
                ZoneDevShadowCityLoaded(CurrentCity());  // zonedev_shadow = 1 only
                HistoryShadowCityLoaded(CurrentCity());  // history_shadow = 1 only
                Clock24ShadowCityLoaded(CurrentCity());  // clock24_shadow = 1 only
                NetMgrShadowCityLoaded(CurrentCity());   // netmgr_shadow = 1 only
                OccMgrShadowCityLoaded(CurrentCity());   // occmgr_shadow = 1 only
                CityDetailShadowCityLoaded(CurrentCity());  // citydetail_shadow = 1 only
                TerrainShadowCityLoaded(CurrentCity());  // terrain_shadow = 1 only
                DisastersShadowCityLoaded(CurrentCity());  // disasters_shadow = 1 only
                CitySitShadowCityLoaded(CurrentCity());  // the glue17 lane: citysit_shadow = 1 only
                DemolitionShadowCityLoaded(CurrentCity());  // demolition_shadow = 1 only
                DemolitionPriceShadowCityLoaded(CurrentCity());  // the glue23 lane: demolition_price_shadow = 1 only
                DispatchShadowCityLoaded(CurrentCity());  // dispatch_shadow = 1 only
                PropLotShadowCityLoaded(CurrentCity());    // propmgr_shadow / lotcfg_shadow / tutorial_shadow = 1 only
                WeatherShadowCityLoaded(CurrentCity());
                EffectsShadowCityLoaded(CurrentCity());  // effects_shadow = 1 only
                TerrainShadowCityLoaded(CurrentCity());  // terrain_shadow = 1 only
                DisastersShadowCityLoaded(CurrentCity());  // disasters_shadow = 1 only
                AutomataShadowCityLoaded(CurrentCity(), static_cast<int32_t>(PublishedJulian()));  // automata_shadow
                fc4::AutomataDriverCityLoaded(CurrentCity());  // the automata riot / fire driver (automata.md 8)
                fc4::MySimsShadowCityLoaded(CurrentCity());  // mysims_shadow = 1 only
                fc4::FormatsShadowCityLoaded();  // the glue-formats4 lane: formats_shadow = 1 only
                fc4::Span1555CityLoaded();  // glue-span1555-impl4: enable = span1555 only (constructed fixture)
                fc4::SpanContextCityLoaded();  // glue-span-context-final4: span1555 / span4444 on only
                fc4::MySimsShadowMonth(CurrentCity(), static_cast<int32_t>(PublishedJulian()));  // the load comparison
                // The snapshot at the next tick (outside the city's init
                // message), for comparison with the one before a save.
                loadSnapshotDue_ = observe_ || saveAfterMonths_ > 0;
                monthsSinceLoad_ = 0;
                break;
            case kMsgPreSave:
                PropLotShadowPreSave(CurrentCity());  // the records just before the save writes them
                WeatherShadowPreSave();
                break;
            case kMsgConnectionsReady:
                PopupDialogsOnConnectionsReady();
                // Then (popup dialogs handled first) a held turbo request may
                // engage (FC4Turbo.h; tooling).
                TurboOnConnectionsReady();
                break;
            case kMsgAuraCycleCompleted:
                AuraShadowCycle(CurrentCity(), static_cast<int32_t>(PublishedJulian()));  // aura_shadow = 1 only
                break;
            case kMsgSituationActive: {
                // The city situation manager's activation message (data1 of
                // 0xABBBCA64): the automata layer's U-Drive-It pause
                // (automata.md 9; the resume is the tick below).
                bool ok = false;
                const int64_t d1 = DataOf(msg, &ok);
                if (ok) fc4::AutomataDriverSituationMessage(static_cast<int32_t>(d1));
                break;
            }
            case kMsgPreCityShutdown:
                TractShadowCityShutdown();  // tract_shadow: the forwarder's core goes with the city
                LotDevShadowCityShutdown();
                CommercialShadowCityShutdown();  // commercial_shadow: the shadow core goes with the city
                CivicShadowCityShutdown();  // civic_shadow: the shadow core and its references go with the city
                ParksShadowCityShutdown();  // parks_shadow: the same
                NetMgrShadowCityShutdown();  // netmgr_shadow: the shadow cores and their references go
                OccMgrShadowCityShutdown();  // occmgr_shadow: the probe, the shadow core and its references go
                PropDevShadowCityShutdown();  // propdev_shadow: the shadow cores and their references go with the city
                DisastersShadowCityShutdown();  // disasters_shadow: the mirror core and its references go with the city
                CitySitShadowCityShutdown();  // the glue17 lane: citysit_shadow's summary, then the core goes
                DemolitionShadowCityShutdown();
                DemolitionPriceShadowCityShutdown();  // the glue23 lane
                DispatchShadowCityShutdown();
                WeatherShadowCityShutdown();
                PropLotShadowCityShutdown();    // the prop / lot configuration / tutorial shadows' totals
                TerrainShadowCityShutdown();  // terrain_shadow: the totals line, then our terrain goes
                DisastersShadowCityShutdown();  // disasters_shadow: the mirror core and its references
                EffectsShadowCityShutdown();  // effects_shadow: the mirror core goes with the city
                AutomataShadowCityShutdown();  // automata_shadow: the shadow cores and their references
                fc4::AutomataDriverCityShutdown();  // the automata riot / fire driver's per-city state
                NetToolRegistry::LogSummary("city shutdown");
                fc4::MySimsShadowCityShutdown();  // mysims_shadow: the summary line, then the core goes
                NetToolRegistry::CityShutdown();  // the glue29 lane: the shared replacement tools
                AutotestOnCityShutdown(false);
                break;
            case kMsgPostCityShutdown:
                AutotestOnCityShutdown(true);
                ClearUnreadRefs();
                break;
            case kMsgPostLoad:
            case kMsgPreCityInit: {
                const uint32_t n = ClearUnreadRefs();
                if (n != 0) Log("streams: store load done (message 0x%08X), %u references read in Read now readable", msg->GetType(), n);
                break;
            }
            case kMsgSimNewDay: {
                bool ok = false;
                const int64_t j = DataOf(msg, &ok);
                if (ok) PublishJulian(static_cast<int32_t>(j));
                // script = <path>: the actions due today (FC4Script.h), then
                // autotest_end_date and the verdict's day count (FC4Autotest.h).
                ScriptOnNewDay(ok ? static_cast<int32_t>(j) : static_cast<int32_t>(PublishedJulian()));
                AutotestOnNewDay(ok ? static_cast<int32_t>(j) : static_cast<int32_t>(PublishedJulian()));
                break;
            }
            case kMsgSimNewMonth: {
                bool ok = false;
                const int64_t j = DataOf(msg, &ok);
                if (ok) PublishJulian(static_cast<int32_t>(j));
                const int32_t julian = ok ? static_cast<int32_t>(j) : static_cast<int32_t>(PublishedJulian());
                if (digest_) WriteDigest(julian);
                if (observe_) {
                    WriteObserve(&ObserveMonthStep, "observe month", julian, nullptr);
                    FpuLog("observe month");
                }
                ProtectionShadowMonth(CurrentCity(), julian);  // protection_shadow = 1 only
                AutomataShadowMonth(julian);                    // automata_shadow = 1 only
                fc4::ZonesMonthLine(julian);                    // enable = zones only (the month status line)
                ZonesShadowMonth(CurrentCity(), julian);        // zones_shadow = 1 only
                fc4::TractMonthLine(julian);                    // enable = tract only (the month status line)
                TractShadowMonth(CurrentCity(), julian);        // tract_shadow = 1 only
                fc4::LotDevMonthLine(julian);                   // enable = lotdev only (the month status line)
                LotDevShadowMonth(CurrentCity(), julian);       // lotdev_shadow = 1 only
                fc4::ZoneDevMonthLine(julian);                  // enable = zonedev only (the month status line)
                ZoneDevShadowMonth(CurrentCity(), julian);      // zonedev_shadow = 1 only
                CitySitShadowMonth(CurrentCity(), julian);  // the glue17 lane: citysit_shadow = 1 only
                DemolitionPriceShadowMonth(CurrentCity(), julian);  // the glue23 lane
                DemolitionShadowMonth(CurrentCity(), julian);  // demolition_shadow = 1 only
                DispatchShadowMonth(CurrentCity(), julian);    // dispatch_shadow = 1 only
                NetMgrShadowMonth(CurrentCity(), julian);       // netmgr_shadow = 1 only                ++monthsSinceLoad_;
                OccMgrShadowMonth(CurrentCity(), julian);       // occmgr_shadow = 1 only
                PropLotShadowMonth(CurrentCity(), julian);      // propmgr / lotcfg / tutorial shadows only                ++monthsSinceLoad_;
                cFC4PropDeveloper::MonthLine(julian);           // enable = propdev only (the month status line)
                PropDevShadowMonth(CurrentCity(), julian);      // propdev_shadow = 1 only
                fc4::MySimsShadowMonth(CurrentCity(), julian);  // mysims_shadow = 1 only
                ++monthsSinceLoad_;                if (saveAfterMonths_ > 0 && !saveStarted_ && monthsSinceLoad_ >= saveAfterMonths_) {
                    // [choice] Saved at the next framework tick, not inside the
                    // simulator's month-change message.
                    saveStarted_ = true;
                    saveDue_ = true;
                    Log("autotest: %ld month-end(s) since the city load; saving at the next tick", monthsSinceLoad_);
                }
                break;
            }
            default:
                break;
        }
        return true;
    }

    // The settings (game start, before the observer starts; tests have no
    // framework, so they never start it but use these).
    void Reload() {
        settingsRead_ = true;
        digest_ = SettingInt("digest", "FC4_DIGEST", 1) != 0;
        observe_ = SettingInt("observe", "FC4_OBSERVE", kCheckedBuild ? 1 : 0) != 0;
        saveAfterMonths_ = SettingInt("autotest_save_after_months", "FC4_AUTOTEST_SAVE_AFTER_MONTHS", 0);
        if (saveAfterMonths_ < 0) saveAfterMonths_ = 0;
        saveFast_ = SettingInt("autotest_save_fast", "FC4_AUTOTEST_SAVE_FAST", 0) != 0;  // [choice] full save
        Log("observer: digest %s, observe %s", digest_ ? "on" : "off (digest=0)", observe_ ? "on" : "off (observe=0)");
        if (saveAfterMonths_ > 0) {
            Log("autotest: save ON - after %ld month-end(s) of a loaded city: snapshot, SaveCity(%s) in place, "
                "verdict, TerminateProcess",
                saveAfterMonths_, saveFast_ ? "fast" : "full");
        }
    }

    bool Start() {
        if (started_) return true;
        if (!settingsRead_) Reload();
        cIGZFrameWork* fw = RZGetFrameWork();
        if (fw == nullptr) {
            Log("observer: no framework; not started");
            return false;
        }
        cIGZMessageServer2Ptr ms;
        if (!ms) {
            Log("observer: no message server; not started");
            return false;
        }
        AddRef();  // our own reference (static object)
        const bool service = fw->AddSystemService(static_cast<cIGZSystemService*>(this));
        const bool tick = fw->AddToTick(static_cast<cIGZSystemService*>(this));
        bool notes = true;
        for (uint32_t id : kObserverMessages) {
            notes = ms->AddNotification(static_cast<cIGZMessageTarget2*>(this), id) && notes;
        }
        started_ = true;
        Log("observer: started (service %d, tick %d, notifications %d; digest %s; observe %s; %s mode)",
            service ? 1 : 0, tick ? 1 : 0, notes ? 1 : 0, digest_ ? "on" : "off (digest=0)",
            observe_ ? "on" : "off (observe=0)", SimulatorReplaced() ? "replacement" : "passthrough");
        return true;
    }

    void Stop() {
        if (!started_) return;
        started_ = false;
        cIGZMessageServer2Ptr ms;
        if (ms) {
            for (uint32_t id : kObserverMessages) {
                ms->RemoveNotification(static_cast<cIGZMessageTarget2*>(this), id);
            }
        }
        if (cIGZFrameWork* fw = RZGetFrameWork()) {
            fw->RemoveFromTick(static_cast<cIGZSystemService*>(this));
            fw->RemoveSystemService(static_cast<cIGZSystemService*>(this));
        }
        Release();
        Log("observer: stopped");
    }

private:
    // Runs the loaded city at `speed`, unpaused, through the simulator's
    // public interface (ours or the original).
    static void Unpause(int speed) {
        cISC4City* city = CurrentCity();
        cFC4ISC4Simulator* sim = city ? AsGame(city->GetSimulator()) : nullptr;
        if (sim == nullptr) {
            Log("autotest: unpause skipped: no city simulator");
            return;
        }
        int resumes = 0;
        while (sim->IsPaused() && resumes < 8) {
            if (!sim->Resume()) break;
            ++resumes;
        }
        const bool setSpeed = sim->GetSimSpeed() != speed;
        if (setSpeed) sim->SetSimSpeed(speed);
        Log("autotest: unpause: %d Resume call(s), speed %d%s; paused=%d hidden=%d emergency=%d", resumes,
            sim->GetSimSpeed(), setSpeed ? " (set)" : "", sim->IsPaused() ? 1 : 0, sim->IsHiddenPaused() ? 1 : 0,
            sim->IsEmergencyPaused() ? 1 : 0);
    }

    // The save autotest: snapshot, SaveCity in place, verdict, TerminateProcess.
    [[noreturn]] void SaveAndEnd() {
        WriteObserve(&SnapshotStep, "observe snapshot", 0, "before save");
        char reason[160];
        AppRef app;
        if (app.app == nullptr) {
            _snprintf_s(reason, sizeof(reason), _TRUNCATE, "save after %ld month(s) NOT done: no cISC4App",
                        saveAfterMonths_);
        } else {
            SaveCall call{app.app, saveFast_, false};
            Log("autotest: SaveCity(%s) ...", saveFast_ ? "fast" : "full");
            const DWORD start = GetTickCount();
            const bool clean = SehGuard("SaveCity", &SaveStep, &call);
            _snprintf_s(reason, sizeof(reason), _TRUNCATE, "save after %ld month(s): SaveCity(%s) returned %d in %lu ms%s",
                        saveAfterMonths_, saveFast_ ? "fast" : "full", call.result ? 1 : 0,
                        static_cast<unsigned long>(GetTickCount() - start),
                        clean ? "" : " [structured exception]");
        }
        AutotestFinish(reason, "observer (save autotest)");
    }

    volatile LONG refs_ = 0;
    uint32_t serviceId_ = kObserverServiceID;
    bool running_ = true;
    bool started_ = false;
    bool settingsRead_ = false;
    bool digest_ = true;
    bool observe_ = false;
    long saveAfterMonths_ = 0;  // autotest_save_after_months
    bool saveFast_ = false;
    long monthsSinceLoad_ = 0;
    bool loadSnapshotDue_ = false;
    bool saveStarted_ = false;
    bool saveDue_ = false;
};

cFC4Observer& Observer() {
    static cFC4Observer s;
    return s;
}

}  // namespace

void ObserverReload() { Observer().Reload(); }
void ObserverStart() { Observer().Start(); }
void ObserverStop() { Observer().Stop(); }

void ObserverTestSetCity(cISC4City* city) {
    g_testCity = city;
    Log("test seam: observer city %p", static_cast<void*>(city));
}
void ObserverTestSetApp(cISC4App* app) {
    g_testApp = app;
    Log("test seam: observer app %p", static_cast<void*>(app));
}
void ObserverTestTick() { Observer().OnTick(0); }
void ObserverTestMessage(uint32_t type, intptr_t data1) {
    cFC4Message2Standard* msg = FC4MakeMessage(type, data1, 0, 0);
    Observer().DoMessage(msg);
    msg->Release();
}

// The harness lane's script (FC4Script.h).
cISC4City* ObserverCity() { return CurrentCity(); }
void ObserverWriteDigest(int32_t julian) { WriteDigest(julian); }
ObserverSave ObserverSaveCity(bool fast) {
    ObserverSave r;
    AppRef app;  // the test seam's app (kFC4SeamObserverApp), else the framework's (one reference)
    if (app.app == nullptr) return r;
    r.haveApp = true;
    SaveCall call{app.app, fast, false};
    const DWORD start = GetTickCount();
    r.clean = SehGuard("script SaveCity", &SaveStep, &call);
    r.ms = static_cast<unsigned long>(GetTickCount() - start);
    r.result = call.result;
    return r;
}

}  // namespace fc4
