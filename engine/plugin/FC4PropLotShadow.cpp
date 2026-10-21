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

#include "FC4PropLotShadow.h"

#include "FC4Config.h"
#include "FC4CoreProps.h"
#include "FC4Diag.h"
#include "FC4DevelopmentInterfaces.h"
#include "FC4GameList.h"
#include "FC4Interfaces.h"
#include "FC4Log.h"
#include "FC4Properties.h"
#include "FC4RefAudit.h"
#include "FC4Streams.h"
#include "FC4TractInterfaces.h"
#include "FC4WriteCapture.h"
#include "cFC4LotDeveloper.h"
#include "cFC4Tutorial.h"

#include "GZServPtrs.h"
#include "SC4List.h"
#include "SC4Point.h"
#include "SC4Preferences.h"
#include "SC4Rect.h"
#include "SC4Vector.h"
#include "cGZPersistResourceKey.h"
#include "cIGZApp.h"
#include "cIGZCOM.h"
#include "cIGZFrameWork.h"
#include "cIGZPersistResourceKeyFilter.h"
#include "cIGZPersistResourceKeyList.h"
#include "cIGZPersistResourceManager.h"
#include "cIGZSerializable.h"
#include "cISC4App.h"
#include "cISC4BuildingDevelopmentSimulator.h"
#include "cISC4City.h"
#include "cISC4FloraSimulator.h"
#include "cISC4Lot.h"
#include "cISC4LotConfiguration.h"
#include "cISC4LotConfigurationManager.h"
#include "cISC4LotManager.h"
#include "cISC4Occupant.h"
#include "cISC4OccupantFilter.h"
#include "cISC4OccupantManager.h"
#include "cISC4PropManager.h"
#include "cISC4PropOccupant.h"
#include "cISC4Simulator.h"
#include "cISC4TutorialSystem.h"
#include "cRZCOMDllDirector.h"
#include "cS3DVector3.h"
#include "cSC4LotConfigurationObject.h"

#include <algorithm>
#include <cmath>
#include <cstdarg>
#include <cstdio>
#include <cstring>
#include <ctime>
#include <memory>
#include <string>
#include <vector>

namespace fc4 {
namespace {

// ---- constants (docs/specs/prop-lotconfig.md, tutorial.md) ---------------------------------------------

constexpr uint32_t kExemplarType = 0x6534284A;
constexpr uint32_t kOccupantTypeProp = 0xA823821E;      // NG25
constexpr uint32_t kOccupantTypeBuilding = 0x278128A0;  // §A7.2 slot 34
constexpr uint32_t kIID_cISC4PropOccupant = 0xE9793A65;
constexpr uint32_t kCLSID_PropManager = 0xA990BD10;
constexpr uint32_t kCLSID_LotConfigurationManager = 0x299062C0;
constexpr uint32_t kCLSID_TutorialSystem = 0xAA3836FB;
constexpr uint32_t kObjectTypeFoundation = 3;  // lot-configuration.md §3.7
// §A6.1 step 3: the built-in prop classes by fc4 kind.
constexpr uint32_t kBuiltinClass[3] = {0x2977AA47, 0x2977AA48, 0x2977AA49};

constexpr long kDefaultCells = 32;
constexpr long kMaxCells = 256;
constexpr uint32_t kCreateSamples = 8;
constexpr uint32_t kLcmFamilySamples = 16;
constexpr uint32_t kLcmSizeSamples = 8;
constexpr uint32_t kLcmConfigSamples = 16;
constexpr unsigned kFirst = 6;          // differences listed per line
constexpr size_t kMaxList = 1u << 16;   // a longer list is not trusted
constexpr size_t kMaxCatalogue = 1u << 20;

// ---- settings and totals -------------------------------------------------------------------------------

bool g_prop = false;
bool g_lcm = false;
bool g_tut = false;
long g_cells = kDefaultCells;

struct Totals {
    uint32_t lines = 0, exact = 0, mismatched = 0, incomplete = 0;
    void Note(intptr_t mismatches) {
        ++lines;
        if (mismatches < 0) {
            ++incomplete;
        } else if (mismatches == 0) {
            ++exact;
        } else {
            ++mismatched;
        }
    }
};
Totals g_propTotals, g_lcmTotals, g_tutTotals;

// ---- formatting -----------------------------------------------------------------------------------------

void AppendF(std::string& s, const char* fmt, ...) {
    char buf[512];
    va_list ap;
    va_start(ap, fmt);
    _vsnprintf_s(buf, sizeof(buf), _TRUNCATE, fmt, ap);
    va_end(ap);
    s += buf;
}

struct FirstList {
    std::string text;
    unsigned listed = 0;
    uint32_t count = 0;
    void Add(const char* fmt, ...) {
        ++count;
        if (listed >= kFirst) return;
        char buf[256];
        va_list ap;
        va_start(ap, fmt);
        _vsnprintf_s(buf, sizeof(buf), _TRUNCATE, fmt, ap);
        va_end(ap);
        if (listed != 0) text += ",";
        text += buf;
        ++listed;
    }
    const char* Str() const { return listed == 0 ? "-" : text.c_str(); }
};

struct Run {
    cISC4City* city = nullptr;
    int32_t julian = 0;
    const char* event = "month";
    bool load = false;
    char date[16] = "-";
};

// One line: "<name> shadow (<what>) <date> <event>: mismatches=N <body>".
void Line(const char* name, Totals& t, const Run& r, const char* what, intptr_t mismatches, const std::string& body) {
    std::string line;
    if (mismatches < 0) {
        AppendF(line, "%s shadow (%s) %s %s: INCOMPLETE (", name, what, r.date, r.event);
        line += body;
        line += "); not compared";
    } else {
        AppendF(line, "%s shadow (%s) %s %s: mismatches=%ld ", name, what, r.date, r.event,
                static_cast<long>(mismatches));
        line += body;
    }
    std::string cont;
    AppendF(cont, "%s shadow (cont.)", name);
    LogShadowLine(cont.c_str(), reinterpret_cast<const uint8_t*>(line.data()), line.size());
    t.Note(mismatches);
}

uint32_t NextRandom(uint32_t& s) {
    s ^= s << 13;
    s ^= s >> 17;
    s ^= s << 5;
    return s;
}

uint32_t Seed(int32_t julian, uint32_t salt) {
    uint32_t s = static_cast<uint32_t>(julian) * 2654435761u ^ salt;
    return s == 0 ? 0x9E3779B9u : s;
}

bool Odd(uint64_t t) { return (t & 1u) != 0; }

// ---- host references --------------------------------------------------------------------------------------
//
// Tokens are the game objects' interface pointers; odd tokens are the
// shadows' own stand-ins (recorded props, bound configurations) and carry no
// reference.

void HostAddRef(void*, uint64_t t) {
    if (t == 0 || Odd(t)) return;
    FC4_ADDREF(FromToken(t));
}

void HostRelease(void*, uint64_t t) {
    if (t == 0 || Odd(t)) return;
    FC4_RELEASE(FromToken(t));
}

// A reference read from the original's record (CaptureIStream): a host object
// carries the reader's reference already; one of FreeCity4's objects carries
// none, so one is taken for the core (FC4WriteCapture.h).
uint64_t HostResolveRef(void*, uint64_t t, uint32_t) {
    if (t != 0 && !Odd(t) && IsOurObject(FromToken(t))) FC4_ADDREF(FromToken(t));
    return t;
}

// ---- tearing a shadow core down on every exit path --------------------------------------------------------
//
// SEH does not unwind C++ frames: if SehGuard catches a structured exception
// inside a run step, that step's own destructor never runs and its core would
// leak. Each step registers the core it is using here and clears it when it
// finishes; RunAll clears whatever is left after every step, so the teardown
// happens however the step ended.

struct LiveCore {
    void* p = nullptr;
    void (*teardown)(void*) = nullptr;
    void Reset() {
        if (p == nullptr) return;
        void* q = p;
        void (*fn)(void*) = teardown;
        p = nullptr;
        teardown = nullptr;
        if (fn != nullptr) fn(q);
    }
};
LiveCore g_liveCore;

// The process-wide generated-tag serial of lot-configuration.md §3.5 (the
// original's global 0x00B2166C): fc4_lot_config_object_array takes the
// counter, not a fresh one, and advances it once per kept object of a
// configuration below version 2. One counter for the process, as the
// original's is. It is seeded when 0 from the same wall clock the plugin uses
// for the session seed (FC4Seed.h), standing in for the GZCOM clock; so the
// absolute tags of version-<2 objects are generated independently on the two
// sides and are not byte-comparable (question GLUE15-1). Every shipped
// configuration is version 2 and carries its own tags, which compare exactly.
uint32_t g_lcmSerial = 0;

uint32_t* NextLotSerial() {
    if (g_lcmSerial == 0) {
        g_lcmSerial = static_cast<uint32_t>(std::time(nullptr));
        if (g_lcmSerial == 0) g_lcmSerial = 1;
    }
    return &g_lcmSerial;
}

// ---- the exemplar keys (§A3.1 step 1, §B3.1 step 3) -------------------------------------------------------

class ExemplarKeyFilter final : public cIGZPersistResourceKeyFilter {
public:
    bool QueryInterface(uint32_t riid, void** ppvObj) override {
        if (ppvObj == nullptr) return false;
        if (riid == GZIID_cIGZPersistResourceKeyFilter || riid == GZIID_cIGZUnknown) {
            *ppvObj = this;
            AddRef();
            return true;
        }
        *ppvObj = nullptr;
        return false;
    }
    uint32_t AddRef() override { return ++refs_; }
    uint32_t Release() override { return refs_ > 0 ? --refs_ : 0; }
    bool IsKeyIncluded(cGZPersistResourceKey const& key) override { return key.type == kExemplarType; }

private:
    uint32_t refs_ = 1;
};

// The resource manager's exemplar keys in its order; false without one.
bool ExemplarKeys(std::vector<cGZPersistResourceKey>& keys) {
    keys.clear();
    cIGZPersistResourceManagerPtr rm;
    if (!rm) return false;
    ExemplarKeyFilter filter;
    cIGZPersistResourceKeyList* l = nullptr;
    rm->GetAvailableResourceList(&l, &filter);
    if (l == nullptr) return false;
    FC4_ACQUIRED(l, "GetAvailableResourceList");
    const uint32_t n = l->Size();  // cIGZPersistResourceKeyList slot 9
    keys.reserve(n);
    for (uint32_t i = 0; i < n; ++i) keys.push_back(l->GetKey(i));
    FC4_RELEASE_CREATED(l);
    return true;
}

bool HostForEachExemplarKey(void*, fc4_key_visitor visit, void* ctx) {
    std::vector<cGZPersistResourceKey> keys;
    if (!ExemplarKeys(keys)) return false;
    if (visit == nullptr) return true;
    for (const cGZPersistResourceKey& k : keys) {
        if (k.type == kExemplarType) visit(ctx, k.type, k.group, k.instance);
    }
    return true;
}

fc4_props* LoadProps(uint32_t t, uint32_t g, uint32_t i) {
    props::Exemplar ex;
    if (!ex.Load(t, g, i)) return nullptr;
    return props::ToCoreAll(ex);
}

fc4_props* HostLoadExemplar(void*, uint32_t t, uint32_t g, uint32_t i) { return LoadProps(t, g, i); }

// ---- an fc4 core's record as a cIGZSerializable (for WriteCapture) ----------------------------------------

class CoreRecord final : public cIGZSerializable {
public:
    using WriteFn = bool (*)(const void* core, const fc4_ostream* out);
    CoreRecord(WriteFn fn, const void* core, uint32_t cls) : fn_(fn), core_(core), cls_(cls) {}
    bool QueryInterface(uint32_t riid, void** ppvObj) override {
        if (ppvObj == nullptr) return false;
        if (riid == GZIID_cIGZSerializable || riid == GZIID_cIGZUnknown) {
            *ppvObj = static_cast<cIGZSerializable*>(this);
            return true;
        }
        *ppvObj = nullptr;
        return false;
    }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    bool Write(cIGZOStream& s) override {
        OStreamAdapter a(s);
        return fn_(core_, a.Get());
    }
    bool Read(cIGZIStream&) override { return false; }
    uint32_t GetGZCLSID() override { return cls_; }

private:
    WriteFn fn_;
    const void* core_;
    uint32_t cls_;
};

// The canonical identity of a written reference (its cIGZUnknown), for the
// reference-by-reference comparison; borrowed (the reference is dropped).
const void* Canonical(const void* obj) {
    if (obj == nullptr) return nullptr;
    cIGZUnknown* u = static_cast<cIGZUnknown*>(const_cast<void*>(obj));
    void* q = nullptr;
    if (!u->QueryInterface(GZIID_cIGZUnknown, &q) || q == nullptr) return obj;
    cIGZUnknown* c = static_cast<cIGZUnknown*>(q);
    FC4_ACQUIRED(c, "QueryInterface");
    FC4_RELEASE(c);
    return c;
}

WriteCapture g_capOrig;
WriteCapture g_capOurs;

// The record comparison: the original's capture (g_capOrig, already taken)
// against ours. Returns the mismatches; fills `body`.
intptr_t CompareRecords(bool oursOk, std::string& body) {
    const std::vector<uint8_t>& a = g_capOrig.Bytes();
    const std::vector<uint8_t>& b = g_capOurs.Bytes();
    intptr_t m = 0;
    size_t firstDiff = SIZE_MAX;
    for (size_t i = 0; i < std::min(a.size(), b.size()); ++i) {
        if (a[i] != b[i]) {
            firstDiff = i;
            break;
        }
    }
    if (!oursOk) ++m;
    if (a.size() != b.size() || firstDiff != SIZE_MAX) ++m;
    // References: the objects written in the same places (the bytes number
    // them in order of first appearance, so equal bytes pair them up). The
    // capture numbers references 1..63 (FC4WriteCapture.h), so a record with
    // more than 63 written references pairs only the first 63.
    uint32_t refs = 0, refsDiff = 0;
    for (uint32_t id = 1; id < 64; ++id) {
        const void* x = g_capOrig.Object(id);
        const void* y = g_capOurs.Object(id);
        if (x == nullptr && y == nullptr) break;
        ++refs;
        if (Canonical(x) != Canonical(y)) ++refsDiff;
    }
    if (refsDiff != 0) ++m;
    AppendF(body, "bytes=%u/%u (orig/ours) write=%u refs=%u refsDiffering=%u", static_cast<unsigned>(a.size()),
            static_cast<unsigned>(b.size()), oursOk ? 1u : 0u, refs, refsDiff);
    if (firstDiff != SIZE_MAX) {
        AppendF(body, " firstByteDiff=+%u (%02X/%02X)", static_cast<unsigned>(firstDiff), a[firstDiff], b[firstDiff]);
    }
    return m;
}

void* P(uint64_t t) { return static_cast<void*>(FromToken(t)); }

void Collect(void* ctx, uint64_t token) { static_cast<std::vector<uint64_t>*>(ctx)->push_back(token); }

void CollectU32s(void* ctx, const uint32_t* v, size_t n) {
    auto* out = static_cast<std::vector<uint32_t>*>(ctx);
    out->assign(v, v + n);
}

// The original's occupant list (one reference each): its entries in order,
// each reference noted and given back (the list's destructor frees the nodes).
void TakeOccupantList(SC4List<cISC4Occupant*>& list, std::vector<uint64_t>& out) {
    out.clear();
    size_t n = 0;
    for (cISC4Occupant*& o : list) {
        if (++n > kMaxList) break;
        out.push_back(TokenOf(o));
        if (o != nullptr) {
            FC4_ACQUIRED(o, "prop list entry");
            FC4_RELEASE(o);
        }
        o = nullptr;
    }
}

// Our list (one reference each for us): released after the comparison.
void ReleaseTokens(const std::vector<uint64_t>& v) {
    for (uint64_t t : v) HostRelease(nullptr, t);
}

std::string ListDiff(const std::vector<uint64_t>& a, const std::vector<uint64_t>& b) {
    std::string s;
    AppendF(s, "%u/%u", static_cast<unsigned>(a.size()), static_cast<unsigned>(b.size()));
    for (size_t i = 0; i < std::max(a.size(), b.size()); ++i) {
        const uint64_t x = i < a.size() ? a[i] : 0;
        const uint64_t y = i < b.size() ? b[i] : 0;
        if (x != y) {
            AppendF(s, " @%u %p/%p", static_cast<unsigned>(i), P(x), P(y));
            break;
        }
    }
    return s;
}

cISC4App* App() {
    cIGZFrameWork* fw = RZGetFrameWork();
    cIGZApp* app = fw ? fw->Application() : nullptr;
    cISC4App* sc4 = nullptr;
    if (app == nullptr || !app->QueryInterface(GZIID_cISC4App, reinterpret_cast<void**>(&sc4)) || sc4 == nullptr) {
        return nullptr;
    }
    FC4_ACQUIRED(sc4, "QueryInterface");
    FC4_RELEASE(sc4);  // the application lives for the process
    return sc4;
}

// ================================================================================================
// The prop manager shadow (Part A)
// ================================================================================================

// The prop occupant filter the shadow's host hands the occupant manager: an
// occupant passes when its type is a prop's and its prop usage is `usage`
// (NG25; §A2's filter, usage at +0xC). Static; its reference count is not kept.
class PropUsageFilter final : public cISC4OccupantFilter {
public:
    uint32_t usage = 0;
    bool QueryInterface(uint32_t riid, void** ppvObj) override {
        if (ppvObj == nullptr) return false;
        if (riid == GZIID_cISC4OccupantFilter || riid == GZIID_cIGZUnknown) {
            *ppvObj = static_cast<cISC4OccupantFilter*>(this);
            return true;
        }
        *ppvObj = nullptr;
        return false;
    }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    bool IsOccupantIncluded(cISC4Occupant* o) override {
        if (o == nullptr || static_cast<uint32_t>(o->GetType()) != kOccupantTypeProp) return false;
        void* q = nullptr;
        if (!o->QueryInterface(kIID_cISC4PropOccupant, &q) || q == nullptr) return false;
        cISC4PropOccupant* p = static_cast<cISC4PropOccupant*>(q);
        FC4_ACQUIRED(p, "QueryInterface");
        const bool pass = p->GetPropUsage() == usage;  // prop slot 22
        FC4_RELEASE(p);
        return pass;
    }
    bool IsOccupantTypeIncluded(uint32_t type) override { return type == kOccupantTypeProp; }
    bool IsPropertyHolderIncluded(cISCPropertyHolder*) override { return true; }
};

struct PropCtx {
    cISC4City* city = nullptr;
    PropUsageFilter filter;
    // CreateProp on the recording host.
    bool recording = false;
    uint32_t created = 0;      // stand-ins handed out
    uint32_t gzClass = 0;      // the class GZCOM created, 0 = none
    uint32_t builtinKind = 0xFFFFFFFFu;
    uint32_t propType = 0;
    bool typeSet = false;
    bool boxSet = false;
    float boxMin[3] = {0, 0, 0}, boxMax[3] = {0, 0, 0};
    // The catalogue message.
    bool posted = false;
    uint32_t postedData2 = 0;
};
PropCtx g_pctx;
fc4_prop_catalogue* g_propCat = nullptr;
uint32_t g_catalogueData2 = 0;
bool g_catalogueData2Known = false;

PropCtx* PC(void* u) { return static_cast<PropCtx*>(u); }

cISC4OccupantManager* PropOm(void* u) { return PC(u)->city ? PC(u)->city->GetOccupantManager() : nullptr; }

void PropPost(void* u, uint64_t, uint32_t data2) {
    PC(u)->posted = true;
    PC(u)->postedData2 = data2;
}

uint64_t PropAcquireService(void* u, uint32_t service) {
    cISC4City* c = PC(u)->city;
    cIGZUnknown* p = nullptr;
    if (service == FC4_PROP_SERVICE_OCCUPANT_MANAGER && c != nullptr) {
        p = reinterpret_cast<cIGZUnknown*>(c->GetOccupantManager());
    } else if (service == FC4_PROP_SERVICE_LOT_MANAGER && c != nullptr) {
        p = reinterpret_cast<cIGZUnknown*>(c->GetLotManager());
    } else if (service == FC4_PROP_SERVICE_RESOURCE_MANAGER) {
        cIGZPersistResourceManagerPtr rm;
        p = static_cast<cIGZUnknown*>(static_cast<cIGZPersistResourceManager*>(rm));
    }
    if (p == nullptr) return 0;
    FC4_ADDREF(p);
    return TokenOf(p);
}

uint64_t PropCreateInstance(void* u, uint32_t cls) {
    PropCtx* c = PC(u);
    if (!c->recording) return 0;
    cIGZCOM* com = RZGetCOMDllDirector() ? GZCOM() : nullptr;
    void* p = nullptr;
    if (com == nullptr || !com->GetClassObject(cls, kIID_cISC4PropOccupant, &p) || p == nullptr) return 0;
    cIGZUnknown* q = static_cast<cIGZUnknown*>(p);
    FC4_ACQUIRED(q, "GetClassObject");
    FC4_RELEASE_CREATED(q);  // only whether GZCOM creates the class is recorded
    c->gzClass = cls;
    return (static_cast<uint64_t>(++c->created) << 1) | 1u;
}

uint64_t PropCreateBuiltin(void* u, uint32_t kind) {
    PropCtx* c = PC(u);
    if (!c->recording) return 0;
    c->builtinKind = kind;
    return (static_cast<uint64_t>(++c->created) << 1) | 1u;
}

void PropSetType(void* u, uint64_t, uint32_t id) {
    PC(u)->typeSet = true;
    PC(u)->propType = id;
}

fc4_props* PropAttach(void* u, uint64_t, uint32_t t, uint32_t g, uint32_t i, bool) {
    if (!PC(u)->recording) return nullptr;
    return LoadProps(t, g, i);
}

void PropSetBox(void* u, uint64_t, const float* mn, const float* mx) {
    PropCtx* c = PC(u);
    c->boxSet = true;
    for (int k = 0; k < 3; ++k) {
        c->boxMin[k] = mn ? mn[k] : 0.0f;
        c->boxMax[k] = mx ? mx[k] : 0.0f;
    }
}

void PropPosition(void*, uint64_t occ, float* pos3) {
    pos3[0] = pos3[1] = pos3[2] = 0.0f;
    if (occ == 0 || Odd(occ)) return;
    cS3DVector3 v;
    reinterpret_cast<cISC4Occupant*>(FromToken(occ))->GetPosition(&v);  // slot 8
    pos3[0] = v.fX;
    pos3[1] = v.fY;
    pos3[2] = v.fZ;
}

void HandOver(SC4List<cISC4Occupant*>& list, fc4_token_callback found, void* ctx) {
    size_t n = 0;
    for (cISC4Occupant*& o : list) {
        if (o == nullptr) continue;
        if (++n > kMaxList) {
            FC4_ACQUIRED(o, "prop list entry");
            FC4_RELEASE(o);
        } else {
            FC4_ACQUIRED(o, "prop list entry");  // the reference goes to the core
            if (found != nullptr) {
                found(ctx, TokenOf(o));
            } else {
                FC4_RELEASE(o);
            }
        }
        o = nullptr;
    }
}

bool PropInBox(void* u, const float* mn, const float* mx, uint32_t usage, fc4_token_callback found, void* ctx) {
    cISC4OccupantManager* om = PropOm(u);
    if (om == nullptr) return false;
    PC(u)->filter.usage = usage;
    SC4List<cISC4Occupant*> list;
    const bool ok = om->GetOccupantsByBBox(list, mn, mx, static_cast<cISC4OccupantFilter*>(&PC(u)->filter),
                                           0xFFFFFFFFu);  // slot 36
    HandOver(list, found, ctx);
    return ok;
}

bool PropInCells(void* u, const fc4_cell_rect* r, uint32_t usage, fc4_token_callback found, void* ctx) {
    cISC4OccupantManager* om = PropOm(u);
    if (om == nullptr || r == nullptr) return false;
    PC(u)->filter.usage = usage;
    const SC4Point<int> mn(r->x1, r->z1);
    const SC4Point<int> mx(r->x2, r->z2);
    SC4List<cISC4Occupant*> list;
    const bool ok = om->GetOccupantsByStandardCityCells(
        list, &mn, &mx, static_cast<cISC4OccupantFilter*>(&PC(u)->filter), 0xFFFFFFFFu);  // slot 40
    HandOver(list, found, ctx);
    return ok;
}

uint64_t PropBuildingAt(void* u, int32_t x, int32_t z) {
    cISC4OccupantManager* om = PropOm(u);
    if (om == nullptr) return 0;
    cISC4Occupant* o = nullptr;
    om->GetFirstOccupantByStandardCityCell(o, x, z, kOccupantTypeBuilding);  // slot 33
    if (o == nullptr) return 0;
    FC4_ACQUIRED(o, "GetFirstOccupantByStandardCityCell");
    return TokenOf(o);
}

uint64_t PropLotAt(void* u, int32_t x, int32_t z) {
    cISC4City* c = PC(u)->city;
    cISC4LotManager* lm = c ? c->GetLotManager() : nullptr;
    if (lm == nullptr) return 0;
    return TokenOf(AsGame(lm)->GetLot(x, z, false));  // slot 6, no reference
}

void PropLotRect(void*, uint64_t lot, fc4_cell_rect* out) {
    *out = fc4_cell_rect{};
    if (lot == 0) return;
    SC4Rect<int32_t> r;
    AsGame(reinterpret_cast<cISC4Lot*>(FromToken(lot)))->GetBoundingRect(r);  // slot 14
    out->x1 = r.topLeftX;
    out->z1 = r.topLeftY;
    out->x2 = r.bottomRightX;
    out->z2 = r.bottomRightY;
}

void PropCellToWorld(void* u, int32_t x, int32_t z, float* wx, float* wz) {
    float fx = 0.0f, fz = 0.0f;
    if (cISC4City* c = PC(u)->city) c->CellCornerToPosition(x, z, fx, fz);  // slot 96
    *wx = fx;
    *wz = fz;
}

void PropWorldToCell(void* u, float x, float z, int32_t* cx, int32_t* cz) {
    int ix = 0, iz = 0;
    if (cISC4City* c = PC(u)->city) c->PositionToCell(x, z, ix, iz);  // slot 95
    *cx = ix;
    *cz = iz;
}

fc4_prop_manager_host PropHost() {
    fc4_prop_manager_host h{};
    h.struct_size = sizeof(h);
    h.user = &g_pctx;
    h.for_each_exemplar_key = &HostForEachExemplarKey;
    h.load_exemplar = &HostLoadExemplar;
    h.post_exemplar_data_updated = &PropPost;
    h.acquire_service = &PropAcquireService;
    h.add_ref = &HostAddRef;
    h.release = &HostRelease;
    h.create_prop_instance = &PropCreateInstance;
    h.create_builtin_prop = &PropCreateBuiltin;
    h.set_prop_type = &PropSetType;
    h.attach_exemplar = &PropAttach;
    h.set_bounding_box = &PropSetBox;
    h.position = &PropPosition;
    h.occupants_in_box = &PropInBox;
    h.occupants_in_cell_rect = &PropInCells;
    h.building_at_cell = &PropBuildingAt;
    h.lot_at_cell = &PropLotAt;
    h.lot_cell_rect = &PropLotRect;
    h.cell_to_world = &PropCellToWorld;
    h.world_to_cell = &PropWorldToCell;
    h.resolve_ref = &HostResolveRef;
    // Everything that changes the city (commands, subscriptions, adding and
    // removing props, the building-prop property, text decals) stays NULL.
    return h;
}

using GameU32s = GameVector<uint32_t>;
SC4Vector<uint32_t>& AsSdk(GameU32s& v) { return reinterpret_cast<SC4Vector<uint32_t>&>(v); }

std::vector<uint32_t> ToStd(GameU32s& v) {
    std::vector<uint32_t> out;
    for (const uint32_t* p = v.begin(); p != v.end() && out.size() < kMaxCatalogue; ++p) out.push_back(*p);
    return out;
}

std::vector<uint32_t> OursU32s(size_t (*fn)(const fc4_prop_manager*, uint32_t*, size_t), const fc4_prop_manager* m) {
    std::vector<uint32_t> v(fn(m, nullptr, 0));
    if (!v.empty()) v.resize(fn(m, v.data(), v.size()));
    return v;
}

// Catalogue: slots 5-9, 14 and data2.
intptr_t PropCatalogue(cISC4PropManager* pm, fc4_prop_manager* s, std::string& body) {
    GameU32s ot, of;
    pm->GetAllPropTypes(AsSdk(ot));        // slot 5
    pm->GetAllPropFamilyTypes(AsSdk(of));  // slot 6
    const std::vector<uint32_t> origTypes = ToStd(ot), origFams = ToStd(of);
    const std::vector<uint32_t> ourTypes = OursU32s(&fc4_prop_manager_all_prop_types, s);
    const std::vector<uint32_t> ourFams = OursU32s(&fc4_prop_manager_all_prop_family_types, s);
    intptr_t m = 0;
    FirstList first;
    size_t typeOrder = SIZE_MAX, famOrder = SIZE_MAX;
    for (size_t i = 0; i < std::max(origTypes.size(), ourTypes.size()); ++i) {
        if (i >= origTypes.size() || i >= ourTypes.size() || origTypes[i] != ourTypes[i]) {
            typeOrder = i;
            break;
        }
    }
    for (size_t i = 0; i < std::max(origFams.size(), ourFams.size()); ++i) {
        if (i >= origFams.size() || i >= ourFams.size() || origFams[i] != ourFams[i]) {
            famOrder = i;
            break;
        }
    }
    if (typeOrder != SIZE_MAX) {
        ++m;
        first.Add("types@%u:%08X/%08X", static_cast<unsigned>(typeOrder),
                  typeOrder < origTypes.size() ? origTypes[typeOrder] : 0,
                  typeOrder < ourTypes.size() ? ourTypes[typeOrder] : 0);
    }
    if (famOrder != SIZE_MAX) {
        ++m;
        first.Add("families@%u:%08X/%08X", static_cast<unsigned>(famOrder),
                  famOrder < origFams.size() ? origFams[famOrder] : 0, famOrder < ourFams.size() ? ourFams[famOrder] : 0);
    }
    // Per type: key (7), validity (8), profile (14).
    uint32_t keyDiff = 0, profDiff = 0, padOnly = 0, validDiff = 0;
    for (uint32_t id : origTypes) {
        cGZPersistResourceKey k;
        const bool ok = pm->GetPropKeyFromType(id, k);
        fc4_res_key rk{};
        const bool ours = fc4_prop_manager_prop_key_from_type(s, id, &rk);
        if (ok != ours || (ok && (k.type != rk.type_id || k.group != rk.group || k.instance != rk.instance))) {
            ++keyDiff;
            first.Add("key %08X", id);
        }
        if (pm->IsPropTypeValid(id) != fc4_prop_manager_is_prop_type_valid(s, id)) ++validDiff;
        const cISC4PropOccupant::PropProfile* op = pm->GetPropProfilePtr(id);  // slot 14
        fc4_prop_profile pp{};
        const bool havePp = fc4_prop_manager_prop_profile_ptr(s, id, &pp);
        if ((op != nullptr) != havePp) {
            ++profDiff;
            first.Add("profile %08X present %d/%d", id, op != nullptr ? 1 : 0, havePp ? 1 : 0);
        } else if (op != nullptr) {
            uint8_t a[0x1C], b[0x1C];
            memcpy(a, op, 0x1C);
            memcpy(b, &pp, 0x1C);
            bool diff = false, pad = false;
            for (size_t i = 0; i < 0x1C; ++i) {
                if (a[i] == b[i]) continue;
                if (i >= 0x15 && i <= 0x17) {
                    pad = true;
                } else {
                    diff = true;
                }
            }
            if (diff) {
                ++profDiff;
                first.Add("profile %08X", id);
            } else if (pad) {
                ++padOnly;
            }
        }
    }
    if (keyDiff != 0) ++m;
    if (profDiff != 0) ++m;
    if (validDiff != 0) ++m;
    // Per family: slot 9 (the members and the count) and slot 11.
    uint32_t memberDiff = 0, members = 0;
    for (uint32_t f : origFams) {
        GameU32s v;
        const uint32_t on = pm->GetPropTypesByFamily(AsSdk(v), f);  // slot 9
        const std::vector<uint32_t> ov = ToStd(v);
        std::vector<uint32_t> uv;
        const uint32_t un = fc4_prop_manager_prop_types_by_family(s, f, nullptr, 0, &CollectU32s, &uv);
        members += static_cast<uint32_t>(ov.size());
        if (on != un || ov != uv || pm->IsPropFamilyTypeValid(f) != fc4_prop_manager_is_prop_family_type_valid(s, f)) {
            ++memberDiff;
            first.Add("family %08X n=%u/%u size=%u/%u", f, on, un, static_cast<unsigned>(ov.size()),
                      static_cast<unsigned>(uv.size()));
        }
    }
    if (memberDiff != 0) ++m;
    // data2 (PL2): the type count.
    if (g_catalogueData2Known && g_catalogueData2 != static_cast<uint32_t>(origTypes.size())) {
        ++m;
        first.Add("data2 %u vs %u types", g_catalogueData2, static_cast<unsigned>(origTypes.size()));
    }
    fc4_prop_catalogue_state cs{};
    cs.struct_size = sizeof(cs);
    fc4_prop_catalogue_get_state(g_propCat, &cs);
    AppendF(body,
            "types=%u/%u families=%u/%u (orig/ours) keysDiffering=%u validDiffering=%u profilesDiffering=%u "
            "padOnly=%u members=%u familiesDiffering=%u data2=%s%u (types %u) catalogue=%u/%u/%u first: %s",
            static_cast<unsigned>(origTypes.size()), static_cast<unsigned>(ourTypes.size()),
            static_cast<unsigned>(origFams.size()), static_cast<unsigned>(ourFams.size()), keyDiff, validDiff, profDiff,
            padOnly, members, memberDiff, g_catalogueData2Known ? "" : "unknown ", g_catalogueData2,
            static_cast<unsigned>(origTypes.size()), cs.types, cs.families, cs.profiles, first.Str());
    return m;
}

// Queries: slots 17, 18, 23, 30, 31 on sample cells.
intptr_t PropQueries(const Run& r, cISC4PropManager* pm, fc4_prop_manager* s, std::string& body) {
    const int32_t cx = static_cast<int32_t>(r.city->CellCountX());
    const int32_t cz = static_cast<int32_t>(r.city->CellCountZ());
    if (cx <= 0 || cz <= 0) {
        body += "no cells";
        return -1;
    }
    cISC4LotManager* lm = r.city->GetLotManager();
    uint32_t seed = Seed(r.julian, 0x50524F50u);
    FirstList first;
    uint32_t calls = 0, props = 0, resultDiff = 0, listDiff = 0, lots = 0;
    auto compare = [&](const char* what, int32_t x, int32_t z, bool ro, SC4List<cISC4Occupant*>& ol, bool rs,
                       std::vector<uint64_t>& us) {
        std::vector<uint64_t> os;
        TakeOccupantList(ol, os);
        ++calls;
        props += static_cast<uint32_t>(os.size());
        if (ro != rs) {
            ++resultDiff;
            first.Add("%s(%d,%d) result %d/%d", what, x, z, ro ? 1 : 0, rs ? 1 : 0);
        }
        if (os != us) {
            ++listDiff;
            first.Add("%s(%d,%d) %s", what, x, z, ListDiff(os, us).c_str());
        }
        ReleaseTokens(us);
    };
    for (long i = 0; i < g_cells; ++i) {
        const int32_t x = static_cast<int32_t>(NextRandom(seed) % static_cast<uint32_t>(cx));
        const int32_t z = static_cast<int32_t>(NextRandom(seed) % static_cast<uint32_t>(cz));
        const int32_t w = 1 + static_cast<int32_t>(NextRandom(seed) % 4u);
        const int32_t x2 = std::min(cx - 1, x + w - 1), z2 = std::min(cz - 1, z + w - 1);
        const fc4_cell_rect cr{x, z, x2, z2};
        {  // slot 18: city props by cells
            SC4List<cISC4Occupant*> ol;
            const bool ro = pm->GetCityProps(ol, SC4Rect<int32_t>(x, z, x2, z2));
            std::vector<uint64_t> us;
            const bool rs = fc4_prop_manager_get_props_in_cells(s, 1, &cr, nullptr, 0, &Collect, &us);
            compare("city", x, z, ro, ol, rs, us);
        }
        {  // slot 30: street props by cells
            SC4List<cISC4Occupant*> ol;
            const bool ro = pm->GetStreetProps(ol, SC4Rect<int32_t>(x, z, x2, z2));
            std::vector<uint64_t> us;
            const bool rs = fc4_prop_manager_get_props_in_cells(s, 4, &cr, nullptr, 0, &Collect, &us);
            compare("street", x, z, ro, ol, rs, us);
        }
        {  // slot 31: street props of one cell
            SC4List<cISC4Occupant*> ol;
            const bool ro = pm->GetStreetProps(ol, x, z);
            std::vector<uint64_t> us;
            const bool rs = fc4_prop_manager_get_props_at(s, 4, x, z, nullptr, 0, &Collect, &us);
            compare("streetAt", x, z, ro, ol, rs, us);
        }
        {  // slot 17: city props by world rectangle (the cell rectangle's corners)
            float wx1 = 0, wz1 = 0, wx2 = 0, wz2 = 0;
            r.city->CellCornerToPosition(x, z, wx1, wz1);
            r.city->CellCornerToPosition(x2 + 1, z2 + 1, wx2, wz2);
            SC4List<cISC4Occupant*> ol;
            const bool ro = pm->GetCityProps(ol, SC4Rect<float>(wx1, wz1, wx2, wz2));
            std::vector<uint64_t> us;
            const bool rs = fc4_prop_manager_get_city_props_world(s, wx1, wz1, wx2, wz2, nullptr, 0, &Collect, &us);
            compare("world", x, z, ro, ol, rs, us);
        }
        // slot 23: a cell with a lot (up to 8 draws), else this cell.
        int32_t lx = x, lz = z;
        for (int k = 0; k < 8 && lm != nullptr; ++k) {
            if (AsGame(lm)->GetLot(lx, lz, false) != nullptr) {
                ++lots;
                break;
            }
            lx = static_cast<int32_t>(NextRandom(seed) % static_cast<uint32_t>(cx));
            lz = static_cast<int32_t>(NextRandom(seed) % static_cast<uint32_t>(cz));
        }
        {
            SC4List<cISC4Occupant*> ol;
            const bool ro = pm->GetLotProps(ol, lx, lz);
            std::vector<uint64_t> us;
            const bool rs = fc4_prop_manager_get_props_at(s, 2, lx, lz, nullptr, 0, &Collect, &us);
            compare("lot", lx, lz, ro, ol, rs, us);
        }
    }
    const intptr_t m = static_cast<intptr_t>(resultDiff + listDiff);
    AppendF(body, "calls=%u cells=%ld lotCells=%u props=%u resultsDiffering=%u listsDiffering=%u first: %s", calls,
            g_cells, lots, props, resultDiff, listDiff, first.Str());
    return m;
}

// CreateProp (slot 15) on sample prop types.
intptr_t PropCreate(const Run& r, cISC4PropManager* pm, fc4_prop_manager* s, std::string& body) {
    const std::vector<uint32_t> types = OursU32s(&fc4_prop_manager_all_prop_types, s);
    if (types.empty()) {
        body += "no prop types";
        return -1;
    }
    uint32_t seed = Seed(r.julian, 0x43524541u);
    FirstList first;
    uint32_t n = 0, resultDiff = 0, classDiff = 0, typeDiff = 0, boxDiff = 0;
    for (uint32_t i = 0; i < kCreateSamples; ++i) {
        const uint32_t id = types[NextRandom(seed) % types.size()];
        // The original's prop: created and released (never initialised or inserted).
        cISC4PropOccupant* op = nullptr;
        const bool ro = pm->CreateProp(id, op) && op != nullptr;
        uint32_t oc = 0, ot = 0;
        cS3DVector3 omn, omx;
        if (op != nullptr) {
            FC4_ACQUIRED(op, "CreateProp");
            void* q = nullptr;
            if (op->QueryInterface(GZIID_cIGZSerializable, &q) && q != nullptr) {
                cIGZSerializable* ser = static_cast<cIGZSerializable*>(q);
                FC4_ACQUIRED(ser, "QueryInterface");
                oc = ser->GetGZCLSID();  // cIGZSerializable GetGZCLSID
                FC4_RELEASE(ser);
            }
            ot = op->GetPropType();                              // slot 7
            // AsOccupant is prop slot 3; [abi-ret] eax-or-void (interfaces.tsv
            // cISC4PropOccupant 3): read as the pointer the original's callers use.
            if (cISC4Occupant* occ = op->AsOccupant()) occ->GetBoundingBox(&omn, &omx);  // occupant slot 10
            FC4_RELEASE_CREATED(op);
        }
        // Ours on the recording host.
        g_pctx.recording = true;
        g_pctx.gzClass = 0;
        g_pctx.builtinKind = 0xFFFFFFFFu;
        g_pctx.typeSet = g_pctx.boxSet = false;
        const uint64_t tok = fc4_prop_manager_create_prop(s, id);
        g_pctx.recording = false;
        const bool rs = tok != 0;
        const uint32_t uc =
            g_pctx.gzClass != 0 ? g_pctx.gzClass : (g_pctx.builtinKind < 3 ? kBuiltinClass[g_pctx.builtinKind] : 0);
        ++n;
        if (ro != rs) {
            ++resultDiff;
            first.Add("%08X result %d/%d", id, ro ? 1 : 0, rs ? 1 : 0);
            continue;
        }
        if (!ro) continue;
        if (oc != uc) {
            ++classDiff;
            first.Add("%08X class %08X/%08X", id, oc, uc);
        }
        if (!g_pctx.typeSet || ot != g_pctx.propType) {
            ++typeDiff;
            first.Add("%08X type %08X/%08X", id, ot, g_pctx.propType);
        }
        const float ob[6] = {omn.fX, omn.fY, omn.fZ, omx.fX, omx.fY, omx.fZ};
        const float ub[6] = {g_pctx.boxMin[0], g_pctx.boxMin[1], g_pctx.boxMin[2],
                             g_pctx.boxMax[0], g_pctx.boxMax[1], g_pctx.boxMax[2]};
        if (!g_pctx.boxSet || memcmp(ob, ub, sizeof(ob)) != 0) {
            ++boxDiff;
            first.Add("%08X box (%g,%g,%g)-(%g,%g,%g)/(%g,%g,%g)-(%g,%g,%g)", id, ob[0], ob[1], ob[2], ob[3], ob[4],
                      ob[5], ub[0], ub[1], ub[2], ub[3], ub[4], ub[5]);
        }
    }
    const intptr_t m = static_cast<intptr_t>(resultDiff + classDiff + typeDiff + boxDiff);
    AppendF(body, "props=%u resultsDiffering=%u classesDiffering=%u typesDiffering=%u boxesDiffering=%u first: %s", n,
            resultDiff, classDiff, typeDiff, boxDiff, first.Str());
    return m;
}

void PropRunStep(void* p) {
    const Run& r = *static_cast<Run*>(p);
    cISC4PropManager* pm = r.city->GetPropManager();
    if (pm == nullptr) {
        Line("propmgr", g_propTotals, r, "record", -1, "the city has no prop manager");
        return;
    }
    if (IsOurObject(pm)) return;  // FreeCity4's (enable = propmgr): logged at game start
    g_pctx = PropCtx{};
    g_pctx.city = r.city;
    if (g_propCat == nullptr) g_propCat = fc4_prop_catalogue_create();
    bool answered = false;
    const bool wrote = g_capOrig.Capture(reinterpret_cast<cIGZUnknown*>(pm), &answered);
    if (!wrote) {
        Line("propmgr", g_propTotals, r, "record", -1,
             answered ? "the original's Write failed" : "the original does not answer cIGZSerializable");
        return;
    }
    const fc4_prop_manager_host host = PropHost();
    fc4_prop_manager* s = fc4_prop_manager_create(&host, g_propCat, 0);
    if (s == nullptr) {
        Line("propmgr", g_propTotals, r, "record", -1, "the shadow core was not created");
        return;
    }
    // Registered at once so a structured exception still tears it down.
    g_liveCore.p = s;
    g_liveCore.teardown = [](void* p) {
        auto* m = static_cast<fc4_prop_manager*>(p);
        fc4_prop_manager_shutdown(m);
        fc4_prop_manager_destroy(m);
    };
    CaptureIStream in(g_capOrig);
    const bool read = fc4_prop_manager_read(s, in.Get());
    const bool wasBuilt = [] {
        fc4_prop_catalogue_state cs{};
        cs.struct_size = sizeof(cs);
        return fc4_prop_catalogue_get_state(g_propCat, &cs) && cs.built != 0;
    }();
    const bool inited = fc4_prop_manager_init(s);
    if (!wasBuilt && g_pctx.posted) {
        g_catalogueData2 = g_pctx.postedData2;
        g_catalogueData2Known = true;
        Log("propmgr shadow: the catalogue was built (data2 %u posted)", g_catalogueData2);
    }
    {
        std::string body;
        CoreRecord rec([](const void* c, const fc4_ostream* o) {
            return fc4_prop_manager_write(static_cast<const fc4_prop_manager*>(c), o);
        }, s, kCLSID_PropManager);
        const bool ok = read && g_capOurs.Capture(&rec);
        intptr_t m = CompareRecords(ok, body);
        AppendF(body, " read=%u atEnd=%u init=%u", read ? 1u : 0u, in.atEnd() ? 1u : 0u, inited ? 1u : 0u);
        if (!in.atEnd()) ++m;
        Line("propmgr", g_propTotals, r, "record", m, body);
    }
    if (r.load) {
        std::string body;
        const intptr_t m = PropCatalogue(pm, s, body);
        Line("propmgr", g_propTotals, r, "catalogue", m, body);
    }
    {
        std::string body;
        const intptr_t m = PropQueries(r, pm, s, body);
        Line("propmgr", g_propTotals, r, "queries", m, body);
    }
    if (std::strcmp(r.event, "save") != 0) {
        std::string body;
        const intptr_t m = PropCreate(r, pm, s, body);
        Line("propmgr", g_propTotals, r, "CreateProp", m, body);
    }
    g_liveCore.Reset();
    g_pctx.city = nullptr;
}

// ================================================================================================
// The lot configuration manager shadow (Part B)
// ================================================================================================

// cISC4LotConfigurationManager as the shadow calls it (§B7): the set queries
// take the game's STLport hash_set<uint32_t> (B) and return the count in EAX;
// slot 27 takes the game's vector<uint32_t> (B, allocators.md).
class LcmView : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                            // 3
    virtual bool Shutdown() = 0;                                                        // 4
    virtual bool AddByKey(void* key) = 0;                                               // 5 (not called)
    virtual bool AddConfiguration(void* cfg, bool add) = 0;                             // 6 (not called)
    virtual bool RemoveLotConfiguration(uint32_t id) = 0;                               // 7 (not called)
    virtual cISC4LotConfiguration* GetLotConfiguration(uint32_t id) = 0;                // 8
    virtual uint32_t ByFilter(void* set, void* filter) = 0;                             // 9 (not called)
    virtual uint32_t ByFamily(void* set, uint32_t family) = 0;                          // 10 (not called)
    virtual uint32_t BySize(void* set, uint32_t x, uint32_t z) = 0;                     // 11 (not called)
    virtual void ClearSet(void* set) = 0;                                               // 12 (not called)
    virtual uint32_t IDsByFilter(GameHashSetU32<>& ids, void* filter) = 0;              // 13 (not called)
    virtual uint32_t IDsByFamily(GameHashSetU32<>& ids, uint32_t family) = 0;           // 14
    virtual uint32_t IDsBySize(GameHashSetU32<>& ids, uint32_t x, uint32_t z) = 0;      // 15
    virtual bool HasBuildingFamily(cISC4LotConfiguration* cfg, uint32_t family,
                                   cSC4LotConfigurationObject** obj) = 0;               // 16
    virtual bool HasBuildingType(cISC4LotConfiguration* cfg, uint32_t type,
                                 cSC4LotConfigurationObject** obj) = 0;                 // 17
    virtual void RegisterFilter(void* filter) = 0;                                      // 18 (not called)
    virtual void UnregisterFilter(void* filter) = 0;                                    // 19 (not called)
    virtual intptr_t GetLotConfigurationMap() = 0;                                      // 20
    virtual intptr_t GetLotConfigurationFamilyIDSet() = 0;                              // 21
    virtual bool CreateGeneric(void* cfgOut, uint32_t type) = 0;                        // 22 (not called)
    virtual uint32_t Fix(void* cfg) = 0;                                                // 23 (not called)
    virtual void MaxFootprintBuildings(void const* ids, void* rect) = 0;                // 24 (not called)
    virtual void MaxFootprintFlora(void const* ids, void* rect) = 0;                    // 25 (not called)
    virtual void MaxFootprintProps(void const* ids, void* rect) = 0;                    // 26 (not called)
    virtual void GetObjectIDArray(cSC4LotConfigurationObject* obj, GameVector<uint32_t>& ids) = 0;  // 27
};

LcmView* AsView(cISC4LotConfigurationManager* p) { return reinterpret_cast<LcmView*>(p); }

// A configuration ours loads, bound to the original's configuration of the
// same ID (the token is odd: no reference).
struct Binding {
    uint32_t id = 0;
    uint32_t type = 0, group = 0, instance = 0;
    bool loaded = false;
};
// Process lifetime: the catalogue caches configurations by these stand-in
// tokens, so they must outlive every run (cleared only at app shutdown).
std::vector<Binding> g_bindings;

struct LcmCtx {
    cISC4City* city = nullptr;
    LcmView* orig = nullptr;
    uint32_t detail = 3;
};
LcmCtx g_lctx;
fc4_lot_config_catalogue* g_lcmCat = nullptr;

LcmCtx* LC(void* u) { return static_cast<LcmCtx*>(u); }

Binding* BindingOf(uint64_t t) {
    if (!Odd(t)) return nullptr;
    const size_t i = static_cast<size_t>(t >> 1);
    return i >= 1 && i <= g_bindings.size() ? &g_bindings[i - 1] : nullptr;
}

// The original's configuration a binding stands for (borrowed), or null.
cISC4LotConfiguration* OrigCfg(void* u, uint64_t t) {
    Binding* b = BindingOf(t);
    if (b == nullptr || !b->loaded || LC(u)->orig == nullptr) return nullptr;
    return LC(u)->orig->GetLotConfiguration(b->id);  // slot 8, no reference
}

// The city detail preference (city-detail.md §2: the byte at +0xF16 of the
// preferences object; the SDK's SC4Preferences::videoPreferences.cityDetail).
uint32_t DetailPreference() {
    cISC4App* app = App();
    SC4Preferences* p = app ? app->GetPreferences() : nullptr;
    if (p == nullptr) return 3;
    const uint32_t d = p->videoPreferences.cityDetail;
    return d >= 1 && d <= 3 ? d : 3;
}

uint32_t LcmDetail(void* u) { return LC(u)->detail; }

uint64_t LcmCreateConfiguration(void*) {
    g_bindings.push_back(Binding{});
    return (static_cast<uint64_t>(g_bindings.size()) << 1) | 1u;
}

bool LcmLoadConfiguration(void* u, uint64_t cfg, uint32_t t, uint32_t g, uint32_t i, uint32_t) {
    Binding* b = BindingOf(cfg);
    if (b == nullptr) return false;
    b->id = i;  // lot-configuration.md §3.1 step 4
    b->type = t;
    b->group = g;
    b->instance = i;
    b->loaded = LC(u)->orig != nullptr && LC(u)->orig->GetLotConfiguration(i) != nullptr;
    return b->loaded;
}

uint32_t LcmConfigId(void*, uint64_t cfg) {
    Binding* b = BindingOf(cfg);
    return b ? b->id : 0;
}

bool LcmBelongs(void* u, uint64_t cfg, uint32_t f) {
    cISC4LotConfiguration* c = OrigCfg(u, cfg);
    return c != nullptr && c->BelongsToFamily(f);  // configuration slot 5
}

void LcmFamilies(void* u, uint64_t cfg, fc4_u32s_sink out, void* ctx) {
    cISC4LotConfiguration* c = OrigCfg(u, cfg);
    std::vector<uint32_t> v;
    if (c != nullptr) {
        if (SC4Vector<uint32_t>* f = c->GetFamilyIDArray()) {  // configuration slot 6
            for (uint32_t id : *f) v.push_back(id);
        }
    }
    if (out != nullptr) out(ctx, v.empty() ? nullptr : v.data(), v.size());
}

void LcmSize(void* u, uint64_t cfg, uint32_t* x, uint32_t* z) {
    uint32_t w = 0, d = 0;
    if (cISC4LotConfiguration* c = OrigCfg(u, cfg)) c->GetSize(w, d);  // configuration slot 7
    *x = w;
    *z = d;
}

void LcmObjects(void* u, uint64_t cfg, fc4_lotdev_cfg_object_callback visit, void* ctx) {
    cISC4LotConfiguration* c = OrigCfg(u, cfg);
    if (c == nullptr || visit == nullptr) return;
    SC4Vector<cSC4LotConfigurationObject>* v = c->GetLotConfigurationObjectArray();  // configuration slot 23
    if (v == nullptr) return;
    std::vector<uint32_t> ids;
    for (cSC4LotConfigurationObject* o = v->begin(); o != v->end(); ++o) {
        const fc4_lotdev_cfg_object co = LotDevObjectOf(o, ids);
        visit(ctx, &co);
    }
}

uint64_t LcmAcquireService(void* u, uint32_t service) {
    cISC4City* c = LC(u)->city;
    if (c == nullptr) return 0;
    cIGZUnknown* p = nullptr;
    if (service == FC4_LCM_SERVICE_BUILDING_DEVELOPMENT) {
        p = reinterpret_cast<cIGZUnknown*>(c->GetBuildingDevelopmentSimulator());
    } else if (service == FC4_LCM_SERVICE_FLORA) {
        p = reinterpret_cast<cIGZUnknown*>(c->GetFloraSimulator());
    } else if (service == FC4_LCM_SERVICE_PROP_MANAGER) {
        p = reinterpret_cast<cIGZUnknown*>(c->GetPropManager());
    }
    if (p == nullptr) return 0;
    FC4_ADDREF(p);
    return TokenOf(p);
}

cFC4ISC4BuildingDevelopmentSimulator* Bds(void* u) {
    cISC4City* c = LC(u)->city;
    return c ? reinterpret_cast<cFC4ISC4BuildingDevelopmentSimulator*>(c->GetBuildingDevelopmentSimulator()) : nullptr;
}

cISC4PropManager* Pm(void* u) { return LC(u)->city ? LC(u)->city->GetPropManager() : nullptr; }

cISC4FloraSimulator* Flora(void* u) { return LC(u)->city ? LC(u)->city->GetFloraSimulator() : nullptr; }

// The building development and flora lookups below read the callee's return
// value. docs/abi/interfaces.tsv gives these slots `eax-or-void` (medium
// confidence): the game does not promise a value. They are read anyway,
// exactly as the original's callers read them, so that the shadow sees the
// same answer the original would have used; a wrong answer then shows up as a
// lotcfg shadow mismatch rather than as a silent difference. Each call site
// carries [abi-ret] naming the slot and the channel.

bool LcmIsBuildingTypeValid(void* u, uint32_t id) {
    auto* b = Bds(u);
    // slot 15; [abi-ret] eax-or-void (interfaces.tsv cISC4BuildingDevelopmentSimulator 15)
    return b != nullptr && b->IsBuildingTypeValid(id);
}

bool LcmIsBuildingFamilyValid(void* u, uint32_t id) {
    auto* b = Bds(u);
    // slot 16; [abi-ret] eax-or-void (interfaces.tsv cISC4BuildingDevelopmentSimulator 16)
    return b != nullptr && b->IsBuildingFamilyTypeValid(id);
}

// A vector the callee assigns or leaves alone: a sentinel element tells the
// two apart (a family whose only member is 0xFFFFFFFF reads as untouched).
constexpr uint32_t kSentinel = 0xFFFFFFFFu;

uint32_t LcmBuildingTypesByFamily(void* u, uint32_t id, fc4_u32s_sink out, void* ctx) {
    auto* b = Bds(u);
    if (b == nullptr) return 0;
    GameU32s v;
    v.push_back(kSentinel);
    // slot 18; [abi-ret] eax-or-void (interfaces.tsv cISC4BuildingDevelopmentSimulator 18): the count
    const uint32_t n = b->GetBuildingTypesByFamily(v, id);
    const std::vector<uint32_t> s = ToStd(v);
    if (!(s.size() == 1 && s[0] == kSentinel) && out != nullptr) out(ctx, s.empty() ? nullptr : s.data(), s.size());
    return n;
}

bool LcmIsBuildingInFamily(void* u, uint32_t type, uint32_t family) {
    auto* b = Bds(u);
    // slot 20; [abi-ret] eax-or-void (interfaces.tsv cISC4BuildingDevelopmentSimulator 20)
    return b != nullptr && b->IsBuildingInFamily(type, family);
}

bool LcmIsFloraTypeValid(void* u, uint32_t id) {
    cISC4FloraSimulator* f = Flora(u);
    // slot 11; [abi-ret] eax-or-void (interfaces.tsv cISC4FloraSimulator 11)
    return f != nullptr && f->IsFloraTypeValid(id);
}

// Slot 14 into a set of hint 100 (§B5.6), its types in bucket order. The
// intptr_t& argument mirrors cISC4FloraSimulator.h's own declaration.
uint32_t LcmFloraTypesIntoSet(void* u, uint32_t id, fc4_u32s_sink out, void* ctx) {
    cISC4FloraSimulator* f = Flora(u);
    if (f == nullptr) return 0;
    GameHashSetU32<> set(100);
    // slot 14; [abi-ret] eax-or-void (interfaces.tsv cISC4FloraSimulator 14): the count
    const uint32_t n = f->GetFloraTypesByFamily(reinterpret_cast<intptr_t&>(set), id);
    std::vector<uint32_t> v;
    for (size_t b = 0; b < set.bucket_count(); ++b) {
        for (auto* p = set.bucket(b); p != nullptr && v.size() < kMaxList; p = p->next) v.push_back(p->value);
    }
    if (out != nullptr) out(ctx, v.empty() ? nullptr : v.data(), v.size());
    return n;
}

bool LcmIsPropTypeValid(void* u, uint32_t id) {
    cISC4PropManager* p = Pm(u);
    return p != nullptr && p->IsPropTypeValid(id);  // slot 8
}

uint32_t LcmPropTypesByFamily(void* u, uint32_t id, fc4_u32s_sink out, void* ctx) {
    cISC4PropManager* p = Pm(u);
    if (p == nullptr) return 0;
    GameU32s v;
    v.push_back(kSentinel);
    const uint32_t n = p->GetPropTypesByFamily(AsSdk(v), id);  // slot 9
    const std::vector<uint32_t> s = ToStd(v);
    if (!(s.size() == 1 && s[0] == kSentinel) && out != nullptr) out(ctx, s.empty() ? nullptr : s.data(), s.size());
    return n;
}

bool LcmIsPropFamilyValid(void* u, uint32_t id) {
    cISC4PropManager* p = Pm(u);
    return p != nullptr && p->IsPropFamilyTypeValid(id);  // slot 11
}

fc4_lot_config_host LcmHost() {
    fc4_lot_config_host h{};
    h.struct_size = sizeof(h);
    h.user = &g_lctx;
    h.detail_level = &LcmDetail;
    h.for_each_exemplar_key = &HostForEachExemplarKey;
    h.create_configuration = &LcmCreateConfiguration;
    h.load_configuration = &LcmLoadConfiguration;
    h.config_id = &LcmConfigId;
    h.config_belongs_to_family = &LcmBelongs;
    h.config_families = &LcmFamilies;
    h.config_size = &LcmSize;
    h.config_objects = &LcmObjects;
    h.acquire_service = &LcmAcquireService;
    h.add_ref = &HostAddRef;
    h.release = &HostRelease;
    h.is_building_type_valid = &LcmIsBuildingTypeValid;
    h.is_building_family_type_valid = &LcmIsBuildingFamilyValid;
    h.building_types_by_family = &LcmBuildingTypesByFamily;
    h.is_building_in_family = &LcmIsBuildingInFamily;
    h.is_flora_type_valid = &LcmIsFloraTypeValid;
    h.flora_types_into_set = &LcmFloraTypesIntoSet;
    h.is_prop_type_valid = &LcmIsPropTypeValid;
    h.prop_types_by_family = &LcmPropTypesByFamily;
    h.is_prop_family_type_valid = &LcmIsPropFamilyValid;
    h.resolve_ref = &HostResolveRef;
    // The writers (set_config_*), the shutdown callback, subscriptions,
    // filters, the building information and the footprints stay NULL.
    return h;
}

// The original's configuration map (slot 20): STLport's hash_map<uint32_t,
// cISC4LotConfiguration*>, the 0x14-byte table of allocators.md hash_set,
// nodes {next, key, value} [inferred: the pair is 4-aligned; layouts.md
// documents only the 8-aligned building profile node]. Each entry is checked
// against slot 8; false (nothing kept) when the walk does not hold together.
struct HashTable {
    uint32_t functors;
    void** begin;
    void** end;
    void** capacityEnd;
    uint32_t count;
};

bool WalkMap(LcmView* orig, std::vector<uint32_t>& ids, const char** why) {
    ids.clear();
    const auto* t = reinterpret_cast<const HashTable*>(orig->GetLotConfigurationMap());
    if (t == nullptr || t->begin == nullptr || t->end < t->begin) {
        *why = "the map's bucket vector is not readable";
        return false;
    }
    const size_t buckets = static_cast<size_t>(t->end - t->begin);
    for (size_t b = 0; b < buckets; ++b) {
        for (auto* n = static_cast<uint32_t*>(t->begin[b]); n != nullptr; n = reinterpret_cast<uint32_t*>(n[0])) {
            if (ids.size() > t->count || ids.size() >= kMaxCatalogue) {
                *why = "the map's chains are longer than its count";
                return false;
            }
            const uint32_t key = n[1];
            if (reinterpret_cast<void*>(static_cast<uintptr_t>(n[2])) != orig->GetLotConfiguration(key)) {
                *why = "a map node's value is not slot 8's configuration (node layout)";
                return false;
            }
            ids.push_back(key);
        }
    }
    if (ids.size() != t->count) {
        *why = "the map's node count differs from its count";
        return false;
    }
    return true;
}

bool WalkSet(intptr_t addr, std::vector<uint32_t>& out) {
    out.clear();
    const auto* t = reinterpret_cast<const HashTable*>(addr);
    if (t == nullptr || t->begin == nullptr || t->end < t->begin) return false;
    const size_t buckets = static_cast<size_t>(t->end - t->begin);
    for (size_t b = 0; b < buckets; ++b) {
        for (auto* n = static_cast<uint32_t*>(t->begin[b]); n != nullptr; n = reinterpret_cast<uint32_t*>(n[0])) {
            if (out.size() > t->count || out.size() >= kMaxCatalogue) return false;
            out.push_back(n[1]);
        }
    }
    return out.size() == t->count;
}

std::vector<uint32_t> SetValues(const GameHashSetU32<>& s) {
    std::vector<uint32_t> v;
    for (size_t b = 0; b < s.bucket_count(); ++b) {
        for (auto* p = s.bucket(b); p != nullptr && v.size() < kMaxList; p = p->next) v.push_back(p->value);
    }
    std::sort(v.begin(), v.end());
    return v;
}

std::vector<uint32_t> OursIds(fc4_lot_config_manager* s, uint32_t which, uint64_t a, uint32_t b) {
    std::vector<uint32_t> v(fc4_lot_config_ids(s, which, a, b, nullptr, 0));
    if (!v.empty()) v.resize(fc4_lot_config_ids(s, which, a, b, v.data(), v.size()));
    return v;
}

bool SameObject(const fc4_lotdev_cfg_object& a, const std::vector<uint32_t>& aIds, const fc4_lotdev_cfg_object& b,
                const std::vector<uint32_t>& bIds) {
    return a.object_type == b.object_type && a.flags == b.flags && a.orientation == b.orientation &&
           a.byte3 == b.byte3 && memcmp(a.offset, b.offset, sizeof(a.offset)) == 0 &&
           memcmp(a.footprint_max, b.footprint_max, sizeof(a.footprint_max)) == 0 &&
           memcmp(a.footprint_min, b.footprint_min, sizeof(a.footprint_min)) == 0 && a.foundation == b.foundation &&
           a.tag == b.tag && aIds == bIds;
}

struct ObjectCopy {
    fc4_lotdev_cfg_object o;
    std::vector<uint32_t> ids;
};

void CopyObject(void* ctx, const fc4_lotdev_cfg_object* o) {
    auto* v = static_cast<std::vector<ObjectCopy>*>(ctx);
    ObjectCopy c;
    c.o = *o;
    if (o->ids != nullptr) c.ids.assign(o->ids, o->ids + o->n_ids);
    c.o.ids = nullptr;
    v->push_back(std::move(c));
}

// Arrays (load only): every configuration of the original's map.
intptr_t LcmArrays(const std::vector<uint32_t>& ids, std::string& body) {
    FirstList first;
    uint32_t cfgs = 0, objects = 0, diffCfgs = 0, noKey = 0, foundations = 0;
    for (uint32_t id : ids) {
        cISC4LotConfiguration* c = g_lctx.orig->GetLotConfiguration(id);
        const Binding* b = nullptr;
        for (const Binding& x : g_bindings) {
            if (x.loaded && x.id == id) b = &x;  // the last load of the ID wins (§B3.4)
        }
        if (c == nullptr) continue;
        ++cfgs;
        if (b == nullptr) {
            ++noKey;
            continue;
        }
        std::vector<ObjectCopy> orig;
        {
            SC4Vector<cSC4LotConfigurationObject>* v = c->GetLotConfigurationObjectArray();
            std::vector<uint32_t> tmp;
            if (v != nullptr) {
                for (cSC4LotConfigurationObject* o = v->begin(); o != v->end(); ++o) {
                    const fc4_lotdev_cfg_object co = LotDevObjectOf(o, tmp);
                    CopyObject(&orig, &co);
                }
            }
        }
        if (!orig.empty() && orig.back().o.object_type == kObjectTypeFoundation) {
            orig.pop_back();  // lot-configuration.md §3.7: appended by Load, not by the reader
            ++foundations;
        }
        std::vector<ObjectCopy> ours;
        fc4_props* ex = LoadProps(b->type, b->group, b->instance);
        if (ex != nullptr) {
            // The process-wide serial, not a fresh one per call (§3.5).
            fc4_lot_config_object_array(ex, g_lctx.detail, NextLotSerial(), &CopyObject, &ours);
            fc4_props_destroy(ex);
        }
        objects += static_cast<uint32_t>(orig.size());
        bool diff = orig.size() != ours.size();
        size_t at = 0;
        for (; !diff && at < orig.size(); ++at) {
            if (!SameObject(orig[at].o, orig[at].ids, ours[at].o, ours[at].ids)) diff = true;
        }
        if (diff) {
            ++diffCfgs;
            first.Add("%08X n=%u/%u @%u", id, static_cast<unsigned>(orig.size()), static_cast<unsigned>(ours.size()),
                      static_cast<unsigned>(at == 0 ? 0 : at - 1));
        }
    }
    AppendF(body, "configurations=%u objects=%u detail=%u foundations=%u unbound=%u configurationsDiffering=%u first: %s",
            cfgs, objects, g_lctx.detail, foundations, noKey, diffCfgs, first.Str());
    return static_cast<intptr_t>(diffCfgs + noKey);
}

intptr_t LcmQueries(const Run& r, fc4_lot_config_manager* s, const std::vector<uint32_t>& ids,
                    const std::vector<uint32_t>& families, std::string& body) {
    LcmView* o = g_lctx.orig;
    uint32_t seed = Seed(r.julian, 0x4C434D51u);
    FirstList first;
    uint32_t famDiff = 0, sizeDiff = 0, hasDiff = 0, idArrDiff = 0, idArrays = 0, hasTests = 0;
    for (uint32_t i = 0; i < kLcmFamilySamples && !families.empty(); ++i) {
        const uint32_t f = families[NextRandom(seed) % families.size()];
        GameHashSetU32<> set(1);
        const uint32_t on = o->IDsByFamily(set, f);  // slot 14
        const std::vector<uint32_t> os = SetValues(set);
        std::vector<uint32_t> us = OursIds(s, 1, f, 0);
        const uint32_t un = static_cast<uint32_t>(us.size());
        std::sort(us.begin(), us.end());
        if (on != un || os != us) {
            ++famDiff;
            first.Add("family %08X n=%u/%u", f, on, un);
        }
    }
    for (uint32_t i = 0; i < kLcmSizeSamples; ++i) {
        const uint32_t x = 1 + NextRandom(seed) % 6u, z = 1 + NextRandom(seed) % 6u;
        GameHashSetU32<> set(1);
        const uint32_t on = o->IDsBySize(set, x, z);  // slot 15
        const std::vector<uint32_t> os = SetValues(set);
        std::vector<uint32_t> us = OursIds(s, 2, x, z);
        const uint32_t un = static_cast<uint32_t>(us.size());
        std::sort(us.begin(), us.end());
        if (on != un || os != us) {
            ++sizeDiff;
            first.Add("size %ux%u n=%u/%u", x, z, on, un);
        }
    }
    for (uint32_t i = 0; i < kLcmConfigSamples && !ids.empty(); ++i) {
        const uint32_t id = ids[NextRandom(seed) % ids.size()];
        cISC4LotConfiguration* c = o->GetLotConfiguration(id);
        const uint64_t ours = fc4_lot_config_configuration(s, id);
        if (c == nullptr || ours == 0) {
            ++hasDiff;
            first.Add("cfg %08X present %d/%d", id, c ? 1 : 0, ours ? 1 : 0);
            continue;
        }
        SC4Vector<cSC4LotConfigurationObject>* v = c->GetLotConfigurationObjectArray();
        if (v == nullptr) continue;
        cSC4LotConfigurationObject* base = v->begin();
        const size_t n = static_cast<size_t>(v->end() - v->begin());
        // Slots 16 / 17 with the building record's first ID, and a type of a
        // family when it is one.
        for (size_t k = 0; k < n; ++k) {
            std::vector<uint32_t> tmp;
            const fc4_lotdev_cfg_object co = LotDevObjectOf(base + k, tmp);
            if (co.object_type == 0 && !tmp.empty()) {
                for (int byType = 0; byType < 2; ++byType) {
                    const uint32_t q = tmp[0];
                    cSC4LotConfigurationObject* hit = nullptr;
                    const bool ro = byType ? o->HasBuildingType(c, q, &hit) : o->HasBuildingFamily(c, q, &hit);
                    uint32_t idx = 0xFFFFFFFFu;
                    const bool rs = fc4_lot_config_has_building(s, ours, q, byType != 0, &idx);
                    const uint32_t oidx = hit ? static_cast<uint32_t>(hit - base) : 0xFFFFFFFFu;
                    ++hasTests;
                    if (ro != rs || (ro && oidx != idx)) {
                        ++hasDiff;
                        first.Add("has%s %08X/%08X %d@%d/%d@%d", byType ? "Type" : "Family", id, q, ro ? 1 : 0,
                                  static_cast<int>(oidx), rs ? 1 : 0, static_cast<int>(idx));
                    }
                }
                break;
            }
        }
        // Slot 27 on up to four objects of types 0, 1 and 4.
        uint32_t taken = 0;
        for (size_t k = 0; k < n && taken < 4; ++k) {
            std::vector<uint32_t> tmp;
            const fc4_lotdev_cfg_object co = LotDevObjectOf(base + k, tmp);
            if (co.object_type != 0 && co.object_type != 1 && co.object_type != 4) continue;
            ++taken;
            GameU32s ov;
            o->GetObjectIDArray(base + k, ov);  // slot 27
            const std::vector<uint32_t> os = ToStd(ov);
            std::vector<uint32_t> us(fc4_lot_config_object_id_array(s, &co, nullptr, 0));
            if (!us.empty()) us.resize(fc4_lot_config_object_id_array(s, &co, us.data(), us.size()));
            ++idArrays;
            if (os != us) {
                ++idArrDiff;
                first.Add("ids %08X#%u type %u n=%u/%u", id, static_cast<unsigned>(k), co.object_type,
                          static_cast<unsigned>(os.size()), static_cast<unsigned>(us.size()));
            }
        }
    }
    AppendF(body,
            "families=%u/%u differing sizes=%u/%u differing hasBuilding=%u/%u differing objectIDArrays=%u/%u "
            "differing first: %s",
            famDiff, kLcmFamilySamples, sizeDiff, kLcmSizeSamples, hasDiff, hasTests, idArrDiff, idArrays, first.Str());
    return static_cast<intptr_t>(famDiff + sizeDiff + hasDiff + idArrDiff);
}

void LcmRunStep(void* p) {
    const Run& r = *static_cast<Run*>(p);
    cISC4LotConfigurationManager* raw = r.city->GetLotConfigurationManager();
    if (raw == nullptr) {
        Line("lotcfg", g_lcmTotals, r, "record", -1, "the city has no lot configuration manager");
        return;
    }
    if (IsOurObject(raw)) return;  // FreeCity4's (enable = lotcfg): logged at game start
    g_lctx = LcmCtx{};
    g_lctx.city = r.city;
    g_lctx.orig = AsView(raw);
    g_lctx.detail = DetailPreference();
    // g_bindings is NOT cleared here: the process catalogue caches
    // configurations by the odd stand-in tokens LcmCreateConfiguration handed
    // out, so the bindings must live as long as the catalogue does. Clearing
    // them made every later run's family and HasBuilding queries answer 0
    // (glue15-sh-6) because the cached tokens no longer resolved.
    if (g_lcmCat == nullptr) g_lcmCat = fc4_lot_config_catalogue_create();
    bool answered = false;
    if (!g_capOrig.Capture(reinterpret_cast<cIGZUnknown*>(raw), &answered)) {
        Line("lotcfg", g_lcmTotals, r, "record", -1,
             answered ? "the original's Write failed" : "the original does not answer cIGZSerializable");
        return;
    }
    const fc4_lot_config_host host = LcmHost();
    fc4_lot_config_manager* s = fc4_lot_config_create(&host, g_lcmCat, 0);
    if (s == nullptr) {
        Line("lotcfg", g_lcmTotals, r, "record", -1, "the shadow core was not created");
        return;
    }
    // Registered at once so a structured exception still tears it down.
    // No fc4_lot_config_unload here: the shadow's managers share one process
    // catalogue, and unload empties it (the family set stays, quirk 10), so the
    // next run would rebuild the map and its bucket order would differ from the
    // original's. The process unload callback runs at app shutdown instead.
    g_liveCore.p = s;
    g_liveCore.teardown = [](void* p) {
        auto* m = static_cast<fc4_lot_config_manager*>(p);
        fc4_lot_config_shutdown(m);
        fc4_lot_config_destroy(m);
    };
    CaptureIStream in(g_capOrig);
    const bool read = fc4_lot_config_read(s, in.Get());
    const bool inited = fc4_lot_config_init(s);
    {
        std::string body;
        CoreRecord rec([](const void* c, const fc4_ostream* o) {
            return fc4_lot_config_write(static_cast<const fc4_lot_config_manager*>(c), o);
        }, s, kCLSID_LotConfigurationManager);
        const bool ok = read && g_capOurs.Capture(&rec);
        intptr_t m = CompareRecords(ok, body);
        AppendF(body, " read=%u atEnd=%u init=%u", read ? 1u : 0u, in.atEnd() ? 1u : 0u, inited ? 1u : 0u);
        if (!in.atEnd()) ++m;
        Line("lotcfg", g_lcmTotals, r, "record", m, body);
    }
    std::vector<uint32_t> origIds, ourIds, origFams, ourFams;
    const char* why = nullptr;
    const bool mapOk = WalkMap(g_lctx.orig, origIds, &why);
    {
        ourIds.resize(fc4_lot_config_configuration_map(s, nullptr, nullptr, 0));
        if (!ourIds.empty()) ourIds.resize(fc4_lot_config_configuration_map(s, ourIds.data(), nullptr, ourIds.size()));
        std::string body;
        if (!mapOk) {
            body = why;
            Line("lotcfg", g_lcmTotals, r, "map", -1, body);
        } else {
            size_t at = SIZE_MAX;
            for (size_t i = 0; i < std::max(origIds.size(), ourIds.size()); ++i) {
                if (i >= origIds.size() || i >= ourIds.size() || origIds[i] != ourIds[i]) {
                    at = i;
                    break;
                }
            }
            fc4_lot_config_catalogue_state cs{};
            cs.struct_size = sizeof(cs);
            fc4_lot_config_catalogue_get_state(g_lcmCat, &cs);
            AppendF(body, "configurations=%u/%u (orig/ours) detail=%u (catalogue built for %u) bindings=%u",
                    static_cast<unsigned>(origIds.size()), static_cast<unsigned>(ourIds.size()), g_lctx.detail,
                    cs.detail, static_cast<unsigned>(g_bindings.size()));
            if (at != SIZE_MAX) {
                AppendF(body, " firstOrderDiff=@%u %08X/%08X", static_cast<unsigned>(at),
                        at < origIds.size() ? origIds[at] : 0, at < ourIds.size() ? ourIds[at] : 0);
            }
            Line("lotcfg", g_lcmTotals, r, "map", at == SIZE_MAX ? 0 : 1, body);
        }
    }
    {
        std::string body;
        const bool setOk = WalkSet(g_lctx.orig->GetLotConfigurationFamilyIDSet(), origFams);
        ourFams.resize(fc4_lot_config_family_id_set(s, nullptr, 0));
        if (!ourFams.empty()) ourFams.resize(fc4_lot_config_family_id_set(s, ourFams.data(), ourFams.size()));
        if (!setOk) {
            Line("lotcfg", g_lcmTotals, r, "families", -1, "the family set is not readable");
        } else {
            const bool same = origFams == ourFams;
            AppendF(body, "families=%u/%u (orig/ours) order %s", static_cast<unsigned>(origFams.size()),
                    static_cast<unsigned>(ourFams.size()), same ? "equal" : "differs");
            Line("lotcfg", g_lcmTotals, r, "families", same ? 0 : 1, body);
        }
    }
    if (r.load && mapOk) {
        std::string body;
        const intptr_t m = LcmArrays(origIds, body);
        Line("lotcfg", g_lcmTotals, r, "arrays", m, body);
    }
    if (mapOk) {
        std::string body;
        const intptr_t m = LcmQueries(r, s, origIds, origFams, body);
        Line("lotcfg", g_lcmTotals, r, "queries", m, body);
    }
    g_liveCore.Reset();
    g_lctx.city = nullptr;
    g_lctx.orig = nullptr;
}

// ================================================================================================
// The tutorial shadow
// ================================================================================================

void TutRunStep(void* p) {
    const Run& r = *static_cast<Run*>(p);
    cISC4TutorialSystem* t = r.city->GetTutorialSystem();
    if (t == nullptr) {
        Line("tutorial", g_tutTotals, r, "record", -1, "the city has no tutorial system");
        return;
    }
    // enable = tutorial (glue-tutorial4): the shadow runs over the live class
    // too, as its game-start line says: a self-check of FreeCity4's record and
    // slots 8 / 11 / 12 against a private core (this return skipped it).
    bool answered = false;
    if (!g_capOrig.Capture(reinterpret_cast<cIGZUnknown*>(t), &answered)) {
        Line("tutorial", g_tutTotals, r, "record", -1,
             answered ? "the original's Write failed" : "the original does not answer cIGZSerializable");
        return;
    }
    fc4_tutorial_host host{};
    host.struct_size = sizeof(host);
    host.add_ref = &HostAddRef;
    host.release = &HostRelease;
    fc4_tutorial* s = fc4_tutorial_create(&host, 0);
    if (s == nullptr) {
        Line("tutorial", g_tutTotals, r, "record", -1, "the shadow core was not created");
        return;
    }
    // Registered at once so a structured exception still tears it down.
    g_liveCore.p = s;
    g_liveCore.teardown = [](void* p) {
        auto* t = static_cast<fc4_tutorial*>(p);
        fc4_tutorial_shutdown(t);
        fc4_tutorial_destroy(t);
    };
    CaptureIStream in(g_capOrig);
    const bool read = fc4_tutorial_read(s, in.Get());
    {
        std::string body;
        CoreRecord rec([](const void* c, const fc4_ostream* o) {
            return fc4_tutorial_write(static_cast<const fc4_tutorial*>(c), o);
        }, s, kCLSID_TutorialSystem);
        const bool ok = read && g_capOurs.Capture(&rec);
        intptr_t m = CompareRecords(ok, body);
        AppendF(body, " read=%u atEnd=%u", read ? 1u : 0u, in.atEnd() ? 1u : 0u);
        if (!in.atEnd()) ++m;
        Line("tutorial", g_tutTotals, r, "record", m, body);
    }
    {
        cFC4ISC4TutorialSystem* g = AsGame(t);
        const uint32_t oIs = g->IsTutorial() ? 1u : 0u;             // slot 8
        const int32_t oTask = g->GetTutorialTaskNumber();           // slot 11
        const uint32_t oGuid = static_cast<uint32_t>(g->GetTutorialGUID());  // slot 12
        const uint32_t uIs = fc4_tutorial_is_tutorial(s);
        const int32_t uTask = fc4_tutorial_task_number(s);
        const uint32_t uGuid = fc4_tutorial_guid(s);
        const intptr_t m = (oIs != uIs ? 1 : 0) + (oTask != uTask ? 1 : 0) + (oGuid != uGuid ? 1 : 0);
        std::string body;
        AppendF(body, "isTutorial=%u/%u task=%d/%d guid=%08X/%08X (orig/ours)", oIs, uIs, oTask, uTask, oGuid, uGuid);
        Line("tutorial", g_tutTotals, r, "state", m, body);
    }
    g_liveCore.Reset();
}

// ---- runs ---------------------------------------------------------------------------------------------------

void RunAll(cISC4City* city, int32_t julian, const char* event) {
    if (city == nullptr || !(g_prop || g_lcm || g_tut)) return;
    Run r;
    r.city = city;
    r.julian = julian;
    r.event = event;
    r.load = std::strcmp(event, "load") == 0;
    if (julian > 0) {
        fc4_date d{};
        fc4_date_from_julian(julian, &d);
        _snprintf_s(r.date, sizeof(r.date), _TRUNCATE, "%04d-%02d-%02d", d.year, d.month, d.day_of_month);
    }
    if (g_prop && !SehGuard("propmgr shadow", &PropRunStep, &r)) {
        Line("propmgr", g_propTotals, r, "run", -1, "structured exception; comparison abandoned");
    }
    g_liveCore.Reset();  // also when the step was abandoned mid-way
    if (g_lcm && !SehGuard("lotcfg shadow", &LcmRunStep, &r)) {
        Line("lotcfg", g_lcmTotals, r, "run", -1, "structured exception; comparison abandoned");
    }
    g_liveCore.Reset();
    if (g_tut && !SehGuard("tutorial shadow", &TutRunStep, &r)) {
        Line("tutorial", g_tutTotals, r, "run", -1, "structured exception; comparison abandoned");
    }
    g_liveCore.Reset();
}

int32_t CityJulian(cISC4City* city) {
    cFC4ISC4Simulator* sim = city ? AsGame(city->GetSimulator()) : nullptr;
    // cISC4Simulator slot 8 GetSimDateNumber (interfaces.tsv: forward, medium).
    return sim != nullptr ? sim->GetSimDateNumber() : 0;
}

void LogTotals(const char* name, bool on, Totals& t) {
    if (!on || t.lines == 0) return;
    Log("%s shadow totals: lines=%u exact=%u mismatched=%u incomplete=%u", name, t.lines, t.exact, t.mismatched,
        t.incomplete);
    t = Totals{};
}

}  // namespace

void PropLotShadowStartup(bool propmgrReplaced, bool lotcfgReplaced, bool tutorialReplaced) {
    g_prop = SettingInt("propmgr_shadow", "FC4_PROPMGR_SHADOW", 0) != 0;
    g_lcm = SettingInt("lotcfg_shadow", "FC4_LOTCFG_SHADOW", 0) != 0;
    g_tut = SettingInt("tutorial_shadow", "FC4_TUTORIAL_SHADOW", 0) != 0;
    long n = SettingInt("propmgr_shadow_cells", "FC4_PROPMGR_SHADOW_CELLS", kDefaultCells);
    if (n <= 0) n = kDefaultCells;
    if (n > kMaxCells) n = kMaxCells;
    g_cells = n;
    struct Item {
        bool* on;
        bool replaced;
        const char* name;
        const char* key;
        const char* what;
    };
    const Item items[] = {
        {&g_prop, propmgrReplaced, "propmgr", "propmgr",
         "the record, the catalogue (load), the queries GetCityProps (17, 18), GetLotProps (23), GetStreetProps "
         "(30, 31) on sample cells and CreateProp (15) on sample types; NOT compared: GetBuildingProps (the "
         "0xA8322B7F property's layout is not in docs/abi) and every call that changes the city"},
        {&g_lcm, lotcfgReplaced, "lotcfg", "lotcfg",
         "the record, the configuration map and family set in bucket order, the object arrays at the city detail "
         "(load), the ID set queries (14, 15), HasBuilding (16, 17) and GetObjectIDArray (27) on samples; NOT "
         "compared: the filter caches, the footprints (24-26), PostCityInit's building information, the editing "
         "helpers (22, 23)"},
        {&g_tut, tutorialReplaced, "tutorial", "tutorial",
         "the record and IsTutorial, the task number and the GUID; NOT compared: the window, registry and cheat "
         "(TU6, TU7)"},
    };
    for (const Item& it : items) {
        if (!*it.on) continue;
        // glue21: a replacement is installed (`enable = <key>`) and the shadow
        // stays on. It reads whatever the city holds - FreeCity4's class when
        // the replacement is in, the game's otherwise - writes that class's own
        // record and runs a private core over it on a read-only host. With a
        // replacement that is a class-layer self-check (the vtable, the QI
        // table and the record round trip against the core's answers); with the
        // original it is the original-vs-core differential of glue15. The two
        // keep separate process catalogues and neither tears the other's down.
        if (it.replaced) {
            Log("%s shadow: ON over FreeCity4's replacement (`enable = %s`): at PostCityInit, each SimNewMonth and "
                "PreSave, the live class's own record (its Write into memory) loads a private core over a read-only "
                "host and the answers are compared with the live class's. %s",
                it.name, it.key, it.what);
            continue;
        }
        Log("%s shadow: ON: at PostCityInit, each SimNewMonth and PreSave, FreeCity4's core loaded from the "
            "original's own record (its Write into memory) over a read-only host, compared: %s",
            it.name, it.what);
    }
}

void PropLotShadowCityLoaded(cISC4City* city) { RunAll(city, CityJulian(city), "load"); }

void PropLotShadowMonth(cISC4City* city, int32_t julian) { RunAll(city, julian, "month"); }

void PropLotShadowPreSave(cISC4City* city) { RunAll(city, CityJulian(city), "save"); }

void PropLotShadowCityShutdown() {
    LogTotals("propmgr", g_prop, g_propTotals);
    LogTotals("lotcfg", g_lcm, g_lcmTotals);
    LogTotals("tutorial", g_tut, g_tutTotals);
}

void PropLotShadowAppShutdown() {
    g_liveCore.Reset();  // nothing should be live here; never leak one
    if (g_propCat != nullptr) {
        fc4_prop_catalogue_app_shutdown(g_propCat);
        fc4_prop_catalogue_destroy(g_propCat);
        g_propCat = nullptr;
    }
    if (g_lcmCat != nullptr) {
        // The lot configuration manager's process unload callback (0x0048A240):
        // the shadow's managers are per run and already gone, so the unload runs
        // here over the process catalogue on a manager made for it (the core
        // needs one to unload through). It empties the map; the catalogue is
        // destroyed right after.
        fc4_lot_config_host h{};
        h.struct_size = sizeof(h);
        h.user = &g_lctx;
        if (fc4_lot_config_manager* m = fc4_lot_config_create(&h, g_lcmCat, 0)) {
            fc4_lot_config_unload(m);
            fc4_lot_config_destroy(m);
        }
        fc4_lot_config_catalogue_destroy(g_lcmCat);  // its configurations are stand-ins (no references)
        g_lcmCat = nullptr;
    }
    g_bindings.clear();
    g_lcmSerial = 0;
}

}  // namespace fc4
