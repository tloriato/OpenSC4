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

#include "cFC4TractDeveloper.h"

#include "FC4TractShadow.h"

#include "FC4Config.h"
#include "FC4Diag.h"
#include "FC4Fpu.h"
#include "FC4Interfaces.h"
#include "FC4Log.h"
#include "FC4Seed.h"
#include "FC4RefAudit.h"
#include "FC4Streams.h"

#include "cIGZMessage2.h"
#include "cISC4City.h"

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <ctime>
#include <mutex>
#include <string>
#include <vector>

using fc4::FromToken;
using fc4::TokenOf;

namespace {

constexpr uint32_t kIID_cIGZMessageTarget2 = 0x452294AA;
constexpr uint32_t kIID_cIGZSerializable = 0xE4FDA3D4;

// §3.
constexpr uint32_t kMsgYear = 0x66956817;

// The tract developer the city runs (Init to Shutdown), for the month line.
std::atomic<cFC4TractDeveloper*> g_live{nullptr};

// Live instances, for the test seam (op 45).
std::mutex g_instancesLock;
std::vector<cFC4TractDeveloper*> g_instances;

bool g_noteOutcomes = false;

// §4.22 [choice]: the session seed S, as crime's (foundations §6.3):
// `tract_seed` / FC4_TRACT_SEED, else the cRZTime clock (seconds since
// 1901-01-01).
uint32_t SessionSeed(bool* fixed) {
    const std::string s = fc4::Setting("tract_seed", "FC4_TRACT_SEED");
    if (!s.empty()) {
        char* end = nullptr;
        const unsigned long v = std::strtoul(s.c_str(), &end, 0);
        if (end != s.c_str()) {
            *fixed = true;
            return static_cast<uint32_t>(v);
        }
    }
    *fixed = false;
    // Else the process's one session seed (FC4Seed.h, foundations §6.3).
    return fc4::SessionSeed();
}

// GrowthProfile as the slots read it (§7.1): only the stage's low byte.
fc4_tract_profile ProfileOf(const FC4GrowthProfile& g) {
    fc4_tract_profile p{};
    p.developer = TokenOf(g.developer);
    p.x = g.x;
    p.z = g.z;
    p.stage = g.stage;
    return p;
}

// The outcome list (§9.2: B, appended, never cleared).
struct OutcomeSink {
    fc4::TractOutcomeList* list;
    uint32_t appended = 0;
    uint32_t dropped = 0;
};

void AppendOutcome(void* ctx, uint32_t code) {
    OutcomeSink* s = static_cast<OutcomeSink*>(ctx);
    if (s->list != nullptr && s->list->valid() && s->list->push_back(static_cast<int32_t>(code))) {
        ++s->appended;
    } else {
        ++s->dropped;
    }
}

void Join(char* out, size_t cap, const uint32_t* v, uint32_t n, bool hex) {
    size_t used = 0;
    out[0] = 0;
    for (uint32_t i = 0; i < n && used < cap; ++i) {
        const int w = _snprintf_s(out + used, cap - used, _TRUNCATE, hex ? "%s0x%X" : "%s%u", i ? "," : "", v[i]);
        if (w < 0) break;
        used += static_cast<size_t>(w);
    }
    if (n == 0) strcpy_s(out, cap, "-");
}

// "code:count" for the nonzero codes of `now - base`.
void OutcomeList(char* out, size_t cap, const uint32_t* now, const uint32_t* base) {
    size_t used = 0;
    out[0] = 0;
    for (uint32_t i = 0; i < 28 && used < cap; ++i) {
        const uint32_t d = now[i] - (base ? base[i] : 0);
        if (d == 0) continue;
        const int w = _snprintf_s(out + used, cap - used, _TRUNCATE, "%s%u:%u", used ? "," : "", i, d);
        if (w < 0) break;
        used += static_cast<size_t>(w);
    }
    if (used == 0) strcpy_s(out, cap, "-");
}

// Write's diagnostics: the record goes through the game stream unchanged;
// the first put that returned false (which, at which byte) is noted.
struct TracedOut {
    const fc4_ostream* inner = nullptr;
    uint32_t bytes = 0;
    const char* firstFailed = nullptr;
    uint32_t failedAt = 0;
    uint32_t failures = 0;
    bool Note(bool ok, const char* what, uint32_t n) {
        if (!ok) {
            if (firstFailed == nullptr) {
                firstFailed = what;
                failedAt = bytes;
            }
            ++failures;
        }
        bytes += n;
        return ok;
    }
};
TracedOut& T(void* u) { return *static_cast<TracedOut*>(u); }
bool TU8(void* u, uint8_t v) { return T(u).Note(T(u).inner->put_u8(T(u).inner->user, v), "u8", 1); }
bool TU16(void* u, uint16_t v) { return T(u).Note(T(u).inner->put_u16(T(u).inner->user, v), "u16", 2); }
bool TU32(void* u, uint32_t v) { return T(u).Note(T(u).inner->put_u32(T(u).inner->user, v), "u32", 4); }
bool TU64(void* u, uint64_t v) { return T(u).Note(T(u).inner->put_u64(T(u).inner->user, v), "u64", 8); }
bool TF32(void* u, float v) { return T(u).Note(T(u).inner->put_f32(T(u).inner->user, v), "f32", 4); }
bool TF64(void* u, double v) { return T(u).Note(T(u).inner->put_f64(T(u).inner->user, v), "f64", 8); }
bool TBytes(void* u, const uint8_t* d, size_t n) {
    return T(u).Note(T(u).inner->put_bytes(T(u).inner->user, d, n), "bytes", static_cast<uint32_t>(n));
}
bool TStr(void* u, const uint8_t* d, size_t n) {
    return T(u).Note(T(u).inner->put_str(T(u).inner->user, d, n), "str", static_cast<uint32_t>(4 + n));
}
bool TRef(void* u, const fc4_ref* r) { return T(u).Note(T(u).inner->put_ref(T(u).inner->user, r), "ref", 4); }
int32_t TError(void* u) { return T(u).inner->error(T(u).inner->user); }

fc4_ostream MakeTraced(TracedOut* t) {
    fc4_ostream s{};
    s.struct_size = sizeof(s);
    s.user = t;
    s.put_u8 = &TU8;
    s.put_u16 = &TU16;
    s.put_u32 = &TU32;
    s.put_u64 = &TU64;
    s.put_f32 = &TF32;
    s.put_f64 = &TF64;
    s.put_bytes = &TBytes;
    s.put_str = &TStr;
    s.put_ref = &TRef;
    s.error = &TError;
    return s;
}

}  // namespace

namespace fc4 {

void TractMonthLine(int32_t julian) {
    cFC4TractDeveloper* t = g_live.load();
    if (t == nullptr) return;
    t->LogMonth(julian);
    TractTallyMonth("ours", julian);  // the same line as the tract shadow's (FC4TractShadow.h)
}

}  // namespace fc4

// ---- object ---------------------------------------------------------------------------------------------

cIGZUnknown* CreateFC4TractDeveloper() {
    return static_cast<cIGZUnknown*>(static_cast<cFC4ISC4TractDeveloper*>(new cFC4TractDeveloper()));
}

cFC4TractDeveloper::cFC4TractDeveloper() {
    InitializeCriticalSection(&lock_);
    mainTid_ = GetCurrentThreadId();  // the class store creates city components on the main thread
    ctx_.target = static_cast<cIGZMessageTarget2*>(this);
    ctx_.filterHost = &host_;
    growLog_ = fc4::SettingInt("tract_grow_log", "FC4_TRACT_GROW_LOG", 0) != 0;
    seed_ = SessionSeed(&seedFixed_);
    CreateCore();
    {
        std::lock_guard<std::mutex> g(g_instancesLock);
        g_instances.push_back(this);
    }
    RefreshSnapshot();
    fc4::Log("cFC4TractDeveloper(%p) created, core %p, session seed 0x%08X (%s)%s", static_cast<void*>(Primary()),
             static_cast<void*>(core_), seed_, seedFixed_ ? "tract_seed" : "session seed",
             growLog_ ? ", tract_grow_log on" : "");
}

// The core over this object's table (the test seam's callbacks answer
// first, through the context). The first core of the process seeds the
// shared statics (fc4.h).
void cFC4TractDeveloper::CreateCore() {
    host_ = fc4::MakeTractHost(&ctx_);
    core_ = fc4_tract_create(&host_, seed_);
    if (core_ == nullptr) fc4::Log("cFC4TractDeveloper: core creation failed");
    MarkStylesChanged();  // a fresh core; the snapshot is rebuilt at the next RefreshSnapshot
}

cFC4TractDeveloper::~cFC4TractDeveloper() {
    {
        std::lock_guard<std::mutex> g(g_instancesLock);
        for (size_t i = 0; i < g_instances.size(); ++i) {
            if (g_instances[i] == this) {
                g_instances.erase(g_instances.begin() + static_cast<std::ptrdiff_t>(i));
                break;
            }
        }
    }
    cFC4TractDeveloper* self = this;
    g_live.compare_exchange_strong(self, nullptr);
    fc4_tract_destroy(core_);
    core_ = nullptr;
    fc4::TractReleaseHeld(&ctx_);
    // What a failed Init (or no Shutdown) leaves: the filters, the agent, the
    // services and the message server.
    fc4::TractFreeFilters(&ctx_);
    if (ctx_.agentLevels != 0 && ctx_.city != nullptr) {
        cFC4ISC4Simulator* sim = fc4::AsGame(ctx_.city->GetSimulator());
        if (sim != nullptr) sim->RemoveAgentAllLevels(ctx_.target);
        ctx_.agentLevels = 0;
    }
    fc4::TractReleaseServices(&ctx_);
    fc4::Log("cFC4TractDeveloper destroyed (%u fallbacks)", fallbacks_.load());
    DeleteCriticalSection(&lock_);
}

// §1.1: which sub-object answers is the core's decision; anything else is
// false and the out pointer is left untouched.
bool cFC4TractDeveloper::QueryInterface(uint32_t riid, void** ppvObj) {
    if (ppvObj == nullptr || !fc4_tract_query_interface(riid)) return false;
    if (riid == kIID_cIGZMessageTarget2) {
        *ppvObj = static_cast<cIGZMessageTarget2*>(this);
    } else if (riid == kIID_cIGZSerializable) {
        *ppvObj = static_cast<cIGZSerializable*>(this);
    } else {
        *ppvObj = Primary();  // 0x00000001 and 0xA818ED1B
    }
    ++refCount_;
    return true;
}

uint32_t cFC4TractDeveloper::AddRef() { return ++refCount_; }

// §1.1 Release: count > 1 → decrement; otherwise Shutdown, then destroy.
uint32_t cFC4TractDeveloper::Release() {
    if (refCount_ > 1) return --refCount_;
    refCount_ = 0;
    if (destroying_) return 0;  // a release from inside the final Shutdown
    destroying_ = true;
    Shutdown();
    delete this;
    return 0;
}

cFC4TractDeveloper* cFC4TractDeveloper::FromPrimary(const void* primary) {
    std::lock_guard<std::mutex> g(g_instancesLock);
    for (cFC4TractDeveloper* t : g_instances) {
        if (static_cast<const void*>(t->Primary()) == primary ||
            static_cast<const void*>(static_cast<cIGZMessageTarget2*>(t)) == primary ||
            static_cast<const void*>(static_cast<cIGZSerializable*>(t)) == primary) {
            return t;
        }
    }
    return nullptr;
}

void cFC4TractDeveloper::SetTestHooks(const fc4_tract_host* test, cISC4LotConfigurationManager* configs) {
    Section sec(this, true);
    ctx_.test = test;
    ctx_.testConfigs = configs;
    fc4_tract_destroy(core_);
    CreateCore();
    fc4::Log("tract: test hooks %s (core %p)", test ? "installed" : "removed", static_cast<void*>(core_));
}

// ---- the lock ---------------------------------------------------------------------------------------------

bool cFC4TractDeveloper::Enter(bool wait) {
    const DWORD me = GetCurrentThreadId();
    if (wait || mainTid_ == 0 || me == mainTid_) {
        EnterCriticalSection(&lock_);
        return true;
    }
    if (TryEnterCriticalSection(&lock_)) return true;
    const ULONGLONG until = GetTickCount64() + kLockWaitMs;
    while (GetTickCount64() < until) {
        Sleep(1);
        if (TryEnterCriticalSection(&lock_)) return true;
    }
    ++fallbacks_;
    return false;
}

void cFC4TractDeveloper::Leave() { LeaveCriticalSection(&lock_); }

cFC4TractDeveloper::Section::Section(const cFC4TractDeveloper* s, bool wait)
    : s_(const_cast<cFC4TractDeveloper*>(s)), ok_(s_->Enter(wait)) {
    if (ok_) ++s_->depth_;
}

cFC4TractDeveloper::Section::~Section() {
    if (!ok_) return;
    if (--s_->depth_ == 0) {
        // The outermost call: the references and lists kept for it go, and
        // the values other threads fall back to are refreshed.
        fc4::TractReleaseHeld(&s_->ctx_);
        s_->RefreshSnapshot();
    }
    s_->Leave();
}

// The snapshot and slot 9's vector from the core's own state (no host
// calls): any thread holding the lock. The vector is rewritten only when the
// styles changed, in place while its buffer is large enough.
//
// glue27: only when MarkStylesChanged said the state could have moved. The
// style state changes on the year message (core `new_year`) and in the style
// setters; Grow, Decline, UpdatePopulation and the getters leave it alone.
// The refresh is 5 core calls plus a vector allocation, and
// `UpdatePopulation` is called once per desirability tract of every growth
// developer's map update (12288 calls a month in a Konradshohe run), so
// refreshing there was about a quarter of that call's cost for nothing.
void cFC4TractDeveloper::RefreshSnapshot() {
    if (core_ == nullptr || !snapDirty_) return;
    snapDirty_ = false;
    snap_.currentStyle = fc4_tract_current_style(core_);
    snap_.allStyles = fc4_tract_is_using_all_styles(core_);
    snap_.yearsBetween = fc4_tract_years_between_style_changes(core_);
    const size_t n = fc4_tract_active_styles(core_, nullptr, 0);
    std::vector<uint32_t> v(n);
    if (n != 0) fc4_tract_active_styles(core_, v.data(), n);
    bool same = styles_.size() == n;
    for (size_t i = 0; same && i < n; ++i) same = styles_[i] == v[i];
    if (same) return;
    if (n > styles_.capacity() && !fc4::TractNodeAllocator::Available()) {
        if (!noteStyles_) {
            noteStyles_ = true;
            fc4::Log("tract: the game's STL allocator is not available; slot 9's style vector is not filled");
        }
        return;
    }
    styles_.clear();
    if (!styles_.reserve(n)) return;
    for (uint32_t s : v) styles_.push_back(s);
}

// The status lines (Init, Read, Write, Shutdown, year, month). Inside a section.
void cFC4TractDeveloper::LogState(const char* what) {
    fc4_tract_state st{};
    st.struct_size = sizeof(st);
    if (core_ == nullptr || !fc4_tract_get_state(core_, &st)) return;
    char styles[200];
    Join(styles, sizeof(styles), st.active_styles, st.active_style_count < 16 ? st.active_style_count : 16, true);
    char outcomes[300];
    OutcomeList(outcomes, sizeof(outcomes), st.outcomes, nullptr);
    fc4::Log("tract %s: init=%u configured=%u style=0x%X index=%d years=%d/%d all=%u styles=%s filters=%u "
             "grows=%u built=%lld declines=%u declined=%lld populations=%u outcomes=%s constructions=%u "
             "reoccupations=%u abandonments=%u lotsCreated=%u lotsDeleted=%u demolitions=%u populationsSet=%u "
             "claims=%u reentrant=%u filtersRegistered=%u/%u refused=%u listsRefused=%u roadsRefused=%u "
             "mapsRefused=%u exemplarMisses=%u configRefs=%llu fallbacks=%u",
             what, st.initialized, st.configured, st.current_style, st.current_style_index,
             st.years_since_style_change, st.years_between_style_changes, st.all_styles, styles, st.filters,
             st.grow_calls, static_cast<long long>(st.built_total), st.decline_calls,
             static_cast<long long>(st.declined_total), st.update_population_calls, outcomes, st.constructions,
             st.reoccupations, st.abandonments, st.lots_created, st.lots_deleted, st.demolitions,
             st.populations_set, st.resource_claims, st.reentrant, ctx_.filtersRegistered,
             ctx_.filtersUnregistered, ctx_.filtersRefused, ctx_.listsRefused, ctx_.roadsRefused, ctx_.mapsRefused,
             ctx_.exemplarMisses, static_cast<unsigned long long>(ctx_.configRefs), fallbacks_.load());
}

// One line per month: the calls and their effects since the previous line.
void cFC4TractDeveloper::LogMonth(int32_t julian) {
    Section sec(this);
    if (!sec.ok() || core_ == nullptr) return;
    fc4_tract_state st{};
    st.struct_size = sizeof(st);
    if (!fc4_tract_get_state(core_, &st)) return;
    fc4_date d{};
    fc4_date_from_julian(julian, &d);
    char outcomes[300];
    OutcomeList(outcomes, sizeof(outcomes), st.outcomes, month_.outcomes);
    fc4::Log("tract month %04d-%02d-%02d: grows=%u built=%lld declines=%u declined=%lld populations=%u outcomes=%s "
             "constructions=%u reoccupations=%u abandonments=%u lotsCreated=%u lotsDeleted=%u demolitions=%u "
             "style=0x%X index=%d years=%d/%d filters=%u reentrant=%u fallbacks=%u",
             d.year, d.month, d.day_of_month, st.grow_calls - month_.grows,
             static_cast<long long>(st.built_total - month_.built), st.decline_calls - month_.declines,
             static_cast<long long>(st.declined_total - month_.declined),
             st.update_population_calls - month_.populations, outcomes, st.constructions - month_.constructions,
             st.reoccupations - month_.reoccupations, st.abandonments - month_.abandonments,
             st.lots_created - month_.lotsCreated, st.lots_deleted - month_.lotsDeleted,
             st.demolitions - month_.demolitions, st.current_style, st.current_style_index,
             st.years_since_style_change, st.years_between_style_changes, st.filters, st.reentrant,
             fallbacks_.load());
    month_.grows = st.grow_calls;
    month_.declines = st.decline_calls;
    month_.populations = st.update_population_calls;
    month_.built = st.built_total;
    month_.declined = st.declined_total;
    std::memcpy(month_.outcomes, st.outcomes, sizeof(month_.outcomes));
    month_.constructions = st.constructions;
    month_.reoccupations = st.reoccupations;
    month_.abandonments = st.abandonments;
    month_.demolitions = st.demolitions;
    month_.lotsCreated = st.lots_created;
    month_.lotsDeleted = st.lots_deleted;
}

// ---- lifecycle (§1.3, §1.4) -------------------------------------------------------------------------------

bool cFC4TractDeveloper::Init() {
    FC4_TRACE("TractDeveloper::Init");
    mainTid_ = GetCurrentThreadId();
    Section sec(this, true);
    const bool ok = fc4_tract_init(core_);  // §1.3, all of it (the core drives every host call)
    MarkStylesChanged();
    g_live = this;
    LogState(ok ? "Init" : "Init FAILED");
    return ok;
}

bool cFC4TractDeveloper::Shutdown() {
    FC4_TRACE("TractDeveloper::Shutdown");
    // Keep this object alive while the simulator and the message server drop
    // their references (unless this already is the final Release).
    if (!destroying_) Primary()->AddRef();
    {
        Section sec(this, true);
        fc4_tract_shutdown(core_);  // §1.4: message server, agent, filters, services
        MarkStylesChanged();
        LogState("Shutdown");
    }
    cFC4TractDeveloper* self = this;
    g_live.compare_exchange_strong(self, nullptr);
    if (!destroying_) Primary()->Release();
    return true;  // AL = 1
}

// ---- cISC4TractDeveloper (§7.1) ---------------------------------------------------------------------------

// Slot 5: EAX = built (int32). The codes go to the caller's list (B), after
// the core returns.
int32_t cFC4TractDeveloper::Grow(FC4GrowthProfile const& profile, fc4::GameList<int32_t>& outcomes, bool& outFlag) {
    const uint64_t t0 = fc4::TractTallyNow();
    Section sec(this);
    if (!sec.ok()) {
        outFlag = false;
        return 0;  // [choice] another thread that cannot get the lock builds nothing
    }
    const fc4_tract_profile p = ProfileOf(profile);
    OutcomeSink sink{&fc4::AsTract(outcomes)};
    fc4_tract_grow_result res{};
    res.struct_size = sizeof(res);
    bool flag = outFlag;
    const size_t before = fc4::TractTallyListSize(outcomes);
    const int32_t built = fc4_tract_grow(core_, &p, &AppendOutcome, &sink, &flag, &res);
    outFlag = flag;
    fc4::TractTallyGrow(profile, built, outcomes, before, fc4::TractTallyNow() - t0);
    if (sink.dropped != 0 && !g_noteOutcomes) {
        g_noteOutcomes = true;
        fc4::Log("tract: %u outcome code(s) not appended to the caller's list (no list or no allocator B)",
                 sink.dropped);
    }
    if (growLog_) {
        char codes[120];
        Join(codes, sizeof(codes), res.outcomes, res.outcome_count < 16 ? res.outcome_count : 16, false);
        fc4::Log("tract Grow dev=%p tract=%d,%d stage=%u -> built=%d flag=%u desired=%d/%d scored=%u codes=%s%s "
                 "cfg=%p building=0x%08X",
                 static_cast<void*>(profile.developer), profile.x, profile.z, profile.stage, built, res.out_flag,
                 res.desired, res.desired_max, res.scored, codes, res.outcome_count > 16 ? ",..." : "",
                 static_cast<void*>(FromToken(res.chosen_cfg)), res.chosen_building);
    }
    return built;
}

// Slot 6: EAX = the capacity abandoned.
int32_t cFC4TractDeveloper::Decline(FC4GrowthProfile const& profile) {
    Section sec(this);
    if (!sec.ok()) return 0;
    const fc4_tract_profile p = ProfileOf(profile);
    const int32_t declined = fc4_tract_decline(core_, &p);
    fc4::TractTallyDecline(profile, declined);
    return declined;
}

// Slot 7: 0 (§7.1; the original leaves EAX garbage).
int32_t cFC4TractDeveloper::UpdatePopulation(FC4GrowthProfile const& profile) {
    Section sec(this);
    if (!sec.ok()) return 0;
    const fc4_tract_profile p = ProfileOf(profile);
    fc4_tract_update_population(core_, &p);
    return 0;
}

int32_t cFC4TractDeveloper::GetCurrentStyle() const {
    Section sec(this);
    if (!sec.ok()) return static_cast<int32_t>(snap_.currentStyle.load());
    return static_cast<int32_t>(fc4_tract_current_style(core_));
}

// Slot 9: the member vector (B), as the core has the styles.
fc4::GameVector<uint32_t>& cFC4TractDeveloper::GetActiveStyles() {
    Section sec(this);
    if (sec.ok()) RefreshSnapshot();
    return fc4::AsGameB(styles_);
}

// Slot 10: EAX = the member vector's address (nonzero; its low byte is never 0).
intptr_t cFC4TractDeveloper::SetActiveStyles(fc4::GameVector<uint32_t> const& styles) {
    {
        Section sec(this);
        if (sec.ok()) {
            auto& v = const_cast<fc4::TractStyleVector&>(fc4::AsTract(styles));
            std::vector<uint32_t> copy(v.begin(), v.end());
            fc4_tract_set_active_styles(core_, copy.data(), copy.size());
            MarkStylesChanged();
        }  // [choice] another thread that cannot get the lock: not set
    }
    return reinterpret_cast<intptr_t>(&styles_);
}

bool cFC4TractDeveloper::IsUsingAllStylesAtOnce() const {
    Section sec(this);
    if (!sec.ok()) return snap_.allStyles.load();
    return fc4_tract_is_using_all_styles(core_);
}

cFC4ISC4TractDeveloper* cFC4TractDeveloper::UseAllStylesAtOnce(bool all) {
    Section sec(this);
    if (sec.ok()) {
        fc4_tract_use_all_styles(core_, all);
        MarkStylesChanged();
    }
    return this;
}

int32_t cFC4TractDeveloper::GetYearsBetweenStyleChanges() const {
    Section sec(this);
    if (!sec.ok()) return snap_.yearsBetween.load();
    return fc4_tract_years_between_style_changes(core_);
}

cFC4ISC4TractDeveloper* cFC4TractDeveloper::SetYearsBetweenStyleChanges(int32_t years) {
    Section sec(this);
    if (sec.ok()) {
        fc4_tract_set_years_between_style_changes(core_, years);
        MarkStylesChanged();
    }
    return this;
}

// ---- messages (§3) ----------------------------------------------------------------------------------------

bool cFC4TractDeveloper::DoMessage(cIGZMessage2* pMessage) {
    if (pMessage == nullptr || core_ == nullptr) return true;
    const uint32_t type = pMessage->GetType();
    FC4_X87_SCOPE("tract DoMessage", type);  // x87 leftover guard (FC4Fpu.h)
    Section sec(this);
    if (!sec.ok()) return true;  // [choice] a message from another thread that cannot get the lock is dropped
    fc4_tract_do_message(core_, type);
    if (type == kMsgYear) {
        MarkStylesChanged();  // core new_year may roll the style (§4.20)
        LogState("NewYear");
    }
    return true;  // §3: always
}

// ---- persistence (§6) -------------------------------------------------------------------------------------

bool cFC4TractDeveloper::Write(cIGZOStream& s) {
    FC4_TRACE("TractDeveloper::Write");
    Section sec(this, true);
    fc4::OStreamAdapter a(s);
    const int32_t errorBefore = s.GetError();
    TracedOut t;
    t.inner = a.Get();
    const fc4_ostream traced = MakeTraced(&t);
    const bool written = fc4_tract_write(core_, &traced);
    const int32_t errorAfter = s.GetError();
    // §6: Write returns GetError() = 0. The core ANDs the puts' results; a
    // put that answered false while the stream's error stayed clear does
    // not fail the record.
    const bool ok = written || (core_ != nullptr && t.failures != 0 && errorAfter == 0);
    if (written && t.failures == 0) {
        fc4::Log("tract Write -> 1 (%u bytes)", t.bytes);
    } else {
        fc4::Log("tract Write -> %d (%u bytes; core %d; %u put(s) answered false, the first %s at byte %u; stream "
                 "error %d before, %d after)",
                 ok ? 1 : 0, t.bytes, written ? 1 : 0, t.failures, t.firstFailed ? t.firstFailed : "-", t.failedAt,
                 errorBefore, errorAfter);
    }
    LogState("Write");
    return ok;
}

// Runs before Init (§1.2).
bool cFC4TractDeveloper::Read(cIGZIStream& s) {
    FC4_TRACE("TractDeveloper::Read");
    Section sec(this, true);
    fc4::IStreamAdapter a(s);
    const bool ok = fc4_tract_read(core_, a.Get());
    MarkStylesChanged();  // the whole record is replaced
    fc4::Log("tract Read -> %d", ok ? 1 : 0);
    LogState("Read");
    return ok;
}

uint32_t cFC4TractDeveloper::GetGZCLSID() { return kFC4_CLSID_cSC4TractDeveloper; }

// ---- test seam op 45 ----------------------------------------------------------------------------------------

intptr_t TractTestSeam(const FC4TractTestInput* in, void* out) {
    if (in == nullptr) return -1;
    if (in->op == kFC4TractTestHooks) {
        fc4::TractNodeAllocator::TestAllocate() = in->allocate;
        fc4::TractNodeAllocator::TestFree() = in->free;
        if (in->tractDeveloper == nullptr) return 0;
        cFC4TractDeveloper* t = cFC4TractDeveloper::FromPrimary(in->tractDeveloper);
        if (t == nullptr) return -1;
        t->SetTestHooks(in->host, in->configManager);
        return 1;
    }
    if (in->op == kFC4TractTestShadow) return fc4::TractShadowTestRun(in->city, in->julian, in->event, in->host);
    if (in->op == kFC4TractTestFilters) {
        auto* o = static_cast<FC4TractFilterCounts*>(out);
        if (o == nullptr) return -1;
        *o = FC4TractFilterCounts{};
        o->live = cFC4TractFilter::Live();
        cFC4TractDeveloper* t = in->tractDeveloper ? cFC4TractDeveloper::FromPrimary(in->tractDeveloper) : nullptr;
        if (t != nullptr) {
            const fc4::TractContext& c = t->Context();
            o->allocated = static_cast<uint32_t>(c.filters.size());
            o->registered = c.filtersRegistered;
            o->unregistered = c.filtersUnregistered;
            o->refused = c.filtersRefused;
        }
        return 1;
    }
    return -1;
}
