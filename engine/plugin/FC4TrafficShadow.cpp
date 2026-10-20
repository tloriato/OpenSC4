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

#include "FC4TrafficShadow.h"

#include "FC4Config.h"
#include "FC4Diag.h"
#include "FC4Fpu.h"
#include "FC4HostGrid.h"
#include "FC4Log.h"
#include "FC4Properties.h"
#include "FC4RefAudit.h"
#include "FC4Seed.h"
#include "FC4Streams.h"
#include "FC4WriteCapture.h"
#include "cFC4Message2Standard.h"
#include "cFC4TrafficSimulator.h"

#include "GZServPtrs.h"
#include "cGZPersistResourceKey.h"
#include "cIGZApp.h"
#include "cIGZFrameWork.h"
#include "cIGZMessage2.h"
#include "cIGZMessage2Standard.h"
#include "cIGZMessageServer2.h"
#include "cIGZMessageTarget2.h"
#include "cIGZSerializable.h"
#include "cIGZString.h"
#include "FC4Interfaces.h"
#include "cISC4App.h"
#include "cISC4BudgetSimulator.h"
#include "cISC4City.h"
#include "cISC4TrafficSimulator.h"

#include <algorithm>
#include <climits>
#include <cstring>
#include <unordered_map>
#include <vector>

namespace fc4 {
namespace {

constexpr uint32_t kMsgSimBegin = 0x66956810;
constexpr uint32_t kMsgSimNewDay = 0x66956814;
constexpr uint32_t kMsgSimNewMonth = 0x66956816;
constexpr uint32_t kMsgInsertOccupant = 0x99EF1142;
constexpr uint32_t kMsgRemoveOccupant = 0x99EF1143;
constexpr uint32_t kMsgLot = 0xE8BDA2E7;
constexpr uint32_t kIID_cIGZMessageTarget2 = 0x452294AA;
// The interfaces the §6 reader asks a reference for (fc4_pollution_capture_ref).
constexpr uint32_t kIID_cISC4SimGrid = 0xC9BD939A;
constexpr uint32_t kIID_cISC4Occupant = 0x26BB767E;
constexpr uint32_t kIID_cISC4DepartmentBudget = 0x223E2B67;
constexpr const char* kAgentName = "cSC4TrafficSimulator";  // traffic-simulator.md §1 Init step 2
// The messages the original subscribes to (§1 Init step 3), forwarded to the copy.
constexpr uint32_t kEvents[] = {0x99EF1142, 0x99EF1143, 0x26D31EC1, 0xE8BDA2E7, 0xC53D10AA,
                                0x8A0D15DA, 0xEA0D163E, 0x08DAC3E7, 0xC2C09554};
constexpr long kDefaultDiffs = 8;
constexpr long kMaxDiffs = 64;
constexpr long kDefaultLines = 8;
constexpr uint32_t kSeed = 0x6990C1AA;  // the copy's draws are not compared (§4.15)

// ---- the capture stream ------------------------------------------------------------------------------
//
// An object's Write goes into memory: primitives little-endian, strings as a
// uint32 length then the bytes, references (WriteGZSerializable) as uint32
// objectID (1-based, first appearance, 0 = null) then uint32 classID, each
// object listed once with its tokens for the interfaces the reader asks for
// (borrowed identities: queried and released at once, no reference count
// changes). Our own stream, never a save's: nothing is registered with the
// class-object store. `lenient` (the lot records): a variant is accepted and
// not written (the strategy byte, the record's last byte, is all that is
// read from a lot).
class CaptureStream final : public cFC4ISC4DBSegmentOStream {
public:
    explicit CaptureStream(bool lenient = false) : lenient_(lenient) {}

    void Reset() {
        bytes_.clear();
        refs_.clear();
        ids_.clear();
        good_ = true;
        refused_ = "nothing";
    }

    fc4_pollution_capture View(bool writeOk) const {
        fc4_pollution_capture c{};
        c.struct_size = sizeof(c);
        c.ok = (writeOk && good_) ? 1u : 0u;
        c.bytes = bytes_.empty() ? nullptr : bytes_.data();
        c.len = bytes_.size();
        c.refs = refs_.empty() ? nullptr : refs_.data();
        c.ref_count = refs_.size();
        return c;
    }

    bool Good() const { return good_; }
    size_t RefCount() const { return refs_.size(); }
    // The first call the stream refused (for the log).
    const char* Refused() const { return refused_; }
    const std::vector<uint8_t>& Bytes() const { return bytes_; }

    // cIGZUnknown
    bool QueryInterface(uint32_t riid, void** ppvObj) override {
        if (ppvObj == nullptr) return false;
        if (riid == kFC4_IID_cISC4DBSegmentOStream || riid == GZIID_cIGZUnknown) {
            *ppvObj = static_cast<cFC4ISC4DBSegmentOStream*>(this);
            return true;
        }
        return false;
    }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }

    // cIGZOStream
    void Flush() override {}
    bool SetSint8(int8_t v) override { return Put(&v, 1); }
    bool SetUint8(uint8_t v) override { return Put(&v, 1); }
    bool SetSint16(int16_t v) override { return Put(&v, 2); }
    bool SetUint16(uint16_t v) override { return Put(&v, 2); }
    bool SetSint32(int32_t v) override { return Put(&v, 4); }
    bool SetUint32(uint32_t v) override { return Put(&v, 4); }
    bool SetSint64(int64_t v) override { return Put(&v, 8); }
    bool SetUint64(uint64_t v) override { return Put(&v, 8); }
    bool SetFloat32(float v) override { return Put(&v, 4); }
    bool SetFloat64(double v) override { return Put(&v, 8); }
    bool SetRZCharStr(char const* s) override { return PutStr(s, s ? strlen(s) : 0); }
    bool SetGZStr(cIGZString const& s) override { return PutStr(s.Data(), s.Strlen()); }
    bool SetGZSerializable(cIGZSerializable const& s) override {
        if (!const_cast<cIGZSerializable&>(s).Write(*this)) Refuse("SetGZSerializable (an inner Write failed)");
        return good_;
    }
    bool SetVoid(void const* p, uint32_t n) override {
        if (n != 0 && p == nullptr) {
            Refuse("SetVoid (null data)");
            return false;
        }
        return Put(p, n);
    }
    int32_t GetError() override { return good_ ? 0 : 1; }
    int32_t SetUserData(cIGZVariant*) override { return 0; }
    int32_t GetUserData() override { return 0; }

    // cISC4DBSegmentOStream
    bool Open(cISC4DBSegment*, cGZPersistResourceKey const&, bool) override { return false; }
    bool Close() override { return true; }
    bool IsOpen() override { return true; }
    int32_t GetRecord() override { return 0; }
    int32_t GetSegment() override { return 0; }
    bool WriteGZSerializable(cIGZSerializable const* p) override {
        if (p == nullptr) return SetUint32(0);
        cIGZSerializable* obj = const_cast<cIGZSerializable*>(p);
        size_t i = 0;
        while (i < ids_.size() && ids_[i] != obj) ++i;
        if (i == ids_.size()) {
            fc4_pollution_capture_ref r{};
            if (!lenient_) {
                r.grid = Borrowed(obj, kIID_cISC4SimGrid);
                r.occupant = Borrowed(obj, kIID_cISC4Occupant);
                r.department = Borrowed(obj, kIID_cISC4DepartmentBudget);
            }
            r.class_id = obj->GetGZCLSID();
            ids_.push_back(obj);
            refs_.push_back(r);
        }
        SetUint32(static_cast<uint32_t>(i + 1));
        return SetUint32(refs_[i].class_id);
    }
    bool WriteResKey(cGZPersistResourceKey const& k) override {
        SetUint32(k.type);
        SetUint32(k.group);
        return SetUint32(k.instance);
    }
    // A variant (the neighbour pools' property holders, §6.1 row 19) as the
    // game's segment streams write it (foundations §9.2), through the core's
    // writer. `lenient` (a lot record): accepted, not written.
    bool WriteVariant(cIGZVariant const& v) override {
        if (lenient_) return good_;
        fc4_props* tmp = fc4_props_create();
        const bool ok = props::CopyVariantToCore(AsGame(&v), 1, tmp) &&
                        fc4_props_write_variant(tmp, 1, &AppendBytes, this);
        fc4_props_destroy(tmp);
        if (!ok) Refuse("WriteVariant (a type the core does not write)");
        return good_;
    }

private:
    static void AppendBytes(void* ctx, const uint8_t* data, size_t len) {
        static_cast<CaptureStream*>(ctx)->Put(data, len);
    }
    void Refuse(const char* what) {
        if (good_) refused_ = what;
        good_ = false;
    }
    bool Put(const void* p, size_t n) {
        const uint8_t* b = static_cast<const uint8_t*>(p);
        bytes_.insert(bytes_.end(), b, b + n);  // x86: already little-endian
        return good_;
    }
    bool PutStr(const char* s, size_t n) {
        if (n > 0x00FFFFFF || (n != 0 && s == nullptr)) {
            Refuse("a string");
            return false;
        }
        SetUint32(static_cast<uint32_t>(n));
        return Put(s, n);
    }
    // The object's identity for `iid` (0 if it does not answer): queried,
    // and the reference dropped at once.
    static uint64_t Borrowed(cIGZSerializable* obj, uint32_t iid) {
        void* q = nullptr;
        if (!obj->QueryInterface(iid, &q) || q == nullptr) return 0;
        cIGZUnknown* u = static_cast<cIGZUnknown*>(q);
        FC4_ACQUIRED(u, "QueryInterface");
        const uint64_t t = TokenOf(u);
        FC4_RELEASE(u);
        return t;
    }

    bool lenient_;
    std::vector<uint8_t> bytes_;
    std::vector<fc4_pollution_capture_ref> refs_;
    std::vector<const void*> ids_;
    bool good_ = true;
    const char* refused_ = "nothing";
};

CaptureStream g_before;
CaptureStream g_after;
CaptureStream g_lotStream(true);

// ---- settings and run state (main thread only) ---------------------------------------------------

bool g_enabled = false;
long g_diffs = kDefaultDiffs;
long g_lines = kDefaultLines;
// traffic_shadow_hypotheses is retired (ABI 69: TS55-TS57 confirmed, the
// core's only behaviour); the key is accepted, ignored and logged once.
bool g_loggedRetiredHypotheses = false;
int32_t g_settingsJulian = INT_MIN;
int32_t g_julian = 0;  // the last day delivered (the one-shot's date)

// Test seam (op 51).
bool g_testMode = false;
const FC4TrafficShadowTestInput* g_test = nullptr;

// The live simulator's agent (its cIGZMessageTarget2), per simulator: the
// ORIGINAL's when the shadow is the differential, FreeCity4's own when it is
// the replacement's self-check (g_selfCheck).
const void* g_agentSim = nullptr;
cIGZMessageTarget2* g_agent = nullptr;
bool g_loggedOurs = false;
// glue27: `enable = traffic` puts FreeCity4's class where the original's was.
// The shadow then stays on as a class-layer self-check of that replacement
// (glue21's shape for the prop / lot configuration managers): it reads
// whatever the city holds, writes that class's own record and runs a private
// core over it on a read-only host. See TrafficShadowStartup.
bool g_selfCheck = false;
bool g_loggedNoSerializable = false;
bool g_listening = false;
DWORD g_mainTid = 0;

// The copy's host: cFC4TrafficSimulator's callbacks over this context (the
// city borrowed for one call), every write callback NULL.
TrafficContext g_ctx;
fc4_traffic_host g_host{};
fc4_traffic_shadow* g_shadow = nullptr;
const void* g_shadowCity = nullptr;
const void* g_shadowSim = nullptr;
long g_shadowDiffs = 0;
long g_shadowLines = 0;
const void* g_shadowFailedFor = nullptr;

// The lot strategy bytes read in this call (lot token -> byte; the lot's own
// Write record's last byte, §6.3), and the counts for the status line.
std::unordered_map<uint64_t, uint8_t> g_strategyBytes;
uint32_t g_strategyReads = 0;
uint32_t g_captureFailures = 0;
uint32_t g_strategyFailures = 0;

// The delivery being wrapped.
struct Delivery {
    cIGZUnknown* sim = nullptr;
    uint32_t type = 0;
    uint32_t data1 = 0;
    int32_t julian = 0;
    bool begun = false;
    bool finished = false;
    int32_t result = -1;
};
Delivery g_d;
bool g_inDelivery = false;
int32_t g_lastResult = -1;

// A message that arrived during a wrapped delivery: forwarded after it (its
// object held until then).
struct Queued {
    uint32_t type, d1, d2, d3;
    cIGZUnknown* object;
};
std::vector<Queued> g_queued;
uint32_t g_forwarded = 0;
// A structured exception inside the copy left it unusable (its state may be
// taken out mid-operation): it is dropped and started again at the next
// delivery.
bool g_faulted = false;
uint32_t g_restarts = 0;
uint32_t g_offThread = 0;

void ReadSettings() {
    g_enabled = SettingInt("traffic_shadow", "FC4_TRAFFIC_SHADOW", 0) != 0;
    long n = SettingInt("traffic_shadow_diffs", "FC4_TRAFFIC_SHADOW_DIFFS", kDefaultDiffs);
    if (n <= 0) n = kDefaultDiffs;
    if (n > kMaxDiffs) n = kMaxDiffs;
    g_diffs = n;
    long l = SettingInt("traffic_shadow_lines", "FC4_TRAFFIC_SHADOW_LINES", kDefaultLines);
    if (l <= 0) l = kDefaultLines;
    g_lines = l;
}

// The retired traffic_shadow_hypotheses key: accepted (old regress lists set
// it), ignored, logged once per process when set.
void LogRetiredHypotheses() {
    if (g_loggedRetiredHypotheses) return;
    const char* src = "";
    const std::string v = Setting("traffic_shadow_hypotheses", "FC4_TRAFFIC_SHADOW_HYPOTHESES", &src);
    if (v.empty()) return;
    g_loggedRetiredHypotheses = true;
    Log("traffic shadow: traffic_shadow_hypotheses = %s (from %s) is retired and ignored: its three readings "
        "(the ConfirmTrips merge, the FullTrips count swap with row 19's (published, building) pairs, the "
        "ReRouteTrips merge) are confirmed (traffic-simulator TS55-TS57) and are the core's only behaviour",
        v.c_str(), src);
}

cISC4City* CurrentCity() {
    if (g_testMode) return g_test->city;
    cIGZFrameWork* fw = RZGetFrameWork();
    cIGZApp* app = fw ? fw->Application() : nullptr;
    cISC4App* sc4 = nullptr;
    if (app == nullptr || !app->QueryInterface(GZIID_cISC4App, reinterpret_cast<void**>(&sc4)) || sc4 == nullptr) {
        return nullptr;
    }
    FC4_ACQUIRED(sc4, "QueryInterface");
    cISC4City* city = sc4->GetCity();  // borrowed
    FC4_RELEASE(sc4);
    return city;
}

// The live traffic simulator if `target` is its agent: its
// cIGZMessageTarget2 interface, or failing that (logged) an agent registered
// under its name.
//
// Which class that is depends on the mode (TrafficShadowStartup):
//   - the differential: the ORIGINAL traffic simulator, and FreeCity4's class
//     is refused (it would be compared against itself);
//   - the self-check (`enable = traffic`): FreeCity4's own class, which is
//     then compared against a private second instance of the same core.
cIGZUnknown* ReferenceFor(cISC4City* city, cIGZMessageTarget2* target, const char* agentName) {
    if (city == nullptr || target == nullptr) return nullptr;
    cIGZUnknown* raw = reinterpret_cast<cIGZUnknown*>(city->GetTrafficSimulator());  // borrowed
    if (raw == nullptr) return nullptr;
    const bool ours = IsOurObject(raw);
    if (ours && !g_selfCheck) {
        if (!g_loggedOurs) {
            g_loggedOurs = true;
            Log("traffic shadow: inactive: the city's traffic simulator is FreeCity4's own (without `enable = "
                "traffic` the shadow compares against the ORIGINAL traffic simulator)");
        }
        return nullptr;
    }
    if (ours && !g_loggedOurs) {
        g_loggedOurs = true;
        Log("traffic shadow: the city's traffic simulator is FreeCity4's own (`enable = traffic`): the reference "
            "for the comparison is that class, so this is its self-check");
    }
    if (raw != g_agentSim) {
        void* agent = nullptr;
        const bool qi = raw->QueryInterface(kIID_cIGZMessageTarget2, &agent) && agent != nullptr;
        g_agent = nullptr;
        if (qi) {
            cIGZUnknown* a = static_cast<cIGZUnknown*>(agent);
            FC4_ACQUIRED(a, "QueryInterface");
            g_agent = static_cast<cIGZMessageTarget2*>(agent);
            FC4_RELEASE(a);  // the simulator keeps it alive; only its identity is kept
        }
        g_agentSim = raw;
        Log("traffic shadow: the live traffic simulator %p has agent %p (%s)", static_cast<void*>(raw),
            static_cast<void*>(g_agent), qi ? "its cIGZMessageTarget2 interface" : "no interface: matched by name");
    }
    const bool same = g_agent != nullptr && target == g_agent;
    const bool byName = g_agent == nullptr && agentName != nullptr && strcmp(agentName, kAgentName) == 0;
    return (same || byName) ? raw : nullptr;
}

// After every call into the shadow core: the references and per-call caches
// the host callbacks kept (cFC4TrafficSimulator::ReleaseHeld), and the city.
void EndCall() {
    std::vector<cIGZUnknown*> held;
    held.swap(g_ctx.held);
    std::vector<std::shared_ptr<void>> lists;
    lists.swap(g_ctx.heldLists);
    g_ctx.lotList.clear();
    g_ctx.lotListValid = false;
    g_ctx.liveLots.clear();
    g_ctx.seenLots.clear();
    g_ctx.liveConns.clear();
    g_ctx.seenConns.clear();
    g_ctx.liveConnsValid = false;
    for (cIGZUnknown* o : held) FC4_RELEASE(o);
    lists.clear();  // each list releases its elements
    g_strategyBytes.clear();
    g_ctx.city = nullptr;
}

// The test seam's city has no services: the copy reads neutral values.
void BeginCall(cISC4City* city) { g_ctx.city = g_testMode ? nullptr : city; }

// §4.1.2's strategy byte: no interface reads it; the lot's own record ends
// with it (§6.3), so the lot's Write into memory gives it. 0 when the lot
// does not answer or its Write fails.
uint8_t LotStrategyByte(void*, uint64_t lot) {
    if (lot == 0) return 0;
    auto it = g_strategyBytes.find(lot);
    if (it != g_strategyBytes.end()) return it->second;
    uint8_t b = 0;
    bool ok = false;
    void* p = nullptr;
    if (FromToken(lot)->QueryInterface(GZIID_cIGZSerializable, &p) && p != nullptr) {
        cIGZSerializable* ser = static_cast<cIGZSerializable*>(p);
        FC4_ACQUIRED(ser, "QueryInterface");
        g_lotStream.Reset();
        ok = ser->Write(g_lotStream) && g_lotStream.Good() && !g_lotStream.Bytes().empty();
        if (ok) b = g_lotStream.Bytes().back();
        FC4_RELEASE(ser);
    }
    ++g_strategyReads;
    if (!ok && ++g_strategyFailures <= 3) {
        Log("traffic shadow: the strategy byte of lot %p could not be read (its Write); 0 is used",
            static_cast<void*>(FromToken(lot)));
    }
    g_strategyBytes[lot] = b;
    return b;
}

// The copy's lot deactivation (§4.1.3 DoLotDeactivated step 3) as a lookup:
// the cost of the holder's 0xCA550301 budget item (budget slot 51); nothing
// is removed. (The table's own callback removes the items.)
int64_t DeactivationCostLookup(void* u, uint64_t holder) {
    TrafficContext* c = static_cast<TrafficContext*>(u);
    if (c == nullptr || c->city == nullptr || holder == 0) return 0;
    cISC4BudgetSimulator* b = c->city->GetBudgetSimulator();  // borrowed
    if (b == nullptr) return 0;
    cFC4ISC4BudgetSimulator::BudgetItem item{};
    cISCPropertyHolder* h = reinterpret_cast<cISCPropertyHolder*>(FromToken(holder));
    if (!AsGame(b)->GetBudgetItemForPurpose(h, 0xCA550301u, item)) return 0;
    return item.cost;
}

void MakeHost() {
    g_ctx.grid = HostGridContext{"traffic shadow", Sw::TrafficGridWrites};
    g_host = MakeTrafficHost(&g_ctx);
    // Read only: the copy keeps its writes, posts, budget, agent and
    // reference calls to itself (core traffic_simulator::shadow); these are
    // NULL so that nothing could reach the game even so.
    g_host.add_agent = nullptr;
    g_host.subscribe = nullptr;
    g_host.post_message = nullptr;
    g_host.add_ref = nullptr;
    g_host.release = nullptr;
    g_host.create_department = nullptr;
    g_host.release_department = nullptr;
    g_host.set_line_item_income = nullptr;
    g_host.activate_budget_items = nullptr;
    g_host.deactivate_budget_items = &DeactivationCostLookup;  // a lookup, nothing removed
    g_host.record_history = nullptr;
    g_host.clear_lot_flag = nullptr;
    g_host.update_traffic_volume = nullptr;
    g_host.set_trip_list = nullptr;
    g_host.set_holder_property = nullptr;
    g_host.remove_holder_property = nullptr;
    g_host.grids.create = nullptr;
    g_host.grids.set = nullptr;
    g_host.grids.fill = nullptr;
    g_host.grids.shutdown = nullptr;
    g_host.grids.release = nullptr;
    g_host.lot_strategy_byte = &LotStrategyByte;
}

void DropShadow() {
    fc4_traffic_shadow_destroy(g_shadow);
    g_shadow = nullptr;
    g_shadowCity = nullptr;
    g_shadowSim = nullptr;
    for (const Queued& q : g_queued) {
        if (q.object != nullptr) FC4_RELEASE(q.object);
    }
    g_queued.clear();
}

bool EnsureShadow(const void* city, const void* sim) {
    if (g_shadow != nullptr && !g_faulted && g_shadowCity == city && g_shadowSim == sim && g_shadowDiffs == g_diffs &&
        g_shadowLines == g_lines) {
        return true;
    }
    DropShadow();
    MakeHost();
    // The copy's generator. Against the original its draws cannot be the
    // original's (the generator is not readable, §4.15), so a fixed seed is as
    // good as any: the draws are excluded from the comparison. In the
    // self-check both sides are our core, so the copy takes the live class's
    // session seed (fc4.h, foundations §6.3) and the excluded draws stay
    // excluded for the reason they are named, not because the seeds differ.
    const uint32_t seed = g_selfCheck ? fc4::SubsystemSeed("traffic_seed", "FC4_TRAFFIC_SEED", nullptr) : kSeed;
    g_shadow = fc4_traffic_shadow_create(&g_host, seed, static_cast<uint32_t>(g_diffs),
                                         static_cast<uint32_t>(g_lines));
    if (g_shadow == nullptr) {
        if (g_shadowFailedFor != sim) {
            g_shadowFailedFor = sim;
            Log("traffic shadow: the shadow core could not be created; deliveries pass through uncompared");
        }
        return false;
    }
    if (g_faulted) {
        g_faulted = false;
        ++g_restarts;
        Log("traffic shadow: the copy is started again after a fault (restart %u); it starts on the original's "
            "record at this delivery",
            g_restarts);
    }
    g_shadowCity = city;
    g_shadowSim = sim;
    g_shadowDiffs = g_diffs;
    g_shadowLines = g_lines;
    // "the live" is the original's in the differential and FreeCity4's own in
    // the self-check (ReferenceFor).
    Log("traffic shadow: comparing with the live traffic simulator %p (city %p); differences named per line: "
        "%ld, delivery lines per month: %ld",
        sim, city, g_diffs, g_lines);
    return true;
}

// The original's record through its own Write (cIGZSerializable).
fc4_pollution_capture Capture(CaptureStream& s) {
    s.Reset();
    bool ok = false;
    void* p = nullptr;
    if (g_d.sim->QueryInterface(GZIID_cIGZSerializable, &p) && p != nullptr) {
        cIGZSerializable* ser = static_cast<cIGZSerializable*>(p);
        FC4_ACQUIRED(ser, "QueryInterface");
        ok = ser->Write(s);
        FC4_RELEASE(ser);
        if ((!ok || !s.Good()) && ++g_captureFailures <= 3) {
            Log("traffic shadow: the original's Write into memory failed (Write returned %d, the stream refused %s; "
                "%u bytes, %u references)",
                ok ? 1 : 0, s.Refused(), static_cast<unsigned>(s.Bytes().size()), static_cast<unsigned>(s.RefCount()));
        }
    } else if (!g_loggedNoSerializable) {
        g_loggedNoSerializable = true;
        Log("traffic shadow: the original traffic simulator does not answer cIGZSerializable; its record cannot "
            "be read: nothing is compared");
    }
    return s.View(ok);
}

void Sink(void*, const uint8_t* data, size_t len) { LogShadowLine("traffic shadow (cont.)", data, len); }

void BeforeStep(void*) {
    const fc4_pollution_capture c = Capture(g_before);
    fc4_traffic_shadow_before(g_shadow, g_d.type, g_d.data1, 0, 0, g_d.julian, &c);
    g_d.begun = true;
}

void AfterStep(void*) {
    const fc4_pollution_capture c = Capture(g_after);
    g_d.result = fc4_traffic_shadow_after(g_shadow, &c, &Sink, nullptr);
    g_d.finished = true;
}

struct Forward {
    uint32_t type, d1, d2, d3;
    uint64_t object;
};

void ForwardStep(void* p) {
    const Forward* f = static_cast<const Forward*>(p);
    if (fc4_traffic_shadow_message(g_shadow, f->type, f->d1, f->d2, f->d3, f->object)) ++g_forwarded;
}

void ForwardNow(cISC4City* city, const Forward& f) {
    BeginCall(city);
    if (!SehGuard("traffic shadow (a message to the copy)", &ForwardStep, const_cast<Forward*>(&f))) {
        g_faulted = true;
        Log("traffic shadow: the copy faulted on message 0x%08X (data %u, %u, %u, object %s); it is started again "
            "at the next delivery",
            f.type, f.d1, f.d2, f.d3, f.object != 0 ? "set" : "none");
    }
    EndCall();
}

// A message the original subscribes to (§1 Init step 3): to the copy, now,
// or after the delivery in progress.
void ForwardToShadow(cIGZMessage2* msg) {
    if (g_shadow == nullptr || msg == nullptr || g_faulted) return;
    if (g_mainTid != 0 && GetCurrentThreadId() != g_mainTid) {
        if (++g_offThread <= 3) Log("traffic shadow: a message off the main thread is not forwarded to the copy");
        return;
    }
    const uint32_t type = msg->GetType();
    void* q = nullptr;
    if (!msg->QueryInterface(GZIID_cIGZMessage2Standard, &q) || q == nullptr) return;
    cIGZMessage2Standard* s = static_cast<cIGZMessage2Standard*>(q);
    FC4_ACQUIRED(reinterpret_cast<cIGZUnknown*>(s), "QueryInterface");
    Forward f{type, static_cast<uint32_t>(s->GetData1()), static_cast<uint32_t>(s->GetData2()),
              static_cast<uint32_t>(s->GetData3()), 0};
    // §3.1: only the lot message (its IGZUnknown) and the occupant messages
    // (void1) carry an object.
    cIGZUnknown* o = nullptr;
    if (type == kMsgLot) {
        o = s->GetIGZUnknown();
    } else if (type == kMsgInsertOccupant || type == kMsgRemoveOccupant) {
        o = static_cast<cIGZUnknown*>(s->GetVoid1());
    }
    f.object = TokenOf(o);
    if (g_inDelivery) {
        if (o != nullptr) FC4_ADDREF(o);
        g_queued.push_back(Queued{f.type, f.d1, f.d2, f.d3, o});
    } else {
        ForwardNow(CurrentCity(), f);
    }
    FC4_RELEASE(reinterpret_cast<cIGZUnknown*>(s));
}

void FlushQueued(cISC4City* city) {
    std::vector<Queued> q;
    q.swap(g_queued);
    for (const Queued& m : q) {
        if (!g_faulted) ForwardNow(city, Forward{m.type, m.d1, m.d2, m.d3, TokenOf(m.object)});
        if (m.object != nullptr) FC4_RELEASE(m.object);
    }
}

// Forwards the original's messages (a static object: counted, never deleted).
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
        FC4_X87_SCOPE("traffic shadow events DoMessage", msg->GetType());  // x87 leftover guard (FC4Fpu.h)
        ForwardToShadow(msg);
        return true;
    }

private:
    uint32_t refs_ = 0;
};

EventTarget& Events() {
    static EventTarget t;
    return t;
}

// Registered once, at the first delivery the shadow takes (the message
// server exists by then), and left registered for the process.
void Listen() {
    if (g_listening || g_testMode) return;
    g_listening = true;
    cIGZMessageServer2Ptr ms;
    if (!ms) {
        Log("traffic shadow: no message server; the copy does not see the original's messages");
        return;
    }
    Events().AddRef();  // our own reference (static object)
    bool ok = true;
    for (uint32_t id : kEvents) ok = ms->AddNotification(static_cast<cIGZMessageTarget2*>(&Events()), id) && ok;
    if (!ok) Log("traffic shadow: some notifications were refused; the copy may miss a network change");
}

}  // namespace

void TrafficShadowStartup(bool simulatorReplaced, bool trafficReplaced) {
    ReadSettings();
    if (!g_enabled) return;  // off by default: not a word in the log
    const char* src = "";
    Setting("traffic_shadow", "FC4_TRAFFIC_SHADOW", &src);
    if (!simulatorReplaced) {
        // Without our simulator there is nothing to wrap the deliveries with.
        Log("traffic shadow: requested (traffic_shadow from %s) but INACTIVE: it wraps the clock deliveries our "
            "simulator makes, so it needs FreeCity4's simulator (do not `disable = simulator`); here traffic is %s",
            src, trafficReplaced ? "ours" : "the original");
        g_enabled = false;
        return;
    }
    // glue27: with `enable = traffic` the city's traffic simulator is ours, so
    // the original is no longer reachable through GetTrafficSimulator. Rather
    // than keeping a second, unregistered copy of the game's class alive (a
    // registry side channel that would have to be driven by hand), the shadow
    // becomes a class-layer self-check of the replacement - the same choice
    // glue21 made for the prop and lot configuration managers. Both sides are
    // our core, so a difference is a defect in the replacement (a host callback
    // that does not agree with the copy's read-only one, a record that does not
    // round trip, or non-determinism the record does not carry), not a
    // difference from the original. The original-vs-ours differential is still
    // what `traffic_shadow = 1` does without `enable = traffic`.
    g_selfCheck = trafficReplaced;
    if (g_selfCheck) {
        Log("traffic shadow: ON over FreeCity4's replacement (traffic_shadow from %s, `enable = traffic`): at each "
            "delivery to our traffic simulator's agents, that class's own record (its Write into memory) and live "
            "inputs start a private second instance of FreeCity4's traffic core on a read-only host, stepped to "
            "where the live class stopped; its record, trip lists, lot and connection holder arrays and grids are "
            "then compared bitwise with the live class's. One line per month, at most %ld delivery lines a month, "
            "%ld differences named per line. Not compared: the strategy draws (property 0x4A611773) and the "
            "pothole and accident draws' choices, posted messages, history, incomes, network costs, budget items, "
            "accidents",
            src, g_lines, g_diffs);
        LogRetiredHypotheses();
        return;
    }
    Log("traffic shadow: ON (traffic_shadow from %s): at each delivery to the ORIGINAL traffic simulator's "
        "agents, FreeCity4's simulator (a private copy on the original's record and live inputs) is stepped "
        "to where the original stopped and its record, trip lists, lot and connection holder arrays and "
        "grids are compared bitwise; one line per month, at most %ld delivery lines a month, %ld differences "
        "named per line. Not compared: the strategy draws (property 0x4A611773) and the pothole and accident "
        "draws' choices (the original's generator is not readable), posted messages, history, incomes, "
        "network costs, budget items, accidents",
        src, g_lines, g_diffs);
    LogRetiredHypotheses();
}

bool TrafficShadowDeliver(cIGZMessageTarget2* target, const char* agentName, cIGZMessage2* msg, uint32_t type,
                          int64_t data1, bool* finished) {
    if ((type != kMsgSimBegin && type != kMsgSimNewDay && type != kMsgSimNewMonth) || target == nullptr ||
        msg == nullptr || finished == nullptr || g_inDelivery) {
        return false;
    }
    if (type == kMsgSimNewDay || type == kMsgSimNewMonth) g_julian = static_cast<int32_t>(data1);
    if (!g_testMode && type == kMsgSimNewDay && g_julian != g_settingsJulian) {
        g_settingsJulian = g_julian;
        ReadSettings();  // once per day (FreeCity4.ini may change between city loads)
    }
    if (!g_testMode && !g_enabled) return false;
    // Cheap rejection for every other agent: ours registers by name (AddAgent).
    if (!g_testMode && target != g_agent && (agentName == nullptr || strcmp(agentName, kAgentName) != 0)) return false;
    cISC4City* city = CurrentCity();
    cIGZUnknown* sim = ReferenceFor(city, target, agentName);
    if (sim == nullptr || !EnsureShadow(city, sim)) return false;
    if (g_mainTid == 0) g_mainTid = GetCurrentThreadId();
    Listen();

    g_d = Delivery{};
    g_d.sim = sim;
    g_d.type = type;
    g_d.data1 = static_cast<uint32_t>(data1);
    g_d.julian = g_julian;
    BeginCall(city);
    const bool beforeClean = SehGuard("traffic shadow (before the original's delivery)", &BeforeStep, nullptr);
    if (!beforeClean && !g_d.begun) fc4_traffic_shadow_before(g_shadow, type, g_d.data1, 0, 0, g_d.julian, nullptr);
    EndCall();
    g_inDelivery = true;
    *finished = target->DoMessage(msg);  // the original traffic simulator, unchanged
    g_inDelivery = false;
    BeginCall(city);
    const bool afterClean = SehGuard("traffic shadow (after the original's delivery)", &AfterStep, nullptr);
    if (!afterClean && !g_d.finished) g_d.result = fc4_traffic_shadow_after(g_shadow, nullptr, &Sink, nullptr);
    EndCall();
    if (!beforeClean || !afterClean) g_faulted = true;
    FlushQueued(city);
    if (type == kMsgSimNewMonth && (g_strategyFailures > 0 || g_offThread > 0)) {
        Log("traffic shadow: strategy bytes read %u (failed %u); messages forwarded %u (off the main thread %u)",
            g_strategyReads, g_strategyFailures, g_forwarded, g_offThread);
    }
    g_lastResult = g_d.result;
    g_d = Delivery{};
    return true;  // delivered (compared or not)
}

}  // namespace fc4

intptr_t TrafficShadowTestDeliver(const FC4TrafficShadowTestInput* in) {
    if (in == nullptr) return -1;
    fc4::g_testMode = true;
    fc4::g_test = in;
    if ((in->fresh & 1u) != 0) {
        fc4::g_agentSim = nullptr;  // a fresh "game"
        fc4::g_agent = nullptr;
        fc4::g_shadowFailedFor = nullptr;
        fc4::DropShadow();
    }
    fc4::g_lastResult = -1;
    cFC4Message2Standard* msg = FC4MakeMessage(in->msgType, static_cast<intptr_t>(in->julian), 0, 0);
    bool finished = false;
    const bool taken = fc4::TrafficShadowDeliver(in->target, nullptr, msg, in->msgType, in->julian, &finished);
    msg->Release();
    if ((in->fresh & 2u) != 0) {
        fc4::DropShadow();
        fc4::g_agentSim = nullptr;
        fc4::g_agent = nullptr;
    }
    fc4::g_testMode = false;
    fc4::g_test = nullptr;
    if (!taken) return -1;
    return fc4::g_lastResult == -1 ? -2 : fc4::g_lastResult;
}
