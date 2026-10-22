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

#include "cFC4Simulator.h"

#include "FC4AuraShadow.h"
#include "FC4BudgetShadow.h"
#include "FC4CrimeShadow.h"
#include "FC4DemandShadow.h"
#include "FC4DevelopmentShadow.h"
#include "FC4FlammabilityShadow.h"
#include "FC4HistoryShadow.h"
#include "FC4FloraShadow.h"
#include "FC4Fpu.h"
#include "FC4LandfillShadow.h"
#include "FC4PlumbingShadow.h"
#include "FC4PollutionShadow.h"
#include "FC4PowerShadow.h"
#include "FC4ResidentialShadow.h"
#include "FC4NeighborsShadow.h"
#include "FC4CommercialShadow.h"
#include "FC4CivicShadow.h"
#include "FC4ParksShadow.h"
#include "FC4PropDevShadow.h"  // the glue12b1 lane
#include "FC4TrafficShadow.h"
#include "FC4PortsShadow.h"
#include "FC4WeatherShadow.h"
#include "FC4HarnessLogic.h"
#include "FC4Pacing.h"
#include "FC4Perf.h"
#include "FC4Turbo.h"
#include "FC4Log.h"
#include "FC4RefAudit.h"
#include "FC4Watchdog.h"
#include "FC4Properties.h"
#include "compat/FC4Compat.h"

#include "GZServPtrs.h"
#include "FC4GameList.h"
#include "SC4String.h"
#include "SC4Vector.h"
#include "cGZPersistResourceKey.h"
#include "cIGZApp.h"
#include "cIGZCheatCodeManager.h"
#include "cIGZCommandServer.h"
#include "cIGZFrameWork.h"
#include "cIGZIStream.h"
#include "cIGZMessage2Standard.h"
#include "cIGZMessageServer2.h"
#include "cIGZOStream.h"
#include "cIGZPersistResourceManager.h"
#include "cIGZString.h"
#include "cIGZVariant.h"
#include "cISC4App.h"
#include "cISCProperty.h"
#include "cISCPropertyHolder.h"
#include "cISCResExemplar.h"
#include "cRZBaseString.h"
#include "cFC4Message2Standard.h"
#include "cRZCOMDllDirector.h"

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>

#include <intrin.h>

#include <cstdlib>
#include <new>
#include <string>
#include <vector>

namespace {

// Interface IDs (spec §1 and the gzcom-dll SDK).
constexpr uint32_t kIID_cISC4Simulator = 0x8695664E;
constexpr uint32_t kIID_cIGZMessageTarget2 = 0x452294AA;  // GZCLSID::kcIGZMessageTarget2 in the SDK
constexpr uint32_t kIID_cISC4App = 0x26CE01C0;            // as used by the SDK examples
constexpr uint32_t kIID_cIGZCheatCodeManager = 0xA1085722;  // as used by the SDK examples

// Service (spec §1).
constexpr uint32_t kServiceID = 0x46956659;
constexpr int32_t kServicePriority = -2000000;

// Messages (spec §9).
constexpr uint32_t kMsgPostCityInit = 0x26D31EC1;  // kSC4MessagePostCityInit (spec §9 erratum)
constexpr uint32_t kCmdGetSimulationDate = 0x8BAC4B43;
constexpr uint32_t kMsgCheatIssued = 0x230E27AC;
constexpr uint32_t kMsgSimNewMonth = 0x66956816;
constexpr uint32_t kMsgSimNewYear = 0x66956817;

// Cheats (spec §9).
constexpr uint32_t kCheatFPS = 0x0AC07E24;
constexpr uint32_t kCheatSimDate = 0x8A78BEFF;

// Tuning (spec §7).
constexpr uint32_t kTuningType = 0x6534284A;
constexpr uint32_t kTuningGroup = 0xE7E2C2DB;
constexpr uint32_t kTuningInstance = 0xE9590E52;
constexpr uint32_t kPropMsPerDay = 0x89590E6D;
constexpr uint32_t kPropAnimationDilation = 0x89590E6E;

cIGZMessageTarget2* AgentPtr(fc4_agent a) { return reinterpret_cast<cIGZMessageTarget2*>(static_cast<uintptr_t>(a)); }
fc4_agent AgentHandle(cIGZMessageTarget2* p) { return static_cast<fc4_agent>(reinterpret_cast<uintptr_t>(p)); }

std::string ToStd(cIGZString const& s) {
    const char* p = s.ToChar();
    return p ? std::string(p, s.Strlen()) : std::string();
}

// A fresh standard message (foundations §5.1), already AddRef'd once.
cFC4Message2Standard* MakeMessage(const fc4_message& src) {
    return FC4MakeMessage(src.type, static_cast<intptr_t>(src.data1), static_cast<intptr_t>(src.data2),
                          static_cast<intptr_t>(src.data3));
}

// The initialized simulator of ours (between Init and Shutdown), for the
// script's `turbo on|off` (FC4Turbo.h). Main thread only.
cFC4Simulator* g_liveSim = nullptr;

void DateOf(int32_t julian, char (&out)[16]) {
    fc4_date d{};
    fc4_date_from_julian(julian, &d);
    _snprintf_s(out, sizeof(out), _TRUNCATE, "%04d-%02d-%02d", d.year, d.month, d.day_of_month);
}

}  // namespace

fc4::TurboScriptResult fc4::TurboScriptSet(bool on) {
    if (g_liveSim == nullptr) return TurboScriptResult::NoSimulator;
    if (on) return g_liveSim->RequestTurbo(0, "script") ? TurboScriptResult::On : TurboScriptResult::Held;
    if (!g_liveSim->DropHeldTurbo("script")) g_liveSim->SetTurbo(false, 0, "script");
    return TurboScriptResult::Off;
}

void fc4::TurboOnConnectionsReady() {
    if (g_liveSim != nullptr) g_liveSim->TurboCityReady();
}

// ---- construction -------------------------------------------------------------

// Factory for the subsystem table (FC4Subsystems.cpp).
cIGZUnknown* CreateFC4Simulator() {
    return static_cast<cIGZUnknown*>(static_cast<cFC4ISC4Simulator*>(new cFC4Simulator()));
}


cFC4Simulator::cFC4Simulator() : serviceId_(kServiceID) {
    FC4_LOGV("cFC4Simulator(%p) constructor enter", static_cast<void*>(this));
    fc4_host host{};
    host.struct_size = sizeof(fc4_host);
    host.user = this;
    host.now_ms = &cFC4Simulator::HostNowMs;
    host.deliver = &cFC4Simulator::HostDeliver;
    host.add_ref = &cFC4Simulator::HostAddRef;
    host.release = &cFC4Simulator::HostRelease;
    host.post = &cFC4Simulator::HostPost;
    FC4_LOGV("cFC4Simulator(%p): calling fc4_sim_create (core ABI %u)", static_cast<void*>(this), fc4_abi_version());
    sim_ = fc4_sim_create(&host);
    FC4_LOGV("cFC4Simulator(%p): fc4_sim_create returned %p", static_cast<void*>(this), static_cast<void*>(sim_));
    InstallTimerSource();
    InstallPacing();
    if (sim_ == nullptr || fc4_abi_version() != FC4_ABI_VERSION) {
        fc4::Log("cFC4Simulator: core creation failed (abi %u, header %u)", fc4_abi_version(), FC4_ABI_VERSION);
    }
}

cFC4Simulator::~cFC4Simulator() {
    fc4::Log("cFC4Simulator destroyed");
    if (g_liveSim == this) g_liveSim = nullptr;
    fc4::WatchdogStop();
    fc4_sim_destroy(sim_);  // releases any agents still registered
    sim_ = nullptr;
}

// ---- cIGZUnknown ----------------------------------------------------------------

bool cFC4Simulator::QueryInterface(uint32_t riid, void** ppvObj) {
    const bool ok = QueryInterfaceImpl(riid, ppvObj);
    if (qiLogged_ < 200) {
        ++qiLogged_;
        FC4_LOGV("cFC4Simulator(%p)::QueryInterface(0x%08X) -> %d (refcount now %u)", static_cast<void*>(this), riid,
                 ok ? 1 : 0, refCount_);
    } else if (fc4::LogLevel() >= fc4::kLogVerbose) {
        fc4::NoteSuppressed();
    }
    return ok;
}

bool cFC4Simulator::QueryInterfaceImpl(uint32_t riid, void** ppvObj) {
    if (ppvObj == nullptr) return false;
    switch (riid) {
        case kIID_cISC4Simulator:
            *ppvObj = static_cast<cFC4ISC4Simulator*>(this);
            break;
        case GZIID_cIGZSerializable:
            *ppvObj = static_cast<cIGZSerializable*>(this);
            break;
        case GZIID_cIGZSystemService:
            *ppvObj = static_cast<cIGZSystemService*>(this);
            break;
        case kIID_cIGZMessageTarget2:
            *ppvObj = static_cast<cIGZMessageTarget2*>(this);
            break;
        case GZIID_cIGZUnknown:
            *ppvObj = static_cast<cIGZUnknown*>(static_cast<cFC4ISC4Simulator*>(this));
            break;
        default:
            *ppvObj = nullptr;
            return false;
    }
    AddRef();
    return true;
}

uint32_t cFC4Simulator::AddRef() { return ++refCount_; }

uint32_t cFC4Simulator::Release() {
    if (refCount_ > 0) --refCount_;
    if (refCount_ == 0) {
        delete this;
        return 0;
    }
    return refCount_;
}

// ---- limit-timer calibration (spec §5.5) -------------------------------------------------

namespace {
uint64_t QpcFrequency() {
    static const uint64_t freq = [] {
        LARGE_INTEGER f;
        QueryPerformanceFrequency(&f);
        return static_cast<uint64_t>(f.QuadPart);
    }();
    return freq;
}

uint64_t QpcNow() {
    LARGE_INTEGER q;
    QueryPerformanceCounter(&q);
    return static_cast<uint64_t>(q.QuadPart);
}

// Initial estimate of milliseconds per TSC cycle: measure the TSC against the
// wall clock over ~2 ms (spec §5.5 "Initial value").
float MeasureMsPerCycle(double* measuredMs) {
    const uint64_t freq = QpcFrequency();
    const uint64_t q0 = QpcNow();
    const uint64_t c0 = __rdtsc();
    uint64_t q1 = q0;
    uint64_t c1 = c0;
    const uint64_t span = freq / 500 > 0 ? freq / 500 : 1;  // 2 ms
    for (int guard = 0; guard < 100000000 && q1 - q0 < span; ++guard) {
        YieldProcessor();
        q1 = QpcNow();
        c1 = __rdtsc();
    }
    const double ms = static_cast<double>(q1 - q0) * 1000.0 / static_cast<double>(freq);
    if (measuredMs != nullptr) *measuredMs = ms;
    const int64_t dc = static_cast<int64_t>(c1 - c0);
    if (dc <= 0 || ms <= 0.0) return 0.0f;
    return static_cast<float>(ms / static_cast<double>(dc));
}
}  // namespace

uint64_t cFC4Simulator::TimerQpc(void*) { return QpcNow(); }
uint64_t cFC4Simulator::TimerTsc(void*) { return __rdtsc(); }

void cFC4Simulator::TimerPublish(void* user, float cyclesToMs, float msToCycles) {
    auto* self = static_cast<cFC4Simulator*>(user);
    self->lastCyclesToMs_ = cyclesToMs;
    self->lastMsToCycles_ = msToCycles;
    self->limitTimersWritten_ = fc4::compat::PublishLimitTimers(cyclesToMs, msToCycles);
}

void cFC4Simulator::InstallTimerSource() {
    if (sim_ == nullptr) return;
    double measuredMs = 0.0;
    float estimate = MeasureMsPerCycle(&measuredMs);
    if (!(estimate > 0.0f)) {
        // [choice] fall back to a 2 GHz guess; the 0.9/0.1 average corrects it within seconds.
        fc4::Log("calibration: initial TSC measurement failed (%.3f ms); assuming 2 GHz", measuredMs);
        estimate = 1.0f / 2.0e6f;
    }
    fc4_timer_source src{};
    src.struct_size = sizeof(src);
    src.user = this;
    src.qpc = &cFC4Simulator::TimerQpc;
    src.tsc = &cFC4Simulator::TimerTsc;
    src.publish = &cFC4Simulator::TimerPublish;
    src.qpc_frequency = QpcFrequency();
    const bool ok = fc4_sim_set_timer_source(sim_, &src, estimate);
    fc4::Log("calibration: QPC %llu Hz; initial estimate %.6g ms/cycle (%.1f MHz TSC, measured over %.3f ms); "
             "installed=%d; published CyclesToMillisecs=%.9g MillisecsToCycles=%.9g; written to the game=%s",
             static_cast<unsigned long long>(src.qpc_frequency), estimate, 1.0 / estimate / 1000.0, measuredMs,
             ok ? 1 : 0, lastCyclesToMs_, lastMsToCycles_, limitTimersWritten_ ? "yes" : "NO");
}

void cFC4Simulator::LogCalibration(const char* when) {
    float c2m = 0.0f, m2c = 0.0f;
    uint64_t samples = 0;
    const bool have = fc4_sim_get_calibration(sim_, &c2m, &m2c, &samples);
    fc4::Log("calibration (%s): %s CyclesToMillisecs=%.9g MillisecsToCycles=%.9g (%.1f MHz), %llu sample(s); "
             "written to the game=%s (%llu writes)",
             when, have ? "installed" : "NOT installed", c2m, m2c, m2c / 1000.0,
             static_cast<unsigned long long>(samples), limitTimersWritten_ ? "yes" : "NO",
             static_cast<unsigned long long>(fc4::compat::PublishCount()));
}

// ---- host callbacks ------------------------------------------------------------------

uint32_t cFC4Simulator::HostNowMs(void*) {
    static LARGE_INTEGER freq = [] {
        LARGE_INTEGER f;
        QueryPerformanceFrequency(&f);
        return f;
    }();
    LARGE_INTEGER now;
    QueryPerformanceCounter(&now);
    return static_cast<uint32_t>((static_cast<unsigned long long>(now.QuadPart) * 1000ull) /
                                 static_cast<unsigned long long>(freq.QuadPart));
}

bool cFC4Simulator::HostDeliver(void* user, fc4_sim*, fc4_agent agent, const fc4_message* msg) {
    cIGZMessageTarget2* target = AgentPtr(agent);
    if (target == nullptr || msg == nullptr) return true;
    // Synchronous delivery (spec §4) with the simulator's pattern from
    // foundations §5.1: create, AddRef, set, send, Release. An agent that
    // keeps the message (AddRef) keeps it alive.
    cFC4Message2Standard* m = MakeMessage(*msg);
    const char* name = "?";
    if (user != nullptr) {
        const auto& stats = static_cast<cFC4Simulator*>(user)->agentStats_;
        const auto it = stats.find(std::make_pair(reinterpret_cast<uintptr_t>(target), LevelOfType(msg->type)));
        if (it != stats.end()) name = it->second.name.c_str();
    }
    fc4::DeliveryBegin(LevelOfType(msg->type), msg->type, target, name);
    // The x87 leftover guard around every delivery to an agent (FC4Fpu.h):
    // what was left before it, and what the agent (ours, the game's, or a
    // shadowed original) left behind.
    static fc4::X87Site beforeSite = {"deliver (before)", 0, 0, 0, nullptr};
    static fc4::X87Site afterSite = {"deliver (after)", 0, 0, 0, nullptr};
    fc4::X87Check(beforeSite, msg->type, name);
    // The budget and demand shadows (opt-in, FC4BudgetShadow.h,
    // FC4DemandShadow.h) wrap the original budget's SimNewMonth and the
    // original demand simulator's SimNewDay / SimIdle deliveries; otherwise
    // they do nothing.
    // The pollution shadow (opt-in, FC4PollutionShadow.h) wraps the original
    // pollution simulator's SimNewDay / SimNewMonth deliveries.
    // The crime shadow (opt-in, FC4CrimeShadow.h) wraps the original crime
    // simulator's delivery that runs its pipeline state 4.
    // The flora shadow (opt-in, FC4FloraShadow.h) wraps the original flora
    // simulator's SimNewMonth deliveries.
    // The landfill shadow (opt-in, FC4LandfillShadow.h) wraps the original
    // landfill developer's SimNewMonth deliveries.
    // The power shadow (opt-in, FC4PowerShadow.h) wraps the original power
    // simulator's SimNewMonth / SimNewDay and its working SimIdle deliveries.
    // The plumbing shadow (opt-in, FC4PlumbingShadow.h) wraps the original
    // plumbing simulator's SimIdle / SimNewDay / SimNewMonth deliveries.
    // The residential shadow (opt-in, FC4ResidentialShadow.h) wraps the
    // original residential simulator's SimNewDay deliveries.
    // The aura shadow (opt-in, FC4AuraShadow.h) wraps the original aura
    // simulator's SimIdle / SimNewDay / SimNewMonth deliveries.
    // The development shadow (opt-in, FC4DevelopmentShadow.h) wraps the
    // original growth developers' SimIdle / SimNewDay and the original BDS's
    // SimNewMonth deliveries.
    // The history shadow (opt-in, FC4HistoryShadow.h) steps its core after
    // the original warehouse's SimNewDay / SimNewYear deliveries.
    // The flammability shadow (opt-in, FC4FlammabilityShadow.h) polls after
    // the original flammability simulator's SimNewDay deliveries.
    // The commercial shadow (opt-in, FC4CommercialShadow.h) wraps the
    // original commercial simulator's SimIdle / SimNewDay deliveries.
    // The civic shadow (opt-in, FC4CivicShadow.h) wraps the original civic
    // building simulator's SimNewMonth deliveries.
    // The parks shadow (opt-in, FC4ParksShadow.h) wraps the original park
    // manager's SimNewMonth deliveries.
    // The traffic shadow (opt-in, FC4TrafficShadow.h) wraps every delivery to
    // the original traffic simulator's agents (one-shot, day, month).
// The prop developer shadow (opt-in, FC4PropDevShadow.h) wraps the
    // original prop developer's Idle / Day deliveries (its block steps).
    // The ports shadow (opt-in, FC4PortsShadow.h) wraps the original airport
    // and seaport developers' SimNewMonth deliveries.
    bool finished = false;
    const uint64_t perfT0 = fc4::perf::On() ? fc4::perf::Now() : 0;  // tooling: perf_profile (FC4Perf.h)
    if (!fc4::BudgetShadowDeliver(target, name, m, msg->type, msg->data1, &finished) &&
        !fc4::DemandShadowDeliver(target, name, m, msg->type, msg->data1, &finished) &&
        !fc4::PollutionShadowDeliver(target, name, m, msg->type, msg->data1, &finished) &&
        !fc4::CrimeShadowDeliver(target, name, m, msg->type, msg->data1, &finished) &&
        !fc4::FloraShadowDeliver(target, name, m, msg->type, msg->data1, &finished) &&
        !fc4::LandfillShadowDeliver(target, name, m, msg->type, msg->data1, &finished) &&
        !fc4::PowerShadowDeliver(target, name, m, msg->type, msg->data1, &finished) &&
        !fc4::PlumbingShadowDeliver(target, name, m, msg->type, msg->data1, &finished) &&
        !fc4::ResidentialShadowDeliver(target, name, m, msg->type, msg->data1, &finished) &&
        !fc4::AuraShadowDeliver(target, name, m, msg->type, msg->data1, &finished) &&
        !fc4::NeighborsShadowDeliver(target, name, m, msg->type, msg->data1, &finished) &&
        !fc4::DevelopmentShadowDeliver(target, name, m, msg->type, msg->data1, &finished) &&
        !fc4::FlammabilityShadowDeliver(target, name, m, msg->type, msg->data1, &finished) &&
        !fc4::CommercialShadowDeliver(target, name, m, msg->type, msg->data1, &finished) &&
        !fc4::CivicShadowDeliver(target, name, m, msg->type, msg->data1, &finished) &&
        !fc4::ParksShadowDeliver(target, name, m, msg->type, msg->data1, &finished) &&
        !fc4::HistoryShadowDeliver(target, name, m, msg->type, msg->data1, &finished) &&
        !fc4::TrafficShadowDeliver(target, name, m, msg->type, msg->data1, &finished) &&
!fc4::PropDevShadowDeliver(target, name, m, msg->type, msg->data1, &finished) &&
        !fc4::PortsShadowDeliver(target, name, m, msg->type, msg->data1, &finished) &&
        !fc4::WeatherShadowDeliver(target, name, m, msg->type, msg->data1, &finished)) {
        finished = target->DoMessage(m);
    }
    if (perfT0 != 0) fc4::perf::AddDelivery(target, LevelOfType(msg->type), name, fc4::perf::Now() - perfT0);
    fc4::DeliveryEnd();
    fc4::X87Check(afterSite, msg->type, name);
    m->Release();
    if (user != nullptr && fc4::LogLevel() >= fc4::kLogVerbose) {
        static_cast<cFC4Simulator*>(user)->RecordDelivery(target, *msg, finished);
    }
    return finished;
}

// ---- delivery diagnostics -----------------------------------------------------------------

namespace {
constexpr uint32_t kSimLevelBase = 0x66956810;
constexpr uint32_t kMaxDeliveryLogs = 300;
// Status line period in ticks: FC4_STATUS_EVERY (default 500).
uint32_t StatusEveryTicks() {
    static const uint32_t every = [] {
        char value[16] = {};
        const DWORD n = GetEnvironmentVariableA("FC4_STATUS_EVERY", value, sizeof(value));
        const long v = (n > 0 && n < sizeof(value)) ? strtol(value, nullptr, 10) : 0;
        return v > 0 ? static_cast<uint32_t>(v) : 500u;
    }();
    return every;
}

uint32_t LevelOf(uint32_t type) {
    const uint32_t l = type - kSimLevelBase;
    return l < FC4_LEVEL_COUNT ? l : 0xFFFFFFFFu;
}
}  // namespace

uint32_t cFC4Simulator::LevelOfType(uint32_t type) { return LevelOf(type); }

cFC4Simulator::AgentStats& cFC4Simulator::Stats(cIGZMessageTarget2* agent, uint32_t level) {
    AgentStats& st = agentStats_[std::make_pair(reinterpret_cast<uintptr_t>(agent), level)];
    if (st.name.empty()) st.name = "?";
    return st;
}

void cFC4Simulator::RecordDelivery(cIGZMessageTarget2* agent, const fc4_message& msg, bool finished) {
    const uint32_t level = LevelOf(msg.type);
    AgentStats& st = Stats(agent, level);
    ++st.deliveries;
    if (finished) {
        st.streak = 0;
    } else {
        ++st.falses;
        ++st.streak;
        if (st.streak > st.maxStreak) st.maxStreak = st.streak;
        if (st.streak == 100 || st.streak == 1000 || st.streak == 10000 || st.streak == 100000 ||
            st.streak == 1000000) {
            FC4_LOGV("agent %p '%s' (level %u) has now returned false %u times in a row (msg 0x%08X d1=%lld d2=%lld "
                     "d3=%lld; total %llu deliveries, %llu false)",
                     static_cast<void*>(agent), st.name.c_str(), level, st.streak, msg.type,
                     static_cast<long long>(msg.data1), static_cast<long long>(msg.data2),
                     static_cast<long long>(msg.data3), static_cast<unsigned long long>(st.deliveries),
                     static_cast<unsigned long long>(st.falses));
        }
    }
    if (deliveriesLogged_ >= kMaxDeliveryLogs) {
        fc4::NoteSuppressed();
    } else {
        ++deliveriesLogged_;
        FC4_LOGV("deliver #%u: level %u type 0x%08X d1=%lld d2=%lld d3=%lld -> agent %p '%s' returned %d",
                 deliveriesLogged_, level, msg.type, static_cast<long long>(msg.data1),
                 static_cast<long long>(msg.data2), static_cast<long long>(msg.data3), static_cast<void*>(agent),
                 st.name.c_str(), finished ? 1 : 0);
    }
}

void cFC4Simulator::LogSchedulerStatus() {
    fc4_state st{};
    st.struct_size = sizeof(st);
    fc4_sim_get_state(sim_, &st);
    fc4_debug dbg{};
    dbg.struct_size = sizeof(dbg);
    fc4_sim_get_debug(sim_, &dbg);
    FC4_LOGV("status @tick %u: julian=%d speed=%d pause=%d hidden=%d emergency=%d budget=%u | interval %u/%u ms "
             "ready=%u hours_complete=%u day_done=%u level0/1_pending=%u tomorrow week/month/year=%u/%u/%u "
             "blocked week/month/year=%u/%u/%u DAY_GATE_OPEN=%u | counters m=%u h=%u d=%u w=%u mo=%u y=%u fired=%u%u%u%u%u%u",
             ticks_, st.julian_day, st.sim_speed, st.pause_count, st.hidden_pause_count, st.emergency_pause_count,
             st.frame_budget_ms, dbg.interval_elapsed_ms, dbg.ms_per_day, dbg.ready, dbg.hours_complete,
             dbg.day_level_done, dbg.one_shot_or_emergency_pending, dbg.tomorrow_starts_week,
             dbg.tomorrow_starts_month, dbg.tomorrow_starts_year, dbg.blocked_by_week, dbg.blocked_by_month,
             dbg.blocked_by_year, dbg.day_gate_open, st.level_counters[2], st.level_counters[3], st.level_counters[4],
             st.level_counters[5], st.level_counters[6], st.level_counters[7], st.level_fired[2], st.level_fired[3],
             st.level_fired[4], st.level_fired[5], st.level_fired[6], st.level_fired[7]);

    LogCalibration("status");
    for (uint32_t level = 0; level < FC4_LEVEL_COUNT; ++level) {
        const uint32_t count = st.agent_counts[level];
        if (count == 0) continue;
        const int32_t cursor = level == FC4_LEVEL_IDLE ? dbg.idle_cursor : dbg.cursors[level];
        char where[160] = "END";
        if (cursor >= 0) {
            fc4_agent_info info{};
            char name[96] = {};
            if (fc4_sim_get_agent_info(sim_, level, static_cast<size_t>(cursor), &info, name, sizeof(name))) {
                const AgentStats& s = Stats(AgentPtr(info.agent), level);
                snprintf(where, sizeof(where), "#%d %p '%s' enabled=%u flags=%u streak=%u", cursor,
                         reinterpret_cast<void*>(static_cast<uintptr_t>(info.agent)), name, info.enabled, info.flags,
                         s.streak);
            } else {
                snprintf(where, sizeof(where), "#%d (no info)", cursor);
            }
        }
        // Top 5 registrations at this level by current consecutive-false streak.
        const AgentStats* top[5] = {};
        for (const auto& kv : agentStats_) {
            if (kv.first.second != level || kv.second.streak == 0) continue;
            const AgentStats* cand = &kv.second;
            for (int i = 0; i < 5; ++i) {
                if (top[i] == nullptr || cand->streak > top[i]->streak) {
                    for (int j = 4; j > i; --j) top[j] = top[j - 1];
                    top[i] = cand;
                    break;
                }
            }
        }
        char streaks[600] = "";
        size_t used = 0;
        for (int i = 0; i < 5 && top[i] != nullptr; ++i) {
            const int k = snprintf(streaks + used, sizeof(streaks) - used, "%s'%s' streak=%u false=%llu/%llu",
                                   i ? ", " : "", top[i]->name.c_str(), top[i]->streak,
                                   static_cast<unsigned long long>(top[i]->falses),
                                   static_cast<unsigned long long>(top[i]->deliveries));
            if (k < 0 || static_cast<size_t>(k) >= sizeof(streaks) - used) break;
            used += static_cast<size_t>(k);
        }
        FC4_LOGV("  level %u: %u agent(s), cursor %s, counter %u, fired %u; top false streaks: %s", level, count, where,
                 st.level_counters[level], st.level_fired[level], used ? streaks : "none");
    }
}

void cFC4Simulator::HostAddRef(void*, fc4_agent agent) {
    if (cIGZMessageTarget2* t = AgentPtr(agent)) FC4_ADDREF(t);
}

void cFC4Simulator::HostRelease(void*, fc4_agent agent) {
    if (cIGZMessageTarget2* t = AgentPtr(agent)) FC4_RELEASE(t);
}

void cFC4Simulator::HostPost(void* user, const fc4_message* msg) {
    if (user != nullptr && msg != nullptr) static_cast<cFC4Simulator*>(user)->Post(*msg);
}

// ---- deterministic pacing (simulator.md §5.7 [choice]; FC4Pacing.h) ------------------

void cFC4Simulator::InstallPacing() {
    if (sim_ == nullptr) return;
    const fc4::PacingMode mode = fc4::Pacing();
    fc4_pacing_hooks hooks{};
    hooks.struct_size = sizeof(hooks);
    hooks.user = this;
    hooks.is_paced = &cFC4Simulator::HostIsPaced;
    hooks.settle = &cFC4Simulator::HostSettle;
    hooks.idle_steps = &cFC4Simulator::HostIdleSteps;
    if (!fc4_sim_set_pacing(sim_, static_cast<uint32_t>(mode), &hooks)) {
        fc4::Log("cFC4Simulator: WARNING fc4_sim_set_pacing(%s) refused", fc4::PacingName(mode));
    }
}

bool cFC4Simulator::HostIsPaced(void* user, fc4_agent agent) {
    if (fc4::PacingIsOurs(AgentPtr(agent))) return true;
    const auto* self = static_cast<const cFC4Simulator*>(user);
    return self != nullptr && self->fixedIdle_.count(static_cast<uintptr_t>(agent)) != 0;
}

uint32_t cFC4Simulator::HostIdleSteps(void* user, fc4_agent agent) {
    const auto* self = static_cast<const cFC4Simulator*>(user);
    if (self == nullptr) return 1;
    const auto it = self->fixedIdle_.find(static_cast<uintptr_t>(agent));
    return it != self->fixedIdle_.end() ? it->second : 1;
}

void cFC4Simulator::HostSettle(void* user, int32_t julian) {
    auto* self = static_cast<cFC4Simulator*>(user);
    fc4::PacingFlush(self != nullptr ? self->msgServer_ : nullptr, julian);
}

void cFC4Simulator::Post(const fc4_message& msg) {
    const bool calendar = msg.type >= 0x66956814 && msg.type <= kMsgSimNewYear;  // SimNewDay..SimNewYear
    if (msg.type == 0x66956814) fc4::PacingDayOpened(static_cast<int32_t>(msg.data1));
    bool posted = false;
    bool deferred = false;
    if (msgServer_ != nullptr) {
        // Queued broadcast (spec §5.2): the queue holds its own reference (foundations §5.1).
        // Deterministic pacing: held for the day's settle point instead (FC4Pacing.h).
        cFC4Message2Standard* m = MakeMessage(msg);
        deferred = fc4::PacingDefer(m);
        posted = deferred || msgServer_->MessagePost(m, false);
        m->Release();
    }
    if (msg.type == kMsgSimNewMonth) fc4::HeartbeatNow("month start");
    if (msg.type == kMsgSimNewMonth && fc4::PacingDeterministic()) {
        const fc4::PacingStats ps = fc4::PacingCounters();
        fc4::Log("pacing: days settled %llu, messages deferred %llu, sent at settle %llu, released unsent %llu, "
                 "message-server ticks %llu, settles with posts left %llu",
                 static_cast<unsigned long long>(ps.settles), static_cast<unsigned long long>(ps.deferred),
                 static_cast<unsigned long long>(ps.sent), static_cast<unsigned long long>(ps.dropped),
                 static_cast<unsigned long long>(ps.pumps), static_cast<unsigned long long>(ps.leftovers));
    }
    if (msg.type == kMsgSimNewMonth && turboUsed_) TurboMonthLine(static_cast<int32_t>(msg.data1));  // tooling
    const bool logIt = calendar || postsLogged_ < 200;
    if (!logIt) {
        if (fc4::LogLevel() >= fc4::kLogVerbose) fc4::NoteSuppressed();
        return;
    }
    ++postsLogged_;
    if (calendar) {
        fc4_date d{};
        fc4_date_from_julian(static_cast<int32_t>(msg.data1), &d);
        fc4::Log("post %s 0x%08X d1=%lld (%04d-%02d-%02d) d2=%lld d3=%lld -> %s %s",
                 msg.type == 0x66956814 ? "SimNewDay" : msg.type == 0x66956815 ? "SimNewWeek"
                 : msg.type == kMsgSimNewMonth ? "SimNewMonth" : "SimNewYear",
                 msg.type, static_cast<long long>(msg.data1), d.year, d.month, d.day_of_month,
                 static_cast<long long>(msg.data2), static_cast<long long>(msg.data3),
                 deferred ? "deferred" : "MessagePost",
                 msgServer_ ? (posted ? "true" : "FALSE") : "skipped (no message server)");
    } else {
        FC4_LOGV("post 0x%08X d1=%lld d2=%lld d3=%lld -> MessagePost %s", msg.type, static_cast<long long>(msg.data1),
                 static_cast<long long>(msg.data2), static_cast<long long>(msg.data3),
                 msgServer_ ? (posted ? "true" : "FALSE") : "skipped (no message server)");
    }
}

// ---- lifecycle (spec §9) ---------------------------------------------------------------

fc4_tuning cFC4Simulator::ReadTuning(uint32_t (&msPerDay)[3], size_t& msCount, float (&dilation)[5], size_t& dilCount) {
    fc4_tuning t{};
    msCount = 0;
    dilCount = 0;
    fc4::props::Exemplar ex;
    if (!ex.Load(kTuningType, kTuningGroup, kTuningInstance)) {
        fc4::Log("tuning: simulator exemplar unavailable; using defaults");
        return t;
    }
    // Spec §7: msPerDayTable only with exactly 3 values; [choice] the dilation
    // array only with exactly 5. Other counts keep the defaults ("missing").
    std::vector<uint32_t> ms;
    if (fc4::props::GetU32s(ex.Holder(), kPropMsPerDay, ms)) {
        fc4::Log("tuning: msPerDay property type 0x%04X count %u", fc4::props::TypeOf(ex.Holder(), kPropMsPerDay),
                 static_cast<unsigned>(ms.size()));
        if (ms.size() == 3) {
            for (size_t i = 0; i < 3; ++i) msPerDay[i] = ms[i];
            msCount = 3;
            t.ms_per_day = msPerDay;
            t.ms_per_day_count = 3;
        }
    }
    std::vector<float> dil;
    if (fc4::props::GetF32s(ex.Holder(), kPropAnimationDilation, dil)) {
        fc4::Log("tuning: dilation property type 0x%04X count %u",
                 fc4::props::TypeOf(ex.Holder(), kPropAnimationDilation), static_cast<unsigned>(dil.size()));
        if (dil.size() == 5) {
            for (size_t i = 0; i < 5; ++i) dilation[i] = dil[i];
            dilCount = 5;
            t.animation_dilation = dilation;
            t.animation_dilation_count = 5;
        }
    }
    return t;
}

bool cFC4Simulator::Init() {
    FC4_LOGV("Init() called (sim=%p, initialized=%d)", static_cast<void*>(sim_),
             sim_ ? (fc4_sim_is_initialized(sim_) ? 1 : 0) : -1);
    if (sim_ == nullptr) return false;
    if (fc4_sim_is_initialized(sim_)) return true;
    fc4::WatchdogStart();  // this is the simulation thread
    fc4::ActivityScope activity("Simulator::Init");
    fc4::SetCityRunning(false);

    // Step 3: notifications. (The message server is acquired first so the
    // SpeedChanged post of step 6 can be broadcast.)
    cIGZMessageServer2Ptr ms;
    if (ms) {
        msgServer_ = ms;
        FC4_ADDREF(msgServer_);
        msgServer_->AddNotification(static_cast<cIGZMessageTarget2*>(this), kMsgPostCityInit);
        postCityInitSubscribed_ = true;
        msgServer_->AddNotification(static_cast<cIGZMessageTarget2*>(this), kCmdGetSimulationDate);
    } else {
        fc4::Log("Init: no message server");
    }

    // Step 4: command registration. [choice] empty parameter description.
    cIGZCommandServerPtr cs;
    if (cs) commandRegistered_ = cs->RegisterCommand(kCmdGetSimulationDate, "GetSimulationDate", "");

    // Steps 1, 2, 5, 6, 7 (core).
    uint32_t msPerDay[3] = {};
    float dilation[5] = {};
    size_t msCount = 0, dilCount = 0;
    const fc4_tuning tuning = ReadTuning(msPerDay, msCount, dilation, dilCount);
    const bool ok = fc4_sim_init(sim_, &tuning);
    g_liveSim = this;
    // Tooling: sim_turbo (FC4Turbo.h), held until this load's first
    // ConnectionsReady. Off and never used: nothing is set or logged.
    cityReady_ = false;
    turboHeld_ = false;
    {
        bool left = false;
        uint32_t leftFrame = 0;
        int32_t leftUntil = 0;
        // A turbo the previous city left on in the core: off until this city is ready.
        if (turboUsed_ && fc4_sim_get_turbo(sim_, &left, &leftFrame, &leftUntil) && left) {
            fc4_sim_set_turbo(sim_, false, leftFrame, 0);
        }
        if (fc4::TurboConfig().on) {
            RequestTurbo(fc4::TurboConfig().untilJulian, "sim_turbo");
        } else if (left) {
            RequestTurbo(leftUntil, "kept from the previous city");
        }
    }

    // Step 8: framework registration.
    if (cIGZFrameWork* fw = RZGetFrameWork()) {
        fw->AddSystemService(static_cast<cIGZSystemService*>(this));
        fw->AddToTick(static_cast<cIGZSystemService*>(this));
        addedToFramework_ = true;
    }
    serviceRunning_ = true;

    // Step 9: cheats.
    RegisterCheats();

    LogCalibration("Init");
    fc4_state st{};
    st.struct_size = sizeof(st);
    fc4_sim_get_state(sim_, &st);
    fc4::Log("Init: ok=%d speed=%d msPerDay=%u fps=%d julian=%d table={%u,%u,%u} cheats=%s command=%s pacing=%s",
             ok ? 1 : 0, st.sim_speed, st.ms_per_day, st.target_fps, st.julian_day, st.ms_per_day_table[0],
             st.ms_per_day_table[1], st.ms_per_day_table[2], cheatMgr_ ? "yes" : "no", commandRegistered_ ? "yes" : "no",
             fc4::PacingName(fc4::Pacing()));
    return ok;
}

bool cFC4Simulator::Shutdown() {
    FC4_LOGV("Shutdown() called");
    if (sim_ == nullptr || !fc4_sim_is_initialized(sim_)) return true;
    AddRef();  // the framework may drop its reference while we unregister
    fc4::SetCityRunning(false);

    UnregisterCheats();          // step 1
    fc4_sim_shutdown(sim_);      // steps 2, 3, 7
    fc4::PacingDrop("simulator shutdown");  // deferred posts of an unsettled day
    if (g_liveSim == this) g_liveSim = nullptr;
    cityReady_ = false;  // tooling: a held turbo request ends with its city
    turboHeld_ = false;

    if (msgServer_ != nullptr) {  // step 4
        if (postCityInitSubscribed_) {
            msgServer_->RemoveNotification(static_cast<cIGZMessageTarget2*>(this), kMsgPostCityInit);
            postCityInitSubscribed_ = false;
        }
        msgServer_->RemoveNotification(static_cast<cIGZMessageTarget2*>(this), kCmdGetSimulationDate);
        FC4_RELEASE(msgServer_);
        msgServer_ = nullptr;
    }
    if (commandRegistered_) {  // step 5
        cIGZCommandServerPtr cs;
        if (cs) cs->UnregisterCommand(kCmdGetSimulationDate);
        commandRegistered_ = false;
    }
    if (addedToFramework_) {  // step 6
        if (cIGZFrameWork* fw = RZGetFrameWork()) {
            fw->RemoveFromTick(static_cast<cIGZSystemService*>(this));
            fw->RemoveSystemService(static_cast<cIGZSystemService*>(this));
        }
        addedToFramework_ = false;
    }
    serviceRunning_ = false;
    fc4::Log("Shutdown");
    fc4::RefSummary("simulator Shutdown");
    Release();  // may delete this; nothing may touch members afterwards
    return true;
}

void cFC4Simulator::RegisterCheats() {
    cIGZFrameWork* fw = RZGetFrameWork();
    cIGZApp* app = fw ? fw->Application() : nullptr;
    cISC4App* sc4 = nullptr;
    if (app == nullptr || !app->QueryInterface(kIID_cISC4App, reinterpret_cast<void**>(&sc4)) || sc4 == nullptr) return;
    FC4_ACQUIRED(sc4, "QueryInterface");
    cIGZCheatCodeManager* cm = sc4->GetCheatCodeManager();
    if (cm != nullptr && cm->QueryInterface(kIID_cIGZCheatCodeManager, reinterpret_cast<void**>(&cheatMgr_))) {
        FC4_ACQUIRED(cheatMgr_, "QueryInterface");
        // Spec §9 step 9 (flora.md §4.1 step 11, FQ11): each cheat is
        // subscribed with its own ID, never 0. The manager delivers a cheat
        // to the pairs whose ID equals it or is 0, so 0 would also deliver
        // every other cheat; a pair already present is refused.
        cIGZMessageTarget2* target = static_cast<cIGZMessageTarget2*>(this);
        cheatMgr_->RegisterCheatCode(kCheatFPS, cRZBaseString("FPS"));
        cheatMgr_->AddNotification2(target, kCheatFPS);
        if (sc4->GetDebugFunctionalityEnabled()) {
            cheatMgr_->RegisterCheatCode(kCheatSimDate, cRZBaseString("SimDate"));
            cheatMgr_->AddNotification2(target, kCheatSimDate);
            simDateCheatRegistered_ = true;
        }
    }
    FC4_RELEASE(sc4);
}

void cFC4Simulator::UnregisterCheats() {
    if (cheatMgr_ == nullptr) return;
    // Spec §9 Shutdown step 1 (FQ11): the same (target, ID) pairs as Init.
    cIGZMessageTarget2* target = static_cast<cIGZMessageTarget2*>(this);
    cheatMgr_->UnregisterCheatCode(kCheatFPS);
    cheatMgr_->RemoveNotification2(target, kCheatFPS);
    if (simDateCheatRegistered_) {
        cheatMgr_->UnregisterCheatCode(kCheatSimDate);
        cheatMgr_->RemoveNotification2(target, kCheatSimDate);
    }
    simDateCheatRegistered_ = false;
    FC4_RELEASE(cheatMgr_);
    cheatMgr_ = nullptr;
}

// ---- date -----------------------------------------------------------------------------------

bool cFC4Simulator::GetSimStartDate(cFC4IGZDate& sDate) {
    // [choice] The spec does not define the start date; we report the
    // initial simulation date, 2000-01-01.
    return sDate.SetMonthDayYear(1u, 1u, 2000u);  // slot 6
}

cFC4IGZDate* cFC4Simulator::GetSimDate() {
    // [choice] Returns an object owned by the simulator (no AddRef), refreshed
    // on every call.
    fc4::NoteOffMainThreadCall("GetSimDate");
    date_.SetJulian(fc4_sim_get_julian_day(sim_));
    if (dateCallsLogged_[0] < 20) {
        ++dateCallsLogged_[0];
        FC4_LOGV("GetSimDate() #%u -> date object %p (julian %d)", dateCallsLogged_[0], static_cast<void*>(&date_),
                 date_.Julian());
    }
    return &date_;
}

void cFC4Simulator::GetSimDateParts(uint32_t* year, uint32_t* month, uint32_t* day, uint32_t* dayOfYear,
                                    uint32_t* weekDay) {
    fc4::NoteOffMainThreadCall("GetSimDateParts");
    fc4_date d{};
    fc4_sim_get_date(sim_, &d);
    if (year) *year = static_cast<uint32_t>(d.year);
    if (month) *month = static_cast<uint32_t>(d.month);
    if (day) *day = static_cast<uint32_t>(d.day_of_month);
    if (dayOfYear) *dayOfYear = static_cast<uint32_t>(d.day_of_year);
    if (weekDay) *weekDay = static_cast<uint32_t>(d.weekday);
    if (dateCallsLogged_[1] < 20) {
        ++dateCallsLogged_[1];
        FC4_LOGV("GetSimDate(5 ptrs) #%u -> %d-%d-%d doy %d weekday %d (julian %d)", dateCallsLogged_[1], d.year,
                 d.month, d.day_of_month, d.day_of_year, d.weekday, d.julian);
    }
}

int32_t cFC4Simulator::GetSimDateNumber() {
    fc4::NoteOffMainThreadCall("GetSimDateNumber");
    const int32_t j = fc4_sim_get_julian_day(sim_);
    if (dateCallsLogged_[2] < 20) {
        ++dateCallsLogged_[2];
        FC4_LOGV("GetSimDateNumber() #%u -> %d", dateCallsLogged_[2], j);
    }
    return j;
}

// ---- pause and speed (spec §6) -------------------------------------------------------------

bool cFC4Simulator::TracePause(const char* name, bool ok) {
    fc4_state st{};
    st.struct_size = sizeof(st);
    fc4_sim_get_state(sim_, &st);
    fc4::PublishSimState(st.julian_day, st.sim_speed, st.pause_count, st.hidden_pause_count, st.emergency_pause_count);
    fc4::Log("%s() -> %d (pause=%d hidden=%d emergency=%d; julian %d speed %d)", name, ok ? 1 : 0, st.pause_count,
             st.hidden_pause_count, st.emergency_pause_count, st.julian_day, st.sim_speed);
    return ok;
}

bool cFC4Simulator::Pause() { return TracePause("Pause", fc4_sim_pause(sim_)); }
bool cFC4Simulator::HiddenPause() { return TracePause("HiddenPause", fc4_sim_hidden_pause(sim_)); }
bool cFC4Simulator::EmergencyPause() { return TracePause("EmergencyPause", fc4_sim_emergency_pause(sim_)); }
bool cFC4Simulator::Resume() { return TracePause("Resume", fc4_sim_resume(sim_)); }
bool cFC4Simulator::HiddenResume() { return TracePause("HiddenResume", fc4_sim_hidden_resume(sim_)); }
bool cFC4Simulator::EmergencyResume() { return TracePause("EmergencyResume", fc4_sim_emergency_resume(sim_)); }
bool cFC4Simulator::IsPaused() { fc4::NoteOffMainThreadCall("IsPaused"); return fc4_sim_is_paused(sim_); }
bool cFC4Simulator::IsHiddenPaused() { fc4::NoteOffMainThreadCall("IsHiddenPaused"); return fc4_sim_is_hidden_paused(sim_); }
bool cFC4Simulator::IsEmergencyPaused() { fc4::NoteOffMainThreadCall("IsEmergencyPaused"); return fc4_sim_is_emergency_paused(sim_); }
bool cFC4Simulator::IsAnyPaused() { fc4::NoteOffMainThreadCall("IsAnyPaused"); return fc4_sim_is_any_paused(sim_); }
int32_t cFC4Simulator::GetSimSpeed() { fc4::NoteOffMainThreadCall("GetSimSpeed"); return fc4_sim_get_sim_speed(sim_); }

bool cFC4Simulator::SetSimSpeed(int32_t lSpeed) {
    const bool ok = fc4_sim_set_sim_speed(sim_, lSpeed);
    fc4_state st{};
    st.struct_size = sizeof(st);
    fc4_sim_get_state(sim_, &st);
    fc4::PublishSimState(st.julian_day, st.sim_speed, st.pause_count, st.hidden_pause_count, st.emergency_pause_count);
    fc4::Log("SetSimSpeed(%d) -> %d (julian %d pause=%d hidden=%d emergency=%d)", lSpeed, ok ? 1 : 0, st.julian_day,
             st.pause_count, st.hidden_pause_count, st.emergency_pause_count);
    return ok;
}

int32_t cFC4Simulator::GetSimTime() { fc4::NoteOffMainThreadCall("GetSimTime"); return fc4_sim_get_sim_time(sim_); }
bool cFC4Simulator::SetSimTime(int32_t lTime) { return fc4_sim_set_sim_time(sim_, lTime); }
bool cFC4Simulator::SetMaxMillisecondsPerTick(uint32_t dwTime) { return fc4_sim_set_max_milliseconds_per_tick(sim_, dwTime); }
float cFC4Simulator::GetAnimationTimeDilation() { fc4::NoteOffMainThreadCall("GetAnimationTimeDilation"); return fc4_sim_get_animation_time_dilation(sim_); }
bool cFC4Simulator::SetCityEstablished(bool bEstablished) {
    FC4_LOGV("SetCityEstablished(%d)", bEstablished ? 1 : 0);
    return fc4_sim_set_city_established(sim_, bEstablished);
}

// ---- agents (spec §4) --------------------------------------------------------------------------

bool cFC4Simulator::AddAgent(cIGZMessageTarget2* pAgent, uint32_t agentType, cIGZString const& szAgentName,
                             uint32_t flags) {
    const std::string name = ToStd(szAgentName);
    if (pAgent == nullptr) {  // [choice]
        FC4_LOGV("AddAgent(null, level %u, '%s', flags %u) -> 0", static_cast<uint32_t>(agentType), name.c_str(),
                 static_cast<uint32_t>(flags));
        return false;
    }
    // Names are kept at every log level: the watchdog reports the agent being delivered to.
    Stats(pAgent, static_cast<uint32_t>(agentType)).name = name.empty() ? std::string("(unnamed)") : name;
    const bool ok = fc4_sim_add_agent(sim_, AgentHandle(pAgent), static_cast<uint32_t>(agentType), name.data(),
                                      name.size(), static_cast<uint32_t>(flags));
    // A fixed-step idle agent gets its N SimIdle per day whether it is the
    // original's or ours (glue-propdev4): FreeCity4's prop developer also
    // does exactly one block step per Idle (prop-developer.md §3.3), so with
    // the single SimIdle of our other (run-to-completion) idle agents it did
    // 1 block step per day against the original's 20 under deterministic pacing.
    if (ok && agentType == 10) {
        const uint32_t steps = fc4::PacingFixedIdleSteps(name.c_str());
        if (steps != 0) {
            fixedIdle_[reinterpret_cast<uintptr_t>(pAgent)] = steps;
            fc4::Log("pacing: the %s idle agent '%s' (%p) gets %u SimIdle per day at the settle point",
                     fc4::PacingIsOurs(pAgent) ? "FreeCity4" : "original's", name.c_str(),
                     static_cast<void*>(pAgent), steps);
        }
    }
    FC4_LOGV("AddAgent(%p, level %u, '%s', flags %u) -> %d", static_cast<void*>(pAgent),
             static_cast<uint32_t>(agentType), name.c_str(), static_cast<uint32_t>(flags), ok ? 1 : 0);
    return ok;
}

bool cFC4Simulator::RemoveAgent(cIGZMessageTarget2* pAgent, uint32_t agentType) {
    const bool ok = fc4_sim_remove_agent(sim_, AgentHandle(pAgent), static_cast<uint32_t>(agentType));
    if (agentType == 10) fixedIdle_.erase(reinterpret_cast<uintptr_t>(pAgent));
    FC4_LOGV("RemoveAgent(%p, level %u) -> %d", static_cast<void*>(pAgent), static_cast<uint32_t>(agentType),
             ok ? 1 : 0);
    return ok;
}

bool cFC4Simulator::RemoveAgentAllLevels(cIGZMessageTarget2* pAgent) {
    const bool ok = fc4_sim_remove_agent_all_levels(sim_, AgentHandle(pAgent));
    fixedIdle_.erase(reinterpret_cast<uintptr_t>(pAgent));
    FC4_LOGV("RemoveAgent(%p, all levels) -> %d", static_cast<void*>(pAgent), ok ? 1 : 0);
    return ok;
}

bool cFC4Simulator::RemoveAllAgents() {
    fixedIdle_.clear();
    return fc4_sim_remove_all_agents(sim_);
}

bool cFC4Simulator::RemoveAllAgentsAt(uint32_t agentType) {
    if (agentType == 10) fixedIdle_.clear();
    return fc4_sim_remove_all_agents_at(sim_, static_cast<uint32_t>(agentType));
}

namespace {
struct NamesFill {
    fc4::GameVector<fc4::GameString>* v;
    bool ok;
};

void AppendName(void* ctx, const char* name, size_t len) {
    auto* f = static_cast<NamesFill*>(ctx);
    if (!f->ok) return;
    fc4::GameString s{};
    if (!fc4::GameStringInit(s, name, len)) {
        f->ok = false;
        return;
    }
    if (!f->v->push_back(s)) {  // bitwise: the vector owns the B buffer now
        fc4::GameStringDestroy(s);
        f->ok = false;
    }
}
}  // namespace

// [abi] std::vector<cRZString> (B buffer, B character buffers): allocators.md
// table (2). Cleared as the original does (strings destroyed with B, capacity
// kept), then appended.
bool cFC4Simulator::EnumerateAgentsByName(fc4::GameVector<fc4::GameString>& sAgents) {
    if (!fc4::StlNodeAllocator::Available() || fc4::StlNodeAllocator::StringVtable() == nullptr) {
        static bool logged = false;
        if (!logged) {
            logged = true;
            fc4::Log("simulator: WARNING EnumerateAgentsByName refused: the game's container allocator is unavailable");
        }
        return false;
    }
    sAgents.clear();
    NamesFill fill{&sAgents, true};
    fc4_sim_enumerate_agents(sim_, &AppendName, &fill);
    return fill.ok;
}

bool cFC4Simulator::GetAgentEnabled(cIGZString const& szAgentName) {
    const std::string name = ToStd(szAgentName);
    return fc4_sim_get_agent_enabled(sim_, name.data(), name.size());
}

bool cFC4Simulator::SetAgentEnabled(cIGZString const& szAgentName, bool bEnabled) {
    const std::string name = ToStd(szAgentName);
    return fc4_sim_set_agent_enabled(sim_, name.data(), name.size(), bEnabled);
}

// ---- persistence (spec §8) -------------------------------------------------------------------------

bool cFC4Simulator::Write(cIGZOStream& gzOut) {
    FC4_LOGV("Write() called (stream %p, error %d)", static_cast<void*>(&gzOut), gzOut.GetError());
    uint8_t buf[256];
    const size_t size = fc4_sim_save(sim_, buf, sizeof(buf));
    const uint8_t* widths = nullptr;
    const size_t count = fc4_record_layout(static_cast<uint16_t>(buf[0] | (buf[1] << 8)), &widths);
    if (size > sizeof(buf) || count == 0) return false;
    size_t pos = 0;
    for (size_t i = 0; i < count && pos + widths[i] <= size; ++i) {
        switch (widths[i]) {
            case 1: gzOut.SetUint8(buf[pos]); break;
            case 2: gzOut.SetUint16(static_cast<uint16_t>(buf[pos] | (buf[pos + 1] << 8))); break;
            case 4:
                gzOut.SetUint32(static_cast<uint32_t>(buf[pos]) | (static_cast<uint32_t>(buf[pos + 1]) << 8) |
                                (static_cast<uint32_t>(buf[pos + 2]) << 16) | (static_cast<uint32_t>(buf[pos + 3]) << 24));
                break;
            default: return false;
        }
        pos += widths[i];
    }
    const bool ok = pos == size && gzOut.GetError() == 0;
    fc4::Log("Write: %u bytes, ok=%d", static_cast<unsigned>(size), ok ? 1 : 0);
    return ok;
}

bool cFC4Simulator::Read(cIGZIStream& gzIn) {
    FC4_LOGV("Read() called (stream %p, error %d)", static_cast<void*>(&gzIn), gzIn.GetError());
    uint16_t version = 0;
    if (!gzIn.GetUint16(version)) return false;
    const uint8_t* widths = nullptr;
    const size_t count = fc4_record_layout(version, &widths);
    if (count == 0) {
        fc4::Log("Read: unsupported simulator record version %u", version);
        return false;
    }
    uint8_t buf[256];
    size_t pos = 0;
    buf[pos++] = static_cast<uint8_t>(version & 0xFF);
    buf[pos++] = static_cast<uint8_t>(version >> 8);
    for (size_t i = 1; i < count; ++i) {
        uint32_t v = 0;
        switch (widths[i]) {
            case 1: { uint8_t x = 0; if (!gzIn.GetUint8(x)) return false; v = x; break; }
            case 2: { uint16_t x = 0; if (!gzIn.GetUint16(x)) return false; v = x; break; }
            case 4: { if (!gzIn.GetUint32(v)) return false; break; }
            default: return false;
        }
        for (uint8_t b = 0; b < widths[i]; ++b) buf[pos++] = static_cast<uint8_t>(v >> (8 * b));
    }
    const int32_t r = fc4_sim_load(sim_, buf, pos);
    fc4::Log("Read: version %u, %u bytes, result %d, julian %d", version, static_cast<unsigned>(pos), r,
             fc4_sim_get_julian_day(sim_));
    return r == FC4_OK && gzIn.GetError() == 0;
}

uint32_t cFC4Simulator::GetGZCLSID() { return kFC4_CLSID_cSC4Simulator; }

// ---- cIGZSystemService ---------------------------------------------------------------------------------

uint32_t cFC4Simulator::GetServiceID() { return serviceId_; }

cIGZSystemService* cFC4Simulator::SetServiceID(uint32_t dwServiceId) {
    serviceId_ = dwServiceId;
    return static_cast<cIGZSystemService*>(this);
}

int32_t cFC4Simulator::GetServicePriority() { return kServicePriority; }
bool cFC4Simulator::IsServiceRunning() { return serviceRunning_; }

cIGZSystemService* cFC4Simulator::SetServiceRunning(bool bRunning) {
    serviceRunning_ = bRunning;
    return static_cast<cIGZSystemService*>(this);
}

bool cFC4Simulator::OnTick(uint32_t unknown1) {
    fc4::TickScope tick;
    FC4_X87_SCOPE("simulator OnTick", 0);  // x87 leftover guard at the entry and the end (FC4Fpu.h)
    ++ticks_;
    if (ticks_ == 1 || ticks_ % 1000 == 0) {
        fc4_state st{};
        st.struct_size = sizeof(st);
        fc4_sim_get_state(sim_, &st);
        FC4_LOGV("OnTick #%u (arg %u): julian=%d speed=%d pause=%d hidden=%d emergency=%d budget=%u msPerDay=%u "
                 "agents[day]=%u",
                 ticks_, unknown1, st.julian_day, st.sim_speed, st.pause_count, st.hidden_pause_count,
                 st.emergency_pause_count, st.frame_budget_ms, st.ms_per_day, st.agent_counts[FC4_LEVEL_DAY]);
    }
    bool r;
    {
        fc4::ActivityScope core("core fc4_sim_on_tick");
        const uint64_t perfT0 = fc4::perf::On() ? fc4::perf::Now() : 0;  // tooling: perf_profile (FC4Perf.h)
        r = fc4_sim_on_tick(sim_);
        if (perfT0 != 0) fc4::perf::AddTick(fc4::perf::Now() - perfT0);
    }
    {
        // Cheap snapshot for the watchdog's heartbeat and stall rule.
        fc4_state st{};
        st.struct_size = sizeof(st);
        fc4_sim_get_state(sim_, &st);
        fc4::PublishSimState(st.julian_day, st.sim_speed, st.pause_count, st.hidden_pause_count,
                             st.emergency_pause_count);
    }
    if (ticks_ % StatusEveryTicks() == 0 && fc4::LogLevel() >= fc4::kLogVerbose) LogSchedulerStatus();
    if (turboUsed_) TurboAfterTick();  // tooling: sim_turbo_until (FC4Turbo.h)
    if (turboHeld_) {
        // Tooling: the fallback of a request held for ConnectionsReady (FC4Turbo.h).
        const int32_t wait = fc4::TurboConfig().waitDays;
        if (wait > 0 && fc4_sim_get_julian_day(sim_) - turboHeldSince_ >= wait) EngageHeldTurbo(true);
    }
    return r;
}

// ---- the turbo clock (tooling, FC4Turbo.h; not simulator.md) -----------------------------------------------

void cFC4Simulator::TurboResetReference(int32_t julian) {
    turboRefMs_ = HostNowMs(nullptr);
    turboRefTicks_ = ticks_;
    turboRefJulian_ = julian;
}

void cFC4Simulator::SetTurbo(bool on, int32_t untilJulian, const char* why) {
    if (sim_ == nullptr) return;
    const uint32_t frameMs = fc4::TurboConfig().frameMs;
    fc4_sim_set_turbo(sim_, on, frameMs, on ? untilJulian : 0);
    const int32_t julian = fc4_sim_get_julian_day(sim_);
    char date[16];
    DateOf(julian, date);
    if (on) {
        turboUsed_ = true;
        TurboResetReference(julian);
        char until[16] = "none";
        if (untilJulian != 0) DateOf(untilJulian, until);
        fc4::Log("simulator: turbo ON (%s) at %s: frame budget %u ms, until %s", why, date, frameMs, until);
    } else {
        fc4::Log("simulator: turbo OFF (%s) at %s", why, date);
    }
}

bool cFC4Simulator::RequestTurbo(int32_t untilJulian, const char* why) {
    if (sim_ == nullptr) return false;
    if (cityReady_) {
        SetTurbo(true, untilJulian, why);
        return true;
    }
    turboHeld_ = true;
    turboHeldUntil_ = untilJulian;
    turboHeldWhy_ = why;
    turboHeldSince_ = fc4_sim_get_julian_day(sim_);
    char date[16];
    DateOf(turboHeldSince_, date);
    const int32_t wait = fc4::TurboConfig().waitDays;
    if (wait > 0) {
        fc4::Log("simulator: turbo requested (%s) at %s: held, days paced by msPerDay, until the city's first "
                 "ConnectionsReady (or %d game day(s) without it, sim_turbo_wait_days)",
                 why, date, wait);
    } else {
        fc4::Log("simulator: turbo requested (%s) at %s: held, days paced by msPerDay, until the city's first "
                 "ConnectionsReady (no fallback: sim_turbo_wait_days = 0)",
                 why, date);
    }
    return false;
}

bool cFC4Simulator::DropHeldTurbo(const char* why) {
    if (!turboHeld_) return false;
    turboHeld_ = false;
    char date[16];
    DateOf(fc4_sim_get_julian_day(sim_), date);
    fc4::Log("simulator: turbo OFF (%s) at %s: the held request (%s) is dropped", why, date, turboHeldWhy_);
    return true;
}

void cFC4Simulator::TurboCityReady() {
    if (cityReady_) return;  // only the first ConnectionsReady of the load counts
    cityReady_ = true;
    if (turboHeld_) EngageHeldTurbo(false);
}

void cFC4Simulator::EngageHeldTurbo(bool fallback) {
    turboHeld_ = false;
    const uint32_t frameMs = fc4::TurboConfig().frameMs;
    fc4_sim_set_turbo(sim_, true, frameMs, turboHeldUntil_);
    const int32_t julian = fc4_sim_get_julian_day(sim_);
    turboUsed_ = true;
    TurboResetReference(julian);
    char date[16], until[16] = "none";
    DateOf(julian, date);
    if (turboHeldUntil_ != 0) DateOf(turboHeldUntil_, until);
    if (fallback) {
        fc4::Log("simulator: WARNING no ConnectionsReady within %d game day(s) of the turbo request (%s); turbo ON at "
                 "%s anyway (sim_turbo_wait_days): frame budget %u ms, until %s",
                 fc4::TurboConfig().waitDays, turboHeldWhy_, date, frameMs, until);
    } else {
        fc4::Log("simulator: turbo ON at %s (after ConnectionsReady; %s): frame budget %u ms, until %s", date,
                 turboHeldWhy_, frameMs, until);
    }
}

void cFC4Simulator::TurboMonthLine(int32_t julian) {
    bool on = false;
    if (!fc4_sim_get_turbo(sim_, &on, nullptr, nullptr) || !on) return;
    const uint32_t ms = HostNowMs(nullptr) - turboRefMs_;
    const uint32_t frames = ticks_ - turboRefTicks_;
    const int32_t days = julian - turboRefJulian_;
    char date[16];
    DateOf(julian, date);
    fc4::Log("simulator: turbo month %s: %d day(s) in %u.%03u s real = %s days/s, %s fps (%u frames)", date, days,
             ms / 1000, ms % 1000,
             fc4::harness::FormatPer100(fc4::harness::RatePer100(days > 0 ? static_cast<uint64_t>(days) : 0, ms)).c_str(),
             fc4::harness::FormatPer100(fc4::harness::RatePer100(frames, ms)).c_str(), frames);
    fc4::perf::Report(date, days, ms);  // tooling: perf_profile (FC4Perf.h)
    TurboResetReference(julian);
}

void cFC4Simulator::TurboAfterTick() {
    const int32_t off = fc4_sim_take_turbo_auto_off(sim_);
    if (off == 0) return;
    char date[16];
    DateOf(off, date);
    fc4::Log("simulator: turbo OFF at %s (sim_turbo_until reached); days paced by msPerDay again", date);
}
bool cFC4Simulator::OnIdle(uint32_t) { return true; }  // [choice] no idle-tick work
int32_t cFC4Simulator::GetServiceTickPriority() { return kServicePriority; }  // [choice]

// ---- cIGZMessageTarget2 (spec §9) -----------------------------------------------------------------------

bool cFC4Simulator::DoMessage(cIGZMessage2* pMessage) {
    if (pMessage == nullptr) return true;
    FC4_X87_SCOPE("simulator DoMessage", pMessage->GetType());  // x87 leftover guard (FC4Fpu.h)
    if (messagesLogged_ < 50) {
        ++messagesLogged_;
        FC4_LOGV("DoMessage(type=0x%08X)", pMessage->GetType());
    } else if (fc4::LogLevel() >= fc4::kLogVerbose) {
        fc4::NoteSuppressed();
    }
    switch (pMessage->GetType()) {
        case kMsgPostCityInit:
            // Spec §9 (erratum): the city finished Init. Unsubscribe and
            // release the hidden pause set by Init so the clock starts.
            // [choice] A copy that was already queued when we unsubscribed is
            // ignored, so the hidden pause is released only once per Init.
            if (postCityInitSubscribed_) {
                postCityInitSubscribed_ = false;
                if (msgServer_ != nullptr) {
                    msgServer_->RemoveNotification(static_cast<cIGZMessageTarget2*>(this), kMsgPostCityInit);
                }
                fc4_sim_on_post_city_init(sim_);
                fc4::SetCityRunning(true);
                fc4::Log("PostCityInit: hidden pause released (hidden=%d)", fc4_sim_is_hidden_paused(sim_) ? 1 : 0);
                // The power shadow (opt-in) of a city start: the original's
                // unsaved values are known now (FC4PowerShadow.h).
                fc4::PowerShadowCityInit();
                // The plumbing shadow (opt-in) likewise: the original's pipe
                // condition is 0 now (FC4PlumbingShadow.h).
                fc4::PlumbingShadowCityInit();
            }
            break;

        case kCmdGetSimulationDate:
            // TODO(spec §9): fill the command's output parameters "year", "month",
            // "day of month", "day of week", "day of year" and "number". The SDK
            // does not describe how a command message carries its parameter
            // sets, nor how parameter names map to cIGZCommandParameterSet IDs,
            // so the values (fc4_sim_get_date) cannot be returned yet.
            break;

        case kMsgCheatIssued: {
            cIGZMessage2Standard* m = nullptr;
            if (pMessage->QueryInterface(GZIID_cIGZMessage2Standard, reinterpret_cast<void**>(&m)) && m != nullptr) {
                FC4_ACQUIRED(m, "QueryInterface");
                const uint32_t id = static_cast<uint32_t>(m->GetData1());
                const cIGZString* text = static_cast<const cIGZString*>(m->GetVoid2());
                const std::string s = text ? ToStd(*text) : std::string();
                if (id == kCheatFPS || id == kCheatSimDate) {
                    fc4_sim_handle_cheat(sim_, id, s.data(), s.size());
                    fc4::Log("cheat %08X '%s'", id, s.c_str());
                }
                FC4_RELEASE(m);
            }
            break;
        }

        default:
            break;
    }
    return true;
}
