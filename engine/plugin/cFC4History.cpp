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

#include "cFC4History.h"

#include "FC4Diag.h"
#include "FC4Fpu.h"
#include "FC4Log.h"
#include "FC4RefAudit.h"
#include "FC4Streams.h"
#include "cFC4Message2Standard.h"

#include "GZServPtrs.h"
#include "cIGZApp.h"
#include "cIGZFrameWork.h"
#include "cIGZMessage2Standard.h"
#include "cIGZMessageServer2.h"
#include "cISC4App.h"
#include "cISC4City.h"
#include "cISC4Simulator.h"
#include "cRZBaseString.h"
#include "cRZCOMDllDirector.h"

#include <string>

namespace {

constexpr uint32_t kMsgRecord = 0x89EFA536;    // §3: the record (data1 key, data2 value)
constexpr uint32_t kMsgSimNewDay = 0x66956814;  // §3: the thinning pass step

cFC4HistoryWarehouse* Self(void* u) { return static_cast<cFC4HistoryWarehouse*>(u); }

cISC4App* App() {
    cIGZFrameWork* fw = RZGetFrameWork();
    cIGZApp* app = fw ? fw->Application() : nullptr;
    cISC4App* sc4 = nullptr;
    if (app != nullptr && app->QueryInterface(GZIID_cISC4App, reinterpret_cast<void**>(&sc4)) && sc4 != nullptr) {
        FC4_ACQUIRED(sc4, "QueryInterface");
        return sc4;  // the caller releases it
    }
    return nullptr;
}

uint32_t DayOf(cFC4IGZDate const& d) { return const_cast<cFC4IGZDate&>(d).DayNumber(); }  // slot 26

// ---- host callbacks (fc4_history_host) ------------------------------------------------------------------
//
// The core runs under the lock: the game calls it asks for are queued
// (CoreLock::Defer) and the date is the one read before the call.

uint32_t SimDate(void* u) { return Self(u)->CachedDate(); }

uint32_t NowMs(void*) {
    static LARGE_INTEGER freq = {};
    if (freq.QuadPart == 0) QueryPerformanceFrequency(&freq);
    LARGE_INTEGER now;
    QueryPerformanceCounter(&now);
    return static_cast<uint32_t>((static_cast<unsigned long long>(now.QuadPart) * 1000ull) /
                                 static_cast<unsigned long long>(freq.QuadPart));
}

void Subscribe(void* u, uint32_t type) {
    cFC4HistoryWarehouse* s = Self(u);
    s->Lock().Defer([s, type] { s->Subscribe(type, true); });
}

void Unsubscribe(void* u, uint32_t type) {
    cFC4HistoryWarehouse* s = Self(u);
    s->Lock().Defer([s, type] { s->Subscribe(type, false); });
}

void AddAgentCb(void* u, uint32_t level, const char* name, size_t len, uint32_t flags) {
    cFC4HistoryWarehouse* s = Self(u);
    std::string n(name != nullptr ? name : "", name != nullptr ? len : 0);
    s->Lock().Defer([s, level, n, flags] { s->Agent(level, n.c_str(), flags, true); });
}

void RemoveAgentCb(void* u) {
    cFC4HistoryWarehouse* s = Self(u);
    s->Lock().Defer([s] { s->Agent(0, "", 0, false); });
}

// RecordHistoryData (§4.1): never under the lock (fc4_history_record holds no
// borrow), so the message may come straight back to DoMessage.
void SendCb(void* u, uint32_t type, uint32_t d1, uint32_t d2) { Self(u)->Send(type, d1, d2); }

fc4_history_host MakeHost(cFC4HistoryWarehouse* self) {
    fc4_history_host h{};
    h.struct_size = sizeof(h);
    h.user = self;
    h.sim_date = &SimDate;
    h.now_ms = &NowMs;
    h.subscribe = &Subscribe;
    h.unsubscribe = &Unsubscribe;
    h.add_agent = &AddAgentCb;
    h.remove_agent = &RemoveAgentCb;
    h.send = &SendCb;
    return h;
}

}  // namespace

cIGZUnknown* CreateFC4HistoryWarehouse() {
    return static_cast<cIGZUnknown*>(static_cast<cFC4ISC4HistoryWarehouse*>(new cFC4HistoryWarehouse()));
}

cFC4HistoryWarehouse::cFC4HistoryWarehouse() {
    host_ = MakeHost(this);
    core_ = fc4_history_create(&host_);
    if (core_ == nullptr) fc4::Log("cFC4HistoryWarehouse: core creation failed");
    fc4::Log("cFC4HistoryWarehouse(%p) created, core %p", static_cast<void*>(Primary()), static_cast<void*>(core_));
}

cFC4HistoryWarehouse::~cFC4HistoryWarehouse() {
    fc4_history_destroy(core_);
    core_ = nullptr;
    fc4::Log("cFC4HistoryWarehouse destroyed (%u records sent, %u queries, %u off-main calls)", records_.load(),
             queries_.load(), lock_.OffMainCalls());
}

// §1: 0x69F4A803 and 1 (the primary), 0xE4FDA3D4 (+8) and nothing else: not
// the message target's 0x452294AA, although the sub-object exists [quirk].
// The out pointer is left untouched on a refusal.
bool cFC4HistoryWarehouse::QueryInterface(uint32_t riid, void** ppvObj) {
    if (ppvObj == nullptr || !fc4_history_answers_iid(riid)) return false;
    if (riid == GZIID_cIGZSerializable) {
        *ppvObj = static_cast<cIGZSerializable*>(this);
    } else {
        *ppvObj = Primary();  // 0x69F4A803 and 1
    }
    AddRef();
    return true;
}

uint32_t cFC4HistoryWarehouse::AddRef() { return ++refCount_; }

// §1: at a count of 1 or less the object is destroyed (the count is not
// decremented first) and 0 returned. [choice] (HW13, re-answers11) The
// original's Release does not call Shutdown (its destructor frees only the
// series map and relies on the city's Shutdown); a warehouse still
// initialised is shut down first here, so no subscription or agent is left
// on a deleted object. The same in the normal order.
uint32_t cFC4HistoryWarehouse::Release() {
    if (refCount_ > 1) return --refCount_;
    refCount_ = 0;
    if (destroying_) return 0;  // a release from inside the final Shutdown
    destroying_ = true;
    if (State().initialized != 0) Shutdown();
    delete this;
    return 0;
}

fc4_history_state cFC4HistoryWarehouse::State() {
    fc4_history_state st{};
    st.struct_size = sizeof(st);
    fc4::CoreLock::Section s(lock_, "history state");
    fc4_history_get_state(core_, &st);
    return st;
}

// ---- the game calls the core asks for --------------------------------------------------------------------

void cFC4HistoryWarehouse::Subscribe(uint32_t type, bool add) {
    if (const FC4HistClockHooks* h = fc4::HistClockHooks()) {
        if (h->notification != nullptr) h->notification(h->ctx, Target(), type, add);
        return;
    }
    if (msgServer_ == nullptr) return;
    if (add) {
        msgServer_->AddNotification(Target(), type);
    } else {
        msgServer_->RemoveNotification(Target(), type);
    }
}

void cFC4HistoryWarehouse::Agent(uint32_t level, const char* name, uint32_t flags, bool add) {
    if (const FC4HistClockHooks* h = fc4::HistClockHooks()) {
        if (h->agent != nullptr) h->agent(h->ctx, Target(), level, name, flags, add);
        return;
    }
    cFC4ISC4Simulator* sim = fc4::AsGame(simulator_);
    if (sim == nullptr) {
        if (add) fc4::Log("history: no simulator; agent level %u not registered", level);
        return;
    }
    if (add) {
        sim->AddAgent(Target(), level, cRZBaseString(name), flags);
    } else {
        sim->RemoveAgentAllLevels(Target());
    }
}

// §4.1: a standard message (type, data1 = key, data2 = value), sent
// synchronously through the message server, then released.
void cFC4HistoryWarehouse::Send(uint32_t type, uint32_t data1, uint32_t data2) {
    ++records_;
    const FC4HistClockHooks* h = fc4::HistClockHooks();
    cIGZMessageServer2Ptr ms;
    if (h == nullptr && !ms) return;  // no message server: nothing is recorded (as in the original)
    cFC4Message2Standard* msg = FC4MakeMessage(type, static_cast<intptr_t>(data1), static_cast<intptr_t>(data2), 0);
    if (h != nullptr) {
        if (h->send != nullptr) h->send(h->ctx, msg);
    } else {
        ms->MessageSend(msg);
    }
    FC4_X87_CHECK("history MessageSend");  // x87 leftover guard after our synchronous send (FC4Fpu.h)
    msg->Release();
}

// The simulator's date number (simulator slot 8), read outside the lock.
uint32_t cFC4HistoryWarehouse::SimDate() {
    if (const FC4HistClockHooks* h = fc4::HistClockHooks()) return h->sim_date ? h->sim_date(h->ctx) : 0;
    if (cFC4ISC4Simulator* sim = fc4::AsGame(simulator_)) return static_cast<uint32_t>(sim->GetSimDateNumber());
    // Before Init (or after Shutdown): the current city's simulator, borrowed.
    uint32_t date = 0;
    if (cISC4App* app = App()) {
        cISC4City* city = app->GetCity();
        cFC4ISC4Simulator* sim = city ? fc4::AsGame(city->GetSimulator()) : nullptr;
        if (sim != nullptr) date = static_cast<uint32_t>(sim->GetSimDateNumber());
        FC4_RELEASE(app);
    }
    return date;
}

// ---- lifecycle (§3) ----------------------------------------------------------------------------------------

// §3: no "already initialized" test - a second Init subscribes and registers
// the agents again, as the original does.
bool cFC4HistoryWarehouse::Init() {
    FC4_TRACE("HistoryWarehouse::Init");
    if (fc4::HistClockHooks() == nullptr) {
        if (msgServer_ == nullptr) {
            cIGZMessageServer2Ptr ms;
            if (ms) {
                msgServer_ = ms;
                FC4_ADDREF(msgServer_);
            }
        }
        if (city_ == nullptr) {
            if (cISC4App* app = App()) {
                city_ = app->GetCity();
                FC4_ADDREF(city_);
                FC4_RELEASE(app);
            }
        }
        if (simulator_ == nullptr && city_ != nullptr) {
            simulator_ = city_->GetSimulator();
            FC4_ADDREF(simulator_);
        }
    }
    bool ok;
    {
        fc4::CoreLock::Section s(lock_, "history Init");
        ok = fc4_history_init(core_);  // subscriptions and agents: queued, made when the section ends
    }
    const fc4_history_state st = State();
    fc4::Log("history Init: ok=%d city=%p simulator=%p server=%p; %u series, %u points", ok ? 1 : 0,
             static_cast<void*>(city_), static_cast<void*>(simulator_), static_cast<void*>(msgServer_),
             st.series_count, st.total_points);
    return true;
}

// §3: unsubscribe, remove the agents, initialized := 0; the series stay.
bool cFC4HistoryWarehouse::Shutdown() {
    FC4_TRACE("HistoryWarehouse::Shutdown");
    if (!destroying_) AddRef();  // keep alive while the server and the simulator let go
    {
        fc4::CoreLock::Section s(lock_, "history Shutdown");
        fc4_history_shutdown(core_);
    }
    FC4_RELEASE(simulator_);
    simulator_ = nullptr;
    FC4_RELEASE(city_);
    city_ = nullptr;
    FC4_RELEASE(msgServer_);
    msgServer_ = nullptr;
    const fc4_history_state st = State();
    fc4::Log("history Shutdown: %u series, %u points; %u records sent, %u queries, %u off-main calls",
             st.series_count, st.total_points, records_.load(), queries_.load(), lock_.OffMainCalls());
    if (!destroying_) Release();
    return true;
}

// ---- queries (§4.6, §4.7): the dates are read before the section ----------------------------------------

bool cFC4HistoryWarehouse::GetCount(uint32_t key, cFC4IGZDate const& from, cFC4IGZDate const& to, float& n) {
    const uint32_t f = DayOf(from), t = DayOf(to);
    ++queries_;
    fc4::CoreLock::Section s(lock_, "history GetCount");
    return fc4_history_get_count(core_, key, f, t, &n);
}

bool cFC4HistoryWarehouse::GetTrend(uint32_t key, cFC4IGZDate const& from, cFC4IGZDate const& to, float& slope,
                                    float& intercept) {
    const uint32_t f = DayOf(from), t = DayOf(to);
    const uint32_t prec = fc4::CallerPrecisionBits();
    ++queries_;
    fc4::CoreLock::Section s(lock_, "history GetTrend");
    return fc4_history_get_trend(core_, key, f, t, prec, &slope, &intercept);
}

bool cFC4HistoryWarehouse::GetMinMaxAverage(uint32_t key, cFC4IGZDate const& from, cFC4IGZDate const& to,
                                            float& min, float& max, float& avg) {
    const uint32_t f = DayOf(from), t = DayOf(to);
    const uint32_t prec = fc4::CallerPrecisionBits();
    ++queries_;
    fc4::CoreLock::Section s(lock_, "history GetMinMaxAverage");
    return fc4_history_get_min_max_average(core_, key, f, t, prec, &min, &max, &avg);
}

// [HW2] The graph window (`cSC4GraphsViewWinProc`) reads a series through this
// slot only — one call per sample date, never `GetCount` — so every curve the
// game draws is a sequence of these answers. The core returns the regression
// value through the sample and its two neighbours at an interior point, not the
// recorded value there (history.md §4.7, §4.8); that smoothing is what the
// graphs show and must not be "corrected" to the recorded value.
bool cFC4HistoryWarehouse::GetPoint(uint32_t key, cFC4IGZDate const& date, float& v) {
    const uint32_t d = DayOf(date);
    const uint32_t prec = fc4::CallerPrecisionBits();
    ++queries_;
    fc4::CoreLock::Section s(lock_, "history GetPoint");
    return fc4_history_get_point(core_, key, d, prec, &v);
}

// ---- edits (§4.2, §4.4, §4.5) --------------------------------------------------------------------------------

bool cFC4HistoryWarehouse::Coalesce(uint32_t key, cFC4IGZDate const& from, cFC4IGZDate const& to,
                                    int32_t granularity) {
    const uint32_t f = DayOf(from), t = DayOf(to);
    fc4::CoreLock::Section s(lock_, "history Coalesce");
    return fc4_history_coalesce(core_, key, f, t, granularity);
}

bool cFC4HistoryWarehouse::DisposeAll(uint32_t key) {
    fc4::CoreLock::Section s(lock_, "history Dispose");
    return fc4_history_dispose(core_, key);
}

bool cFC4HistoryWarehouse::DisposeRange(uint32_t key, cFC4IGZDate const& from, cFC4IGZDate const& to) {
    const uint32_t f = DayOf(from), t = DayOf(to);
    fc4::CoreLock::Section s(lock_, "history Dispose(range)");
    return fc4_history_dispose_range(core_, key, f, t);
}

// §4.1: sends the record message and does not touch the series; outside the
// lock. Returns false (HW12 [compat]: the original returns the final
// Release's EAX, 0 once the message is destroyed; no caller reads it).
bool cFC4HistoryWarehouse::RecordHistoryData(uint32_t key, int32_t value) {
    return fc4_history_record(core_, key, value);
}

bool cFC4HistoryWarehouse::SetHistoryDataQueueLength(uint32_t key, uint32_t n) {
    fc4::CoreLock::Section s(lock_, "history SetHistoryDataQueueLength");
    return fc4_history_set_queue_length(core_, key, n);
}

// ---- messages (§3) -------------------------------------------------------------------------------------------

bool cFC4HistoryWarehouse::DoMessage(cIGZMessage2* pMessage) {
    if (pMessage == nullptr || core_ == nullptr) return true;
    FC4_X87_SCOPE("history DoMessage", pMessage->GetType());  // x87 leftover guard at the entry and exit (FC4Fpu.h)
    const uint32_t type = pMessage->GetType();
    uint32_t data1 = 0, data2 = 0;
    cIGZMessage2Standard* m = nullptr;
    if (pMessage->QueryInterface(GZIID_cIGZMessage2Standard, reinterpret_cast<void**>(&m)) && m != nullptr) {
        FC4_ACQUIRED(m, "QueryInterface");
        data1 = static_cast<uint32_t>(m->GetData1());
        data2 = static_cast<uint32_t>(m->GetData2());
        FC4_RELEASE(m);
    }
    // Store (§4.1) and the yearly pass (§4.3) read the date: before the lock.
    const uint32_t date = (type == kMsgRecord || type == kMsgSimNewDay) ? SimDate() : 0;
    fc4::CoreLock::Section s(lock_, "history DoMessage");
    date_ = date;
    return fc4_history_do_message(core_, type, data1, data2);
}

// ---- persistence (§7) ----------------------------------------------------------------------------------------
//
// The game stream is written / read under the lock (on the main thread; the
// stream never waits for another thread).

bool cFC4HistoryWarehouse::Write(cIGZOStream& s) {
    FC4_TRACE("HistoryWarehouse::Write");
    fc4::OStreamAdapter a(s);
    bool ok;
    fc4::MemOStream mem;
    {
        fc4::CoreLock::Section sec(lock_, "history Write");
        ok = fc4_history_write(core_, a.Get());
        fc4_history_write(core_, mem.Get());
    }
    // The record read back by a scratch core and written again (§7): the
    // save's bytes must survive a load.
    bool same = false;
    {
        fc4_history_host h{};
        h.struct_size = sizeof(h);
        fc4_history* scratch = fc4_history_create(&h);
        fc4::MemIStream in(mem.bytes);
        fc4::MemOStream again;
        if (scratch != nullptr && fc4_history_read(scratch, in.Get()) && in.Remaining() == 0 &&
            fc4_history_write(scratch, again.Get())) {
            same = again.bytes == mem.bytes;
        }
        fc4_history_destroy(scratch);
    }
    const fc4_history_state st = State();
    fc4::Log("history Write -> %d (%u series, %u points; %u bytes, read back and rewritten %s)", ok ? 1 : 0,
             st.series_count, st.total_points, static_cast<unsigned>(mem.bytes.size()),
             same ? "identical" : "DIFFERENT");
    return ok;
}

// §7: versions 0-2; Read runs before Init and adds to the series already there.
bool cFC4HistoryWarehouse::Read(cIGZIStream& s) {
    FC4_TRACE("HistoryWarehouse::Read");
    fc4::IStreamAdapter a(s);
    bool ok;
    {
        fc4::CoreLock::Section sec(lock_, "history Read");
        ok = fc4_history_read(core_, a.Get());
    }
    const fc4_history_state st = State();
    fc4::Log("history Read -> %d (%u series, %u points)", ok ? 1 : 0, st.series_count, st.total_points);
    return ok;
}

uint32_t cFC4HistoryWarehouse::GetGZCLSID() { return kFC4_CLSID_cSC4HistoryWarehouse; }
