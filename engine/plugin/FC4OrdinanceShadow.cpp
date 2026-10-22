// SPDX-License-Identifier: GPL-3.0-or-later
// Clean-room public-interface observer. No executable addresses, private layouts,
// native algorithm forwarding, state repair, or promotion. Native remains authoritative.
#include "FC4OrdinanceShadow.h"
#include "FC4Config.h"
#include "FC4Log.h"
#include "FC4Observer.h"
#include "FC4RefAudit.h"
#include "FC4WriteCapture.h"
#include "cFC4Ordinance.h"
#include "cISC4City.h"
#include "cISC4Ordinance.h"
#include "cIGZMessageTarget2.h"
#include <array>
#include <cstring>
#include <mutex>
#include <vector>

namespace fc4 {
namespace {
std::recursive_mutex g_lock;
bool g_enabled = false, g_valid = false;
thread_local bool g_native = false;
cISC4OrdinanceSimulator* g_original = nullptr; // borrowed until PreCityShutdown
fc4_ordinance_sim* g_core = nullptr;
std::vector<fc4_ordinance*> g_objects;
std::vector<uint64_t> g_tokens;
std::array<void*, 26> g_saved{}, g_table{}; // ordinance.md compiled SDK slot table
void** g_originalTable = nullptr;
uint64_t g_checks = 0, g_differences = 0, g_months = 0, g_refusals = 0;
uint64_t g_posts = 0, g_effects = 0, g_nested = 0, g_bootstraps = 0;
std::array<uint64_t, 26> g_calls{};
void Records();

struct NativeScope {
    bool old = g_native;
    NativeScope() { g_native = true; }
    ~NativeScope() { g_native = old; }
};
void Check(const char* what, bool same) {
    ++g_checks;
    if (!same) {
        ++g_differences;
        Log("ordinance shadow difference: %s check=%llu differences=%llu", what, g_checks, g_differences);
    }
}
void NoRef(void*, uint64_t) {} // core tokens are identities, not owned native references
void Post(void*, uint32_t, uint32_t, uint32_t) { ++g_posts; }
void Demolish(void*, const uint64_t*, size_t n) { g_effects += n; }
void Effect(void*, uint32_t) { ++g_effects; }
void NoLots(void*, uint32_t, fc4_token_callback, void*) { ++g_refusals; }

template<size_t Slot, typename R, typename... A>
R Original(cISC4OrdinanceSimulator* self, A... args) {
    using Fn = R(__thiscall*)(cISC4OrdinanceSimulator*, A...);
    NativeScope scope;
    return reinterpret_cast<Fn>(g_saved[Slot])(self, args...);
}
template<size_t Slot, uint32_t Which>
bool __fastcall Query(cISC4OrdinanceSimulator* self, void*, uint32_t id) {
    std::lock_guard<std::recursive_mutex> lock(g_lock);
    if (g_native) { ++g_nested; return Original<Slot, bool>(self, id); }
    ++g_calls[Slot];
    const bool a = Original<Slot, bool>(self, id);
    if (g_valid) Check("natural query", a == fc4_ordinance_sim_query(g_core, Which, id));
    return a;
}
template<size_t Slot, uint32_t Which>
bool __fastcall Set(cISC4OrdinanceSimulator* self, void*, uint32_t id, bool value) {
    std::lock_guard<std::recursive_mutex> lock(g_lock);
    if (g_native) { ++g_nested; return Original<Slot, bool>(self, id, value); }
    ++g_calls[Slot];
    const bool b = g_valid && fc4_ordinance_sim_set(g_core, Which, id, value);
    const bool a = Original<Slot, bool>(self, id, value);
    if (g_valid) { Check("natural setter result", a == b); Records(); }
    return a;
}
template<size_t Slot, uint32_t Which>
uint32_t __fastcall Count(cISC4OrdinanceSimulator* self, void*) {
    std::lock_guard<std::recursive_mutex> lock(g_lock);
    if (g_native) { ++g_nested; return Original<Slot, uint32_t>(self); }
    ++g_calls[Slot];
    const auto a = Original<Slot, uint32_t>(self);
    if (g_valid) Check("natural count", a == fc4_ordinance_sim_count(g_core, Which));
    return a;
}
template<size_t Slot, uint32_t Which>
uint32_t __fastcall Array(cISC4OrdinanceSimulator* self, void*, uint32_t* out, uint32_t& n) {
    std::lock_guard<std::recursive_mutex> lock(g_lock);
    using Fn = uint32_t(__thiscall*)(cISC4OrdinanceSimulator*, uint32_t*, uint32_t&);
    if (g_native) { ++g_nested; return reinterpret_cast<Fn>(g_saved[Slot])(self, out, n); }
    ++g_calls[Slot];
    const uint32_t capacity = n;
    // Refuse unreasonable buffers rather than allocate from an unchecked caller size.
    std::vector<uint32_t> expected(capacity <= 4096 && out ? capacity : 0);
    uint32_t bn = capacity;
    const auto b = g_valid && capacity <= 4096 ?
        fc4_ordinance_sim_array(g_core, Which, out ? expected.data() : nullptr, &bn) : 0;
    uint32_t a;
    { NativeScope scope; a = reinterpret_cast<Fn>(g_saved[Slot])(self, out, n); }
    if (g_valid && capacity <= 4096) {
        Check("natural array total/count", a == b && n == bn);
        Check("natural array IDs", n <= capacity && (!out ||
            (n == bn && (n == 0 || std::memcmp(out, expected.data(), n * sizeof(uint32_t)) == 0))));
    } else if (capacity > 4096) ++g_refusals;
    return a;
}
template<size_t Slot, uint32_t Which>
int64_t __fastcall Money(cISC4OrdinanceSimulator* self, void*) {
    std::lock_guard<std::recursive_mutex> lock(g_lock);
    if (g_native) { ++g_nested; return Original<Slot, int64_t>(self); }
    ++g_calls[Slot];
    const auto a = Original<Slot, int64_t>(self);
    if (g_valid) Check("natural money", a == fc4_ordinance_sim_money(g_core, Which));
    return a;
}
int64_t __fastcall Offset(cISC4OrdinanceSimulator* self, void*, uint32_t prop, uint32_t index) {
    std::lock_guard<std::recursive_mutex> lock(g_lock);
    if (!g_native) ++g_calls[24]; else ++g_nested;
    const bool compare = !g_native && g_valid;
    const auto a = Original<24, int64_t>(self, prop, index);
    // §6.1 specifies only the low 32 bits for native; upper bits are untested.
    if (compare) Check("natural offset low32", static_cast<uint32_t>(a) ==
        static_cast<uint32_t>(fc4_ordinance_sim_cumulative_offset(g_core, prop, index)));
    return a;
}
float __fastcall Factor(cISC4OrdinanceSimulator* self, void*, uint32_t prop, uint32_t index) {
    std::lock_guard<std::recursive_mutex> lock(g_lock);
    if (!g_native) ++g_calls[25]; else ++g_nested;
    const bool compare = !g_native && g_valid;
    const float a = Original<25, float>(self, prop, index);
    if (compare) {
        const float b = fc4_ordinance_sim_cumulative_factor(g_core, prop, index);
        Check("natural factor bits", std::memcmp(&a, &b, 4) == 0);
    }
    return a;
}
template<size_t Slot>
bool __fastcall Membership(cISC4OrdinanceSimulator* self, void*, cISC4Ordinance& o) {
    std::lock_guard<std::recursive_mutex> lock(g_lock);
    using Fn = bool(__thiscall*)(cISC4OrdinanceSimulator*, cISC4Ordinance&);
    if (!g_native) {
        ++g_calls[Slot]; ++g_refusals; g_valid = false;
        Log("ordinance shadow: refused natural membership change slot=%u; following invalid, no resync", unsigned(Slot));
    }
    NativeScope scope;
    return reinterpret_cast<Fn>(g_saved[Slot])(self, o);
}

// Resolve only independent Rust standard objects. Never manufacture foreign
// wrappers whose calls would forward the algorithm back to native.
struct ReadContext {
    CaptureIStream stream;
    explicit ReadContext(const WriteCapture& c) : stream(c) {}
};
bool ReadRef(void* u, uint32_t, fc4_ref* out) {
    auto& r = *static_cast<ReadContext*>(u);
    uint32_t id = 0, cls = 0;
    if (!r.stream.Take(&id, 4) || (id && !r.stream.Take(&cls, 4))) return false;
    *out = {};
    if (id == 0) return true;
    if (id > g_objects.size()) return false;
    out->token = g_tokens[id - 1]; out->class_id = cls; out->core_handle = g_objects[id - 1];
    return true;
}
// Keep original CaptureIStream callback user intact; get_ref uses a separate
// thread-local context only during synchronous bootstrap.
thread_local ReadContext* g_read = nullptr;
bool ReadRefBridge(void*, uint32_t iid, fc4_ref* out) { return ReadRef(g_read, iid, out); }

bool Bootstrap(cISC4OrdinanceSimulator* original) {
    NativeScope scope;
    WriteCapture sim;
    if (!sim.Capture(original)) return false;
    auto host = FC4MakeOrdinanceHost(nullptr);
    host.add_ref = &NoRef; host.release = &NoRef; host.post = &Post;
    host.create_ordinance = nullptr; host.foreign_ordinance = nullptr;
    host.ordinance_call = nullptr; host.ordinance_text = nullptr; host.ordinance_misc_property = nullptr;
    host.demolish_lots = &Demolish; host.hide_conditional_building = &Effect;
    host.remove_view_input_control = &Effect; host.lots_with_building_type = &NoLots;
    // Capture's references are numbered in write order. Parse count from the
    // specified simulator header, not a native object layout.
    if (sim.Bytes().size() < 22) return false;
    uint32_t count = 0; std::memcpy(&count, sim.Bytes().data() + 18, 4);
    if (count > 4096) return false;
    for (uint32_t i = 1; i <= count; ++i) {
        auto* saved = sim.SerializableAt(i);
        if (!saved || !FC4IsOrdinanceClass(sim.ClassOf(i))) return false;
        void* p = nullptr;
        if (!saved->QueryInterface(0xAA5E2247, &p) || !p) return false;
        // Capture holds no reference: take/drop exactly this QI reference.
        auto* ord = static_cast<cISC4Ordinance*>(p);
        FC4_ACQUIRED(ord, "QueryInterface");
        const uint64_t token = TokenOf(ord);
        WriteCapture record;
        const bool captured = record.Capture(ord);
        FC4_RELEASE(ord);
        if (!captured) return false;
        auto* core = fc4_ordinance_create(&host, sim.ClassOf(i), token);
        if (!core) return false;
        g_objects.push_back(core); g_tokens.push_back(token);
        CaptureIStream input(record);
        if (!fc4_ordinance_read(core, input.Get()) || !input.good() || !input.atEnd()) return false;
    }
    g_core = fc4_ordinance_sim_create(&host, TokenOf(original));
    if (!g_core) return false;
    ReadContext read(sim);
    fc4_istream api = *read.stream.Get(); api.get_ref = &ReadRefBridge;
    g_read = &read;
    const bool ok = fc4_ordinance_sim_read(g_core, &api);
    g_read = nullptr;
    if (!ok || !read.stream.good() || !read.stream.atEnd() || !fc4_ordinance_sim_init(g_core)) return false;
    ++g_bootstraps;
    Log("ordinance shadow: bootstrap original=%p Rust=%p objects=%u; baseline excluded; no resync", original, g_core, count);
    return true;
}

void Records() {
    NativeScope scope;
    WriteCapture native, ours;
    const bool a = native.Capture(g_original);
    OStreamAdapter out(ours);
    const bool b = fc4_ordinance_sim_write(g_core, out.Get());
    Check("simulator serialization success", a && b);
    if (a && b) Check("simulator serialized bytes", native.Bytes() == ours.Bytes());
    for (size_t i = 0; i < g_objects.size(); ++i) {
        WriteCapture x, y;
        const bool ax = x.Capture(FromToken(g_tokens[i]));
        OStreamAdapter oy(y);
        const bool by = fc4_ordinance_write(g_objects[i], oy.Get());
        Check("ordinance serialization success", ax && by);
        if (ax && by) Check("ordinance serialized bytes", x.Bytes() == y.Bytes());
    }
}
void Install(cISC4OrdinanceSimulator* p) {
    // Only the public SDK dispatch table (26 slots), never executable bytes.
    g_originalTable = *reinterpret_cast<void***>(p);
    std::memcpy(g_saved.data(), g_originalTable, sizeof(g_saved)); g_table = g_saved;
    g_table[5] = reinterpret_cast<void*>(&Membership<5>); g_table[6] = reinterpret_cast<void*>(&Membership<6>);
    g_table[7] = reinterpret_cast<void*>(&Query<7, 0>); g_table[8] = reinterpret_cast<void*>(&Query<8, 1>);
    g_table[9] = reinterpret_cast<void*>(&Query<9, 2>); g_table[10] = reinterpret_cast<void*>(&Set<10, 0>);
    g_table[11] = reinterpret_cast<void*>(&Count<11, 0>); g_table[12] = reinterpret_cast<void*>(&Array<12, 0>);
    g_table[13] = reinterpret_cast<void*>(&Set<13, 1>); g_table[14] = reinterpret_cast<void*>(&Count<14, 1>);
    g_table[15] = reinterpret_cast<void*>(&Array<15, 1>); g_table[16] = reinterpret_cast<void*>(&Query<16, 3>);
    g_table[17] = reinterpret_cast<void*>(&Set<17, 2>); g_table[18] = reinterpret_cast<void*>(&Array<18, 2>);
    g_table[22] = reinterpret_cast<void*>(&Money<22, 0>); g_table[23] = reinterpret_cast<void*>(&Money<23, 1>);
    g_table[24] = reinterpret_cast<void*>(&Offset); g_table[25] = reinterpret_cast<void*>(&Factor);
    *reinterpret_cast<void***>(p) = g_table.data();
}
}
void OrdinanceShadowStartup(bool replaced) {
    g_enabled = SettingInt("ordinance_shadow", "FC4_ORDINANCE_SHADOW", 0) != 0 && !replaced;
    if (SettingInt("ordinance_shadow", "FC4_ORDINANCE_SHADOW", 0))
        Log("ordinance shadow: %s; native authoritative; no promotion; host-input equality and side-effect outputs UNTESTED",
            g_enabled ? "ON" : "INACTIVE (ordinance replaced)");
}
bool OrdinanceShadowDeliver(cIGZMessageTarget2* target, const char* name, cIGZMessage2* msg,
                            uint32_t type, bool* finished) {
    if (!g_enabled || !name || std::strcmp(name, "cSC4OrdinanceSimulator") || type != 0x66956816) return false;
    std::lock_guard<std::recursive_mutex> lock(g_lock);
    if (!g_original) {
        auto* city = ObserverCity();
        g_original = city ? city->GetOrdinanceSimulator() : nullptr;
        if (!g_original) { ++g_refusals; return false; }
        g_valid = Bootstrap(g_original);
        if (g_valid) Install(g_original);
        else { ++g_refusals; Log("ordinance shadow: bootstrap refused; no comparisons, no native fallback"); }
    }
    const bool b = g_valid && fc4_ordinance_sim_handle_message(g_core, type);
    { NativeScope scope; *finished = target->DoMessage(msg); }
    ++g_months;
    if (g_valid) { Check("natural month return", b == *finished); Records(); }
    Log("ordinance shadow: month=%llu checks=%llu differences=%llu refusals=%llu valid=%d dry_posts=%llu dry_effects=%llu",
        g_months, g_checks, g_differences, g_refusals, g_valid, g_posts, g_effects);
    return true;
}
void OrdinanceShadowCityShutdown() {
    std::lock_guard<std::recursive_mutex> lock(g_lock);
    if (!g_enabled) return;
    Log("ordinance shadow: shutdown months=%llu exact=%llu/%llu differences=%llu bootstraps=%llu resync=0 fallback=0 refusals=%llu nested=%llu",
        g_months, g_checks - g_differences, g_checks, g_differences, g_bootstraps, g_refusals, g_nested);
    for (size_t i = 0; i < g_calls.size(); ++i) Log("ordinance shadow: natural slot=%u calls=%llu", unsigned(i), g_calls[i]);
    Log("ordinance shadow: UNTESTED native host-read capture, effects/messages, direct ordinance calls, Init/Read/Write/Shutdown interception, names; zero-call slots untested");
    if (g_originalTable && g_original) *reinterpret_cast<void***>(g_original) = g_originalTable;
    if (g_core) fc4_ordinance_sim_destroy(g_core);
    for (auto* o : g_objects) fc4_ordinance_destroy(o);
    g_objects.clear(); g_tokens.clear(); g_core = nullptr; g_original = nullptr; g_originalTable = nullptr; g_valid = false;
}
}
