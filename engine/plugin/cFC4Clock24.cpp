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

#include "cFC4Clock24.h"

#include "FC4Diag.h"
#include "FC4Fpu.h"
#include "FC4Log.h"
#include "FC4Properties.h"
#include "FC4RefAudit.h"
#include "FC4Schedule.h"
#include "FC4Streams.h"
#include "cFC4Message2Standard.h"

#include "GZServPtrs.h"
#include "cIGZApp.h"
#include "cIGZCheatCodeManager.h"
#include "cIGZFrameWork.h"
#include "cIGZMessage2Standard.h"
#include "cIGZMessageServer2.h"
#include "cIGZString.h"
#include "cISC4App.h"
#include "cRZBaseString.h"
#include "cRZCOMDllDirector.h"

#include <string>
#include <vector>

namespace {

constexpr uint32_t kMsgCheatIssued = 0x230E27AC;            // §3.2
constexpr uint32_t kIID_cIGZCheatCodeManager = 0xA1085722;  // as used by the SDK examples
constexpr uint32_t kIID_cIGZMessageTarget2 = 0x452294AA;
constexpr uint32_t kMsgFirst = 0x44778230;                  // §3.3: HourChanged ... HiddenPauseChanged

// §6: the "Main Simulator" exemplar and the clock's three properties.
constexpr uint32_t kTuningType = 0x6534284A;
constexpr uint32_t kTuningGroup = 0xE7E2C2DB;
constexpr uint32_t kTuningInstance = 0xE9590E52;
constexpr uint32_t kPropMagnification = 0x897E709F;
constexpr uint32_t kPropNightStart = 0x09B73421;
constexpr uint32_t kPropNightEnd = 0x09B73422;

cFC4Clock24* Self(void* u) { return static_cast<cFC4Clock24*>(u); }

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

// ---- host callbacks (fc4_clock24_host) ------------------------------------------------------------------
//
// The core runs under the lock: the game calls it asks for are queued
// (CoreLock::Defer) and made in order once the section is left.

// §4.3 timer model, CK2 [choice]: any monotonic microsecond clock.
uint64_t NowMicros(void*) {
    if (const FC4HistClockHooks* h = fc4::HistClockHooks()) return h->now_micros ? h->now_micros(h->ctx) : 0;
    static LARGE_INTEGER freq = {};
    if (freq.QuadPart == 0) QueryPerformanceFrequency(&freq);
    const unsigned long long t = static_cast<unsigned long long>(fc4::ScheduleQpc());
    const unsigned long long f = static_cast<unsigned long long>(freq.QuadPart);
    return (t / f) * 1000000ull + ((t % f) * 1000000ull) / f;
}

void PostCb(void* u, uint32_t type, uint32_t d2, uint32_t d3) {
    cFC4Clock24* s = Self(u);
    s->Lock().Defer([s, type, d2, d3] { s->Post(type, d2, d3); });
}

void FrameworkOp(void* u, uint32_t op) {
    cFC4Clock24* s = Self(u);
    s->Lock().Defer([s, op] { s->Framework(op); });
}
void AddService(void* u) { FrameworkOp(u, 0); }
void AddIdle(void* u) { FrameworkOp(u, 1); }
void RemoveIdle(void* u) { FrameworkOp(u, 2); }
void RemoveService(void* u) { FrameworkOp(u, 3); }

bool HasCheatMgr(void* u) { return Self(u)->HasCheatManager(); }

void RegisterCheatCb(void* u, uint32_t id, const char* name, size_t len) {
    cFC4Clock24* s = Self(u);
    std::string n(name != nullptr ? name : "", name != nullptr ? len : 0);
    s->Lock().Defer([s, id, n] { s->Cheat(1, id, n.c_str()); });
}
void CheatOp(void* u, uint32_t op, uint32_t id) {
    cFC4Clock24* s = Self(u);
    s->Lock().Defer([s, op, id] { s->Cheat(op, id, nullptr); });
}
void AddCheatNote(void* u, uint32_t id) { CheatOp(u, 2, id); }
void RemoveCheatNote(void* u, uint32_t id) { CheatOp(u, 3, id); }
void UnregisterCheatCb(void* u, uint32_t id) { CheatOp(u, 4, id); }

fc4_clock24_host MakeHost(cFC4Clock24* self) {
    fc4_clock24_host h{};
    h.struct_size = sizeof(h);
    h.user = self;
    h.now_micros = &NowMicros;
    h.post = &PostCb;
    h.add_system_service = &AddService;
    h.remove_system_service = &RemoveService;
    h.add_to_on_idle = &AddIdle;
    h.remove_from_on_idle = &RemoveIdle;
    h.has_cheat_manager = &HasCheatMgr;
    h.register_cheat = &RegisterCheatCb;
    h.add_cheat_notification = &AddCheatNote;
    h.remove_cheat_notification = &RemoveCheatNote;
    h.unregister_cheat = &UnregisterCheatCb;
    return h;
}

}  // namespace

// §6: the "Main Simulator" exemplar's clock properties; false = no exemplar.
// A property of another numeric type is converted (FC4Properties.h); an
// array property gives its first value [choice].
bool FC4Clock24ReadTuning(fc4_clock24_tuning& t) {
    t = fc4_clock24_tuning{};
    t.struct_size = sizeof(t);
    if (const FC4HistClockHooks* h = fc4::HistClockHooks()) {
        return h->clock_tuning != nullptr && h->clock_tuning(h->ctx, &t);
    }
    fc4::props::Exemplar ex;
    if (!ex.Load(kTuningType, kTuningGroup, kTuningInstance)) return false;
    std::vector<float> f;
    if (fc4::props::GetF32s(ex.Holder(), kPropMagnification, f) && !f.empty()) {
        t.has_magnification = 1;
        t.magnification = f[0];
    }
    std::vector<uint32_t> u;
    if (fc4::props::GetU32s(ex.Holder(), kPropNightStart, u) && !u.empty()) {
        t.has_night_start = 1;
        t.night_start = u[0];
    }
    u.clear();
    if (fc4::props::GetU32s(ex.Holder(), kPropNightEnd, u) && !u.empty()) {
        t.has_night_end = 1;
        t.night_end = u[0];
    }
    return true;
}

cIGZUnknown* CreateFC4Clock24() {
    return static_cast<cIGZUnknown*>(static_cast<cFC4ISC424HourClock*>(new cFC4Clock24()));
}

cFC4Clock24::cFC4Clock24() {
    host_ = MakeHost(this);
    core_ = fc4_clock24_create(&host_);
    if (core_ == nullptr) fc4::Log("cFC4Clock24: core creation failed");
    fc4::Log("cFC4Clock24(%p) created, core %p", static_cast<void*>(Primary()), static_cast<void*>(core_));
}

cFC4Clock24::~cFC4Clock24() {
    fc4_clock24_destroy(core_);
    core_ = nullptr;
    if (cheatMgr_ != nullptr) FC4_RELEASE(cheatMgr_);
    cheatMgr_ = nullptr;
    fc4::Log("cFC4Clock24 destroyed (%u idle passes, %u off-main calls)", idles_.load(), lock_.OffMainCalls());
}

// §1: 0x681BD53A and 1 (+0), 0x452294AA (+4), 0xE4FDA3D4 (+8), and the
// system-service base's 0x287FB697 (+0xC) *[inferred]*; nothing else. The
// out pointer is left untouched on a refusal.
bool cFC4Clock24::QueryInterface(uint32_t riid, void** ppvObj) {
    if (ppvObj == nullptr || !fc4_clock24_answers_iid(riid)) return false;
    switch (riid) {
        case kIID_cIGZMessageTarget2: *ppvObj = Target(); break;
        case GZIID_cIGZSerializable: *ppvObj = static_cast<cIGZSerializable*>(this); break;
        case GZIID_cIGZSystemService: *ppvObj = Service(); break;
        default: *ppvObj = Primary(); break;  // 0x681BD53A and 1
    }
    AddRef();
    return true;
}

uint32_t cFC4Clock24::AddRef() { return ++refCount_; }

// §8: at a count of 1 or less the object is destroyed without decrementing
// first: Shutdown (nothing when not initialised), then the destructor.
uint32_t cFC4Clock24::Release() {
    if (refCount_ > 1) return --refCount_;
    refCount_ = 0;
    if (destroying_) return 0;  // a release from inside the final Shutdown
    destroying_ = true;
    Shutdown();
    delete this;
    return 0;
}

fc4_clock24_state cFC4Clock24::State() {
    fc4_clock24_state st{};
    st.struct_size = sizeof(st);
    fc4::CoreLock::Section s(lock_, "clock24 state");
    fc4_clock24_get_state(core_, &st);
    return st;
}

// ---- the game calls the core asks for --------------------------------------------------------------------

// §3.3: a standard message with void1 = the IGZUnknown field = the clock,
// data2 and data3, posted (queued) and released. No server: nothing posted.
void cFC4Clock24::Post(uint32_t type, uint32_t data2, uint32_t data3) {
    if (type >= kMsgFirst && type < kMsgFirst + 7) ++posts_[type - kMsgFirst];
    const FC4HistClockHooks* h = fc4::HistClockHooks();
    cIGZMessageServer2Ptr ms;
    if (h == nullptr && !ms) return;
    cFC4Message2Standard* msg = FC4MakeMessage(type, 0, static_cast<intptr_t>(data2), static_cast<intptr_t>(data3));
    msg->SetVoid1(Primary());
    msg->SetIGZUnknown(Primary());
    if (h != nullptr) {
        if (h->post != nullptr) h->post(h->ctx, msg);
    } else {
        ms->MessagePost(msg, false);
    }
    msg->Release();
}

// §4.2: the framework registration of the system service (+0xC).
void cFC4Clock24::Framework(uint32_t op) {
    if (const FC4HistClockHooks* h = fc4::HistClockHooks()) {
        if (h->framework != nullptr) h->framework(h->ctx, Service(), op);
        return;
    }
    cIGZFrameWork* fw = RZGetFrameWork();
    if (fw == nullptr) return;
    bool ok = false;
    switch (op) {
        case 0: ok = fw->AddSystemService(Service()); break;
        case 1: ok = fw->AddToOnIdle(Service()); break;
        case 2: ok = fw->RemoveFromOnIdle(Service()); break;
        case 3: ok = fw->RemoveSystemService(Service()); break;
        default: break;
    }
    static const char* const kNames[] = {"AddSystemService", "AddToOnIdle", "RemoveFromOnIdle",
                                         "RemoveSystemService"};
    fc4::Log("clock24: %s -> %d", op < 4 ? kNames[op] : "?", ok ? 1 : 0);
}

// §4.2 (CK12, CK13): the cheat code manager, slots 3, 7, 8 and 4.
void cFC4Clock24::Cheat(uint32_t op, uint32_t id, const char* name) {
    if (const FC4HistClockHooks* h = fc4::HistClockHooks()) {
        if (h->cheat != nullptr) h->cheat(h->ctx, op, id, name);
        return;
    }
    if (cheatMgr_ == nullptr) return;
    switch (op) {
        case 1: cheatMgr_->RegisterCheatCode(id, cRZBaseString(name != nullptr ? name : "")); break;
        case 2: cheatMgr_->AddNotification2(Target(), id); break;
        case 3: cheatMgr_->RemoveNotification2(Target(), id); break;
        case 4: cheatMgr_->UnregisterCheatCode(id); break;
        default: break;
    }
}

// The cheat code manager (the app's), kept from Init to Shutdown.
bool cFC4Clock24::AcquireCheatManager() {
    if (const FC4HistClockHooks* h = fc4::HistClockHooks()) {
        return h->cheat != nullptr && h->cheat(h->ctx, 0, 0, nullptr);
    }
    if (cheatMgr_ != nullptr) return true;
    if (cISC4App* sc4 = App()) {
        cIGZCheatCodeManager* cm = sc4->GetCheatCodeManager();
        if (cm != nullptr && cm->QueryInterface(kIID_cIGZCheatCodeManager, reinterpret_cast<void**>(&cheatMgr_)) &&
            cheatMgr_ != nullptr) {
            FC4_ACQUIRED(cheatMgr_, "QueryInterface");
        } else {
            cheatMgr_ = nullptr;
        }
        FC4_RELEASE(sc4);
    }
    return cheatMgr_ != nullptr;
}

// ---- lifecycle (§4.2) ----------------------------------------------------------------------------------------

bool cFC4Clock24::Init() {
    FC4_TRACE("Clock24::Init");
    // State() samples the timer; an idempotence check must not advance the shared counter.
    if (initialized_.load()) return true;  // also the framework's own Init through the +0xC thunk
    hasCheatManager_ = AcquireCheatManager();
    fc4_clock24_tuning tuning{};
    const bool haveTuning = FC4Clock24ReadTuning(tuning);
    bool ok;
    {
        fc4::CoreLock::Section s(lock_, "clock24 Init");
        ok = fc4_clock24_init(core_, haveTuning ? &tuning : nullptr);  // framework and cheats: queued
        initialized_ = ok;  // before the queued framework registration can call Init again
    }
    fc4::Log("clock24 Init: ok=%d cheats=%s tuning=%s", ok ? 1 : 0, hasCheatManager_ ? "yes" : "no",
             haveTuning ? "yes" : "no");
    return true;
}

bool cFC4Clock24::Shutdown() {
    FC4_TRACE("Clock24::Shutdown");
    if (core_ == nullptr || !initialized_.load()) return true;
    if (!destroying_) AddRef();  // keep alive while the framework lets go
    hasCheatManager_ = AcquireCheatManager();
    {
        fc4::CoreLock::Section s(lock_, "clock24 Shutdown");
        fc4_clock24_shutdown(core_);  // cheats, then the framework: queued
        initialized_ = false;
    }
    fc4::Log("clock24 Shutdown: %u idle passes; posted hour %u, tick %u, visual hour %u, visual tick %u, "
             "day/night %u, pause %u, hidden pause %u; %u off-main calls",
             idles_.load(), posts_[0].load(), posts_[1].load(), posts_[2].load(), posts_[3].load(), posts_[4].load(),
             posts_[5].load(), posts_[6].load(), lock_.OffMainCalls());
    if (cheatMgr_ != nullptr) FC4_RELEASE(cheatMgr_);
    cheatMgr_ = nullptr;
    hasCheatManager_ = false;
    if (!destroying_) Release();
    return true;
}

// ---- cISC424HourClock (§4.4 - §4.7, §8) -----------------------------------------------------------------------

uint32_t cFC4Clock24::SetVisualTimeMode(int32_t mode) {
    fc4::CoreLock::Section s(lock_, "clock24 SetVisualTimeMode");
    fc4_clock24_set_visual_time_mode(core_, mode);
    return 1;  // no meaningful value (§8)
}

int32_t cFC4Clock24::GetVisualTimeMode() {
    fc4::CoreLock::Section s(lock_, "clock24 GetVisualTimeMode");
    return fc4_clock24_get_visual_time_mode(core_);
}

void cFC4Clock24::GetTimePercentage(float* out) {
    fc4::CoreLock::Section s(lock_, "clock24 GetTimePercentage");
    fc4_clock24_get_time_percentage(core_, out);
}

uint32_t cFC4Clock24::SetTimePercentage(float& p) {
    const float v = p;
    fc4::CoreLock::Section s(lock_, "clock24 SetTimePercentage");
    fc4_clock24_set_time_percentage(core_, v);
    return 1;  // no meaningful value (§8)
}

void cFC4Clock24::GetVisualTimePercentage(float* out) {
    fc4::CoreLock::Section s(lock_, "clock24 GetVisualTimePercentage");
    fc4_clock24_get_visual_time_percentage(core_, out);
}

uint32_t cFC4Clock24::GetHour() {
    fc4::CoreLock::Section s(lock_, "clock24 GetHour");
    return fc4_clock24_get_hour(core_);
}

uint32_t cFC4Clock24::SetHour(uint32_t hour) {
    fc4::CoreLock::Section s(lock_, "clock24 SetHour");
    fc4_clock24_set_hour(core_, hour);
    return 1;  // no meaningful value (§8)
}

uint32_t cFC4Clock24::GetVisualHour() {
    fc4::CoreLock::Section s(lock_, "clock24 GetVisualHour");
    return fc4_clock24_get_visual_hour(core_);
}

uint32_t cFC4Clock24::GetMinute() {
    fc4::CoreLock::Section s(lock_, "clock24 GetMinute");
    return fc4_clock24_get_minute(core_);
}

uint32_t cFC4Clock24::SetMinute(uint32_t minute) {
    fc4::CoreLock::Section s(lock_, "clock24 SetMinute");
    return fc4_clock24_set_minute(core_, minute);
}

uint32_t cFC4Clock24::GetVisualMinute() {
    fc4::CoreLock::Section s(lock_, "clock24 GetVisualMinute");
    return fc4_clock24_get_visual_minute(core_);
}

uint32_t cFC4Clock24::GetSeconds() {
    fc4::CoreLock::Section s(lock_, "clock24 GetSeconds");
    return fc4_clock24_get_seconds(core_);
}

uint32_t cFC4Clock24::SetSeconds(uint32_t seconds) {
    fc4::CoreLock::Section s(lock_, "clock24 SetSeconds");
    return fc4_clock24_set_seconds(core_, seconds);
}

uint32_t cFC4Clock24::GetVisualSeconds() {
    fc4::CoreLock::Section s(lock_, "clock24 GetVisualSeconds");
    return fc4_clock24_get_visual_seconds(core_);
}

uint32_t cFC4Clock24::GetEventHour(uint32_t index) {
    fc4::CoreLock::Section s(lock_, "clock24 GetEventHour");
    return fc4_clock24_get_event_hour(core_, index);
}

uint32_t cFC4Clock24::GetTimeMagnification(float& out) {
    fc4::CoreLock::Section s(lock_, "clock24 GetTimeMagnification");
    return fc4_clock24_get_time_magnification(core_, &out);
}

bool cFC4Clock24::SetTimeMagnification(float m) {
    fc4::CoreLock::Section s(lock_, "clock24 SetTimeMagnification");
    fc4_clock24_set_time_magnification(core_, m);
    return true;
}

uint32_t cFC4Clock24::GetTimeMagnificationMinMax(float& min, float& max) {
    fc4::CoreLock::Section s(lock_, "clock24 GetTimeMagnificationMinMax");
    return fc4_clock24_get_time_magnification_min_max(core_, &min, &max);
}

bool cFC4Clock24::SetTimeMagnificationMinMax(float min, float max) {
    fc4::CoreLock::Section s(lock_, "clock24 SetTimeMagnificationMinMax");
    fc4_clock24_set_time_magnification_min_max(core_, min, max);
    return true;
}

bool cFC4Clock24::Pause(bool on) {
    fc4::CoreLock::Section s(lock_, "clock24 Pause");
    fc4_clock24_pause(core_, on);
    return true;
}

bool cFC4Clock24::IsPaused() {
    fc4::CoreLock::Section s(lock_, "clock24 IsPaused");
    return fc4_clock24_is_paused(core_);
}

bool cFC4Clock24::HiddenPause(bool on) {
    fc4::CoreLock::Section s(lock_, "clock24 HiddenPause");
    fc4_clock24_hidden_pause(core_, on);
    return true;
}

bool cFC4Clock24::IsHiddenPaused() {
    fc4::CoreLock::Section s(lock_, "clock24 IsHiddenPaused");
    return fc4_clock24_is_hidden_paused(core_);
}

bool cFC4Clock24::IsNight() {
    fc4::CoreLock::Section s(lock_, "clock24 IsNight");
    return fc4_clock24_is_night(core_);
}

void cFC4Clock24::EnableDayNightTransitionNotifications(bool on) {
    fc4::CoreLock::Section s(lock_, "clock24 EnableDayNightTransitionNotifications");
    fc4_clock24_enable_day_night_notifications(core_, on);
}

// ---- messages (§3.2, §4.8) -------------------------------------------------------------------------------------

// Only the cheat message does anything; true for every message.
bool cFC4Clock24::DoMessage(cIGZMessage2* pMessage) {
    if (pMessage == nullptr || core_ == nullptr) return true;
    FC4_X87_SCOPE("clock24 DoMessage", pMessage->GetType());  // x87 leftover guard at the entry and exit (FC4Fpu.h)
    const uint32_t type = pMessage->GetType();
    if (type != kMsgCheatIssued) return true;
    uint32_t id = 0;
    std::string text;
    bool haveText = false;
    cIGZMessage2Standard* m = nullptr;
    if (pMessage->QueryInterface(GZIID_cIGZMessage2Standard, reinterpret_cast<void**>(&m)) && m != nullptr) {
        FC4_ACQUIRED(m, "QueryInterface");
        id = static_cast<uint32_t>(m->GetData1());
        if (const cIGZString* s = static_cast<const cIGZString*>(m->GetVoid2())) {
            const char* p = s->ToChar();
            if (p != nullptr) text.assign(p, s->Strlen());
            haveText = true;
        }
        FC4_RELEASE(m);
    }
    fc4::Log("clock24 cheat 0x%08X '%s'", id, text.c_str());
    fc4::CoreLock::Section s(lock_, "clock24 DoMessage");
    fc4_clock24_do_message(core_, type, id, haveText ? text.data() : nullptr, text.size());
    return true;
}

// ---- persistence (§7) ----------------------------------------------------------------------------------------

bool cFC4Clock24::Write(cIGZOStream& s) {
    FC4_TRACE("Clock24::Write");
    fc4::OStreamAdapter a(s);
    bool ok;
    fc4::MemOStream mem;
    {
        fc4::CoreLock::Section sec(lock_, "clock24 Write");
        ok = fc4_clock24_write(core_, a.Get());
        fc4_clock24_write(core_, mem.Get());
    }
    // The record read back by a scratch core and written again (§7): the
    // save's bytes must survive a load.
    bool same = false;
    {
        fc4_clock24_host h{};
        h.struct_size = sizeof(h);
        fc4_clock24* scratch = fc4_clock24_create(&h);
        fc4::MemIStream in(mem.bytes);
        fc4::MemOStream again;
        if (scratch != nullptr && fc4_clock24_read(scratch, in.Get()) && in.Remaining() == 0 &&
            fc4_clock24_write(scratch, again.Get())) {
            same = again.bytes == mem.bytes;
        }
        fc4_clock24_destroy(scratch);
    }
    fc4::Log("clock24 Write -> %d (%u bytes, read back and rewritten %s)", ok ? 1 : 0,
             static_cast<unsigned>(mem.bytes.size()), same ? "identical" : "DIFFERENT");
    return ok;
}

// §7: no version check; Read runs before Init.
bool cFC4Clock24::Read(cIGZIStream& s) {
    FC4_TRACE("Clock24::Read");
    fc4::IStreamAdapter a(s);
    bool ok;
    {
        fc4::CoreLock::Section sec(lock_, "clock24 Read");
        ok = fc4_clock24_read(core_, a.Get());
    }
    fc4::Log("clock24 Read -> %d", ok ? 1 : 0);
    return ok;
}

uint32_t cFC4Clock24::GetGZCLSID() { return kFC4_CLSID_cSC424HourClock; }

// ---- cIGZSystemService (§1, §4.3) ------------------------------------------------------------------------------

uint32_t cFC4Clock24::GetServiceID() { return serviceId_; }

cIGZSystemService* cFC4Clock24::SetServiceID(uint32_t id) {
    serviceId_ = id;
    return Service();
}

int32_t cFC4Clock24::GetServicePriority() { return kFC4_Clock24ServicePriority; }
bool cFC4Clock24::IsServiceRunning() { return serviceRunning_; }

cIGZSystemService* cFC4Clock24::SetServiceRunning(bool running) {
    serviceRunning_ = running;
    return Service();
}

// §1: OnTick is the shared default. [inferred] (question) The spec does not
// give its value; the SDK's base answers true.
bool cFC4Clock24::OnTick(uint32_t) { return true; }

// §4.3: the step; true.
bool cFC4Clock24::OnIdle(uint32_t unknown1) {
    ++idles_;
    fc4::CoreLock::Section s(lock_, "clock24 OnIdle");
    return fc4_clock24_on_idle(core_, unknown1);
}

// [inferred] (question) The spec does not give the tick priority; the SDK's
// base answers the service priority.
int32_t cFC4Clock24::GetServiceTickPriority() { return kFC4_Clock24ServicePriority; }
