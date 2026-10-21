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

#include "FC4AuraShadow.h"

#include "FC4Config.h"
#include "FC4Diag.h"
#include "FC4Fpu.h"
#include "FC4HostGrid.h"
#include "FC4Interfaces.h"
#include "FC4Log.h"
#include "FC4RefAudit.h"
#include "FC4Streams.h"
#include "FC4Watchdog.h"
#include "cFC4Aura.h"
#include "cFC4Message2Standard.h"
#include "compat/FC4Compat.h"

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
#include "cISC4App.h"
#include "cISC4AuraSimulator.h"
#include "cISC4City.h"
#include "cRZCOMDllDirector.h"

#include <algorithm>
#include <cstring>
#include <vector>

namespace fc4 {
namespace {

// The interface the §11.3 reader asks a reference for (fc4_aura_capture_ref).
constexpr uint32_t kIID_cISC4SimGrid = 0xC9BD939A;
constexpr long kDefaultCells = 4;
constexpr long kMaxCells = 64;
// fc4::Log formats into a fixed buffer: longer shadow lines are split.
constexpr size_t kLogChunk = 2800;

// ---- the capture stream ------------------------------------------------------------------------------
//
// The original's Write goes into memory: primitives little-endian (as the
// game's streams write them), strings as a uint32 length then the bytes, and
// references (WriteGZSerializable) as uint32 objectID (index into the
// reference table + 1, 0 = null) then uint32 classID, each object listed once
// with its cISC4SimGrid identity. The identities are borrowed: the original
// keeps its grids alive (the comparison runs inside one message delivery).
// A process-lifetime object (the reference count is not kept).
class CaptureStream final : public cFC4ISC4DBSegmentOStream {
public:
    void Reset() {
        bytes_.clear();
        refs_.clear();
        ids_.clear();
        good_ = true;
    }

    fc4_aura_capture View(bool writeOk) const {
        fc4_aura_capture c{};
        c.struct_size = sizeof(c);
        c.ok = (writeOk && good_) ? 1u : 0u;
        c.bytes = bytes_.empty() ? nullptr : bytes_.data();
        c.len = bytes_.size();
        c.refs = refs_.empty() ? nullptr : refs_.data();
        c.ref_count = refs_.size();
        return c;
    }

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
        cIGZSerializable* obj = const_cast<cIGZSerializable*>(p);
        size_t i = 0;
        while (i < ids_.size() && ids_[i] != obj) ++i;
        if (i == ids_.size()) {
            fc4_aura_capture_ref r{};
            r.grid = Borrowed(obj, kIID_cISC4SimGrid);
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
    bool WriteVariant(cIGZVariant const&) override {
        good_ = false;  // not part of the aura record (§11.2)
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

    std::vector<uint8_t> bytes_;
    std::vector<fc4_aura_capture_ref> refs_;
    std::vector<const void*> ids_;
    bool good_ = true;
};

CaptureStream g_capture;

// ---- settings and run state (main thread only) ---------------------------------------------------

constexpr uint32_t kMsgSimNewDay = 0x66956814;
constexpr uint32_t kMsgSimNewMonth = 0x66956816;
constexpr uint32_t kMsgSimIdle = 0x6695681A;
constexpr uint32_t kMsgOccupantInserted = 0x99EF1142;
constexpr uint32_t kMsgOccupantRemoved = 0x99EF1143;
constexpr uint32_t kIID_cIGZMessageTarget2 = 0x452294AA;
constexpr uint32_t kIID_cISC4Occupant = 0x26BB767E;
constexpr const char* kAgentName = "cSC4AuraSimulator";  // §5.1, the simulator's agent name
// The original object, as offsets from its cISC4AuraSimulator pointer (§14.3).
constexpr size_t kOffAuraGrid = 0x40;
constexpr size_t kOffWorkGrid = 0x44;
constexpr size_t kOffTransientGrid = 0x4C;
constexpr size_t kOffState = 0x70;
constexpr size_t kOffDepartmentCursor = 0x7C;  // aura.md §14.3 (u32, -1 = none)

bool g_enabled = false;
long g_cells = kDefaultCells;
bool g_simulatorReplaced = false;
bool g_auraReplaced = false;

// Test seams (ops 14 and 35).
bool g_testMode = false;
const FC4AuraShadowDeliverInput* g_testIn = nullptr;

bool g_loggedOurs = false;
bool g_loggedNoSerializable = false;
bool g_loggedNoAura = false;
bool g_listening = false;

// The original being followed: its identity, its agent and what was checked
// when it was first seen (each original once).
const void* g_sim = nullptr;
cIGZMessageTarget2* g_agent = nullptr;
const void* g_vtable = nullptr;  // the expected cISC4AuraSimulator vtable
bool g_followed = false;         // its layout checked: the state is read
bool g_workDiagnostic = false;   // +0x44 holds a grid of the aura grid's class

// The shadow core, per city and original simulator. Its reads go to g_game,
// filled with the city's borrowed pointer for each call into the core.
FC4AuraGame g_game;
const HostGridContext g_gridCtx{"aura shadow", Sw::AuraGridWrites};
fc4_aura_shadow* g_shadow = nullptr;
const void* g_shadowCity = nullptr;
const void* g_shadowSim = nullptr;
long g_shadowCells = 0;
const void* g_shadowFailedFor = nullptr;
intptr_t g_lastResult = -1;

void ReadSettings() {
    g_enabled = SettingInt("aura_shadow", "FC4_AURA_SHADOW", 0) != 0;
    long n = SettingInt("aura_shadow_cells", "FC4_AURA_SHADOW_CELLS", kDefaultCells);
    if (n <= 0) n = kDefaultCells;
    if (n > kMaxCells) n = kMaxCells;
    g_cells = n;
}

// Enabled, and in a configuration where the original's deliveries reach
// AuraShadowDeliver.
bool Active() { return g_testMode || (g_enabled && g_simulatorReplaced && !g_auraReplaced); }

cISC4City* CurrentCity() {
    if (g_testMode) return g_testIn != nullptr ? g_testIn->city : nullptr;
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

// The original aura simulator of `city` (borrowed), or nullptr (logged once
// when it is ours).
cISC4AuraSimulator* OriginalOf(cISC4City* city) {
    if (city == nullptr) return nullptr;
    cISC4AuraSimulator* raw = city->GetAuraSimulator();  // compiled slot 51, borrowed
    if (raw == nullptr) {
        if (!g_loggedNoAura) {
            g_loggedNoAura = true;
            Log("aura shadow: the city has no aura simulator; nothing compared");
        }
        return nullptr;
    }
    if (IsOurObject(raw)) {
        if (!g_loggedOurs) {
            g_loggedOurs = true;
            Log("aura shadow: inactive: the city's aura simulator is FreeCity4's own (the shadow compares against the "
                "ORIGINAL aura simulator: do not enable aura)");
        }
        return nullptr;
    }
    return raw;
}

// ---- reading the original (§14.3) -------------------------------------------------------------------
//
// Raw reads of the original object at the spec's offsets, read-only, under a
// structured-exception guard, and only while its first word is the expected
// vtable.
struct RawRead {
    const void* obj = nullptr;
    const void* vtable = nullptr;
    bool vtableOk = false;
    int32_t state = -1;
    uint32_t departmentCursor = 0xFFFFFFFFu;
    const void* aura = nullptr;
    const void* work = nullptr;
    const void* transient = nullptr;
    const void* auraVtable = nullptr;
    const void* workVtable = nullptr;
};

template <class T>
T At(const void* obj, size_t off) {
    return *reinterpret_cast<const T*>(static_cast<const uint8_t*>(obj) + off);
}

void ReadRaw(void* ctx) {
    RawRead* r = static_cast<RawRead*>(ctx);
    r->vtableOk = r->vtable != nullptr && At<const void*>(r->obj, 0) == r->vtable;
    if (!r->vtableOk) return;
    r->state = At<int32_t>(r->obj, kOffState);
    r->departmentCursor = At<uint32_t>(r->obj, kOffDepartmentCursor);
    r->aura = At<const void*>(r->obj, kOffAuraGrid);
    r->work = At<const void*>(r->obj, kOffWorkGrid);
    r->transient = At<const void*>(r->obj, kOffTransientGrid);
    if (r->aura != nullptr) r->auraVtable = At<const void*>(r->aura, 0);
    if (r->work != nullptr) r->workVtable = At<const void*>(r->work, 0);
}

bool ReadOriginal(const void* sim, RawRead* out) {
    *out = RawRead{};
    out->obj = sim;
    out->vtable = g_vtable;
    return SehGuard("aura shadow (reading the original's state)", &ReadRaw, out) && out->vtableOk;
}

const void* ExpectedVtable() {
    if (g_testMode) return g_testIn != nullptr ? g_testIn->vtable : nullptr;
    return compat::AuraSimulatorVtable();
}

// A new original (a new or loaded city): its agent identity (its
// cIGZMessageTarget2, queried and released at once) and the layout checks
// that allow the state read.
void Track(cISC4AuraSimulator* sim) {
    g_sim = sim;
    g_followed = false;
    g_workDiagnostic = false;
    void* agent = nullptr;
    const bool qi = sim->QueryInterface(kIID_cIGZMessageTarget2, &agent) && agent != nullptr;
    g_agent = qi ? static_cast<cIGZMessageTarget2*>(agent) : nullptr;
    if (qi) g_agent->Release();  // the simulator keeps it alive; only its identity is kept
    g_vtable = ExpectedVtable();
    const char* why = nullptr;
    RawRead r;
    if (g_agent == nullptr) {
        why = "it does not answer cIGZMessageTarget2 (its agent cannot be recognised)";
    } else if (g_vtable == nullptr) {
        why = "the original's vtable is not known for this executable (compat table)";
    } else if (!ReadOriginal(sim, &r)) {
        why = r.vtableOk ? "its state could not be read" : "its vtable is not the original's (compat table)";
    } else if (r.state < 0 || r.state > 17) {
        why = "the value at +0x70 is not a pipeline state 0..17";
    } else if (r.aura == nullptr || r.aura != static_cast<const void*>(sim->GetAuraGrid())) {  // slot 12
        why = "+0x40 is not its aura grid (GetAuraGrid)";
    } else if (r.transient != static_cast<const void*>(sim->GetTransientAuraGrid())) {  // slot 13
        why = "+0x4C is not its transient grid (GetTransientAuraGrid)";
    }
    if (why != nullptr) {
        Log("aura shadow: the original aura simulator %p is NOT followed: %s; nothing compared",
            static_cast<const void*>(sim), why);
        return;
    }
    g_followed = true;
    g_workDiagnostic = r.work != nullptr && r.workVtable == r.auraVtable;
    Log("aura shadow: following the original aura simulator %p (agent %p, vtable %p): state %d at +0x70; work grid "
        "diagnostic %s",
        static_cast<const void*>(sim), static_cast<void*>(g_agent), g_vtable, static_cast<int>(r.state),
        g_workDiagnostic ? "on (+0x44)" : "OFF (+0x44 is not a grid of the aura grid's class)");
}

void DropShadow() {
    fc4_aura_shadow_destroy(g_shadow);
    g_shadow = nullptr;
    g_shadowCity = nullptr;
    g_shadowSim = nullptr;
}

// A fresh shadow core when the city, the original or the cell setting
// changed (or `fresh`: a city load). g_game.city must be set (the core reads
// the cell widths at creation).
bool EnsureShadow(cISC4City* city, const void* sim, bool fresh) {
    if (!fresh && g_shadow != nullptr && g_shadowCity == city && g_shadowSim == sim && g_shadowCells == g_cells) {
        return true;
    }
    DropShadow();
    const int32_t cx = static_cast<int32_t>(city->CellCountX());  // slot 93
    const int32_t cz = static_cast<int32_t>(city->CellCountZ());  // slot 94
    const fc4_aura_host host = AuraReadOnlyHost(&g_game, &g_gridCtx);  // copied by the core
    g_shadow = fc4_aura_shadow_create(&host, cx, cz, static_cast<uint32_t>(g_cells));
    if (g_shadow == nullptr) {
        if (g_shadowFailedFor != sim) {
            g_shadowFailedFor = sim;
            Log("aura shadow: the shadow core could not be created (city %d x %d cells); nothing compared", cx, cz);
        }
        return false;
    }
    g_shadowCity = city;
    g_shadowSim = sim;
    g_shadowCells = g_cells;
    Log("aura shadow: comparing with the original aura simulator %p (city %p, %d x %d cells); differing tracts "
        "listed per grid: %ld",
        sim, static_cast<void*>(city), cx, cz, g_cells);
    return true;
}

// The original's record through its own Write (cIGZSerializable).
fc4_aura_capture Capture(cISC4AuraSimulator* sim) {
    g_capture.Reset();
    bool ok = false;
    void* p = nullptr;
    if (sim->QueryInterface(GZIID_cIGZSerializable, &p) && p != nullptr) {
        cIGZSerializable* ser = static_cast<cIGZSerializable*>(p);
        FC4_ACQUIRED(ser, "QueryInterface");
        ok = ser->Write(g_capture);
        FC4_RELEASE(ser);
    } else if (!g_loggedNoSerializable) {
        g_loggedNoSerializable = true;
        Log("aura shadow: the original aura simulator does not answer cIGZSerializable; its record (and with it "
            "the grids) cannot be read: nothing is compared");
    }
    return g_capture.View(ok);
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
            Log("aura shadow (cont.) %.*s", static_cast<int>(n), p + off);
        }
        off += n;
        first = false;
    }
}

// ---- the calls into the core (SehGuard's contexts) -------------------------------------------------

struct Call {
    cISC4AuraSimulator* sim = nullptr;
    int32_t from = 0, to = 0, julian = 0;
    uint64_t work = 0, transient = 0, occupant = 0;
    bool inserted = false;
    intptr_t result = -1;
};

void CallBaseline(void* ctx) {
    Call* c = static_cast<Call*>(ctx);
    const fc4_aura_capture cap = Capture(c->sim);
    c->result = fc4_aura_shadow_baseline(g_shadow, &cap, &Sink, nullptr) ? 1 : 0;
}

void CallBefore(void* ctx) {
    Call* c = static_cast<Call*>(ctx);
    c->result = fc4_aura_shadow_before_step(g_shadow, c->from, c->transient) ? 1 : 0;
}

void CallStep(void* ctx) {
    Call* c = static_cast<Call*>(ctx);
    fc4_aura_shadow_step_in in{};
    in.struct_size = sizeof(in);
    in.from = c->from;
    in.to = c->to;
    in.julian = c->julian;
    in.work_grid = c->work;
    c->result = fc4_aura_shadow_step(g_shadow, &in);
    if (c->to == 0) {
        const fc4_aura_capture cap = Capture(c->sim);
        c->result = fc4_aura_shadow_cycle(g_shadow, &cap, c->julian, &Sink, nullptr);
    }
}

void CallOccupant(void* ctx) {
    Call* c = static_cast<Call*>(ctx);
    c->result = fc4_aura_shadow_occupant(g_shadow, c->occupant, c->inserted) ? 1 : 0;
}

// One guarded call into the core with the city's services; a fault stops
// following this original until the next city load.
bool Guarded(cISC4City* city, const char* what, void (*fn)(void*), Call* c) {
    g_game.gated = false;  // the original's inputs as they are
    g_game.tag = "aura shadow";
    g_game.city = city;  // borrowed for this call
    const bool clean = SehGuard(what, fn, c);
    g_game.DropHeld();  // e.g. the population grids of state 15
    g_game.city = nullptr;
    if (!clean) {
        g_followed = false;
        DropShadow();
        Log("aura shadow: a fault in %s; the original is no longer followed (until the next city load)", what);
    }
    return clean;
}

// ---- occupant messages (§5.2: posted, delivered to ours when the original gets them) -----------------

intptr_t OnOccupant(cIGZMessage2* msg) {
    if (msg == nullptr || g_shadow == nullptr || !g_followed) return 0;
    const uint32_t type = msg->GetType();
    if (type != kMsgOccupantInserted && type != kMsgOccupantRemoved) return 0;
    void* p = nullptr;
    if (!msg->QueryInterface(GZIID_cIGZMessage2Standard, &p) || p == nullptr) return 0;
    cIGZMessage2Standard* s = static_cast<cIGZMessage2Standard*>(p);
    FC4_ACQUIRED(s, "QueryInterface");
    cIGZUnknown* v1 = static_cast<cIGZUnknown*>(s->GetVoid1());
    void* o = nullptr;
    intptr_t r = 0;
    if (v1 != nullptr && v1->QueryInterface(kIID_cISC4Occupant, &o) && o != nullptr) {
        cIGZUnknown* occ = static_cast<cIGZUnknown*>(o);
        FC4_ACQUIRED(occ, "QueryInterface");
        Call c;
        c.occupant = TokenOf(occ);
        c.inserted = type == kMsgOccupantInserted;
        if (Guarded(CurrentCity(), "aura shadow (occupant message)", &CallOccupant, &c)) r = c.result;
        FC4_RELEASE(occ);
    }
    FC4_RELEASE(s);
    return r;
}

// Our subscription to the occupant messages (a static object: counted,
// never deleted).
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
        FC4_X87_SCOPE("aura shadow DoMessage", msg->GetType());  // x87 leftover guard (FC4Fpu.h)
        OnOccupant(msg);
        return true;
    }

private:
    uint32_t refs_ = 0;
};

EventTarget& Events() {
    static EventTarget t;
    return t;
}

// Registered once, at the first delivery the shadow follows (the message
// server exists by then), and left registered for the process.
void Listen() {
    if (g_listening || g_testMode) return;
    g_listening = true;
    cIGZMessageServer2Ptr ms;
    if (!ms) {
        Log("aura shadow: no message server; occupant messages do not reach ours (park, landmark and building "
            "effect grids can differ)");
        return;
    }
    Events().AddRef();  // our own reference (static object)
    bool ok = true;
    for (uint32_t id : {kMsgOccupantInserted, kMsgOccupantRemoved}) {
        ok = ms->AddNotification(static_cast<cIGZMessageTarget2*>(&Events()), id) && ok;
    }
    if (!ok) Log("aura shadow: an occupant notification was refused; ours can miss occupant messages");
}

// ---- the delivery ------------------------------------------------------------------------------------

intptr_t Deliver(cISC4City* city, cISC4AuraSimulator* sim, cIGZMessageTarget2* target, cIGZMessage2* msg,
                 uint32_t type, int64_t data1, bool* finished) {
    Listen();
    RawRead before;
    if (!ReadOriginal(sim, &before)) {
        g_followed = false;
        Log("aura shadow: the original aura simulator %p can no longer be read (its vtable changed); no longer "
            "followed",
            static_cast<const void*>(sim));
        return -1;
    }
    int32_t julian = static_cast<int32_t>(PublishedJulian());
    if (g_testMode && g_testIn != nullptr) {
        julian = g_testIn->julian;
    } else if (type == kMsgSimNewDay) {
        julian = static_cast<int32_t>(data1);
    }
    g_game.city = city;  // the core reads the cell widths at creation
    const bool core = EnsureShadow(city, sim, false);
    g_game.city = nullptr;
    // §5.3: states 15 and 17 read the transient grid (17 decays it): ours
    // gets the original's before the original's step runs.
    if (core && (before.state == 15 || before.state == 17)) {
        Call c;
        c.from = before.state;
        c.transient = TokenOf(sim->GetTransientAuraGrid());  // slot 13, borrowed
        Guarded(city, "aura shadow (transient copy)", &CallBefore, &c);
    }
    *finished = target->DoMessage(msg);
    RawRead after;
    if (!ReadOriginal(sim, &after)) {
        g_followed = false;
        Log("aura shadow: the original aura simulator %p could not be read after a delivery; no longer followed",
            static_cast<const void*>(sim));
        return -2;
    }
    // §6.3: state 3 takes one department per call. When the original stayed
    // in 3 but moved its cursor, ours runs one state-3 call too (the core's
    // 3 -> 3 step), so both read each department grid at the same moment.
    if (g_shadow != nullptr && g_followed && before.state == 3 && after.state == 3 &&
        after.departmentCursor != before.departmentCursor) {
        Call c;
        c.sim = sim;
        c.from = 3;
        c.to = 3;
        c.julian = julian;
        Guarded(city, "aura shadow (state 3 department)", &CallStep, &c);
    }
    if (g_shadow != nullptr && g_followed && after.state != before.state) {
        Call c;
        c.sim = sim;
        c.from = before.state;
        c.to = after.state;
        c.julian = julian;
        c.work = g_workDiagnostic ? TokenOf(after.work) : 0;
        Guarded(city, after.state == 0 ? "aura shadow (steps and cycle)" : "aura shadow (steps)", &CallStep, &c);
    }
    return after.state;
}

}  // namespace

void AuraShadowStartup(bool simulatorReplaced, bool auraReplaced) {
    g_simulatorReplaced = simulatorReplaced;
    g_auraReplaced = auraReplaced;
    ReadSettings();
    if (!g_enabled) return;  // off by default: not a word in the log
    const char* src = "";
    Setting("aura_shadow", "FC4_AURA_SHADOW", &src);
    if (auraReplaced) {
        Log("aura shadow: requested (aura_shadow from %s) but INACTIVE: it compares against the ORIGINAL aura "
            "simulator, and aura is enabled (ours)",
            src);
    } else if (!simulatorReplaced) {
        Log("aura shadow: requested (aura_shadow from %s) but INACTIVE: it follows the original's deliveries, which "
            "pass through FreeCity4's simulator, and simulator is not enabled",
            src);
    } else {
        Log("aura shadow: ON (aura_shadow from %s): FreeCity4's aura core follows the ORIGINAL aura simulator step "
            "by step (aura.md §14.3): at city load it starts from the original's record; after each delivery to "
            "the original's agent that moves its state (+0x70, read-only, vtable checked) our steps run at once, "
            "reading every input read-only; occupant messages reach ours when the original gets them; the "
            "transient grid is copied before states 15 and 17; at each cycle end (state back to 0) the record is "
            "compared tract by tract (base, aura, park and landmark maps, transient; the work grid after each step "
            "5-13), with the mayor rating, best / worst and hotspots; one line per cycle; differing tracts listed "
            "per grid: %ld. Not compared: the riot roll (wall-clock RNG, aura.md §9), and posts, history, riots "
            "and strikes (writes)",
            src, g_cells);
    }
}

void AuraShadowCityLoaded(cISC4City* city) {
    if (!g_testMode) ReadSettings();  // FreeCity4.ini may change between city loads
    if (!Active()) return;
    g_lastResult = -1;
    cISC4AuraSimulator* sim = OriginalOf(city);
    if (sim == nullptr) return;
    Track(sim);
    if (!g_followed) return;
    g_game.city = city;
    const bool core = EnsureShadow(city, sim, true);
    g_game.city = nullptr;
    if (!core) return;
    Call c;
    c.sim = sim;
    if (Guarded(city, "aura shadow (city load)", &CallBaseline, &c)) g_lastResult = c.result;
}

void AuraShadowCycle(cISC4City*, int32_t) {
    // §5.3: the cycle-completed message is posted and arrives later than the
    // original's state 17; the comparison runs when its state returns to 0.
}

bool AuraShadowDeliver(cIGZMessageTarget2* target, const char* agentName, cIGZMessage2* msg, uint32_t type,
                       int64_t data1, bool* finished) {
    if ((type != kMsgSimIdle && type != kMsgSimNewDay && type != kMsgSimNewMonth) || target == nullptr ||
        msg == nullptr || finished == nullptr || !Active()) {
        return false;
    }
    // Cheap rejection for every other agent.
    if (target != g_agent && (agentName == nullptr || strcmp(agentName, kAgentName) != 0)) return false;
    cISC4City* city = CurrentCity();
    cISC4AuraSimulator* sim = OriginalOf(city);
    if (sim == nullptr) return false;
    if (sim != g_sim) Track(sim);  // a city whose load the shadow did not see
    if (!g_followed || target != g_agent) return false;
    const intptr_t r = Deliver(city, sim, target, msg, type, data1, finished);
    if (r == -1) return false;  // not delivered: the caller delivers
    if (g_testMode) g_lastResult = r;
    return true;
}

}  // namespace fc4

intptr_t AuraShadowTestCityLoad(const FC4AuraShadowDeliverInput* in) {
    if (in == nullptr) return -1;
    fc4::g_testMode = true;
    fc4::g_testIn = in;
    fc4::g_shadowFailedFor = nullptr;
    fc4::g_lastResult = -1;
    fc4::DropShadow();  // each city load is a fresh "game"
    fc4::g_sim = nullptr;
    fc4::g_agent = nullptr;
    fc4::g_loggedOurs = false;
    fc4::g_loggedNoAura = false;
    fc4::g_loggedNoSerializable = false;
    fc4::AuraShadowCityLoaded(in->city);
    fc4::g_testIn = nullptr;
    fc4::g_testMode = false;
    return fc4::g_lastResult;
}

intptr_t AuraShadowTestDeliver(const FC4AuraShadowDeliverInput* in) {
    if (in == nullptr) return -1;
    fc4::g_testMode = true;
    fc4::g_testIn = in;
    intptr_t r = -1;
    if (in->type == fc4::kMsgOccupantInserted || in->type == fc4::kMsgOccupantRemoved) {
        cFC4Message2Standard* m = FC4MakeMessage(in->type, 0, 0, 0);
        m->SetVoid1(in->void1);
        r = fc4::OnOccupant(m);
        m->Release();
    } else {
        cFC4Message2Standard* m = FC4MakeMessage(in->type, static_cast<intptr_t>(in->julian), 0, 0);
        bool finished = false;
        fc4::g_lastResult = -1;
        if (fc4::AuraShadowDeliver(in->target, nullptr, m, in->type, in->julian, &finished)) {
            r = fc4::g_lastResult;
        } else if (in->target != nullptr) {
            in->target->DoMessage(m);
        }
        m->Release();
    }
    fc4::g_testIn = nullptr;
    fc4::g_testMode = false;
    return r;
}
