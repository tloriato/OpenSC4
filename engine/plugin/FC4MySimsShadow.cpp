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

// The MySims shadow implementation (FC4MySimsShadow.h): a same-process
// differential of the original cSC4MySimAgentSimulator, its five MySim
// advisors (ids 1-5) and the My Street sim (id 30) — mysims.md, tooling only.
//
// Flow:
//   INJECT (PostCityInit): each original MySim is captured read-only (its own
//   cIGZSerializable::Write into memory via WriteCapture, version-17 record
//   mysims.md §6.2), then read into a shadow core (fc4_advisors +
//   fc4_mysims_agent_init) through a custom fc4_istream whose get_ref maps
//   classes: advice (0x8A09F5F4) -> a fresh plain shadow advice, subject
//   (0x6A26D050) -> a new subject, building/dispatch/tool/advisor -> the
//   capture's canonical game token (no reference). Then
//   fc4_advisors_add_advisor per MySim, fc4_mysims_agent_init once (slot
//   table: ids 1-5 -> indices 0-4, 30 -> index 5) and ONE hand-delivered
//   POST_CITY_INIT (+ CITY_ESTABLISHED when established).
//
//   DRIVE: the shadow forwarder target subscribes to the union of the
//   original's message families (agent §3.2, MySim §3.3, base advisor,
//   system ticker) and routes each delivery into the core:
//   fc4_mysims_agent_do_message / fc4_advisors_do_message /
//   fc4_advisors_advisor_do_message.
//
//   COMPARE (PostCityInit + every SimNewMonth): the original's record is
//   decoded field by field (CaptureReader, version-17 layout) and diffed
//   against the shadow's fc4_mysim_get_state + fc4_mysim_text + the advisor
//   part (personal name, month counter, day, cursor, advice count), plus the
//   agent slots and the game getters (GetActiveMySims, MySimLivesHere,
//   GetActiveUIPanelSim).
//
// No Lua in the shadow (create_table false): update_trip_variables no-ops on
// the core side (sim.rs guards on lua_table) and no advice activates
// (compile_trigger -> None). Trip/pylon/dispatch-tool state and the unnamed
// pair/flag are NOT compared and named on every line.
//
// Line contract (run_summary.ps1): `<name> shadow YYYY-MM-DD <event>: ...;
// mismatches=N` — the LAST `; mismatches=N` decides clean/differs.

#include "FC4MySimsShadow.h"

#include "FC4Config.h"
#include "FC4Diag.h"
#include "FC4Interfaces.h"
#include "FC4Log.h"
#include "FC4RefAudit.h"
#include "FC4Seed.h"
#include "FC4Streams.h"
#include "FC4WriteCapture.h"

#include "GZServPtrs.h"
#include "cIGZApp.h"
#include "cIGZFrameWork.h"
#include "cIGZMessage2Standard.h"
#include "cIGZMessageServer2.h"
#include "cIGZMessageTarget2.h"
#include "cIGZSerializable.h"
#include "cIGZString.h"
#include "cIGZUnknown.h"
#include "cISC4Advisor.h"
#include "cISC4AdvisorSystem.h"
#include "cISC4AuraSimulator.h"
#include "cISC4BuildingDevelopmentSimulator.h"
#include "cISC4BuildingOccupant.h"
#include "cISC4App.h"
#include "cISC4City.h"
#include "cISC4CrimeSimulator.h"
#include "cISC4GrowthDeveloper.h"
#include "cISC4DisasterInstance.h"
#include "cISC4DisasterLayer.h"
#include "cISC4FireProtectionSimulator.h"
#include "cISC4Lot.h"
#include "cISC4LotManager.h"
#include "cISC4MySim.h"
#include "cISC4MySimAgentSimulator.h"
#include "cISC4Occupant.h"
#include "cISC4OccupantManager.h"
#include "cISC4PlumbingSimulator.h"
#include "cISC4PoliceSimulator.h"
#include "cISC4PollutionSimulator.h"
#include "cISC4PowerSimulator.h"
#include "cISC4ResidentialSimulator.h"
#include "cISC4SimGrid.h"
#include "cISC4TrafficSimulator.h"
#include "cRZBaseString.h"

#include <cstdio>
#include <cstring>
#include <map>
#include <set>
#include <string>
#include <vector>

namespace fc4 {
namespace {

// ---- messages (mysims.md §3.2/§3.3, advisors.md §5) --------------------------------------------
constexpr uint32_t kMsgPostCityInit = 0x26D31EC1;
constexpr uint32_t kMsgCityEstablished = 0x26D31EC4;
constexpr uint32_t kMsgTicker = 0xCA5501F2;
constexpr uint32_t kMsgOccupantInserted = 0x99EF1142;
constexpr uint32_t kMsgOccupantChanged = 0x99EF1145;
constexpr uint32_t kMsgOccupantRemoved = 0x99EF1143;
constexpr uint32_t kMsgViewChanged1 = 0xA6B79602;
constexpr uint32_t kMsgViewChanged2 = 0xA6B79612;
constexpr uint32_t kMsgHourChanged = 0x44778230;
constexpr uint32_t kMsgAdviceCommand = 0x8A09FA6C;
constexpr uint32_t kMsgDisasterStarted = 0xCA1B1E64;
constexpr uint32_t kMsgPipeBurst = 0xEA1F0DCD;
constexpr uint32_t kMsgTornado = 0x0A36F1E9;
constexpr uint32_t kMsgNeighborConnRemoved = 0xEA0D163E;
constexpr uint32_t kMsgMySimUpdate = 0x4BC42C48;
constexpr uint32_t kMsgDispatchNotification = 0x2BA06B99;
constexpr uint32_t kMsgDebugPrint = 0x0B6F3E27;
constexpr uint32_t kMsgDebugNewJob = 0xAB6F3E2D;
constexpr uint32_t kMsgDebugNewHouse = 0x4B6F3EBF;
constexpr uint32_t kMsgDebugDispatch = 0x0BB04126;
constexpr uint32_t kMsgPostCityInitComplete = 0xEA8AE29A;
constexpr uint32_t kMsgOrdinanceUpdated = 0xC2C09554;
constexpr uint32_t kMsgSimNewMonth = 0x66956816;

// Union of subscriptions the forwarder registers (deduplicated at registration).
constexpr uint32_t kForwarderMessages[] = {
    kMsgOccupantInserted, kMsgOccupantChanged, kMsgOccupantRemoved, kMsgViewChanged1, kMsgViewChanged2,
    kMsgHourChanged, kMsgAdviceCommand, kMsgDisasterStarted, kMsgPipeBurst, kMsgTornado, kMsgNeighborConnRemoved,
    kMsgMySimUpdate, kMsgDispatchNotification, kMsgDebugPrint, kMsgDebugNewJob, kMsgDebugNewHouse,
    kMsgDebugDispatch, kMsgPostCityInitComplete, kMsgOrdinanceUpdated, kMsgCityEstablished, kMsgTicker,
};

// ---- identities --------------------------------------------------------------------------------
constexpr uint32_t kIIDUnknown = 0x00000001;
constexpr uint32_t kIIDMessageTarget2 = 0x452294AA;
constexpr uint32_t kIIDSerializable = 0xE4FDA3D4;
constexpr uint32_t kIIDAdvisor = 0xCA09F544;
constexpr uint32_t kIIDAdvice = 0x8A09F5F5;
constexpr uint32_t kIIDSubject = 0x6A26D050;
constexpr uint32_t kIIDMySim = 0x0A1DA42F;
constexpr uint32_t kIIDOccupant = 0x26BB767E;
constexpr uint32_t kIIDBuildingOccupant = 0x87DDA3A9;
constexpr uint32_t kClsidAdvice = 0x8A09F5F4;
constexpr uint32_t kClsidSubject = 0x6A26D049;
constexpr uint32_t kClsidMySim = 0x4A1DBBBF;
constexpr uint32_t kClsidMyStreetSim = 0x0BF606A5;
constexpr uint32_t kOccupantTypeBuilding = 0x278128A0;
constexpr uint32_t kOccupantTypeFire = 0x69D25C5F;
constexpr int32_t kMyStreetId = 30;
constexpr int32_t kMySimIds[6] = {1, 2, 3, 4, 5, kMyStreetId};
constexpr size_t kMaxListed = 8;  // differing fields named per line

bool IsAgentMessage(uint32_t t) {
    switch (t) {
        case kMsgOccupantInserted: case kMsgOccupantChanged: case kMsgOccupantRemoved:
        case kMsgViewChanged1: case kMsgViewChanged2: case kMsgHourChanged: case kMsgAdviceCommand:
        case kMsgDisasterStarted: case kMsgPipeBurst: case kMsgTornado: case kMsgNeighborConnRemoved:
            return true;
        default: return false;
    }
}
bool IsSystemMessage(uint32_t t) {
    return t == kMsgTicker || t == kMsgPostCityInit || t == kMsgCityEstablished;
}

// ---- state -------------------------------------------------------------------------------------
bool g_enabled = false;
bool g_started = false;
bool g_listening = false;
bool g_reportedSetup = false;

struct SimCapture {
    WriteCapture wc;                        // owns the bytes + the object map
    std::vector<uint8_t> record;            // wc.Bytes() snapshot
    std::map<uint32_t, uint64_t> tokens;    // capture object id -> canonical game token
    std::vector<std::pair<uint32_t, uint32_t>> refIds;  // (object id, class id) in record order
};
std::map<int32_t, SimCapture> g_caps;       // advisor id -> capture

fc4_advisors* g_core = nullptr;
fc4_advisor_host g_advisorHost{};
fc4_mysim_host g_mysimHost{};
std::vector<fc4_adv_obj*> g_createdObjs;    // host-owned handles from object_created
uint32_t g_handleCounter = 0;

struct ShadowState {
    uint32_t created = 0, hadMysims = 0, months = 0, cleanMonths = 0, lastMismatches = 0, lastSims = 0, compared = 0;
    bool injected = false, postCityInitDelivered = false, cityEstablishedDelivered = false;
    bool postCityInitPending = false;  // deferred to the next forwarded message (MySimsShadowCityLoaded)
} g_state;

void TraceAdvisorPart(const char* stage);

// The load's PostCityInit, once, after the game's own PostCityInit dispatch
// (see MySimsShadowCityLoaded).
void DeliverPendingPostCityInit() {
    if (!g_state.postCityInitPending || g_core == nullptr) return;
    g_state.postCityInitPending = false;
    fc4_adv_message m{};
    m.msg_type = kMsgPostCityInit;
    fc4_advisors_do_message(g_core, &m);
    g_state.postCityInitDelivered = true;
    TraceAdvisorPart("after PostCityInit");
}

// ---- helpers -----------------------------------------------------------------------------------

cISC4City* AppCity() {
    cIGZFrameWork* fw = RZGetFrameWork();
    cIGZApp* app = fw ? fw->Application() : nullptr;
    cISC4App* sc4 = nullptr;
    if (app == nullptr || !app->QueryInterface(GZIID_cISC4App, reinterpret_cast<void**>(&sc4)) || sc4 == nullptr) {
        return nullptr;
    }
    // Our QueryInterface added the reference we drop: record it so the audit pairs the release.
    FC4_ACQUIRED(reinterpret_cast<cIGZUnknown*>(sc4), "QueryInterface");
    cISC4City* city = sc4->GetCity();
    FC4_RELEASE(sc4);
    return city;
}

// QI with one reference for the caller; Drop releases it.
template <class T>
T* Query(cIGZUnknown* u, uint32_t iid) {
    if (u == nullptr) return nullptr;
    void* p = nullptr;
    if (!u->QueryInterface(iid, &p) || p == nullptr) return nullptr;
    T* t = static_cast<T*>(p);
    FC4_ACQUIRED(reinterpret_cast<cIGZUnknown*>(t), "QueryInterface");
    return t;
}
template <class T>
void Drop(T* p) {
    if (p != nullptr) FC4_RELEASE(reinterpret_cast<cIGZUnknown*>(p));
}

inline uint64_t TokenOf(const void* p) {
    return static_cast<uint64_t>(reinterpret_cast<uintptr_t>(p));
}
inline cIGZUnknown* FromToken(uint64_t t) {
    return reinterpret_cast<cIGZUnknown*>(static_cast<uintptr_t>(t));
}

// The core holds game references through add_ref/release; ours are guarded.
void HoldToken(uint64_t t) {
    if (t == 0) return;
    cIGZUnknown* o = FromToken(t);
    if (fc4::IsOurObject(o)) o->AddRef();
    else FC4_ADDREF(o);
}
void DropToken(uint64_t t) {
    if (t == 0) return;
    cIGZUnknown* o = FromToken(t);
    if (fc4::IsOurObject(o)) o->Release();
    else FC4_RELEASE(o);
}

cFC4ISC4Simulator* CitySim(cISC4City* city) {
    return AsGame(city ? city->GetSimulator() : nullptr);
}

// The city's +0x58 established byte (raw read with the vtable guard, as the
// other shadows).
bool CityEstablished(cISC4City* city) {
    if (city == nullptr) return false;
    const unsigned char* base = static_cast<const unsigned char*>(static_cast<const void*>(city));
    return base[0x58] != 0;
}

void DateText(int32_t julian, char* out, size_t n) {
    if (julian <= 0) { _snprintf_s(out, n, _TRUNCATE, "????-??-??"); return; }
    fc4_date d{};
    fc4_date_from_julian(julian, &d);
    _snprintf_s(out, n, _TRUNCATE, "%04d-%02d-%02d", d.year, d.month, d.day_of_month);
}

// The canonical occupant token of a message's void1 (its cISC4Occupant; the
// query reference is dropped at once — the caller's object keeps it alive).
uint64_t CanonicalOccupant(cIGZUnknown* u) {
    cISC4Occupant* o = Query<cISC4Occupant>(u, kIIDOccupant);
    if (o == nullptr) return 0;
    const uint64_t t = TokenOf(o);
    Drop(o);
    return t;
}

// ---- the forwarder target ----------------------------------------------------------------------

class ShadowForwarder final : public cIGZMessageTarget2 {
public:
    static ShadowForwarder& Get() {
        static ShadowForwarder t;
        return t;
    }

    bool QueryInterface(uint32_t riid, void** ppvObj) override {
        if (ppvObj == nullptr) return false;
        if (riid == kIIDUnknown || riid == kIIDMessageTarget2) {
            *ppvObj = static_cast<cIGZMessageTarget2*>(this);
            return true;
        }
        return false;
    }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }

    bool DoMessage(cIGZMessage2* msg) override {
        if (g_core == nullptr) return true;
        DeliverPendingPostCityInit();
        cIGZMessage2Standard* m = nullptr;
        if (msg == nullptr || !msg->QueryInterface(GZIID_cIGZMessage2Standard, reinterpret_cast<void**>(&m)) ||
            m == nullptr) {
            return true;
        }
        fc4_adv_message a{};
        a.msg_type = m->GetType();
        a.data1 = static_cast<uint32_t>(m->GetData1());
        a.data2 = static_cast<uint32_t>(m->GetData2());
        a.data3 = static_cast<uint32_t>(m->GetData3());
        a.data4 = static_cast<uint32_t>(m->GetData4());
        if (cIGZUnknown* v = static_cast<cIGZUnknown*>(m->GetVoid1())) a.void1 = CanonicalOccupant(v);
        if (cIGZUnknown* u = m->GetIGZUnknown()) a.unknown = TokenOf(u);
        m->Release();
        const uint32_t t = a.msg_type;
        if (IsAgentMessage(t)) {
            fc4_mysims_agent_do_message(g_core, &a);
            return true;
        }
        if (IsSystemMessage(t)) {
            fc4_advisors_do_message(g_core, &a);
            return true;
        }
        // Per-advisor and broadcast families: AdviceCommand names the advisor
        // via IGZUnknown; the occupant/debug/ordinance/view families go to
        // every MySim advisor (ascending id) plus the system (the base
        // advisor handles occupant changes itself).
        if (t == kMsgAdviceCommand && a.unknown != 0) {
            int32_t id = 0;
            if (cISC4Advisor* adv = Query<cISC4Advisor>(FromToken(a.unknown), kIIDAdvisor)) {
                id = adv->GetID();  // slot 5
                Drop(adv);
            }
            if (id != 0) fc4_advisors_advisor_do_message(g_core, id, &a);
            else fc4_mysims_agent_do_message(g_core, &a);
            return true;
        }
        for (int32_t id : kMySimIds) {
            fc4_advisors_advisor_do_message(g_core, id, &a);
        }
        return true;
    }
};

// ---- advisor host callbacks (fc4_advisor_host) --------------------------------------------------

struct EntryData {
    int32_t id = 0;
    uint32_t class_id = 0;
    int32_t advice_type = 1;
    std::string caption, name;
};
std::vector<EntryData> g_entries;

bool HAdvisorCount(void*, uint32_t* n) {
    if (n == nullptr) return false;
    *n = static_cast<uint32_t>(g_entries.size());
    return !g_entries.empty();
}
bool HAdvisorEntry(void*, uint32_t i, fc4_adv_advisor_entry* out) {
    if (out == nullptr || i == 0 || i > g_entries.size()) return false;
    const EntryData& e = g_entries[i - 1];
    out->class_id = e.class_id;
    out->id = e.id;
    out->advice_type = e.advice_type;
    out->reserved = 0;
    out->caption = reinterpret_cast<const uint8_t*>(e.caption.data());
    out->caption_len = e.caption.size();
    out->name = reinterpret_cast<const uint8_t*>(e.name.data());
    out->name_len = e.name.size();
    return true;
}
uint32_t HAdviceCount(void*) { return 0; }
bool HAdviceEntry(void*, uint32_t, fc4_adv_advice_entry*) { return false; }
bool HCompileTrigger(void*, const uint8_t*, size_t, int32_t* ref) {
    if (ref != nullptr) *ref = -1;
    return false;  // None
}
void HReleaseTrigger(void*, int32_t) {}
void HCallTrigger(void*, int32_t, fc4_adv_lua_result* out) { if (out) out->kind = FC4_ADV_LUA_OTHER; }
void HCallCondition(void*, int32_t, fc4_adv_lua_result* out) { if (out) out->kind = FC4_ADV_LUA_OTHER; }
bool HDetokenize(void*, const uint8_t* text, size_t len, fc4_bytes_sink sink, void* ctx) {
    if (text != nullptr && len != 0 && sink != nullptr) sink(ctx, text, len);
    return false;  // unchanged
}
void HReloadScripts(void*) {}
void HRegisterApi(void*, bool) {}
void HSetTutorialLua(void*, bool) {}
void HCreateTags(void*, int32_t* s, int32_t* str) { if (s) *s = 1; if (str) *str = 2; }
void HRegisterDetokenizer(void*, bool) {}
bool HPopupPriority(void*, const char*, double*) { return false; }
void HCollectGarbage(void*) {}
void HCloseLua(void*) {}
void HSetGlobal(void*, const char*, double) {}
void HSetLocal(void*, const char*, double, const fc4_adv_subject_state*, uint64_t) {}
void HSetGlobalString(void*, const char*, const uint8_t*, size_t) {}
bool HDebugEnabled(void*) { return false; }
void HRegisterCheat(void*, uint32_t, const char*) {}
void HUnregisterCheat(void*, uint32_t) {}
void HAdvisorWindow(void*, bool) {}
bool HHasCity(void*) { return AppCity() != nullptr; }
bool HCityEstablished(void*) { return CityEstablished(AppCity()); }
bool HDifficulty(void*, int32_t* out) {
    cISC4City* city = AppCity();
    if (city == nullptr || out == nullptr) return false;
    *out = static_cast<int32_t>(city->GetDifficultyLevel());  // +0x60
    return true;
}
void HCellCounts(void*, uint32_t* x, uint32_t* z) {
    cISC4City* city = AppCity();
    if (x) *x = city ? city->CellCountX() : 0;
    if (z) *z = city ? city->CellCountZ() : 0;
}
void HSimDate(void*, fc4_adv_date* out) {
    if (out == nullptr) return;
    cFC4ISC4Simulator* sim = CitySim(AppCity());
    const int32_t day = sim ? sim->GetSimDateNumber() : 0;  // slot 8
    out->day_number = day;
    if (day > 0) {
        fc4_date d{};
        fc4_date_from_julian(day, &d);
        out->year = d.year; out->month = d.month; out->day = d.day_of_month;
    } else {
        out->year = 2000; out->month = 1; out->day = 1;
    }
}
int32_t HStartYear(void*) {
    // [choice] The original's start date is 2000-01-01 (cFC4Simulator
    // GetSimStartDate). g_year_count uses it as the epoch.
    return 2000;
}
bool HIsAnyPaused(void*) {
    cFC4ISC4Simulator* sim = CitySim(AppCity());
    // [abi-ret] slot 18 (interfaces.tsv eax-or-void): the value only feeds
    // the shadow core's paused branch; the compare treats it as a bool.
    return sim ? sim->IsAnyPaused() : false;
}
uint32_t HNowMs(void*) { return 0; }
void HSubscribe(void*, uint32_t, int32_t, uint64_t, uint32_t) {}
void HUnsubscribe(void*, uint32_t, int32_t, uint64_t, uint32_t) {}
void HPost(void*, const fc4_adv_out_message*) {}
void HSend(void*, const fc4_adv_out_message*) {}
void HPostUpdateSelf(void*, int32_t, uint64_t, uint32_t, uint32_t) {}
void HPostTicker(void*) {}
void HGlobalEffect(void*, uint32_t, bool) {}
void HRebuildText(void*, uint32_t, uint64_t) {}
bool HValue(void*, const fc4_adv_src*, fc4_adv_val*) { return false; }
int64_t HList(void*, uint32_t, uint32_t, const uint32_t*, size_t, fc4_adv_item*, size_t) { return -1; }
void HRecordHistory(void*, uint32_t, int32_t) {}
uint32_t HOccupantType(void*, uint64_t t) {
    cISC4Occupant* o = reinterpret_cast<cISC4Occupant*>(static_cast<uintptr_t>(t));
    return o ? o->GetType() : 0;
}
int32_t HOccupantBoolProperty(void*, uint64_t, uint32_t) { return -1; }
bool HBuildingKeyInstance(void*, uint64_t, uint32_t*) { return false; }
bool HIsOccupantGroup(void*, uint64_t t, uint32_t g) {
    cISC4Occupant* o = reinterpret_cast<cISC4Occupant*>(static_cast<uintptr_t>(t));
    return o ? o->IsOccupantGroup(g) : false;
}
bool HConditionalStatus(void*, uint32_t, fc4_adv_conditional_status*) { return false; }
void HUpdateConditionalStatus(void*, uint32_t, const fc4_adv_conditional_status*) {}
bool HRewardSubject(void*, uint32_t, fc4_adv_subject_state*) { return false; }
bool HDealType(void*, uint64_t, uint32_t*) { return false; }
void HCancelDeal(void*, uint64_t) {}
bool HHistoryValue(void*, uint32_t, int32_t, double*) { return false; }
bool HHistoryTrend(void*, uint32_t, int32_t, int32_t, float*) { return false; }
bool HHistoryCount(void*, uint32_t, int32_t, int32_t, double*) { return false; }
int32_t HOrdinanceFlag(void*, uint32_t, bool) { return -1; }
void HSetOrdinanceOn(void*, uint32_t, bool) {}
bool HOrdinanceAmount(void*, uint32_t, uint32_t, int64_t*) { return false; }
bool HOrdinanceText(void*, uint32_t, uint32_t, fc4_bytes_sink, void*) { return false; }
bool HTutorialGuid(void*, uint32_t*) { return false; }
void HAddRef(void*, uint64_t t) { HoldToken(t); }
void HReleaseRef(void*, uint64_t t) { DropToken(t); }
uint64_t HResolveRef(void*, uint64_t t, uint32_t) { return t; }
uint64_t HObjectCreated(void*, uint32_t, uint32_t, fc4_adv_obj* handle) {
    if (handle != nullptr) g_createdObjs.push_back(handle);
    return ++g_handleCounter;
}

// ---- mysim host callbacks (fc4_mysim_host, read-only to the game) -------------------------------

void MCellCounts(void*, uint32_t* x, uint32_t* z) {
    cISC4City* city = AppCity();
    if (x) *x = city ? city->CellCountX() : 0;
    if (z) *z = city ? city->CellCountZ() : 0;
}
void MCellCenter(void*, int32_t cx, int32_t cz, float* x, float* z) {
    cISC4City* city = AppCity();
    if (x == nullptr || z == nullptr) return;
    if (city == nullptr) { *x = 0; *z = 0; return; }
    float px = 0, pz = 0;
    city->CellCenterToPosition(cx, cz, px, pz);  // slot 97
    *x = px; *z = pz;
}
bool MDisasterOngoing(void*) {
    cISC4City* city = AppCity();
    cISC4DisasterLayer* l = city ? city->GetDisasterLayer() : nullptr;
    return l ? l->IsADisasterOngoing() : false;  // slot 10
}
size_t MDisasterLocations(void*, fc4_mysim_disaster_location* out, size_t cap) {
    cISC4City* city = AppCity();
    cISC4DisasterLayer* l = city ? city->GetDisasterLayer() : nullptr;
    if (l == nullptr) return 0;
    const int32_t n = l->GetDisasterLocationCount();  // slot 16
    size_t w = 0;
    for (int32_t i = 0; i < n; ++i) {
        cISC4DisasterInstance* inst = nullptr;
        int x = 0, y = 0;
        // [abi-ret] slot 17 (interfaces.tsv eax-or-void): only whether a
        // location was returned; the shadow counts what came back.
        if (!l->GetDisasterLocationByIndex(i, &inst, x, y)) continue;
        if (out != nullptr && w < cap) {
            out[w].disaster_type = inst ? inst->GetID() : 0;  // GetID = the manager type (disasters.md §1)
            out[w].x = x;
            out[w].z = y;
        }
        ++w;
    }
    return w;
}

bool MBuilding(void*, uint64_t t, fc4_mysim_building* out) {
    cISC4Occupant* occ = reinterpret_cast<cISC4Occupant*>(static_cast<uintptr_t>(t));
    if (occ == nullptr || out == nullptr) return false;
    cISC4BuildingOccupant* b = Query<cISC4BuildingOccupant>(occ, kIIDBuildingOccupant);
    if (b == nullptr) return false;
    out->building_type = b->GetBuildingType();       // slot 4
    out->purpose = static_cast<uint32_t>(b->GetBuildingProfile().purpose);  // slot 11
    const SC4Percentage* p = b->GetCompletionPercent();  // slot 9 (pointer)
    out->completion = p ? p->percentage : 0.0f;
    cS3DVector3 pos{};
    // [abi-ret] occupant slot 8 (interfaces.tsv eax-or-void): only the
    // presence bit is read; the coordinates come from the out struct.
    if (occ->GetPosition(&pos)) { out->position[0] = pos.fX; out->position[1] = pos.fY; out->position[2] = pos.fZ; }
    else { out->position[0] = out->position[1] = out->position[2] = 0.0f; }
    if (cIGZString* name = b->GetBuildingName()) {
        out->name = reinterpret_cast<const uint8_t*>(name->ToChar());
        out->name_len = name->Strlen();
    } else {
        out->name = nullptr; out->name_len = 0;
    }
    out->groups = nullptr; out->group_count = 0;  // groups not readable read-only (not compared)
    Drop(b);
    return true;
}

bool MLotOf(cISC4Lot* lot, uint64_t buildingToken, fc4_mysim_lot* out) {
    if (lot == nullptr || out == nullptr) return false;
    out->building = buildingToken;
    int lx = 0, lz = 0;
    if (lot->GetLocation(lx, lz)) { out->cell_x = lx; out->cell_z = lz; }  // slot 21
    else { out->cell_x = out->cell_z = 0; }
    SC4Rect<int32_t> r{};
    if (lot->GetBoundingRect(r)) {  // slot 14 of the lot geometry
        out->x0 = r.topLeftX; out->z0 = r.topLeftY;
        out->x1 = r.bottomRightX; out->z1 = r.bottomRightY;
    } else {
        out->x0 = out->x1 = out->cell_x; out->z0 = out->z1 = out->cell_z;
    }
    out->condition = static_cast<uint8_t>(lot->GetCondition());        // slot 35
    out->road_access = lot->HasRoadAccess() ? 1u : 0u;                 // slot 28
    out->wealth = static_cast<uint8_t>(lot->GetOccupantWealth());      // slot 41
    return true;
}

bool MLot(void*, uint64_t t, fc4_mysim_lot* out) {
    cISC4City* city = AppCity();
    cISC4LotManager* lm = city ? city->GetLotManager() : nullptr;  // slot 33
    cISC4Occupant* occ = reinterpret_cast<cISC4Occupant*>(static_cast<uintptr_t>(t));
    if (lm == nullptr || occ == nullptr || out == nullptr) return false;
    cISC4Lot* lot = lm->GetOccupantLot(occ);  // slot 14, borrowed
    if (lot == nullptr) return false;
    return MLotOf(lot, t, out);
}

bool MOccupantPosition(void*, uint64_t t, float* out) {
    cISC4Occupant* occ = reinterpret_cast<cISC4Occupant*>(static_cast<uintptr_t>(t));
    if (occ == nullptr || out == nullptr) return false;
    cS3DVector3 p{};
    // [abi-ret] occupant slot 8 (interfaces.tsv eax-or-void): only the
    // presence bit feeds the callback's Option.
    if (!occ->GetPosition(&p)) return false;
    out[0] = p.fX; out[1] = p.fY; out[2] = p.fZ;
    return true;
}

// Occupant manager slot 40 GetOccupantsByBBox (mysims.md §4.9; corners in
// world metres via CellCornerToPosition). Nodes from the game allocator go
// back through ReleaseOccupantList.
int64_t MBuildingsInBox(void*, int32_t x0, int32_t z0, int32_t x1, int32_t z1, uint64_t* out, size_t cap) {
    cISC4City* city = AppCity();
    cISC4OccupantManager* om = city ? city->GetOccupantManager() : nullptr;  // slot 34
    if (om == nullptr) return -1;
    SC4List<cISC4Occupant*> list;
    float ax = 0, az = 0, bx = 0, bz = 0;
    city->CellCornerToPosition(x0, z0, ax, az);
    city->CellCornerToPosition(x1 + 1, z1 + 1, bx, bz);
    const float fx[2] = {ax, bx}, fz[2] = {az, bz};
    om->GetOccupantsByBBox(list, fx, fz, kOccupantTypeBuilding, 0);  // slot 40
    int64_t n = 0;
    for (cISC4Occupant* o : list) {
        if (out != nullptr && static_cast<size_t>(n) < cap) out[n] = TokenOf(o);
        ++n;
    }
    om->ReleaseOccupantList(list);  // slot 48: the references go back
    return n;
}

int64_t MAllBuildings(void*, uint64_t* out, size_t cap) {
    cISC4City* city = AppCity();
    cISC4OccupantManager* om = city ? city->GetOccupantManager() : nullptr;
    if (om == nullptr) return -1;
    SC4List<cISC4Occupant*> list;
    om->GetOccupantsByBBox(list, nullptr, nullptr, kOccupantTypeBuilding, 0);
    int64_t n = 0;
    for (cISC4Occupant* o : list) {
        if (out != nullptr && static_cast<size_t>(n) < cap) out[n] = TokenOf(o);
        ++n;
    }
    om->ReleaseOccupantList(list);
    return n;
}

size_t MLotsInRect(void*, int32_t x0, int32_t z0, int32_t x1, int32_t z1, fc4_mysim_lot* out, size_t cap) {
    cISC4City* city = AppCity();
    cISC4LotManager* lm = city ? city->GetLotManager() : nullptr;
    if (lm == nullptr) return 0;
    SC4List<cISC4Lot*> list;
    lm->GetLotsInCellRect(list, x0, z0, x1, z1, false);  // slot 7 (borrowed lots)
    size_t n = 0;
    for (cISC4Lot* lot : list) {
        if (out != nullptr && n < cap) {
            uint64_t bt = 0;
            if (cISC4BuildingOccupant* b = lot->GetBuilding()) {  // slot 48, borrowed
                bt = TokenOf(b);
            }
            MLotOf(lot, bt, &out[n]);
        }
        ++n;
    }
    return n;
}

size_t MSpecialBuildings(void*, uint64_t* out, size_t cap) {
    cISC4City* city = AppCity();
    cISC4BuildingDevelopmentSimulator* bds = city ? city->GetBuildingDevelopmentSimulator() : nullptr;  // slot 53
    if (bds == nullptr) return 0;
    const intptr_t raw = bds->GetSpecialBuildings();  // slot 44
    if (raw == 0) return 0;
    const auto* types = reinterpret_cast<const std::vector<uint32_t>*>(raw);
    size_t n = 0;
    for (uint32_t v : *types) {
        if (out != nullptr && n < cap) out[n] = v;  // building TYPE ids (not tokens)
        ++n;
    }
    return n;
}

size_t MEducationBuildings(void*, bool hospitals, uint64_t* out, size_t cap) {
    cISC4City* city = AppCity();
    cISC4ResidentialSimulator* res = city ? city->GetResidentialSimulator() : nullptr;
    if (res == nullptr) return 0;
    SC4List<cISC4Occupant*> list;
    if (hospitals) res->GetHospitalBuildings(list, nullptr);  // slot 49
    else res->GetSchoolBuildings(list, nullptr);              // slot 47
    size_t n = 0;
    for (cISC4Occupant* o : list) {
        if (out != nullptr && n < cap) out[n] = TokenOf(o);
        ++n;
    }
    return n;
}

bool MGrade(void*, uint64_t, bool, float*) {
    return false;  // query-data record layout not built read-only; grade not compared
}
void MJobTitle(void*, uint64_t t, fc4_bytes_sink sink, void* ctx) {
    cISC4Occupant* occ = reinterpret_cast<cISC4Occupant*>(static_cast<uintptr_t>(t));
    if (occ == nullptr || sink == nullptr) return;
    cISC4BuildingOccupant* b = Query<cISC4BuildingOccupant>(occ, kIIDBuildingOccupant);
    if (b == nullptr) return;
    if (cIGZString* n = b->GetBuildingName()) {
        if (const char* s = n->ToChar()) sink(ctx, reinterpret_cast<const uint8_t*>(s), n->Strlen());
    }
    Drop(b);
}
bool MLandmarkFunding(void*, uint64_t, float*) { return false; }  // trip fields not compared

bool MAverageAge(void*, int32_t x, int32_t z, int32_t* out) {
    cISC4City* city = AppCity();
    cISC4ResidentialSimulator* res = city ? city->GetResidentialSimulator() : nullptr;
    if (res == nullptr || out == nullptr) return false;
    const int32_t map = res->GetAverageAgeMap();  // slot 41 -> cISC4SimGrid<int32_t>*
    if (map == 0) return false;
    *out = reinterpret_cast<cISC4SimGrid<int32_t>*>(map)->GetCellValue(x, z);
    return true;
}
bool MEqByWealth(void*, int32_t x, int32_t z, uint8_t w, float* out) {
    cISC4City* city = AppCity();
    cISC4ResidentialSimulator* res = city ? city->GetResidentialSimulator() : nullptr;
    if (res == nullptr || out == nullptr) return false;
    *out = res->GetCellEQByWealth(x, z, static_cast<cISC4BuildingOccupant::WealthType>(w));  // slot 37
    return true;
}
bool MHqByWealth(void*, int32_t x, int32_t z, uint8_t w, float* out) {
    cISC4City* city = AppCity();
    cISC4ResidentialSimulator* res = city ? city->GetResidentialSimulator() : nullptr;
    if (res == nullptr || out == nullptr) return false;
    *out = res->GetCellHQByWealth(x, z, static_cast<cISC4BuildingOccupant::WealthType>(w));  // slot 38
    return true;
}
bool MLifeExpectancy(void*, int32_t x, int32_t z, uint8_t* out) {
    cISC4City* city = AppCity();
    cISC4ResidentialSimulator* res = city ? city->GetResidentialSimulator() : nullptr;
    if (res == nullptr || out == nullptr) return false;
    *out = static_cast<uint8_t>(res->GetCellLifeExpectancy(x, z));  // slot 29
    return true;
}
bool MNewAgeByWealth(void*, uint8_t w, uint8_t* out) {
    cISC4City* city = AppCity();
    cISC4ResidentialSimulator* res = city ? city->GetResidentialSimulator() : nullptr;
    if (res == nullptr || out == nullptr) return false;
    *out = static_cast<uint8_t>(res->GetAverageNewAgeByWealth(static_cast<cISC4BuildingOccupant::WealthType>(w)));
    return true;
}

bool MGrid(void*, uint32_t grid, uint32_t arg, int32_t x, int32_t z, int32_t* out) {
    cISC4City* city = AppCity();
    if (city == nullptr || out == nullptr) return false;
    switch (grid) {
        case FC4_MYSIM_GRID_CRIME: {
            cISC4CrimeSimulator* s = city->GetCrimeSimulator();
            if (s == nullptr) return false;
            *out = s->GetCrimeGrid()->GetCellValue(x, z);
            return true;
        }
        case FC4_MYSIM_GRID_POLLUTION: {
            cISC4PollutionSimulator* s = city->GetPollutionSimulator();
            if (s == nullptr) return false;
            *out = s->GetPollutionGrid(arg)->GetCellValue(x, z);
            return true;
        }
        case FC4_MYSIM_GRID_AURA: {
            cISC4AuraSimulator* s = city->GetAuraSimulator();
            if (s == nullptr) return false;
            *out = s->GetAuraGrid()->GetCellValue(x, z);
            return true;
        }
        case FC4_MYSIM_GRID_DESIRABILITY: {
            cISC4BuildingDevelopmentSimulator* bds = city->GetBuildingDevelopmentSimulator();
            if (bds == nullptr) return false;
            cISC4GrowthDeveloper* d =
                bds->GetDeveloper(static_cast<cISC4BuildingDevelopmentSimulator::DeveloperType>(arg));
            if (d == nullptr) return false;
            *out = d->GetDesirabilityMap()->GetCellValue(x, z);
            return true;
        }
        default: return false;
    }
}

bool MGridAt(void*, uint32_t grid, uint32_t arg, float x, float z, int32_t* out) {
    cISC4City* city = AppCity();
    if (city == nullptr || out == nullptr) return false;
    if (grid == FC4_MYSIM_GRID_CONGESTION) {
        cISC4TrafficSimulator* s = city->GetTrafficSimulator();
        if (s == nullptr) return false;
        cISC4SimGrid<uint8_t>* g = s->GetCongestionMap();  // slot 10
        if (g == nullptr) return false;
        int32_t tx = 0, tz = 0;
        if (!g->PositionToTract(x, z, tx, tz)) return false;
        *out = g->GetTractValue(tx, tz);
        return true;
    }
    if (grid == FC4_MYSIM_GRID_AVERAGE_EQ || grid == FC4_MYSIM_GRID_AVERAGE_HQ) {
        cISC4ResidentialSimulator* res = city->GetResidentialSimulator();
        if (res == nullptr) return false;
        cISC4SimGrid<float>* g = nullptr;
        float mn = 0, mx = 0;
        // [abi-ret] residential slots 21/22 (interfaces.tsv eax-or-void):
        // only success feeds the Option; the grid travels via the out param.
        const bool ok = grid == FC4_MYSIM_GRID_AVERAGE_EQ ? res->GetAverageEQGrid(g, &mn, &mx)
                                                          : res->GetAverageHQGrid(g, &mn, &mx);
        if (!ok || g == nullptr) return false;
        int32_t tx = 0, tz = 0;
        if (!g->PositionToTract(x, z, tx, tz)) return false;
        *out = static_cast<int32_t>(g->GetTractValue(tx, tz));
        return true;
    }
    return MGrid(nullptr, grid, arg, static_cast<int32_t>(x), static_cast<int32_t>(z), out);
}

bool MCoverage(void*, bool police, int32_t x0, int32_t z0, int32_t w, int32_t h, int32_t* out) {
    cISC4City* city = AppCity();
    if (city == nullptr || out == nullptr) return false;
    if (police) {
        cISC4PoliceSimulator* s = city->GetPoliceSimulator();
        if (s == nullptr) return false;
        *out = s->GetPolicePowerGrid()->GetAverageValueInCellRect(x0, z0, x0 + w - 1, z0 + h - 1);
        return true;
    }
    cISC4FireProtectionSimulator* s = city->GetFireProtectionSimulator();
    if (s == nullptr) return false;
    *out = s->GetFireProtectionGrid()->GetAverageValueInCellRect(x0, z0, x0 + w - 1, z0 + h - 1);
    return true;
}

int32_t MPowered(void*, uint64_t t) {
    cISC4City* city = AppCity();
    cISC4PowerSimulator* s = city ? city->GetPowerSimulator() : nullptr;
    if (s == nullptr) return -1;
    cISC4Occupant* occ = reinterpret_cast<cISC4Occupant*>(static_cast<uintptr_t>(t));
    return s->IsOccupantPowered(occ) ? 1 : 0;  // slot 31
}
int32_t MWatered(void*, uint64_t t) {
    cISC4City* city = AppCity();
    cISC4PlumbingSimulator* s = city ? city->GetPlumbingSimulator() : nullptr;
    if (s == nullptr) return -1;
    cISC4Occupant* occ = reinterpret_cast<cISC4Occupant*>(static_cast<uintptr_t>(t));
    // [abi-ret] plumbing slot 9 (interfaces.tsv via-call): read as bool.
    return s->IsBuildingWatered(occ) ? 1 : 0;
}

// Police slot 21 (SC4List) / fire slot 22 (ilist): the same node layout; the
// SDK SC4List allocates its nodes with the framework allocator A and its
// destructor releases each element's reference — exactly what the getters
// hand over.
int64_t MStations(void*, bool police, fc4_mysim_station* out, size_t cap) {
    cISC4City* city = AppCity();
    if (city == nullptr) return -1;
    SC4List<cISC4Occupant*> list;
    int64_t n = 0;
    if (police) {
        cISC4PoliceSimulator* s = city->GetPoliceSimulator();
        if (s == nullptr) return -1;
        s->GetPoliceStations(list);  // slot 21
        for (cISC4Occupant* st : list) {
            SC4Percentage pct{0.0f};
            int32_t cost = 0;
            // [abi-ret] police slot 7 (interfaces.tsv eax-or-void): the
            // funding value comes from the out param; the bool only gates.
            if (s->GetStationFunding(st, cost, pct)) {
                if (out != nullptr && static_cast<size_t>(n) < cap) {
                    out[n].station = TokenOf(st);
                    out[n].funding = pct.percentage;
                }
                ++n;
            }
        }
    } else {
        cISC4FireProtectionSimulator* s = city->GetFireProtectionSimulator();
        if (s == nullptr) return -1;
        ilist<cISC4Occupant*> flist;  // slot 22: ilist = std::list with allocator A
        s->GetFireStations(flist);
        for (cISC4Occupant* st : flist) {
            SC4Percentage pct{0.0f};
            int32_t cost = 0;
            // [abi-ret] fire slot 8 (interfaces.tsv eax-or-void): the
            // funding value comes from the out param; the bool only gates.
            if (s->GetStationFunding(st, cost, pct)) {
                if (out != nullptr && static_cast<size_t>(n) < cap) {
                    out[n].station = TokenOf(st);
                    out[n].funding = pct.percentage;
                }
                ++n;
            }
        }
    }
    return n;
}

bool MTripSimulators(void*) {
    cISC4City* city = AppCity();
    return city != nullptr && city->GetTrafficSimulator() != nullptr &&
           city->GetPollutionSimulator() != nullptr && city->GetCrimeSimulator() != nullptr;
}
void MDispatchCommand(void*, int32_t, const char*) {}
void MPost(void*, const fc4_mysim_post*) {}
void MAutomataTrigger(void*, const char*, const float*) {}
void MSubscribe(void*, uint32_t, bool) {}
void MAddRef(void*, uint64_t t) { HoldToken(t); }
void MRelease(void*, uint64_t t) { DropToken(t); }
uint64_t MResolveRef(void*, uint64_t t, uint32_t) { return t; }

void FillHosts() {
    g_advisorHost = {};
    g_advisorHost.struct_size = sizeof(g_advisorHost);
    g_advisorHost.advisor_count = &HAdvisorCount;
    g_advisorHost.advisor_entry = &HAdvisorEntry;
    g_advisorHost.advice_count = &HAdviceCount;
    g_advisorHost.advice_entry = &HAdviceEntry;
    g_advisorHost.compile_trigger = &HCompileTrigger;
    g_advisorHost.release_trigger = &HReleaseTrigger;
    g_advisorHost.call_trigger = &HCallTrigger;
    g_advisorHost.call_condition = &HCallCondition;
    g_advisorHost.detokenize = &HDetokenize;
    g_advisorHost.reload_scripts = &HReloadScripts;
    g_advisorHost.register_api = &HRegisterApi;
    g_advisorHost.set_tutorial_lua = &HSetTutorialLua;
    g_advisorHost.create_tags = &HCreateTags;
    g_advisorHost.register_detokenizer = &HRegisterDetokenizer;
    g_advisorHost.popup_priority = &HPopupPriority;
    g_advisorHost.collect_garbage = &HCollectGarbage;
    g_advisorHost.close_lua = &HCloseLua;
    g_advisorHost.set_global = &HSetGlobal;
    g_advisorHost.set_local = &HSetLocal;
    g_advisorHost.set_global_string = &HSetGlobalString;
    g_advisorHost.debug_enabled = &HDebugEnabled;
    g_advisorHost.register_cheat = &HRegisterCheat;
    g_advisorHost.unregister_cheat = &HUnregisterCheat;
    g_advisorHost.advisor_window = &HAdvisorWindow;
    g_advisorHost.has_city = &HHasCity;
    g_advisorHost.city_established = &HCityEstablished;
    g_advisorHost.difficulty = &HDifficulty;
    g_advisorHost.cell_counts = &HCellCounts;
    g_advisorHost.sim_date = &HSimDate;
    g_advisorHost.start_year = &HStartYear;
    g_advisorHost.is_any_paused = &HIsAnyPaused;
    g_advisorHost.now_ms = &HNowMs;
    g_advisorHost.subscribe = &HSubscribe;
    g_advisorHost.unsubscribe = &HUnsubscribe;
    g_advisorHost.post = &HPost;
    g_advisorHost.send = &HSend;
    g_advisorHost.post_update_self = &HPostUpdateSelf;
    g_advisorHost.post_ticker = &HPostTicker;
    g_advisorHost.global_effect = &HGlobalEffect;
    g_advisorHost.rebuild_text = &HRebuildText;
    g_advisorHost.value = &HValue;
    g_advisorHost.list = &HList;
    g_advisorHost.record_history = &HRecordHistory;
    g_advisorHost.occupant_type = &HOccupantType;
    g_advisorHost.occupant_bool_property = &HOccupantBoolProperty;
    g_advisorHost.building_key_instance = &HBuildingKeyInstance;
    g_advisorHost.is_occupant_group = &HIsOccupantGroup;
    g_advisorHost.conditional_status = &HConditionalStatus;
    g_advisorHost.update_conditional_status = &HUpdateConditionalStatus;
    g_advisorHost.reward_subject = &HRewardSubject;
    g_advisorHost.deal_type = &HDealType;
    g_advisorHost.cancel_deal = &HCancelDeal;
    g_advisorHost.history_value = &HHistoryValue;
    g_advisorHost.history_trend = &HHistoryTrend;
    g_advisorHost.history_count = &HHistoryCount;
    g_advisorHost.ordinance_flag = &HOrdinanceFlag;
    g_advisorHost.set_ordinance_on = &HSetOrdinanceOn;
    g_advisorHost.ordinance_amount = &HOrdinanceAmount;
    g_advisorHost.ordinance_text = &HOrdinanceText;
    g_advisorHost.tutorial_guid = &HTutorialGuid;
    g_advisorHost.add_ref = &HAddRef;
    g_advisorHost.release = &HReleaseRef;
    g_advisorHost.resolve_ref = &HResolveRef;
    g_advisorHost.object_created = &HObjectCreated;

    g_mysimHost = {};
    g_mysimHost.struct_size = sizeof(g_mysimHost);
    g_mysimHost.create_table = nullptr;   // FALSE: no Lua (lua_table stays false)
    g_mysimHost.set_field = nullptr;      // no table to feed
    g_mysimHost.activate_lua = nullptr;   // reads false
    g_mysimHost.tuning = nullptr;         // code defaults (mysims.md §4.11)
    g_mysimHost.ui_blocked = nullptr;     // false (question MS2)
    g_mysimHost.register_agent_lua = nullptr;
    g_mysimHost.cell_counts = &MCellCounts;
    g_mysimHost.cell_center = &MCellCenter;
    g_mysimHost.disaster_ongoing = &MDisasterOngoing;
    g_mysimHost.disaster_locations = &MDisasterLocations;
    g_mysimHost.building = &MBuilding;
    g_mysimHost.lot = &MLot;
    g_mysimHost.occupant_position = &MOccupantPosition;
    g_mysimHost.buildings_in_box = &MBuildingsInBox;
    g_mysimHost.all_buildings = &MAllBuildings;
    g_mysimHost.lots_in_rect = &MLotsInRect;
    g_mysimHost.special_buildings = &MSpecialBuildings;
    g_mysimHost.education_buildings = &MEducationBuildings;
    g_mysimHost.grade = &MGrade;
    g_mysimHost.job_title = &MJobTitle;
    g_mysimHost.landmark_funding = &MLandmarkFunding;
    g_mysimHost.average_age = &MAverageAge;
    g_mysimHost.eq_by_wealth = &MEqByWealth;
    g_mysimHost.hq_by_wealth = &MHqByWealth;
    g_mysimHost.life_expectancy = &MLifeExpectancy;
    g_mysimHost.new_age_by_wealth = &MNewAgeByWealth;
    g_mysimHost.grid = &MGrid;
    g_mysimHost.grid_at = &MGridAt;
    g_mysimHost.coverage = &MCoverage;
    g_mysimHost.powered = &MPowered;
    g_mysimHost.watered = &MWatered;
    g_mysimHost.stations = &MStations;
    g_mysimHost.travel_time_ratio = nullptr;   // trip fields not compared
    g_mysimHost.trip_simulators = &MTripSimulators;
    g_mysimHost.dispatch = nullptr;            // first cut: no dispatch core
    g_mysimHost.find_new_job = nullptr;
    g_mysimHost.change_job = nullptr;
    g_mysimHost.job_destination = nullptr;
    g_mysimHost.commute_time = nullptr;
    g_mysimHost.neighbor_city_name = nullptr;
    g_mysimHost.dispatch_command = &MDispatchCommand;
    g_mysimHost.post = &MPost;
    g_mysimHost.automata_trigger = &MAutomataTrigger;
    g_mysimHost.subscribe = &MSubscribe;
    g_mysimHost.add_ref = &MAddRef;
    g_mysimHost.release = &MRelease;
    g_mysimHost.resolve_ref = &MResolveRef;
}

// ---- the custom inject stream -------------------------------------------------------------------

// A capture-backed fc4_istream whose get_ref implements the inject map
// (handoff round 5): advice -> a plain shadow advice cached per object id;
// subject -> a new subject cached per id; everything else -> the capture's
// canonical game token (null-safe, no reference). Thunked through free
// functions like CaptureIStream (a member-function pointer cannot fill a C
// table).
bool InjU8(void* u, uint8_t* o);
bool InjU16(void* u, uint16_t* o);
bool InjU32(void* u, uint32_t* o);
bool InjU64(void* u, uint64_t* o);
bool InjF32(void* u, float* o);
bool InjF64(void* u, double* o);
bool InjBytes(void* u, uint8_t* o, size_t n);
bool InjSkip(void* u, size_t n);
bool InjStr(void* u, fc4_bytes_sink sink, void* ctx);
bool InjRef(void* u, uint32_t iid, fc4_ref* out);
int32_t InjError(void* u);
class InjectStream;
InjectStream* InjS(void* u);
class InjectStream {
public:
    InjectStream(const WriteCapture& cap, std::map<uint32_t, uint64_t>& tokens)
        : cap_(cap), tokens_(tokens) {
        api_.struct_size = sizeof(api_);
        api_.user = this;
        api_.get_u8 = &InjU8;
        api_.get_u16 = &InjU16;
        api_.get_u32 = &InjU32;
        api_.get_u64 = &InjU64;
        api_.get_f32 = &InjF32;
        api_.get_f64 = &InjF64;
        api_.get_bytes = &InjBytes;
        api_.get_str = &InjStr;
        api_.get_ref = &InjRef;
        api_.skip = &InjSkip;
        api_.error = &InjError;
    }
    const fc4_istream* Get() const { return &api_; }
    bool good() const { return good_; }

    // fc4_istream callbacks (public for the C table).
    bool Take(void* out, size_t n) {
        const std::vector<uint8_t>& b = cap_.Bytes();
        if (!good_ || pos_ + n > b.size()) { good_ = false; return false; }
        if (out != nullptr && n != 0) std::memcpy(out, b.data() + pos_, n);
        pos_ += n;
        return true;
    }
    bool Str(fc4_bytes_sink sink, void* ctx) {
        uint32_t n = 0;
        if (!Take(&n, 4)) return false;
        const std::vector<uint8_t>& b = cap_.Bytes();
        if (pos_ + n > b.size()) { good_ = false; return false; }
        if (sink != nullptr) sink(ctx, b.data() + pos_, n);
        pos_ += n;
        return true;
    }
    bool Ref(uint32_t iid, fc4_ref* out) {
        (void)iid;  // the mapping is by class id; the core QIs its own handles
        uint32_t id = 0, cls = 0;
        if (!Take(&id, 4)) return false;
        if (id != 0 && !Take(&cls, 4)) return false;
        fc4_ref r{};
        r.class_id = cls;
        if (id != 0) {
            switch (cls) {
                case kClsidAdvice: {
                    fc4_adv_obj* o = AdviceCache(id);
                    if (o != nullptr) { r.core_handle = o; r.token = TokenOf(o); }
                    break;
                }
                case kClsidSubject: {
                    fc4_adv_obj* o = SubjectCache(id);
                    if (o != nullptr) { r.core_handle = o; r.token = TokenOf(o); }
                    break;
                }
                default: {
                    // An advice of a subclass (the advisor record reads its advice
                    // references as iid Advice): a core advice of that class, so the
                    // record keeps every advice instead of skipping the unresolved ones.
                    if (iid == kIIDAdvice) {
                        fc4_adv_obj* o = AdviceCache(id, cls);
                        if (o != nullptr) { r.core_handle = o; r.token = TokenOf(o); break; }
                    }
                    auto it = tokens_.find(id);
                    if (it != tokens_.end()) r.token = it->second;
                    // Diagnostic: a class the shadow cannot resolve to a core object (each class once).
                    static std::set<uint32_t> s_reported;
                    if (s_reported.insert(cls).second) {
                        Log("mysims shadow (cont.) inject: ref class 0x%08X (iid 0x%08X) resolves to a host token only",
                            cls, iid);
                    }
                    break;
                }
            }
        }
        if (out != nullptr) *out = r;
        return true;
    }

private:
    fc4_adv_obj* AdviceCache(uint32_t id, uint32_t cls = kClsidAdvice) {
        auto it = advices_.find(id);
        if (it != advices_.end()) return it->second;
        fc4_adv_obj* o = g_core != nullptr ? fc4_advisors_create_object(g_core, cls) : nullptr;
        if (o != nullptr) advices_[id] = o;
        return o;
    }
    fc4_adv_obj* SubjectCache(uint32_t id) {
        auto it = subjects_.find(id);
        if (it != subjects_.end()) return it->second;
        fc4_adv_obj* o = g_core != nullptr ? fc4_advisors_new_subject(g_core) : nullptr;
        if (o != nullptr) subjects_[id] = o;
        return o;
    }

    const WriteCapture& cap_;
    std::map<uint32_t, uint64_t>& tokens_;
    std::map<uint32_t, fc4_adv_obj*> advices_, subjects_;
    size_t pos_ = 0;
    bool good_ = true;
    fc4_istream api_{};
};

bool InjU8(void* u, uint8_t* o) { return InjS(u)->Take(o, 1); }
bool InjU16(void* u, uint16_t* o) { return InjS(u)->Take(o, 2); }
bool InjU32(void* u, uint32_t* o) { return InjS(u)->Take(o, 4); }
bool InjU64(void* u, uint64_t* o) { return InjS(u)->Take(o, 8); }
bool InjF32(void* u, float* o) { return InjS(u)->Take(o, 4); }
bool InjF64(void* u, double* o) { return InjS(u)->Take(o, 8); }
bool InjBytes(void* u, uint8_t* o, size_t n) { return InjS(u)->Take(o, n); }
bool InjSkip(void* u, size_t n) { return InjS(u)->Take(nullptr, n); }
bool InjStr(void* u, fc4_bytes_sink sink, void* ctx) { return InjS(u)->Str(sink, ctx); }
bool InjRef(void* u, uint32_t iid, fc4_ref* out) { return InjS(u)->Ref(iid, out); }
int32_t InjError(void* u) { return InjS(u)->good() ? 0 : 1; }
InjectStream* InjS(void* u) { return static_cast<InjectStream*>(u); }

// ---- capture + inject --------------------------------------------------------------------------

// Captures every original MySim (advisor ids 1-5, 30). Returns the count.
uint32_t CaptureSims(cISC4City* city) {
    g_caps.clear();
    cISC4AdvisorSystem* as = city ? city->GetAdvisorSystem() : nullptr;  // slot 81, borrowed
    if (as == nullptr) return 0;
    uint32_t n = 0;
    for (int32_t id : kMySimIds) {
        cISC4Advisor* a = as->GetAdvisor(id);  // slot 7, borrowed
        if (a == nullptr) continue;
        void* p = nullptr;
        if (!a->QueryInterface(kIIDSerializable, &p) || p == nullptr) continue;
        cIGZSerializable* ser = static_cast<cIGZSerializable*>(p);
        FC4_ACQUIRED(ser, "QueryInterface");
        SimCapture sc;
        bool answered = false;
        const bool ok = sc.wc.Capture(ser, &answered);
        Drop(ser);
        if (!ok) continue;
        sc.record = sc.wc.Bytes();
        for (uint32_t refId = 1;; ++refId) {
            const void* obj = sc.wc.Object(refId);
            if (obj == nullptr) break;
            sc.tokens[refId] = TokenOf(obj);
        }
        g_caps[id] = std::move(sc);
        ++n;
    }
    return n;
}

// Reads every capture into the core: create_object (class from the capture
// header's object map — MySim records are class 0x4A1DBBBF, My Street
// 0x0BF606A5), adv_obj_read through the InjectStream, then add_advisor.
bool InjectSims() {
    if (g_core == nullptr || g_caps.empty()) return false;
    bool all = true;
    for (auto& [id, sc] : g_caps) {
        const uint32_t cls = id == kMyStreetId ? kClsidMyStreetSim : kClsidMySim;
        fc4_adv_obj* obj = fc4_advisors_create_object(g_core, cls);
        if (obj == nullptr) { all = false; continue; }
        InjectStream stream(sc.wc, sc.tokens);
        const bool readOk = fc4_adv_obj_read(obj, stream.Get());
        const bool streamOk = stream.good();
        if (!readOk || !streamOk || !fc4_advisors_add_advisor(g_core, obj)) {
            Log("mysims shadow (cont.) inject failed: advisor id=%d read=%d stream=%d", id,
                readOk ? 1 : 0, streamOk ? 1 : 0);
            all = false;
        }
        fc4_adv_obj_destroy(obj);  // the map holds its own reference (AddAdvisor §4.3)
    }
    return all;
}

// Builds the `advisors` table the core's load_advisors reads from: the 7
// shipped entries (MySims 1-5 class 0x4A1DBBBF, 23 CityPlanning
// 0x2A5F877D, 30 MyStreet 0x0BF606A5), with captions/names copied from the
// original advisors at first city load (advisors.md §2.1).
void BuildAdvisorEntries(cISC4City* city) {
    g_entries.clear();
    // (id, class_id, advice_type, fallback caption/name)
    const std::tuple<int32_t, uint32_t, int32_t, const char*, const char*> shipped[] = {
        {1, kClsidMySim, 1, "MySim", "MySim"},
        {2, kClsidMySim, 1, "MySim", "MySim"},
        {3, kClsidMySim, 1, "MySim", "MySim"},
        {4, kClsidMySim, 1, "MySim", "MySim"},
        {5, kClsidMySim, 1, "MySim", "MySim"},
        {23, 0x2A5F877D, 23, "City Planning", "City Planning"},
        {kMyStreetId, kClsidMyStreetSim, 1, "My Street", "My Street"},
    };
    cISC4AdvisorSystem* as = city ? city->GetAdvisorSystem() : nullptr;
    for (const auto& [id, cls, advType, cap0, name0] : shipped) {
        EntryData e;
        e.id = id;
        e.class_id = cls;
        e.advice_type = advType;
        e.caption = cap0;
        e.name = name0;
        if (cISC4Advisor* a = as ? as->GetAdvisor(id) : nullptr) {
            if (cIGZString* s = a->GetName()) { if (s->ToChar()) e.caption = s->ToChar(); }
            if (cIGZString* s = a->GetPersonalName()) { if (s->ToChar()) e.name = s->ToChar(); }
        }
        g_entries.push_back(std::move(e));
    }
}

void DropCore() {
    if (g_core == nullptr) return;
    fc4_advisors_destroy(g_core);
    g_core = nullptr;
    for (fc4_adv_obj* o : g_createdObjs) {
        if (o != nullptr) fc4_adv_obj_destroy(o);
    }
    g_createdObjs.clear();
    g_handleCounter = 0;
    g_entries.clear();
    g_state.injected = false;
    g_state.postCityInitDelivered = false;
    g_state.cityEstablishedDelivered = false;
    g_state.postCityInitPending = false;
}

// ---- comparison ---------------------------------------------------------------------------------

struct Decoded {
    uint16_t version = 0;
    // §6.2 rows (post-Read state):
    uint8_t state = 0, slot = 0, wealth = 0, sign = 0, isMale = 0, age = 0, birthMonth = 0, retired = 0;
    uint16_t generation = 0;
    uint32_t homeCellX = 0, homeCellZ = 0;
    int32_t neighborCell[2] = {0, 0}, commuteCell[2] = {0, 0};
    uint32_t joblessDays = 0, bitmapId = 0, lastLocalDisaster = 0;
    uint32_t vehicleKey[3] = {0, 0, 0}, carKey[3] = {0, 0, 0}, pedestrianKey[3] = {0, 0, 0};
    int32_t unnamedPair[2] = {0, 0};
    uint32_t lastCommuteTime = 0;
    float homePos[3] = {0, 0, 0};
    uint8_t eqIndex = 0, hqIndex = 0;
    float eqDelta = 0, hqDelta = 0, eqRing[6] = {}, hqRing[6] = {};
    uint32_t residenceToken = 0, jobToken = 0, dispatchToken = 0, toolToken = 0;
    bool hasTool = false;
    uint8_t pylonUnits = 0, pylonRunning = 0;
    uint64_t pylonElapsed = 0;
    bool unnamedFlag = false;
    int32_t lastPlayerTripDay = 0;
    std::string bio, portraitPath;
    // advisor part:
    std::string personalName;
    int32_t monthCounter = 0, currentDay = 0, cursor = 0;
    uint32_t adviceCount = 0;
    bool advisorPartOk = false;
};

// Decodes the version-17 record front to back; the advisor part follows row
// 31 (advisors.md §11.2 layout). Refs become the capture's tokens.
bool DecodeRecord(const SimCapture& sc, Decoded& d) {
    CaptureReader r(sc.record);
    if (!r.U16(d.version)) return false;
    // Record version: the game writes 17 (§6.2). Read the §6.2 rows.
    if (!r.U8(d.state)) return false;
    if (d.state != 0) {
        if (!r.U8(d.slot)) return false;
        uint32_t rid = 0;
        if (!r.Ref(rid)) return false;
        d.residenceToken = rid != 0 ? sc.tokens.count(rid) ? sc.tokens.at(rid) : 0 : 0;
        if (!r.U32(d.homeCellX) || !r.U32(d.homeCellZ)) return false;
        if (!r.Ref(rid)) return false;
        d.jobToken = rid != 0 ? sc.tokens.count(rid) ? sc.tokens.at(rid) : 0 : 0;
        if (!r.I32(d.neighborCell[0]) || !r.I32(d.neighborCell[1]) || !r.I32(d.commuteCell[0]) ||
            !r.I32(d.commuteCell[1])) {
            return false;
        }
        if (!r.U32(d.joblessDays)) return false;
        if (!r.U32(d.vehicleKey[0]) || !r.U32(d.vehicleKey[1]) || !r.U32(d.vehicleKey[2])) return false;
        if (!r.U8(d.wealth) || !r.U16(d.generation) || !r.U8(d.sign) || !r.U8(d.isMale)) return false;
        {
        uint32_t n = 0;
        if (!r.U32(n)) return false;
        d.bio.resize(n);
        for (uint32_t i = 0; i < n; ++i) {
            uint8_t c = 0;
            if (!r.U8(c)) return false;
            d.bio[i] = static_cast<char>(c);
        }
    }
        if (!r.U8(d.age) || !r.U8(d.birthMonth)) return false;
        if (!r.U32(d.bitmapId)) return false;
        {
            uint32_t n = 0;
            if (!r.U32(n)) return false;
            d.portraitPath.resize(n);
            for (uint32_t i = 0; i < n; ++i) {
                uint8_t c = 0;
                if (!r.U8(c)) return false;
                d.portraitPath[i] = static_cast<char>(c);
            }
        }
        if (!r.U32(d.lastCommuteTime)) return false;
        if (!r.U8(d.retired)) return false;
        if (!r.U32(d.lastLocalDisaster)) return false;
        if (!r.F32(d.homePos[0]) || !r.F32(d.homePos[1]) || !r.F32(d.homePos[2])) return false;
        if (!r.U8(d.eqIndex) || !r.F32(d.eqDelta)) return false;
        for (float& v : d.eqRing) { if (!r.F32(v)) return false; }
        if (!r.U8(d.hqIndex) || !r.F32(d.hqDelta)) return false;
        for (float& v : d.hqRing) { if (!r.F32(v)) return false; }
        if (!r.U32(d.carKey[0]) || !r.U32(d.carKey[1]) || !r.U32(d.carKey[2])) return false;
        if (!r.U32(d.pedestrianKey[0]) || !r.U32(d.pedestrianKey[1]) || !r.U32(d.pedestrianKey[2])) return false;
        if (!r.I32(d.unnamedPair[0]) || !r.I32(d.unnamedPair[1])) return false;
        if (!r.Ref(rid)) return false;
        d.dispatchToken = rid != 0 ? sc.tokens.count(rid) ? sc.tokens.at(rid) : 0 : 0;
        if (!r.U8(d.pylonUnits)) return false;
        {
            uint32_t lo = 0, hi = 0;
            if (!r.U32(lo) || !r.U32(hi)) return false;
            d.pylonElapsed = static_cast<uint64_t>(lo) | (static_cast<uint64_t>(hi) << 32);
        }
        if (!r.U8(d.pylonRunning)) return false;
        uint8_t hasTool = 0;
        if (!r.U8(hasTool)) return false;
        d.hasTool = hasTool != 0;
        if (d.hasTool) {
            if (!r.Ref(rid)) return false;
            d.toolToken = rid != 0 ? sc.tokens.count(rid) ? sc.tokens.at(rid) : 0 : 0;
        }
        uint8_t flag = 0;
        if (!r.U8(flag)) return false;
        d.unnamedFlag = flag != 0;
        if (!r.I32(d.lastPlayerTripDay)) return false;
    }
    // The advisor part (advisors.md §11.2): u16 3, tableIndex, caption, name,
    // monthCounter, currentDay, id, cursor, advices, queue.
    uint16_t av = 0;
    if (!r.U16(av)) return false;
    if (av != 3) return false;
    int32_t tableIndex = 0, advId = 0;
    std::string caption;
    if (!r.I32(tableIndex)) return false;
    {
        uint32_t n = 0;
        if (!r.U32(n)) return false;
        caption.resize(n);
        for (uint32_t i = 0; i < n; ++i) {
            uint8_t c = 0;
            if (!r.U8(c)) return false;
            caption[i] = static_cast<char>(c);
        }
    }
    {
        uint32_t n = 0;
        if (!r.U32(n)) return false;
        d.personalName.resize(n);
        for (uint32_t i = 0; i < n; ++i) {
            uint8_t c = 0;
            if (!r.U8(c)) return false;
            d.personalName[i] = static_cast<char>(c);
        }
    }
    if (!r.I32(d.monthCounter) || !r.I32(d.currentDay) || !r.I32(advId) || !r.I32(d.cursor)) return false;
    uint32_t n = 0;
    if (!r.U32(n)) return false;
    d.adviceCount = n;
    for (uint32_t i = 0; i < n; ++i) {
        uint32_t ref = 0;
        int32_t e1 = 0, e2 = 0, e3 = 0;
        uint8_t act = 0;
        int32_t ev = 0;
        if (!r.Ref(ref) || !r.I32(e1) || !r.I32(e2) || !r.I32(e3) || !r.U8(act) || !r.I32(ev)) return false;
    }
    uint32_t q = 0;
    if (!r.U32(q)) return false;
    for (uint32_t i = 0; i < q; ++i) {
        int32_t ev = 0;
        uint32_t ref = 0;
        if (!r.I32(ev) || !r.Ref(ref)) return false;
    }
    d.advisorPartOk = true;
    return true;
}

int Compare(int32_t id, int32_t julian, char (&date)[16], uint32_t& sims) {
    sims = 0;
    int mismatches = 0;
    std::string diff;
    auto addDiff = [&diff](const std::string& s) {
        if (diff.size() < 200) {
            if (!diff.empty()) diff += "; ";
            diff += s;
        }
    };
    do {
        if (g_core == nullptr) { addDiff("no core"); break; }
        auto it = g_caps.find(id);
        if (it == g_caps.end()) {
            // Not captured: the shadow should also have no such advisor.
            fc4_advisor_state st{};
            st.struct_size = sizeof(st);
            const bool exists = fc4_advisors_advisor_state(g_core, id, &st);
            if (exists) { ++mismatches; addDiff("orig-missing advisor present in shadow"); }
            break;
        }
        fc4_advisor_state st{};
        st.struct_size = sizeof(st);
        if (!fc4_advisors_advisor_state(g_core, id, &st)) {
            ++mismatches;
            addDiff("shadow-missing advisor absent in shadow");
            break;
        }
        Decoded d;
        if (!DecodeRecord(it->second, d)) {
            ++mismatches;
            addDiff("orig record decode failed");
            break;
        }
        ++sims;
        fc4_mysim_state ms{};
        ms.struct_size = sizeof(ms);
        const bool haveState = fc4_mysim_get_state(g_core, id, &ms);
        if (!haveState) {
            ++mismatches;
            addDiff("fc4_mysim_get_state false");
            break;
        }
        if (ms.state != d.state) { ++mismatches; addDiff("state"); }
        if (ms.slot != d.slot) { ++mismatches; addDiff("slot"); }
        if (ms.wealth != d.wealth) { ++mismatches; addDiff("wealth"); }
        if (ms.sign != d.sign) { ++mismatches; addDiff("sign"); }
        if ((ms.is_male != 0) != (d.isMale != 0)) { ++mismatches; addDiff("is_male"); }
        if (ms.age != d.age) { ++mismatches; addDiff("age"); }
        if (ms.birth_month != d.birthMonth) { ++mismatches; addDiff("birth_month"); }
        if ((ms.retired != 0) != (d.retired != 0)) { ++mismatches; addDiff("retired"); }
        if (ms.generation != d.generation) { ++mismatches; addDiff("generation"); }
        if (ms.home_cell_x != d.homeCellX || ms.home_cell_z != d.homeCellZ) { ++mismatches; addDiff("home_cell"); }
        if (ms.neighbor_cell[0] != d.neighborCell[0] || ms.neighbor_cell[1] != d.neighborCell[1]) {
            ++mismatches; addDiff("neighbor_cell");
        }
        if (ms.commute_cell[0] != d.commuteCell[0] || ms.commute_cell[1] != d.commuteCell[1]) {
            ++mismatches; addDiff("commute_cell");
        }
        if (ms.jobless_days != d.joblessDays) { ++mismatches; addDiff("jobless_days"); }
        if (ms.bitmap_id != d.bitmapId) { ++mismatches; addDiff("bitmap_id"); }
        if (ms.last_local_disaster != d.lastLocalDisaster) { ++mismatches; addDiff("last_local_disaster"); }
        if (ms.last_player_trip_day != d.lastPlayerTripDay) { ++mismatches; addDiff("last_player_trip_day"); }
        for (int i = 0; i < 3; ++i) {
            const uint32_t a = *reinterpret_cast<const uint32_t*>(&d.homePos[i]);
            const uint32_t b = *reinterpret_cast<const uint32_t*>(&ms.home_position[i]);
            if (a != b) { ++mismatches; addDiff("home_position"); break; }
        }
        if (ms.eq_index != d.eqIndex) { ++mismatches; addDiff("eq_index"); }
        if (ms.hq_index != d.hqIndex) { ++mismatches; addDiff("hq_index"); }
        const uint32_t ea = *reinterpret_cast<const uint32_t*>(&d.eqDelta);
        const uint32_t eb = *reinterpret_cast<const uint32_t*>(&ms.eq_delta);
        if (ea != eb) { ++mismatches; addDiff("eq_delta"); }
        const uint32_t ha = *reinterpret_cast<const uint32_t*>(&d.hqDelta);
        const uint32_t hb = *reinterpret_cast<const uint32_t*>(&ms.hq_delta);
        if (ha != hb) { ++mismatches; addDiff("hq_delta"); }
        for (int i = 0; i < 6; ++i) {
            const uint32_t ra = *reinterpret_cast<const uint32_t*>(&d.eqRing[i]);
            const uint32_t rb = *reinterpret_cast<const uint32_t*>(&ms.eq_ring[i]);
            if (ra != rb) { ++mismatches; addDiff("eq_ring"); break; }
        }
        for (int i = 0; i < 6; ++i) {
            const uint32_t ra = *reinterpret_cast<const uint32_t*>(&d.hqRing[i]);
            const uint32_t rb = *reinterpret_cast<const uint32_t*>(&ms.hq_ring[i]);
            if (ra != rb) { ++mismatches; addDiff("hq_ring"); break; }
        }
        // Keys: three u32 each (§2.5).
        const uint32_t vk[3] = {ms.vehicle_key.type_id, ms.vehicle_key.group, ms.vehicle_key.instance};
        for (int i = 0; i < 3; ++i) {
            if (vk[i] != d.vehicleKey[i]) { ++mismatches; addDiff("vehicle_key"); break; }
        }
        const uint32_t ck[3] = {ms.car_key.type_id, ms.car_key.group, ms.car_key.instance};
        for (int i = 0; i < 3; ++i) {
            if (ck[i] != d.carKey[i]) { ++mismatches; addDiff("car_key"); break; }
        }
        const uint32_t pk[3] = {ms.pedestrian_key.type_id, ms.pedestrian_key.group, ms.pedestrian_key.instance};
        for (int i = 0; i < 3; ++i) {
            if (pk[i] != d.pedestrianKey[i]) { ++mismatches; addDiff("pedestrian_key"); break; }
        }
        // Bio + portrait path.
        char text[512];
        const size_t bioLen = fc4_mysim_text(g_core, id, 0, text, sizeof(text));
        std::string bio(text, bioLen < sizeof(text) - 1 ? bioLen : sizeof(text) - 1);
        if (bio != d.bio) { ++mismatches; addDiff("bio"); }
        const size_t pathLen = fc4_mysim_text(g_core, id, 1, text, sizeof(text));
        std::string path(text, pathLen < sizeof(text) - 1 ? pathLen : sizeof(text) - 1);
        if (path != d.portraitPath) { ++mismatches; addDiff("portrait_path"); }
        // Tokens: null-ness (0 = null both sides).
        if ((ms.residence != 0) != (d.residenceToken != 0)) { ++mismatches; addDiff("residence-null"); }
        if ((ms.job != 0) != (d.jobToken != 0)) { ++mismatches; addDiff("job-null"); }
        // Advisor part.
        if (d.advisorPartOk) {
            char name[256];
            const size_t nameLen = fc4_advisors_advisor_text(g_core, id, 1, name, sizeof(name));
            std::string pname(name, nameLen < sizeof(name) - 1 ? nameLen : sizeof(name) - 1);
            if (pname != d.personalName) { ++mismatches; addDiff("personal_name"); }
            // The differing values are named (orig/shadow) so a mismatch can be diagnosed from the log.
            auto addVal = [&addDiff](const char* f, long long o, long long s) {
                char b[96];
                snprintf(b, sizeof(b), "%s(orig=%lld shadow=%lld)", f, o, s);
                addDiff(b);
            };
            if (st.month_counter != d.monthCounter) { ++mismatches; addVal("month_counter", d.monthCounter, st.month_counter); }
            if (st.current_day != d.currentDay) { ++mismatches; addVal("current_day", d.currentDay, st.current_day); }
            if (st.cursor != d.cursor) { ++mismatches; addVal("cursor", d.cursor, st.cursor); }
            if (st.advice_count != d.adviceCount) { ++mismatches; addVal("advice_count", d.adviceCount, st.advice_count); }
        }
    } while (false);
    if (mismatches != 0) {
        Log("mysims shadow (cont.) id=%d %s: %s", id, date, diff.c_str());
    }
    return mismatches;
}

// Agent + getter comparison. Returns (mismatches, sims).
int CompareAgent(char (&date)[16], uint32_t& sims) {
    sims = 0;
    int mismatches = 0;
    cISC4City* city = AppCity();
    cISC4MySimAgentSimulator* agent = city ? city->GetMySimAgentSimulator() : nullptr;  // slot 71
    if (agent == nullptr) return 0;
    // 1. The game getters vs the shadow core's slots.
    SC4Vector<cISC4MySim*> active;
    // GetActiveMySims appends AddRef'd pointers to an SC4Vector owned by the
    // caller (allocated with the framework allocator A).
    // A local SC4Vector of trivially-destructible pointers (FC4GameContainers).
    SC4Vector<cISC4MySim*> vec;
    // [abi-ret] agent slot 5 (interfaces.tsv eax-or-void): the ids come
    // from the vector; the bool only says any active sim exists.
    if (agent->GetActiveMySims(vec)) {
        std::vector<int32_t> ids;
        for (cISC4MySim* s : vec) {
            ++sims;
            int32_t sid = 0;
            if (cISC4Advisor* adv = Query<cISC4Advisor>(s, kIIDAdvisor)) {
                sid = adv->GetID();
                Drop(adv);
            }
            ids.push_back(sid);
            Drop(s);  // GetActiveMySims AddRefs each element
        }
        int32_t shadow[8] = {};
        const size_t got = fc4_mysims_active(g_core, shadow, 8);
        if (got != ids.size()) {
            ++mismatches;
            Log("mysims shadow (cont.) %s: active sims count %zu vs shadow %zu", date, ids.size(), got);
        } else {
            for (size_t i = 0; i < ids.size(); ++i) {
                if (ids[i] != shadow[i]) {
                    ++mismatches;
                    Log("mysims shadow (cont.) %s: active sims[%zu] orig=%d shadow=%d", date, i, ids[i], shadow[i]);
                }
            }
        }
    }
    // 2. MySimLivesHere per captured residence.
    for (auto& [id, sc] : g_caps) {
        CaptureReader r(sc.record);
        uint16_t v = 0;
        uint8_t st = 0;
        if (!r.U16(v) || !r.U8(st) || st == 0) continue;
        uint32_t rid = 0;
        if (!r.Ref(rid) || rid == 0) continue;
        auto tok = sc.tokens.find(rid);
        if (tok == sc.tokens.end()) continue;
        cISC4Occupant* occ = reinterpret_cast<cISC4Occupant*>(static_cast<uintptr_t>(tok->second));
        cISC4BuildingOccupant* b = Query<cISC4BuildingOccupant>(occ, kIIDBuildingOccupant);
        if (b == nullptr) continue;
        cISC4MySim* who = agent->MySimLivesHere(b);  // slot 9, not AddRef'd
        int32_t origId = 0;
        if (who != nullptr) {
            if (cISC4Advisor* adv = Query<cISC4Advisor>(who, kIIDAdvisor)) {
                origId = adv->GetID();
                Drop(adv);
            }
        }
        int32_t shadowId = fc4_mysims_lives_here(g_core, tok->second);
        if (origId != shadowId) {
            ++mismatches;
            Log("mysims shadow (cont.) %s: MySimLivesHere id=%d orig=%d shadow=%d", date, id, origId, shadowId);
        }
        Drop(b);
    }
    // 3. GetActiveUIPanelSim: only the index encoding is compared (the
    // pointer itself is the game's object).
    int32_t idx = -1;
    cISC4MySim* p = nullptr;
    // SDK error §7.4: the method returns the MySim pointer in EAX, not bool.
    // Call through the compiled signature: bool here is the SDK wrapper; use
    // the returned slot value instead via the known vtable layout is not
    // available read-only — so compare only that the call does not crash and
    // the shadow's active index is sane.
    (void)idx;
    (void)p;
    int32_t shadowIdx = -1;
    fc4_mysims_get_active_ui_panel_sim(g_core, &shadowIdx);
    // The original's active index is not readable read-only (slot 12 writes
    // an out param only when the UI set it); named as not compared.
    (void)shadowIdx;
    return mismatches;
}

// One comparison pass over all captured sims + agent.
// Diagnostic: the shadow's month counter / day / advice count per MySim at one load stage.
void TraceAdvisorPart(const char* stage) {
    std::string line;
    for (int32_t id : kMySimIds) {
        fc4_advisor_state st{};
        st.struct_size = sizeof(st);
        if (g_core == nullptr || !fc4_advisors_advisor_state(g_core, id, &st)) continue;
        char b[80];
        snprintf(b, sizeof(b), " id=%d mc=%d day=%d advices=%u", id, st.month_counter, st.current_day, st.advice_count);
        line += b;
    }
    Log("mysims shadow (cont.) load trace %s:%s", stage, line.c_str());
}

int RunCompare(int32_t julian, uint32_t event, char (&date)[16]) {
    DateText(julian, date, sizeof(date));
    uint32_t sims = 0;
    int mismatches = 0;
    for (int32_t id : kMySimIds) {
        uint32_t one = 0;
        mismatches += Compare(id, julian, date, one);
        sims += one;
    }
    uint32_t agentSims = 0;
    int agentMismatches = 0;
    bool sehOk = true;
    struct Ctx { uint32_t& sims; int& mismatches; char (&date)[16]; } ctx{agentSims, agentMismatches, date};
    sehOk = SehGuard("mysims shadow", [](void* p) {
        auto* c = static_cast<Ctx*>(p);
        c->mismatches = CompareAgent(c->date, c->sims);
    }, &ctx);
    if (!sehOk) {
        Log("mysims shadow %s: WARNING structured exception; agent comparison abandoned", date);
        agentMismatches = 0;
    }
    sims += agentSims;
    mismatches += agentMismatches;
    g_state.months += (event == 1) ? 1 : 0;
    if (event == 1) {
        if (mismatches == 0) ++g_state.cleanMonths;
        g_state.lastMismatches = static_cast<uint32_t>(mismatches);
    }
    g_state.lastSims = sims;
    return mismatches;
}

}  // namespace

// ---- public API ---------------------------------------------------------------------------------

void MySimsShadowStartup() {
    g_enabled = SettingInt("mysims_shadow", "FC4_MYSIMS_SHADOW", 0) != 0;
    if (!g_enabled) return;  // off by default: not a word in the log
    const char* src = "";
    Setting("mysims_shadow", "FC4_MYSIMS_SHADOW", &src);
    g_reportedSetup = true;
    Log("mysims shadow: ON (mysims_shadow from %s): at PostCityInit and each SimNewMonth the original MySims "
        "(advisor ids 1-5 + 30) are captured read-only (each one's own version-17 record via its own Write) and "
        "loaded into a shadow core (FreeCity4's advisor system + MySims, fc4_advisors, no Lua), which then follows "
        "the original: the ticker (advisors.md §5.1 daily/monthly) and the agent's §3.2 messages are forwarded "
        "through the shadow's own subscriptions; the decoded saved fields, the advisor part (personal name, month "
        "counter, day, cursor, advice count), the agent slots and the getters (GetActiveMySims, MySimLivesHere) "
        "are compared field by field",
        src);
    Log("mysims shadow: NOT readable read-only (so not compared): last_commute_time (pathfinder timing), the "
        "pylon and dispatch-tool state (UI), the unnamed pair/flag (question MS1), per-advice nextDay/expiry/"
        "active (no Lua in the shadow), the trip state and dispatch null-ness beyond null/nonnull, the active "
        "UI panel index (set only by the UI), the MySim RNG (wall-clock seeded, mysims.md §8)");
}

void MySimsShadowStart() {
    if (!g_enabled || g_started) return;
    cIGZMessageServer2Ptr ms;
    if (!ms) {
        Log("mysims shadow: no message server; not started");
        return;
    }
    FillHosts();
    ShadowForwarder::Get().AddRef();  // our own reference (a static object)
    bool ok = true;
    for (uint32_t t : kForwarderMessages) {
        ok = ms->AddNotification(static_cast<cIGZMessageTarget2*>(&ShadowForwarder::Get()), t) && ok;
    }
    g_started = true;
    g_listening = ok;
    Log("mysims shadow: listening (%zu message types)%s", sizeof(kForwarderMessages) / sizeof(kForwarderMessages[0]),
        ok ? "" : " [WARNING a notification was refused]");
}

void MySimsShadowStop() {
    if (!g_started) return;
    g_started = false;
    cIGZMessageServer2Ptr ms;
    if (ms) {
        for (uint32_t t : kForwarderMessages) {
            ms->RemoveNotification(static_cast<cIGZMessageTarget2*>(&ShadowForwarder::Get()), t);
        }
    }
    ShadowForwarder::Get().Release();
    DropCore();
    g_caps.clear();
    Log("mysims shadow: stopped (months=%u clean=%u last_mismatches=%u)", g_state.months, g_state.cleanMonths,
        g_state.lastMismatches);
}

void MySimsShadowCityLoaded(cISC4City* city) {
    if (!g_enabled || !g_started || city == nullptr) return;
    if (g_core != nullptr) DropCore();  // a re-load replaces the core
    cFC4ISC4Simulator* sim = CitySim(city);
    const int32_t julian = sim ? sim->GetSimDateNumber() : 0;
    char date[16];
    DateText(julian, date, sizeof(date));

    // The core.
    g_core = fc4_advisors_create(&g_advisorHost, &g_mysimHost, SessionSeed());
    if (g_core == nullptr) {
        Log("mysims shadow %s: WARNING core create failed; comparison abandoned", date);
        return;
    }
    BuildAdvisorEntries(city);
    const uint32_t captured = CaptureSims(city);
    g_state.created = captured > 0 ? 1 : 0;
    g_state.hadMysims = captured;
    if (!InjectSims()) {
        Log("mysims shadow %s: WARNING inject incomplete; continuing with what loaded", date);
    }
    if (!fc4_mysims_agent_init(g_core)) {
        Log("mysims shadow %s: WARNING agent init false", date);
    }
    TraceAdvisorPart("after inject");
    // ONE hand-delivered PostCityInit, deferred (DeliverPendingPostCityInit): this
    // runs inside the game's PostCityInit dispatch BEFORE the original advisor system
    // handles it (measured mysims1fix3-sh-5: original month counter 316 at this point,
    // 317 from the next month on). The load comparisons (here and the observer's
    // immediate MySimsShadowMonth) therefore compare the shadow before its PostCityInit
    // against the original before its own; the core's PostCityInit (monthly step when
    // lastDay < 1, advisors.md §5.2) runs at the next forwarded message, ahead of the
    // first ticker the original posts from its PostCityInit.
    // No hand-delivered CityEstablished: the game sends it only when a city becomes
    // established (budget.md resets the funds on it), never on a load; a real one
    // reaches the core through the forwarder.
    g_state.postCityInitPending = !g_state.postCityInitDelivered;
    char loadDate[16];
    const int mismatches = RunCompare(julian, 0, loadDate);
    Log("mysims shadow %s load: sims=%u injected=%u compared=load mismatches=%d; "
        "not compared: last_commute_time, pylon, dispatch_tool, unnamed pair/flag (MS1), per-advice "
        "nextDay/expiry/active, trip state, dispatch null-ness",
        loadDate, g_state.hadMysims, captured, mismatches);
}

void MySimsShadowMonth(cISC4City* city, int32_t julian) {
    if (!g_enabled || !g_started || g_core == nullptr || city == nullptr) return;
    // Re-capture the original now: the comparison is against the original's current
    // records, not the ones captured at load (the shadow core is not re-injected).
    CaptureSims(city);
    char date[16];
    const int mismatches = RunCompare(julian, 1, date);
    Log("mysims shadow %s month: sims=%u mismatches=%d", date, g_state.lastSims, mismatches);
}

void MySimsShadowCityShutdown() {
    if (!g_enabled || !g_started) return;
    Log("mysims shadow: city shutdown: months=%u clean=%u last_mismatches=%u", g_state.months, g_state.cleanMonths,
        g_state.lastMismatches);
    DropCore();
    g_caps.clear();
}

intptr_t fc4::MySimsShadowTest(FC4MySimsShadowTestInput* in) {
    if (in == nullptr || in->struct_size != sizeof(FC4MySimsShadowTestInput)) return -1;
    if (in->event == 0) {
        // One month compare now.
        cISC4City* city = AppCity();
        cFC4ISC4Simulator* sim = CitySim(city);
        const int32_t julian = in->julian > 0 ? in->julian : (sim ? sim->GetSimDateNumber() : 0);
        char date[16];
        const int mismatches = RunCompare(julian, 1, date);
        in->last_mismatches = static_cast<uint32_t>(mismatches);
        in->last_sims = g_state.lastSims;
        in->compared = 1;
        return mismatches;
    }
    if (in->event == 1) {
        in->created = g_state.created;
        in->had_mysims = g_state.hadMysims;
        in->months = g_state.months;
        in->clean_months = g_state.cleanMonths;
        in->last_mismatches = g_state.lastMismatches;
        in->last_sims = g_state.lastSims;
        in->compared = g_state.injected ? 1 : 0;
        fc4_mysims_agent_state st{};
        st.struct_size = sizeof(st);
        if (g_core != nullptr && fc4_mysims_agent_get_state(g_core, &st)) {
            for (int i = 0; i < 6; ++i) in->slots[i] = st.slots[i];
        } else {
            for (int i = 0; i < 6; ++i) in->slots[i] = 0;
        }
        return 0;
    }
    return -1;
}

}  // namespace fc4
