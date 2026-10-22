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

#include "FC4FloraShadow.h"

#include "FC4Config.h"
#include "FC4CoreProps.h"
#include "FC4Diag.h"
#include "FC4Interfaces.h"
#include "FC4Log.h"
#include "FC4Properties.h"
#include "FC4RefAudit.h"
#include "FC4Streams.h"
#include "cFC4Flora.h"
#include "cFC4Message2Standard.h"

#include "SC4List.h"
#include "cGZPersistResourceKey.h"
#include "cIGZApp.h"
#include "cIGZCOM.h"
#include "cIGZFrameWork.h"
#include "cIGZMessage2.h"
#include "cIGZMessageTarget2.h"
#include "cIGZSerializable.h"
#include "cIGZString.h"
#include "cISC4App.h"
#include "cISC4City.h"
#include "cISC4FloraOccupant.h"
#include "cISC4FloraSimulator.h"
#include "cISC4Occupant.h"
#include "cISC4OccupantManager.h"
#include "cISC4Simulator.h"
#include "cISC4WeatherSimulator.h"
#include "cISTETerrain.h"
#include "cRZCOMDllDirector.h"
#include "cS3DVector3.h"

#include <algorithm>
#include <cstdio>
#include <cstring>
#include <map>
#include <memory>
#include <string>
#include <utility>
#include <vector>

namespace fc4 {
namespace {

constexpr uint32_t kMsgSimNewMonth = 0x66956816;
constexpr uint32_t kIID_cIGZMessageTarget2 = 0x452294AA;
constexpr uint32_t kTypeFlora = 0x74758926;
constexpr uint32_t kTuningType = 0x6534284A;
constexpr uint32_t kTuningGroup = 0x7A4A8458;
constexpr uint32_t kTuningInstance = 0x1A2FDB6A;
constexpr const char* kAgentName = "cSC4FloraSimulator";  // flora.md §4.1 step 4
// fc4::Log formats into a fixed buffer: longer shadow lines are split.
constexpr size_t kLogChunk = 2800;
constexpr size_t kListedBuckets = 6;

// ---- the capture stream ------------------------------------------------------------------------------
//
// The original's Write goes into memory: primitives little-endian (as the
// game's streams write them), strings as a uint32 length then the bytes, and
// references (WriteGZSerializable, the burnt trees) as uint32 objectID (the
// object's index + 1, 0 = null) then uint32 classID. The core reads the
// record up to the burnt count only. Process-lifetime objects (the reference
// count is not kept).
class CaptureStream final : public cFC4ISC4DBSegmentOStream {
public:
    void Reset() {
        bytes_.clear();
        ids_.clear();
        good_ = true;
    }
    const uint8_t* Data() const { return bytes_.empty() ? nullptr : bytes_.data(); }
    size_t Size() const { return bytes_.size(); }
    bool Good() const { return good_; }

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
        if (!const_cast<cIGZSerializable&>(s).Write(*this)) good_ = false;
        return good_;
    }
    bool SetVoid(void const* p, uint32_t n) override {
        if (n != 0 && p == nullptr) {
            good_ = false;
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
        size_t i = 0;
        while (i < ids_.size() && ids_[i] != p) ++i;
        if (i == ids_.size()) ids_.push_back(p);
        SetUint32(static_cast<uint32_t>(i + 1));
        return SetUint32(const_cast<cIGZSerializable*>(p)->GetGZCLSID());
    }
    bool WriteResKey(cGZPersistResourceKey const& k) override {
        SetUint32(k.type);
        SetUint32(k.group);
        return SetUint32(k.instance);
    }
    bool WriteVariant(cIGZVariant const&) override {
        good_ = false;  // not part of the flora record (§10.1)
        return false;
    }

private:
    bool Put(const void* p, size_t n) {
        const uint8_t* b = static_cast<const uint8_t*>(p);
        bytes_.insert(bytes_.end(), b, b + n);  // x86: already little-endian
        return good_;
    }
    bool PutStr(const char* s, size_t n) {
        if (n > 0x00FFFFFF || (n != 0 && s == nullptr)) {
            good_ = false;
            return false;
        }
        SetUint32(static_cast<uint32_t>(n));
        return Put(s, n);
    }

    std::vector<uint8_t> bytes_;
    std::vector<const void*> ids_;
    bool good_ = true;
};

#include "FC4FloraOccupantProbe.inc"

CaptureStream g_before;
CaptureStream g_after;

// ---- settings and run state (main thread only) ---------------------------------------------------

bool g_enabled = false;

// Test seam (op 17).
bool g_testMode = false;
const FC4FloraShadowTestInput* g_test = nullptr;

// The original simulator's agent (its cIGZMessageTarget2), per simulator.
const void* g_agentSim = nullptr;
cIGZMessageTarget2* g_agent = nullptr;
bool g_loggedOurs = false;
bool g_loggedNoSerializable = false;

// The shadow core, per city and original simulator.
fc4_flora_shadow* g_shadow = nullptr;
const void* g_shadowCity = nullptr;
const void* g_shadowSim = nullptr;
const void* g_shadowFailedFor = nullptr;
bool g_registryChecked = false;
float g_elevationScale = 0.0f;

using OccList = SC4List<cISC4Occupant*>;

// One census: the list keeps the occupants (and their identities) alive
// until the delivery ends.
struct Census {
    std::unique_ptr<OccList> list;
    std::vector<fc4_flora_census_entry> entries;
};

// The delivery being wrapped. The city's services are borrowed for it (the
// test seam's city has none).
struct Delivery {
    cISC4FloraSimulator* sim = nullptr;
    cISC4City* city = nullptr;
    cISC4OccupantManager* om = nullptr;
    cISTETerrain* terrain = nullptr;
    cISC4WeatherSimulator* weather = nullptr;
    int32_t julian = 0;
    uint32_t simDate = 0;
    float seaLevel = 0.0f;
    bool beforeOk = false;
    bool afterOk = false;
    Census before, after;
    bool finished = false;
    int32_t result = -1;
};
Delivery g_d;
int32_t g_lastResult = -1;
int32_t g_lastRegistry = -1;

void ReadSettings() { g_enabled = SettingInt("flora_shadow", "FC4_FLORA_SHADOW", 0) != 0; }

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

// The original flora simulator if `target` is its agent: its
// cIGZMessageTarget2 interface, or failing that (logged) an agent registered
// under its name.
cISC4FloraSimulator* OriginalFor(cISC4City* city, cIGZMessageTarget2* target, const char* agentName) {
    if (city == nullptr || target == nullptr) return nullptr;
    cISC4FloraSimulator* raw = city->GetFloraSimulator();  // compiled slot 59, borrowed
    if (raw == nullptr) return nullptr;
    if (IsOurObject(raw)) {
        if (!g_loggedOurs) {
            g_loggedOurs = true;
            Log("flora shadow: inactive: the city's flora simulator is FreeCity4's own (the shadow compares against "
                "the ORIGINAL flora simulator: do not enable flora)");
        }
        return nullptr;
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
        Log("flora shadow: the original flora simulator %p has agent %p (%s)", static_cast<void*>(raw),
            static_cast<void*>(g_agent), qi ? "its cIGZMessageTarget2 interface" : "no interface: matched by name");
    }
    const bool same = g_agent != nullptr && target == g_agent;
    const bool byName = g_agent == nullptr && agentName != nullptr && strcmp(agentName, kAgentName) == 0;
    return (same || byName) ? raw : nullptr;
}

void DropShadow() {
    fc4_flora_shadow_destroy(g_shadow);
    g_shadow = nullptr;
    g_shadowCity = nullptr;
    g_shadowSim = nullptr;
    g_registryChecked = false;
}

// The tuning exemplar's properties (the elevation scale, §7), as they are.
fc4_props* TuningProps() {
    props::Exemplar ex;
    if (!ex.Load(kTuningType, kTuningGroup, kTuningInstance)) return nullptr;
    return props::ToCoreAll(ex.Holder());
}

bool EnsureShadow(const void* city, const void* sim) {
    if (g_shadow != nullptr && g_shadowCity == city && g_shadowSim == sim) return true;
    DropShadow();
    g_shadow = fc4_flora_shadow_create();
    if (g_shadow == nullptr) {
        if (g_shadowFailedFor != sim) {
            g_shadowFailedFor = sim;
            Log("flora shadow: the shadow core could not be created; deliveries pass through uncompared");
        }
        return false;
    }
    g_elevationScale = fc4_flora_shadow_set_tuning(g_shadow, TuningProps());
    g_shadowCity = city;
    g_shadowSim = sim;
    Log("flora shadow: comparing with the original flora simulator %p (city %p); elevation scale %g", sim, city,
        static_cast<double>(g_elevationScale));
    return true;
}

// fc4::Log has a fixed buffer: a long line goes out in pieces, split at a
// space where there is one.
void Sink(void*, const uint8_t* data, size_t len) {
    const char* p = reinterpret_cast<const char*>(data);
    size_t off = 0;
    bool first = true;
    while (first || off < len) {
        size_t n = std::min(kLogChunk, len - off);
        if (off + n < len) {
            size_t cut = n;
            while (cut > kLogChunk / 2 && p[off + cut - 1] != ' ') --cut;
            if (cut > kLogChunk / 2) n = cut;
        }
        if (first) {
            Log("%.*s", static_cast<int>(n), p + off);
        } else {
            Log("flora shadow (cont.) %.*s", static_cast<int>(n), p + off);
        }
        off += n;
        first = false;
    }
}

// The original's record through its own Write (cIGZSerializable).
bool Capture(CaptureStream& s) {
    s.Reset();
    void* p = nullptr;
    if (g_d.sim->QueryInterface(GZIID_cIGZSerializable, &p) && p != nullptr) {
        cIGZSerializable* ser = static_cast<cIGZSerializable*>(p);
        FC4_ACQUIRED(ser, "QueryInterface");
        const bool ok = ser->Write(s);
        FC4_RELEASE(ser);
        return ok && s.Good();
    }
    if (!g_loggedNoSerializable) {
        g_loggedNoSerializable = true;
        Log("flora shadow: the original flora simulator does not answer cIGZSerializable; its record (and with it "
            "the visited blocks) cannot be read: the months are not compared");
    }
    return false;
}

// Every flora occupant of the city (the occupant manager's standard-cell
// query with null corners, flora.md §6.10.4), with the score inputs at its
// position when `inputs`.
void TakeCensus(Census& c, bool inputs) {
    c.entries.clear();
    if (g_d.om == nullptr) return;
    c.list.reset(new OccList());
    g_d.om->GetOccupantsByStandardCityCells(*c.list, nullptr, nullptr, kTypeFlora, 0xFFFFFFFFu);  // slot 41
    for (cISC4Occupant* o : *c.list) {
        if (o == nullptr) continue;
        fc4_flora_census_entry e{};
        e.token = TokenOf(o);
        void* q = nullptr;
        if (o->QueryInterface(GZIID_cISC4FloraOccupant, &q) && q != nullptr) {
            cISC4FloraOccupant* f = static_cast<cISC4FloraOccupant*>(q);
            FC4_ACQUIRED(reinterpret_cast<cIGZUnknown*>(f), "QueryInterface");
            e.flora_type = f->GetFloraType();
            e.birth_date = f->GetBirthDate();
            e.last_seeding_date = f->GetLastSeedingDate();
            e.state = static_cast<int8_t>(f->GetState());
            e.is_flora = 1;
            FC4_RELEASE(reinterpret_cast<cIGZUnknown*>(f));
        }
        e.in_manager = o->IsFlagSet(1) ? 1 : 0;  // slot 32
        cS3DVector3 pos, mn, mx;
        o->GetPosition(&pos);           // slot 8
        o->GetBoundingBox(&mn, &mx);  // slot 10
        e.pos[0] = pos.fX;
        e.pos[1] = pos.fY;
        e.pos[2] = pos.fZ;
        e.bbox_min[0] = mn.fX;
        e.bbox_min[1] = mn.fY;
        e.bbox_min[2] = mn.fZ;
        e.bbox_max[0] = mx.fX;
        e.bbox_max[1] = mx.fY;
        e.bbox_max[2] = mx.fZ;
        if (inputs) {
            e.moisture = g_d.weather ? AsGame(g_d.weather)->GetMoistureType(pos.fX, pos.fZ) : 0;
            e.altitude = g_d.terrain ? g_d.terrain->GetAltitude(pos.fX, pos.fZ) : 0.0f;  // compiled slot 13
        }
        c.entries.push_back(e);
    }
}

void FillServices(cISC4City* city) {
    g_d.city = city;
    if (g_testMode || city == nullptr) return;  // the test city answers slot 59 only
    g_d.om = city->GetOccupantManager();     // borrowed
    g_d.terrain = city->GetTerrain();        // borrowed
    g_d.weather = city->GetWeatherSimulator();  // borrowed (may be null, §6.2)
    cISC4Simulator* s = city->GetSimulator();
    g_d.simDate = s ? static_cast<uint32_t>(AsGame(s)->GetSimDateNumber()) : 0;  // slot 8
    g_d.seaLevel = g_d.terrain ? g_d.terrain->GetSeaLevel() : 0.0f;              // slot 29
}

void ExemplarsAsTheyAre(void*, fc4_flora_exemplar_visitor visit, void* ctx) {
    FloraForEachExemplar(false, false, visit, ctx);  // no references kept
}

// Once per city and simulator: the original's registry against ours, built
// from the same exemplars.
void RegistryStep(void*) {
    uint32_t n = 0;
    g_d.sim->GetAllFloraTypes(nullptr, n);  // slot 9: the count
    std::vector<uint32_t> types(n);
    if (n != 0) {
        uint32_t m = n;
        g_d.sim->GetAllFloraTypes(types.data(), m);
        if (m < n) types.resize(m);
    }
    std::vector<fc4_flora_record> recs(types.size());
    for (size_t i = 0; i < types.size(); ++i) {
        if (!FloraBlockValues(g_d.sim->GetStaticFloraData(types[i]), &recs[i])) recs[i].type_id = 0;  // slot 7
    }
    fc4_flora_record key0{};
    const bool hasKey0 = FloraBlockValues(g_d.sim->GetStaticFloraData(0), &key0);
    const bool key0Valid = g_d.sim->IsFloraTypeValid(0);  // slot 11
    g_lastRegistry = fc4_flora_shadow_registry(g_shadow, &ExemplarsAsTheyAre, nullptr,
                                               types.empty() ? nullptr : types.data(), types.size(),
                                               recs.empty() ? nullptr : recs.data(), hasKey0 ? &key0 : nullptr,
                                               key0Valid, &Sink, nullptr);
}

void BeforeStep(void*) {
    g_d.beforeOk = Capture(g_before);
    TakeCensus(g_d.before, true);
}

const char* const kStageNames[] = {"young", "maturing", "mature", "burnt", "dying"};

// "N (young a, maturing b, mature c, burnt d, dying e, other f; types T)".
std::string CensusText(const std::vector<fc4_flora_census_entry>& es, std::map<std::pair<uint32_t, int>, uint32_t>& buckets) {
    uint32_t stage[6] = {};
    std::map<uint32_t, uint32_t> types;
    for (const fc4_flora_census_entry& e : es) {
        if (e.is_flora == 0) {
            ++stage[5];
            continue;
        }
        const int s = (e.state >= 0 && e.state <= 4) ? e.state : 5;
        ++stage[s];
        ++types[e.flora_type];
        ++buckets[{e.flora_type, s}];
    }
    char buf[200];
    std::snprintf(buf, sizeof(buf), "%u (young %u, maturing %u, mature %u, burnt %u, dying %u, other %u; types %u)",
                  static_cast<unsigned>(es.size()), stage[0], stage[1], stage[2], stage[3], stage[4], stage[5],
                  static_cast<unsigned>(types.size()));
    return buf;
}

// The census by stage and type, before and after: the counts, and the first
// (type, stage) buckets whose counts changed.
void LogCensus() {
    std::map<std::pair<uint32_t, int>, uint32_t> b, a;
    const std::string before = CensusText(g_d.before.entries, b);
    const std::string after = CensusText(g_d.after.entries, a);
    std::map<std::pair<uint32_t, int>, std::pair<uint32_t, uint32_t>> all;
    for (const auto& kv : b) all[kv.first].first = kv.second;
    for (const auto& kv : a) all[kv.first].second = kv.second;
    std::string changed;
    size_t n = 0;
    for (const auto& kv : all) {
        if (kv.second.first == kv.second.second) continue;
        if (n++ < kListedBuckets) {
            char buf[80];
            const int s = kv.first.second;
            std::snprintf(buf, sizeof(buf), "%s%08X %s %u->%u", changed.empty() ? "" : ", ", kv.first.first,
                          s <= 4 ? kStageNames[s] : "other", kv.second.first, kv.second.second);
            changed += buf;
        }
    }
    fc4_date d{};
    fc4_date_from_julian(g_d.julian, &d);
    Log("flora shadow census %04d-%02d-%02d: before %s after %s; type/stage buckets changed %u%s%s%s", d.year,
        d.month, d.day_of_month, before.c_str(), after.c_str(), static_cast<unsigned>(n),
        n != 0 ? " (" : "", changed.c_str(), n > kListedBuckets ? ", ...)" : (n != 0 ? ")" : ""));
}

void AfterStep(void*) {
    g_d.afterOk = Capture(g_after);
    TakeCensus(g_d.after, false);
    // The original's records for every type of either census.
    std::vector<uint32_t> types;
    for (const Census* c : {&g_d.before, &g_d.after}) {
        for (const fc4_flora_census_entry& e : c->entries) {
            if (e.is_flora != 0) types.push_back(e.flora_type);
        }
    }
    std::sort(types.begin(), types.end());
    types.erase(std::unique(types.begin(), types.end()), types.end());
    std::vector<fc4_flora_record> recs;
    recs.reserve(types.size());
    for (uint32_t t : types) {
        fc4_flora_record r{};
        if (FloraBlockValues(g_d.sim->GetStaticFloraData(t), &r)) recs.push_back(r);  // slot 7
    }
    fc4_flora_shadow_month_in m{};
    m.struct_size = sizeof(m);
    m.julian = static_cast<uint32_t>(g_d.julian);
    m.sim_date = g_d.simDate;
    m.sea_level = g_d.seaLevel;
    if (g_d.beforeOk) {
        m.before = g_before.Data();
        m.before_len = g_before.Size();
    }
    if (g_d.afterOk) {
        m.after = g_after.Data();
        m.after_len = g_after.Size();
    }
    m.census_before = g_d.before.entries.empty() ? nullptr : g_d.before.entries.data();
    m.before_count = g_d.before.entries.size();
    m.census_after = g_d.after.entries.empty() ? nullptr : g_d.after.entries.data();
    m.after_count = g_d.after.entries.size();
    m.records = recs.empty() ? nullptr : recs.data();
    m.record_count = recs.size();
    g_d.result = fc4_flora_shadow_month(g_shadow, &m, &Sink, nullptr);
    g_d.finished = true;
    LogCensus();
}

// A month whose steps faulted: compared with what was captured (nothing).
void FinishUncaptured() {
    fc4_flora_shadow_month_in m{};
    m.struct_size = sizeof(m);
    m.julian = static_cast<uint32_t>(g_d.julian);
    g_d.result = fc4_flora_shadow_month(g_shadow, &m, &Sink, nullptr);
}

}  // namespace

void FloraShadowStartup(bool simulatorReplaced, bool floraReplaced) {
    ReadSettings();
    if (!g_enabled) return;  // off by default: not a word in the log
    const char* src = "";
    Setting("flora_shadow", "FC4_FLORA_SHADOW", &src);
    if (simulatorReplaced && !floraReplaced) {
        Log("flora shadow: ON (flora_shadow from %s): once per city, the ORIGINAL flora simulator's registry after "
            "load (type array, IsFloraTypeValid(0), every record's values from its block) against one FreeCity4 "
            "builds from the same exemplars; at each of its SimNewMonth, its record (its own Write) and the census "
            "of every flora occupant (type, state, birth, last seeding, position, box, flag 1) just before and "
            "after, the visited blocks from the record's iterator, and the deterministic part of the aging rule "
            "(flora.md 6.7.2: rebirth, stage, removal, last seeding date) applied to each of their occupants; "
            "other occupants and the record's scalars must not change. Not compared (random, 8): where seeds land "
            "and their acceptance, cluster merges (counted as vanished / new), wild seeds, the generator state; "
            "nor the family map, the type sets of slots 8, 10, 13, 14, flag bits 3-7, the key-0 record's flags "
            "and plop cost; a pass that wraps the iterator twice. One registry line, one month line and one "
            "census line by stage and type",
            src);
    } else {
        Log("flora shadow: requested (flora_shadow from %s) but INACTIVE: it needs FreeCity4's simulator and the "
            "original flora simulator (do not enable flora); here the simulator is %s and flora is %s",
            src, simulatorReplaced ? "ours" : "the original", floraReplaced ? "ours" : "the original");
    }
}

bool FloraShadowDeliver(cIGZMessageTarget2* target, const char* agentName, cIGZMessage2* msg, uint32_t type,
                        int64_t data1, bool* finished) {
    if (type != kMsgSimNewMonth || target == nullptr || msg == nullptr || finished == nullptr) return false;
    if (!g_testMode && SettingInt("flora_occupant_verify", "FC4_FLORA_OCCUPANT_VERIFY", 0) != 0 &&
        agentName != nullptr && strcmp(agentName, kAgentName) == 0)
        FloraOccupantProbe(CurrentCity(), data1);
    if (!g_testMode) ReadSettings();  // once a month (FreeCity4.ini may change between city loads)
    if (!g_testMode && !g_enabled) return false;
    // Cheap rejection for every other agent: the original registers by name (AddAgent).
    if (!g_testMode && target != g_agent && (agentName == nullptr || strcmp(agentName, kAgentName) != 0)) return false;
    cISC4City* city = CurrentCity();
    cISC4FloraSimulator* sim = OriginalFor(city, target, agentName);
    if (sim == nullptr || !EnsureShadow(city, sim)) return false;

    g_d = Delivery{};
    g_d.sim = sim;
    g_d.julian = static_cast<int32_t>(data1);
    FillServices(city);
    if (!g_registryChecked) {
        g_registryChecked = true;
        g_lastRegistry = -1;
        SehGuard("flora shadow (the original's registry)", &RegistryStep, nullptr);
    }
    SehGuard("flora shadow (before the original's month)", &BeforeStep, nullptr);
    *finished = target->DoMessage(msg);  // the original flora simulator, unchanged
    const bool afterClean = SehGuard("flora shadow (after the original's month)", &AfterStep, nullptr);
    if (!afterClean && !g_d.finished) FinishUncaptured();
    g_lastResult = g_d.result;
    g_d = Delivery{};  // the census lists release their occupants
    return true;       // delivered (compared or not)
}

}  // namespace fc4

intptr_t FloraShadowTestDeliver(const FC4FloraShadowTestInput* in, intptr_t* registryOut) {
    if (registryOut != nullptr) *registryOut = -1;
    if (in == nullptr) return -1;
    fc4::g_testMode = true;
    fc4::g_test = in;
    fc4::g_agentSim = nullptr;  // each call is a fresh "game"
    fc4::g_agent = nullptr;
    fc4::g_shadowFailedFor = nullptr;
    fc4::DropShadow();
    fc4::g_lastResult = -1;
    fc4::g_lastRegistry = -1;
    cFC4Message2Standard* msg = FC4MakeMessage(in->msgType, static_cast<intptr_t>(in->julian), 0, 0);
    bool finished = false;
    const bool taken = fc4::FloraShadowDeliver(in->target, nullptr, msg, in->msgType, in->julian, &finished);
    msg->Release();
    fc4::DropShadow();
    fc4::g_agentSim = nullptr;
    fc4::g_agent = nullptr;
    fc4::g_testMode = false;
    fc4::g_test = nullptr;
    if (registryOut != nullptr) *registryOut = fc4::g_lastRegistry;
    if (!taken) return -1;
    return fc4::g_lastResult == -1 ? -2 : fc4::g_lastResult;
}
