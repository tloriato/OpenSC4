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

#include "FC4OccMgrShadow.h"

#include "FC4Config.h"
#include "FC4Diag.h"
#include "FC4Fpu.h"
#include "FC4Log.h"
#include "FC4RefAudit.h"
#include "FC4Streams.h"
#include "FC4WriteCapture.h"
#include "fc4.h"

#include "GZServPtrs.h"
#include "SC4List.h"
#include "cIGZMessage2.h"
#include "cIGZMessage2Standard.h"
#include "cIGZMessageServer2.h"
#include "cIGZMessageTarget2.h"
#include "cIGZSerializable.h"
#include "cISC4City.h"
#include "cISC4Occupant.h"
#include "cISC4OccupantFilter.h"
#include "cISC4OccupantManager.h"
#include "cS3DVector3.h"

#include <windows.h>

#include <cstdarg>
#include <cstdio>
#include <cstring>
#include <functional>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <vector>

namespace fc4 {
namespace {

constexpr uint32_t kMsgDestroyed = 0x99EF1141;  // §5.5 (the occupant's Shutdown)
constexpr uint32_t kMsgInserted = 0x99EF1142;   // §5.5
constexpr uint32_t kMsgRemoved = 0x99EF1143;
constexpr uint32_t kMsgMoved = 0x99EF1146;
constexpr uint32_t kIID_cIGZMessageTarget2 = 0x452294AA;
constexpr uint32_t kIID_cISC4Occupant = 0x26BB767E;
constexpr uint32_t kFlagInserted = 1;  // §5.6
constexpr size_t kMaxListed = 10;
constexpr int kFirstSlot = 25;
constexpr int kLastSlot = 47;
constexpr int kSlots = 50;

bool g_enabled = false;
bool g_probeWanted = true;
int g_samples = 4;
bool g_listening = false;
bool g_loggedOurs = false;

CRITICAL_SECTION g_cs;
bool g_csReady = false;

struct Lock {
    Lock() {
        if (g_csReady) EnterCriticalSection(&g_cs);
    }
    ~Lock() {
        if (g_csReady) LeaveCriticalSection(&g_cs);
    }
    Lock(const Lock&) = delete;
    Lock& operator=(const Lock&) = delete;
};

// ---- the shadow host's per-occupant state ---------------------------------------------------------

struct OccState {
    uint8_t box[4] = {0, 0, 0, 0};
    bool hasBox = false;
    uint32_t flags = 0;
};

// A core and its host's state (the host's `user`).
struct CoreCtx {
    fc4_occmgr* core = nullptr;
    fc4_occmgr_host host{};
    std::unordered_map<uint64_t, OccState> state;
    std::unordered_map<uint64_t, uint32_t> holds;  // our references per occupant
    uint32_t stray = 0;                            // releases of references the core does not hold
};

CoreCtx g_fol;    // the followed core: from the record at the city load, then the messages
CoreCtx g_fresh;  // a month's fresh core from the record, for the queries while messages are queued
fc4_occmgr* g_qcore = nullptr;  // the core the month's queries run on
const void* g_orig = nullptr;   // the original the followed core follows

CoreCtx* Ctx(void* u) { return static_cast<CoreCtx*>(u); }

// One message of the §5.5 stream, as it was posted (kind, occupant, data4).
struct MsgRec {
    uint32_t kind = 0;
    uint64_t occ = 0;
    uint32_t data4 = 0;
    bool operator==(const MsgRec& o) const {
        return kind == o.kind && occ == o.occ && data4 == o.data4;
    }
};

const char* MsgName(uint32_t kind) {
    switch (kind) {
        case kMsgDestroyed: return "destroy";
        case kMsgInserted: return "insert";
        case kMsgRemoved: return "remove";
        case kMsgMoved: return "move";
        default: return "other";
    }
}

// Counters since the last month line (messages) and since the city load.
struct Follow {
    uint32_t ins = 0, rem = 0, mov = 0, refused = 0, boxTotal = 0, boxExact = 0, offThread = 0;
    std::string boxDiffs;
    size_t boxListed = 0;
    // The message-order comparison (§5.5): the manager's own messages as the
    // original posts them against the followed core's, in order.
    std::vector<MsgRec> origMsgs, coreMsgs;
    uint32_t msgExact = 0, msgTotal = 0;
    std::string msgDiffs;
    size_t msgListed = 0;
    // RemoveOccupants' interleaving (§5.3 step 4): Remove(o1), Destroy(o1),
    // Remove(o2), Destroy(o2), ... A Destroy of an occupant whose own Remove is
    // outstanding but is NOT the message just before it means the batch was
    // posted the other way round. A Destroy of an occupant with no outstanding
    // Remove is an occupant shut down outside the manager (counted only).
    std::unordered_set<uint64_t> removed;
    uint32_t destroyOk = 0, destroyOther = 0, destroyElse = 0;
    std::string rmDiffs;
    size_t rmListed = 0;
};
Follow g_follow;

uint32_t g_months = 0, g_cleanMonths = 0, g_resyncs = 0;
uint64_t g_slotTotal[kSlots] = {}, g_slotExact[kSlots] = {};
DWORD g_mainTid = 0;

cISC4Occupant* Occ(uint64_t t) { return reinterpret_cast<cISC4Occupant*>(static_cast<uintptr_t>(t)); }

void HoldObject(cIGZUnknown* o) {
    if (o == nullptr) return;
    if (IsOurObject(o)) {
        o->AddRef();
    } else {
        FC4_ADDREF(o);
    }
}

void DropObject(cIGZUnknown* o) {
    if (o == nullptr) return;
    if (IsOurObject(o)) {
        o->Release();
    } else {
        FC4_RELEASE(o);
    }
}

bool RealBox(uint64_t t, uint8_t out[4]) {
    uint8_t b[8] = {0, 0, 0, 0, 0, 0, 0, 0};
    if (!Occ(t)->GetOccupantManagerBBox(b)) return false;  // occupant slot 25
    std::memcpy(out, b, 4);
    return true;
}

bool RealFlag(uint64_t t) { return Occ(t)->IsFlagSet(kFlagInserted); }  // occupant slot 32

// The canonical token of an object: its cISC4Occupant interface.
uint64_t Canonical(cIGZUnknown* u) {
    if (u == nullptr) return 0;
    void* p = nullptr;
    if (!u->QueryInterface(kIID_cISC4Occupant, &p) || p == nullptr) return 0;
    cIGZUnknown* q = static_cast<cIGZUnknown*>(p);
    if (IsOurObject(q)) {
        q->Release();
    } else {
        FC4_ACQUIRED(q, "QueryInterface");
        FC4_RELEASE(q);
    }
    return TokenOf(q);
}

// ---- the deterministic filter both managers are given ---------------------------------------------

class SampleFilter final : public cISC4OccupantFilter {
public:
    uint32_t salt = 0;
    uint32_t keep = 4;  // of 8
    bool QueryInterface(uint32_t riid, void** ppvObj) override {
        if (ppvObj == nullptr) return false;
        if (riid != GZIID_cISC4OccupantFilter && riid != GZIID_cIGZUnknown) return false;
        *ppvObj = static_cast<cISC4OccupantFilter*>(this);
        return true;
    }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    bool IsOccupantIncluded(cISC4Occupant* o) override {
        uint32_t h = static_cast<uint32_t>(reinterpret_cast<uintptr_t>(o)) ^ salt;
        h *= 0x9E3779B1u;
        return (h >> 29) < keep;
    }
    bool IsOccupantTypeIncluded(uint32_t) override { return true; }
    bool IsPropertyHolderIncluded(cISCPropertyHolder*) override { return true; }
};

SampleFilter& TheFilter() {
    static SampleFilter f;
    return f;
}

// ---- the read-only host ---------------------------------------------------------------------------

void HAddRef(void* u, uint64_t t) {
    if (t == 0) return;
    HoldObject(FromToken(t));
    ++Ctx(u)->holds[t];
}

void HRelease(void* u, uint64_t t) {
    if (t == 0) return;
    CoreCtx* c = Ctx(u);
    auto it = c->holds.find(t);
    if (it == c->holds.end() || it->second == 0) {
        ++c->stray;  // not ours to release: kept
        return;
    }
    if (--it->second == 0) c->holds.erase(it);
    DropObject(FromToken(t));
}

void HInit(void*, uint64_t) {}
void HShutdown(void*, uint64_t) {}
bool HHasHolder(void*, uint64_t t) { return t != 0 && Occ(t)->AsPropertyHolder() != nullptr; }
uint32_t HType(void*, uint64_t t) { return t != 0 ? static_cast<uint32_t>(Occ(t)->GetType()) : 0u; }

void HBox(void*, uint64_t t, fc4_occ_bbox* out) {
    if (t == 0 || out == nullptr) return;
    cS3DVector3 mn(0.0f, 0.0f, 0.0f), mx(0.0f, 0.0f, 0.0f);
    Occ(t)->GetBoundingBox(&mn, &mx);  // occupant slot 10
    out->min[0] = mn.fX;
    out->min[1] = mn.fY;
    out->min[2] = mn.fZ;
    out->max[0] = mx.fX;
    out->max[1] = mx.fY;
    out->max[2] = mx.fZ;
}

// The followed core's own posts, kept for the message-order comparison. The
// fresh core (a month's query copy) is not followed and posts nothing here.
void HPost(void* u, uint64_t occ, uint32_t msg_type, uint32_t data4) {
    if (Ctx(u) != &g_fol) return;
    g_follow.coreMsgs.push_back(MsgRec{msg_type, occ, data4});
}

bool HGetBox(void* u, uint64_t t, fc4_occ_cell_box* out) {
    const auto& state = Ctx(u)->state;
    auto it = state.find(t);
    if (it == state.end() || !it->second.hasBox) return false;
    if (out != nullptr) {
        out->x0 = it->second.box[0];
        out->z0 = it->second.box[1];
        out->x1 = it->second.box[2];
        out->z1 = it->second.box[3];
    }
    return true;
}

// As the base occupant's (§4.3): the first byte 0xFF is the "no box" mark.
void HSetBox(void* u, uint64_t t, const fc4_occ_cell_box* b) {
    if (t == 0 || b == nullptr) return;
    OccState& s = Ctx(u)->state[t];
    s.box[0] = b->x0;
    s.box[1] = b->z0;
    s.box[2] = b->x1;
    s.box[3] = b->z1;
    s.hasBox = b->x0 != 0xFF;
}

void HSetFlag(void* u, uint64_t t, uint32_t f) {
    if (t != 0) Ctx(u)->state[t].flags |= f;
}
void HClearFlag(void* u, uint64_t t, uint32_t f) {
    auto& state = Ctx(u)->state;
    auto it = state.find(t);
    if (it != state.end()) it->second.flags &= ~f;
}
bool HIsFlagSet(void* u, uint64_t t, uint32_t f) {
    const auto& state = Ctx(u)->state;
    auto it = state.find(t);
    return it != state.end() && (it->second.flags & f) != 0;
}

bool HFilter(void*, uint64_t filter, uint64_t occ) {
    if (filter == 0) return true;
    cISC4OccupantFilter* f = reinterpret_cast<cISC4OccupantFilter*>(static_cast<uintptr_t>(filter));
    return f->IsOccupantIncluded(Occ(occ));
}

// A host object arrives from the capture with one reference (now the core's);
// one of ours arrives without one, so it is taken here (FC4WriteCapture.h).
uint64_t HResolve(void* u, uint64_t t, uint32_t) {
    cIGZUnknown* o = FromToken(t);
    if (o == nullptr) return 0;
    if (IsOurObject(o)) o->AddRef();
    ++Ctx(u)->holds[t];
    return t;
}

fc4_occmgr_host MakeHost(CoreCtx* c) {
    fc4_occmgr_host h{};
    h.struct_size = sizeof(h);
    h.user = c;
    h.add_ref = &HAddRef;
    h.release = &HRelease;
    h.init = &HInit;
    h.shutdown = &HShutdown;
    h.has_property_holder = &HHasHolder;
    h.occupant_type = &HType;
    h.bounding_box = &HBox;
    h.post_message = &HPost;
    h.manager_bbox = &HGetBox;
    h.set_manager_bbox = &HSetBox;
    h.set_flag = &HSetFlag;
    h.clear_flag = &HClearFlag;
    h.is_flag_set = &HIsFlagSet;
    h.filter_includes = &HFilter;
    h.resolve_ref = &HResolve;
    return h;
}

// ---- the original's record (§13) ------------------------------------------------------------------

struct Record {
    bool ok = false;
    float size[2] = {0.0f, 0.0f};
    int32_t counts[4] = {0, 0, 0, 0};
    std::vector<uint32_t> rowStart;  // first flat cell of each row; one past the end last
    std::vector<uint32_t> cellStart;  // first reference of each flat cell; one past the end last
    std::vector<uint64_t> refs;       // canonical tokens, in list order
    std::vector<uint64_t> occs;       // each occupant once, in first-reference order
};

WriteCapture g_capture;

Record Parse(const WriteCapture& cap) {
    Record r;
    CaptureReader b(cap.Bytes());
    uint16_t version = 0;
    if (!b.U16(version) || version != 1) return r;
    if (!b.F32(r.size[0]) || !b.F32(r.size[1])) return r;
    for (int32_t& c : r.counts) {
        if (!b.I32(c)) return r;
    }
    uint32_t rows = 0;
    if (!b.U32(rows) || rows > 4096) return r;
    std::vector<uint64_t> byId;
    std::unordered_set<uint64_t> seen;
    for (uint32_t x = 0; x < rows; ++x) {
        uint32_t cells = 0;
        if (!b.U32(cells) || cells > 4096) return r;
        r.rowStart.push_back(static_cast<uint32_t>(r.cellStart.size()));
        for (uint32_t z = 0; z < cells; ++z) {
            uint32_t n = 0;
            if (!b.U32(n) || n > 10000000u) return r;
            r.cellStart.push_back(static_cast<uint32_t>(r.refs.size()));
            for (uint32_t i = 0; i < n; ++i) {
                uint32_t id = 0;
                if (!b.Ref(id)) return r;
                if (id >= byId.size()) byId.resize(static_cast<size_t>(id) + 1, ~uint64_t{0});
                if (byId[id] == ~uint64_t{0}) {
                    byId[id] = id != 0 ? Canonical(static_cast<cIGZUnknown*>(cap.SerializableAt(id))) : 0;
                }
                const uint64_t t = byId[id];
                if (t == 0) continue;  // §13 step 4.1: a reference that does not resolve is skipped
                r.refs.push_back(t);
                if (seen.insert(t).second) r.occs.push_back(t);
            }
        }
    }
    r.rowStart.push_back(static_cast<uint32_t>(r.cellStart.size()));
    r.cellStart.push_back(static_cast<uint32_t>(r.refs.size()));
    uint8_t extra = 0;
    r.ok = !b.U8(extra);  // the whole record was read
    return r;
}

// ---- the shadow core ------------------------------------------------------------------------------

// The core goes, then every reference it still holds (the core calls no
// host on destruction).
void DropCore(CoreCtx& c) {
    if (c.core != nullptr) fc4_occmgr_destroy(c.core);
    c.core = nullptr;
    for (auto& h : c.holds) {
        for (uint32_t i = 0; i < h.second; ++i) DropObject(FromToken(h.first));
    }
    c.holds.clear();
    c.state.clear();
    if (&c == &g_fol) g_orig = nullptr;
}

// A fresh core from the capture: each occupant's stored box and flag 1 read
// from the occupant, then Read and Init.
bool Restart(CoreCtx& c, const Record& rec, const void* orig) {
    DropCore(c);
    for (uint64_t t : rec.occs) {
        OccState s;
        s.hasBox = RealBox(t, s.box);
        s.flags = RealFlag(t) ? kFlagInserted : 0u;
        c.state[t] = s;
    }
    c.host = MakeHost(&c);
    c.core = fc4_occmgr_create(&c.host);
    if (c.core == nullptr) return false;
    CaptureIStream in(g_capture);
    if (!fc4_occmgr_read(c.core, in.Get())) {
        DropCore(c);
        return false;
    }
    fc4_occmgr_init(c.core);
    if (&c == &g_fol) g_orig = orig;
    return true;
}

std::vector<uint64_t> OurCell(int32_t x, int32_t z) {
    std::vector<uint64_t> v(16);
    int32_t n = fc4_occmgr_cell(g_fol.core, x, z, v.data(), nullptr, v.size());
    if (n > static_cast<int32_t>(v.size())) {
        v.resize(static_cast<size_t>(n));
        n = fc4_occmgr_cell(g_fol.core, x, z, v.data(), nullptr, v.size());
    }
    v.resize(n > 0 ? static_cast<size_t>(n) : 0u);
    return v;
}

// ---- small helpers --------------------------------------------------------------------------------

std::string DateText(int32_t julian) {
    if (julian <= 0) return "0000-00-00";
    fc4_date d{};
    fc4_date_from_julian(julian, &d);
    char b[24];
    snprintf(b, sizeof(b), "%04d-%02d-%02d", d.year, d.month, d.day_of_month);
    return b;
}

struct Diff {
    uint32_t mismatches = 0;
    std::string listed;
    size_t n = 0;
    void Add(const char* fmt, ...) {
        ++mismatches;
        if (n++ >= kMaxListed) return;
        char b[200];
        va_list ap;
        va_start(ap, fmt);
        vsnprintf(b, sizeof(b), fmt, ap);
        va_end(ap);
        if (!listed.empty()) listed += ", ";
        listed += b;
    }
};

cISC4OccupantManager* OriginalOf(cISC4City* city) {
    if (city == nullptr) return nullptr;
    cISC4OccupantManager* om = city->GetOccupantManager();  // borrowed
    if (om == nullptr) return nullptr;
    if (IsOurObject(om)) {
        if (!g_loggedOurs) {
            g_loggedOurs = true;
            Log("occmgr shadow: inactive: the city's occupant manager is FreeCity4's own");
        }
        return nullptr;
    }
    return om;
}

// ---- the thread probe (OM1, OM3) ------------------------------------------------------------------

constexpr uintptr_t kOriginalVtable = 0x00A8F7F0;  // §1, §11
constexpr int kProbeSlots = 18;
constexpr int kProbeThreads = 16;

struct ProbeEntry {
    int slot;
    uintptr_t expected;  // §11
};
constexpr ProbeEntry kProbe[kProbeSlots] = {
    {1, 0x0049DF80},  {2, 0x0049DFB0},  {12, 0x0049DA30}, {20, 0x0049EF60}, {21, 0x0049F240}, {22, 0x0049FCD0},
    {24, 0x0049F3C0}, {25, 0x0049E490}, {27, 0x0049FB30}, {28, 0x0049E4D0}, {30, 0x0049E5E0}, {32, 0x0049E690},
    {34, 0x0049E800}, {36, 0x004A0670}, {38, 0x004A0A70}, {40, 0x0049E250}, {42, 0x004A0D70},
    {44, 0x004A1360},
};

uintptr_t g_probeOrig[kProbeSlots] = {};
void** g_probeEntry[kProbeSlots] = {};
bool g_probeOn[kProbeSlots] = {};
bool g_probeTried = false;

struct ProbeThread {
    volatile LONG tid;
    volatile LONG calls[kProbeSlots];
};
ProbeThread g_threads[kProbeThreads];
volatile LONG g_threadOverflow = 0;
volatile LONG g_depth = 0;
volatile LONG g_ownerTid = 0;
volatile LONG g_overlaps = 0;
volatile LONG g_shadowCalling = 0;

bool ProbeEnter(int i) {
    const DWORD tid = GetCurrentThreadId();
    if (g_shadowCalling != 0 && tid == g_mainTid) return false;
    ProbeThread* row = nullptr;
    for (ProbeThread& t : g_threads) {
        const LONG cur = t.tid;
        if (cur == static_cast<LONG>(tid)) {
            row = &t;
            break;
        }
        if (cur == 0) {
            const LONG was = InterlockedCompareExchange(&t.tid, static_cast<LONG>(tid), 0);
            if (was == 0 || was == static_cast<LONG>(tid)) {
                row = &t;
                break;
            }
        }
    }
    if (row != nullptr) {
        InterlockedIncrement(&row->calls[i]);
    } else {
        InterlockedIncrement(&g_threadOverflow);
    }
    const LONG d = InterlockedIncrement(&g_depth);
    if (d == 1) {
        InterlockedExchange(&g_ownerTid, static_cast<LONG>(tid));
    } else if (g_ownerTid != static_cast<LONG>(tid)) {
        InterlockedIncrement(&g_overlaps);
    }
    return true;
}

void ProbeExit() { InterlockedDecrement(&g_depth); }

// A thiscall slot as fastcall (ecx = this, edx unused, the rest on the stack,
// callee pops): every argument and the return passed through unchanged.
template <int I, class... A>
uint32_t __fastcall ProbeForward(void* self, void* edx, A... a) {
    const bool counted = ProbeEnter(I);
    using Fn = uint32_t(__fastcall*)(void*, void*, A...);
    const uint32_t r = reinterpret_cast<Fn>(g_probeOrig[I])(self, edx, a...);
    if (counted) ProbeExit();
    return r;
}

using P = void*;
using CP = const void*;
void* const kProbeFwd[kProbeSlots] = {
    reinterpret_cast<void*>(&ProbeForward<0>),
    reinterpret_cast<void*>(&ProbeForward<1>),
    reinterpret_cast<void*>(&ProbeForward<2, int, int, P, P>),
    reinterpret_cast<void*>(&ProbeForward<3, P, uint32_t>),
    reinterpret_cast<void*>(&ProbeForward<4, P, bool, uint32_t>),
    reinterpret_cast<void*>(&ProbeForward<5, P, bool, uint32_t>),
    reinterpret_cast<void*>(&ProbeForward<6, P, bool>),
    reinterpret_cast<void*>(&ProbeForward<7, int, int>),
    reinterpret_cast<void*>(&ProbeForward<8, P, bool, P, P>),
    reinterpret_cast<void*>(&ProbeForward<9, P, float, float, P>),
    reinterpret_cast<void*>(&ProbeForward<10, P, int, int, P>),
    reinterpret_cast<void*>(&ProbeForward<11, P, int, int, P>),
    reinterpret_cast<void*>(&ProbeForward<12, P, CP, CP, P>),
    reinterpret_cast<void*>(&ProbeForward<13, P, CP, CP, P, uint32_t>),
    reinterpret_cast<void*>(&ProbeForward<14, P, CP, CP, P, uint32_t>),
    reinterpret_cast<void*>(&ProbeForward<15, P, CP, CP, P, uint32_t>),
    reinterpret_cast<void*>(&ProbeForward<16, P, P, CP, CP, P>),
    reinterpret_cast<void*>(&ProbeForward<17, P, P, CP, CP, P>),
};

bool WriteEntry(void** entry, void* value) {
    DWORD before = 0;
    if (!VirtualProtect(entry, sizeof(void*), PAGE_READWRITE, &before)) return false;
    *entry = value;
    DWORD ignored = 0;
    VirtualProtect(entry, sizeof(void*), before, &ignored);
    FlushInstructionCache(GetCurrentProcess(), entry, sizeof(void*));
    return true;
}

void InstallProbe(const void* orig) {
    if (g_probeTried || !g_probeWanted) return;
    g_probeTried = true;
    void** vt = *reinterpret_cast<void** const*>(orig);
    if (reinterpret_cast<uintptr_t>(vt) != kOriginalVtable) {
        Log("occmgr shadow: probe NOT installed: the manager's vtable is %p, not the original's 0x%08lX",
            static_cast<void*>(vt), static_cast<unsigned long>(kOriginalVtable));
        return;
    }
    int installed = 0;
    std::string refused;
    for (int i = 0; i < kProbeSlots; ++i) {
        void** entry = vt + kProbe[i].slot;
        const uintptr_t found = reinterpret_cast<uintptr_t>(*entry);
        if (found != kProbe[i].expected) {
            refused += " " + std::to_string(kProbe[i].slot);
            continue;
        }
        g_probeOrig[i] = found;
        if (!WriteEntry(entry, kProbeFwd[i])) {
            refused += " " + std::to_string(kProbe[i].slot) + "(protect)";
            continue;
        }
        g_probeEntry[i] = entry;
        g_probeOn[i] = true;
        ++installed;
    }
    Log("occmgr shadow: probe installed on %d of %d slots of vtable %p (main thread %lu)%s%s", installed, kProbeSlots,
        static_cast<void*>(vt), static_cast<unsigned long>(g_mainTid), refused.empty() ? "" : "; refused:",
        refused.c_str());
}

void RemoveProbe() {
    for (int i = 0; i < kProbeSlots; ++i) {
        if (!g_probeOn[i]) continue;
        if (WriteEntry(g_probeEntry[i], reinterpret_cast<void*>(g_probeOrig[i]))) {
            g_probeOn[i] = false;
        } else {
            Log("occmgr shadow: WARNING probe slot %d NOT removed (VirtualProtect failed)", kProbe[i].slot);
        }
    }
    g_probeTried = false;
}

std::string ProbeText(bool perSlot) {
    std::string s;
    unsigned long mainCalls = 0, otherCalls = 0, slot12 = 0;
    std::string others;
    for (const ProbeThread& t : g_threads) {
        if (t.tid == 0) continue;
        unsigned long n = 0;
        for (int i = 0; i < kProbeSlots; ++i) n += static_cast<unsigned long>(t.calls[i]);
        slot12 += static_cast<unsigned long>(t.calls[2]);
        const bool isMain = static_cast<DWORD>(t.tid) == g_mainTid;
        if (isMain) {
            mainCalls += n;
        } else {
            otherCalls += n;
            others += " " + std::to_string(static_cast<unsigned long>(t.tid)) + ":" + std::to_string(n);
        }
        if (perSlot) {
            s += "; thread " + std::to_string(static_cast<unsigned long>(t.tid)) + (isMain ? " (main)" : "") + ":";
            for (int i = 0; i < kProbeSlots; ++i) {
                if (t.calls[i] != 0) {
                    s += " s" + std::to_string(kProbe[i].slot) + "=" + std::to_string(static_cast<long>(t.calls[i]));
                }
            }
        }
    }
    int on = 0;
    for (bool b : g_probeOn) on += b ? 1 : 0;
    char head[200];
    snprintf(head, sizeof(head), "probe: slots=%d main=%lu other=%lu (threads%s) overlaps=%ld slot12=%lu overflow=%ld",
             on, mainCalls, otherCalls, others.empty() ? " none" : others.c_str(), static_cast<long>(g_overlaps),
             slot12, static_cast<long>(g_threadOverflow));
    return std::string(head) + s;
}

// ---- the sampled queries --------------------------------------------------------------------------

struct Lcg {
    uint32_t s;
    uint32_t Next() {
        s = s * 1664525u + 1013904223u;
        return s >> 8;
    }
    int32_t Range(int32_t lo, int32_t hi) {
        return lo + static_cast<int32_t>(Next() % static_cast<uint32_t>(hi - lo + 1));
    }
    bool Chance(uint32_t num, uint32_t den) { return Next() % den < num; }
};

struct Result {
    bool ret = false;
    std::vector<uint64_t> seq;
    int32_t x = 0, z = 0;
    bool operator==(const Result& o) const { return ret == o.ret && seq == o.seq && x == o.x && z == o.z; }
};

template <class F>
F VSlot(const void* obj, int slot) {
    return reinterpret_cast<F>((*reinterpret_cast<void* const* const*>(obj))[slot]);
}

// An original's call under the structured-exception guard.
bool Guarded(const char* what, const std::function<void()>& f) {
    struct Ctx {
        const std::function<void()>* f;
    } c{&f};
    InterlockedExchange(&g_shadowCalling, 1);
    const bool ok = SehGuard(what, [](void* p) { (*static_cast<Ctx*>(p)->f)(); }, &c);
    InterlockedExchange(&g_shadowCalling, 0);
    return ok;
}

bool __cdecl OrigIter(cISC4Occupant* o, void* d);
bool OurIter(void* d, uint64_t o);

struct IterRec {
    std::vector<uint64_t> seq;
    uint32_t stopAfter = 0;
};

bool __cdecl OrigIter(cISC4Occupant* o, void* d) {
    IterRec* r = static_cast<IterRec*>(d);
    r->seq.push_back(TokenOf(o));
    return !(r->stopAfter != 0 && r->seq.size() >= r->stopAfter);
}

bool OurIter(void* d, uint64_t o) {
    IterRec* r = static_cast<IterRec*>(d);
    r->seq.push_back(o);
    return !(r->stopAfter != 0 && r->seq.size() >= r->stopAfter);
}

void OurSink(void* ctx, uint64_t o) { static_cast<std::vector<uint64_t>*>(ctx)->push_back(o); }

void Append(char* a, size_t cap, const char* t) {
    const size_t have = strlen(a);
    size_t n = strlen(t);
    if (have + 1 >= cap) return;
    if (n > cap - have - 1) n = cap - have - 1;
    std::memcpy(a + have, t, n);
    a[have + n] = 0;
}

std::vector<uint64_t> OrigList(SC4List<cISC4Occupant*>& l) {
    std::vector<uint64_t> v;
    for (cISC4Occupant* o : l) v.push_back(TokenOf(o));
    return v;  // the list's destructor releases its references
}

void ReleaseOurs(const std::vector<uint64_t>& v) {
    if (!v.empty()) fc4_occmgr_release_list(g_qcore, v.data(), v.size());
}

struct Sampler {
    const Record& rec;
    Lcg r;
    float size[2];
    int32_t mcount[2], scount[2];
    Diff& d;
    uint32_t month[kSlots][2] = {};  // exact, total
    bool fault = false;

    Sampler(const Record& rr, uint32_t seed, Diff& dd) : rec(rr), r{seed}, d(dd) {
        size[0] = rec.size[0];
        size[1] = rec.size[1];
        mcount[0] = rec.counts[0];
        mcount[1] = rec.counts[1];
        scount[0] = rec.counts[2];
        scount[1] = rec.counts[3];
    }

    uint64_t AnyOcc() { return rec.occs.empty() ? 0 : rec.occs[r.Next() % rec.occs.size()]; }

    // A position on axis a: near an occupant's box edge or centre, or anywhere
    // in and around the city (never far outside the grid: §5.7).
    void Point(float out[2]) {
        const uint64_t t = AnyOcc();
        if (t != 0 && r.Chance(3, 4)) {
            fc4_occ_bbox b{};
            HBox(nullptr, t, &b);
            static const float kJitter[] = {0.0f, 0.05f, -0.05f, 0.1f, -0.1f, 0.5f, -3.0f, 7.25f};
            for (int a = 0; a < 2; ++a) {
                const int k = a == 0 ? 0 : 2;
                const uint32_t pick = r.Next() % 3;
                float v = pick == 0 ? b.min[k] : pick == 1 ? b.max[k] : (b.min[k] + b.max[k]) * 0.5f;
                v += kJitter[r.Next() % 8];
                out[a] = Clamp(v, a);
            }
            return;
        }
        for (int a = 0; a < 2; ++a) {
            out[a] = static_cast<float>(r.Range(-640, static_cast<int32_t>((size[a] + 80.0f) * 8.0f))) / 8.0f;
        }
    }
    float Clamp(float v, int a) const {
        const float lo = -80.0f, hi = size[a] + 80.0f;
        return v < lo ? lo : (v > hi ? hi : v);
    }
    static int32_t Floor(float v, float unit) {
        const float q = v / unit;
        int32_t i = static_cast<int32_t>(q);
        if (static_cast<float>(i) > q) --i;
        return i;
    }
    void WorldCell(int32_t out[2]) {
        float p[2];
        Point(p);
        out[0] = Floor(p[0], 64.0f) + 64 + (r.Chance(1, 6) ? r.Range(-1, 1) : 0);
        out[1] = Floor(p[1], 64.0f) + 64 + (r.Chance(1, 6) ? r.Range(-1, 1) : 0);
    }
    void StdCell(int32_t out[2]) {
        float p[2];
        Point(p);
        out[0] = Floor(p[0], 16.0f) + (r.Chance(1, 6) ? r.Range(-2, 2) : 0);
        out[1] = Floor(p[1], 16.0f) + (r.Chance(1, 6) ? r.Range(-2, 2) : 0);
        for (int a = 0; a < 2; ++a) {
            if (out[a] < -2) out[a] = -2;
            if (out[a] > scount[a] + 1) out[a] = scount[a] + 1;
        }
    }
    // The extent of a rectangle in cells (mostly small, sometimes large).
    int32_t Extent(int32_t big) { return r.Chance(1, 3) ? 0 : r.Chance(5, 6) ? r.Range(1, 4) : r.Range(5, big); }
    // A filter: null or ours (with a fresh salt). `ours` = the core's view.
    cISC4OccupantFilter* Filter(fc4_occ_filter& ours, const fc4_occ_filter*& oursPtr) {
        ours = fc4_occ_filter{};
        if (r.Chance(1, 2)) {
            oursPtr = nullptr;
            return nullptr;
        }
        SampleFilter& f = TheFilter();
        f.salt = r.Next();
        f.keep = 1u + r.Next() % 7u;
        ours.kind = FC4_OCC_FILTER_HOST;
        ours.token = TokenOf(&f);
        oursPtr = &ours;
        return &f;
    }
    uint32_t Type() {
        const uint32_t k = r.Next() % 10;
        if (k == 0) return 0;
        if (k == 1) return 0x5EED0CC5u;  // no occupant has it
        const uint64_t t = AnyOcc();
        return t != 0 ? HType(nullptr, t) : 0u;
    }
    uint32_t MaxCount() {
        static const uint32_t kCounts[] = {0xFFFFFFFFu, 0xFFFFFFFFu, 0xFFFFFFFFu, 1u, 2u, 7u, 0u};
        return kCounts[r.Next() % 7];
    }
    uint32_t StopAfter() {
        static const uint32_t kStops[] = {0u, 0u, 0u, 1u, 2u, 5u};
        return kStops[r.Next() % 6];
    }

    void Judge(int slot, const Result& orig, const Result& ours, const char* args) {
        ++month[slot][1];
        ++g_slotTotal[slot];
        if (orig == ours) {
            ++month[slot][0];
            ++g_slotExact[slot];
            return;
        }
        d.Add("s%d(%s) ret %d/%d n %u/%u%s", slot, args, orig.ret ? 1 : 0, ours.ret ? 1 : 0,
              static_cast<unsigned>(orig.seq.size()), static_cast<unsigned>(ours.seq.size()),
              orig.x != ours.x || orig.z != ours.z ? " xz" : "");
    }

    // ---- one sample of each slot ----

    void S25() {
        const int32_t x = r.Range(-2, 193), z = r.Range(-2, 193);
        Result o, u;
        using F = uint32_t(__fastcall*)(const void*, void*, int32_t, int32_t);
        if (!Guarded("occmgr shadow slot 25", [&] { o.ret = (VSlot<F>(g_orig, 25)(g_orig, nullptr, x, z) & 0xFF) != 0; })) {
            fault = true;
            return;
        }
        u.ret = fc4_occmgr_is_cell_empty(g_qcore, x, z);
        char a[48];
        snprintf(a, sizeof(a), "%d,%d", x, z);
        Judge(25, o, u, a);
    }

    void S27() {
        const uint64_t t = AnyOcc();
        if (t == 0) return;
        const bool all = r.Chance(1, 2);
        Result o, u;
        o.x = o.z = u.x = u.z = -12345;
        using F = uint32_t(__fastcall*)(const void*, void*, cISC4Occupant*, bool, int32_t*, int32_t*);
        if (!Guarded("occmgr shadow slot 27", [&] {
                o.ret = (VSlot<F>(g_orig, 27)(g_orig, nullptr, Occ(t), all, &o.x, &o.z) & 0xFF) != 0;
            })) {
            fault = true;
            return;
        }
        u.ret = fc4_occmgr_find_occupant(g_qcore, t, all, &u.x, &u.z);
        Judge(27, o, u, all ? "all" : "guess");
    }

    // Slots 28-35: one first-occupant query; `typed` = the odd slot.
    void First(int slot) {
        const bool typed = (slot & 1) != 0;
        const int base = slot & ~1;
        fc4_occ_filter of{};
        const fc4_occ_filter* ofp = nullptr;
        cISC4OccupantFilter* f = nullptr;
        uint32_t type = 0;
        if (typed) {
            type = Type();
        } else {
            f = Filter(of, ofp);
        }
        cISC4Occupant* out = nullptr;
        uint64_t mine = 0;
        Result o, u;
        char a[96];
        bool ok = true;
        if (base == 28) {
            float p[2];
            Point(p);
            snprintf(a, sizeof(a), "%.9g,%.9g", static_cast<double>(p[0]), static_cast<double>(p[1]));
            using FF = uint32_t(__fastcall*)(const void*, void*, cISC4Occupant**, float, float, cISC4OccupantFilter*);
            using FT = uint32_t(__fastcall*)(const void*, void*, cISC4Occupant**, float, float, uint32_t);
            ok = Guarded("occmgr shadow slot 28/29", [&] {
                o.ret = ((typed ? VSlot<FT>(g_orig, slot)(g_orig, nullptr, &out, p[0], p[1], type)
                               : VSlot<FF>(g_orig, slot)(g_orig, nullptr, &out, p[0], p[1], f)) &
                         0xFFu) != 0;
            });
            u.ret = typed ? fc4_occmgr_first_by_position_typed(g_qcore, p[0], p[1], type, &mine)
                          : fc4_occmgr_first_by_position(g_qcore, p[0], p[1], ofp, &mine);
        } else if (base == 30 || base == 32) {
            int32_t c[2];
            if (base == 30) {
                WorldCell(c);
            } else {
                StdCell(c);
            }
            snprintf(a, sizeof(a), "%d,%d", c[0], c[1]);
            using FF = uint32_t(__fastcall*)(const void*, void*, cISC4Occupant**, int32_t, int32_t,
                                             cISC4OccupantFilter*);
            using FT = uint32_t(__fastcall*)(const void*, void*, cISC4Occupant**, int32_t, int32_t, uint32_t);
            ok = Guarded("occmgr shadow slots 30-33", [&] {
                o.ret = ((typed ? VSlot<FT>(g_orig, slot)(g_orig, nullptr, &out, c[0], c[1], type)
                               : VSlot<FF>(g_orig, slot)(g_orig, nullptr, &out, c[0], c[1], f)) &
                         0xFFu) != 0;
            });
            if (base == 30) {
                u.ret = typed ? fc4_occmgr_first_in_cell_typed(g_qcore, c[0], c[1], type, &mine)
                              : fc4_occmgr_first_in_cell(g_qcore, c[0], c[1], ofp, &mine);
            } else {
                u.ret = typed ? fc4_occmgr_first_by_standard_cell_typed(g_qcore, c[0], c[1], type, &mine)
                              : fc4_occmgr_first_by_standard_cell(g_qcore, c[0], c[1], ofp, &mine);
            }
        } else {  // 34, 35
            int32_t mn[2], mx[2];
            StdCell(mn);
            const int32_t ex = Extent(scount[0]), ez = Extent(scount[1]);
            mx[0] = mn[0] + ex;
            mx[1] = mn[1] + ez;
            const int32_t* pmn = r.Chance(1, 8) ? nullptr : mn;
            const int32_t* pmx = r.Chance(1, 8) ? nullptr : mx;
            snprintf(a, sizeof(a), "%s%d,%d..%s%d,%d", pmn ? "" : "null:", mn[0], mn[1], pmx ? "" : "null:", mx[0],
                     mx[1]);
            using FF = uint32_t(__fastcall*)(const void*, void*, cISC4Occupant**, const int32_t*, const int32_t*,
                                             cISC4OccupantFilter*);
            using FT = uint32_t(__fastcall*)(const void*, void*, cISC4Occupant**, const int32_t*, const int32_t*,
                                             uint32_t);
            ok = Guarded("occmgr shadow slots 34/35", [&] {
                o.ret = ((typed ? VSlot<FT>(g_orig, slot)(g_orig, nullptr, &out, pmn, pmx, type)
                               : VSlot<FF>(g_orig, slot)(g_orig, nullptr, &out, pmn, pmx, f)) &
                         0xFFu) != 0;
            });
            u.ret = typed ? fc4_occmgr_first_by_standard_cells_typed(g_qcore, pmn, pmx, type, &mine)
                          : fc4_occmgr_first_by_standard_cells(g_qcore, pmn, pmx, ofp, &mine);
        }
        if (!ok) {
            fault = true;
            return;
        }
        if (out != nullptr) {
            FC4_ACQUIRED(out, "occupant manager first-occupant query");  // §7: one reference for the caller
            o.seq.push_back(TokenOf(out));
            FC4_RELEASE(out);
        }
        if (mine != 0) {
            u.seq.push_back(mine);
            fc4_occmgr_release_list(g_qcore, &mine, 1);
        }
        if (typed) {
            char t[24];
            snprintf(t, sizeof(t), " t%08X", type);
            Append(a, sizeof(a), t);
        } else if (f != nullptr) {
            Append(a, sizeof(a), " f");
        }
        Judge(slot, o, u, a);
    }

    // Slots 36-41: one list query.
    void List(int slot) {
        const bool typed = (slot & 1) != 0;
        const int base = slot & ~1;
        fc4_occ_filter of{};
        const fc4_occ_filter* ofp = nullptr;
        cISC4OccupantFilter* f = nullptr;
        uint32_t type = 0;
        if (typed) {
            type = Type();
        } else {
            f = Filter(of, ofp);
        }
        const uint32_t maxCount = MaxCount();
        Result o, u;
        std::vector<uint64_t> mine;
        char a[128];
        bool ok = true;
        SC4List<cISC4Occupant*> list;
        if (base == 36) {
            float p[2];
            Point(p);
            float mn[3] = {p[0], static_cast<float>(r.Range(-100, 20)), p[1]};
            float mx[3] = {p[0] + static_cast<float>(Extent(40) * 16) + (r.Chance(1, 2) ? 0.5f : 0.0f),
                           static_cast<float>(r.Range(-20, 200)), p[1] + static_cast<float>(Extent(40) * 16)};
            const float* pmn = r.Chance(1, 10) ? nullptr : mn;
            const float* pmx = r.Chance(1, 10) ? nullptr : mx;
            snprintf(a, sizeof(a), "%s%.9g,%.9g,%.9g..%s%.9g,%.9g,%.9g", pmn ? "" : "null:", static_cast<double>(mn[0]),
                     static_cast<double>(mn[1]), static_cast<double>(mn[2]), pmx ? "" : "null:",
                     static_cast<double>(mx[0]), static_cast<double>(mx[1]), static_cast<double>(mx[2]));
            using FF = uint32_t(__fastcall*)(const void*, void*, SC4List<cISC4Occupant*>*, const float*, const float*,
                                             cISC4OccupantFilter*, uint32_t);
            using FT = uint32_t(__fastcall*)(const void*, void*, SC4List<cISC4Occupant*>*, const float*, const float*,
                                             uint32_t, uint32_t);
            ok = Guarded("occmgr shadow slots 36/37", [&] {
                o.ret = ((typed ? VSlot<FT>(g_orig, slot)(g_orig, nullptr, &list, pmn, pmx, type, maxCount)
                               : VSlot<FF>(g_orig, slot)(g_orig, nullptr, &list, pmn, pmx, f, maxCount)) &
                         0xFFu) != 0;
            });
            const int32_t mc = static_cast<int32_t>(maxCount);
            u.ret = typed ? fc4_occmgr_occupants_by_bbox_typed(g_qcore, pmn, pmx, type, mc, &OurSink, &mine)
                          : fc4_occmgr_occupants_by_bbox(g_qcore, pmn, pmx, ofp, mc, &OurSink, &mine);
        } else {  // 38-41: cells
            int32_t mn[2], mx[2];
            if (base == 38) {
                WorldCell(mn);
                mx[0] = mn[0] + Extent(mcount[0]);
                mx[1] = mn[1] + Extent(mcount[1]);
            } else {
                StdCell(mn);
                mx[0] = mn[0] + Extent(scount[0]);
                mx[1] = mn[1] + Extent(scount[1]);
            }
            const int32_t* pmn = r.Chance(1, 10) ? nullptr : mn;
            const int32_t* pmx = r.Chance(1, 10) ? nullptr : mx;
            snprintf(a, sizeof(a), "%s%d,%d..%s%d,%d", pmn ? "" : "null:", mn[0], mn[1], pmx ? "" : "null:", mx[0],
                     mx[1]);
            using FF = uint32_t(__fastcall*)(const void*, void*, SC4List<cISC4Occupant*>*, const int32_t*,
                                             const int32_t*, cISC4OccupantFilter*, uint32_t);
            using FT = uint32_t(__fastcall*)(const void*, void*, SC4List<cISC4Occupant*>*, const int32_t*,
                                             const int32_t*, uint32_t, uint32_t);
            ok = Guarded("occmgr shadow slots 38-41", [&] {
                o.ret = ((typed ? VSlot<FT>(g_orig, slot)(g_orig, nullptr, &list, pmn, pmx, type, maxCount)
                               : VSlot<FF>(g_orig, slot)(g_orig, nullptr, &list, pmn, pmx, f, maxCount)) &
                         0xFFu) != 0;
            });
            const int32_t mc = static_cast<int32_t>(maxCount);
            if (base == 38) {
                u.ret = typed ? fc4_occmgr_occupants_by_cells_typed(g_qcore, pmn, pmx, type, mc, &OurSink, &mine)
                              : fc4_occmgr_occupants_by_cells(g_qcore, pmn, pmx, ofp, mc, &OurSink, &mine);
            } else {
                u.ret = typed
                            ? fc4_occmgr_occupants_by_standard_cells_typed(g_qcore, pmn, pmx, type, mc, &OurSink, &mine)
                            : fc4_occmgr_occupants_by_standard_cells(g_qcore, pmn, pmx, ofp, mc, &OurSink, &mine);
            }
        }
        if (!ok) {
            fault = true;
            return;
        }
        o.seq = OrigList(list);
        u.seq = mine;
        ReleaseOurs(mine);
        char t[40];
        snprintf(t, sizeof(t), " max%d%s", static_cast<int32_t>(maxCount), typed ? "" : (f != nullptr ? " f" : ""));
        Append(a, sizeof(a), t);
        if (typed) {
            snprintf(t, sizeof(t), " t%08X", type);
            Append(a, sizeof(a), t);
        }
        Judge(slot, o, u, a);
    }

    // Slots 42-47: one iteration. Slot 46 is typed and 47 takes the filter
    // (§9.4); every other pair has the filter overload first.
    void Iterate(int slot) {
        const bool typed = slot == 46 ? true : slot == 47 ? false : (slot & 1) != 0;
        fc4_occ_filter of{};
        const fc4_occ_filter* ofp = nullptr;
        cISC4OccupantFilter* f = nullptr;
        uint32_t type = 0;
        if (typed) {
            type = Type();
        } else {
            f = Filter(of, ofp);
        }
        IterRec ro, ru;
        ro.stopAfter = ru.stopAfter = StopAfter();
        Result o, u;
        char a[128];
        bool ok = true;
        using IterFn = bool(__cdecl*)(cISC4Occupant*, void*);
        const IterFn fn = &OrigIter;
        if (slot == 42 || slot == 43) {
            float p[2];
            Point(p);
            float mn[3] = {p[0], static_cast<float>(r.Range(-100, 20)), p[1]};
            float mx[3] = {p[0] + static_cast<float>(Extent(40) * 16) + (r.Chance(1, 2) ? 0.25f : 0.0f),
                           static_cast<float>(r.Range(-20, 200)), p[1] + static_cast<float>(Extent(40) * 16)};
            snprintf(a, sizeof(a), "%.9g,%.9g,%.9g..%.9g,%.9g,%.9g", static_cast<double>(mn[0]),
                     static_cast<double>(mn[1]), static_cast<double>(mn[2]), static_cast<double>(mx[0]),
                     static_cast<double>(mx[1]), static_cast<double>(mx[2]));
            using FF = uint32_t(__fastcall*)(const void*, void*, IterFn, void*, const float*, const float*,
                                             cISC4OccupantFilter*);
            using FT = uint32_t(__fastcall*)(const void*, void*, IterFn, void*, const float*, const float*, uint32_t);
            ok = Guarded("occmgr shadow slots 42/43", [&] {
                o.ret = ((typed ? VSlot<FT>(g_orig, slot)(g_orig, nullptr, fn, &ro, mn, mx, type)
                               : VSlot<FF>(g_orig, slot)(g_orig, nullptr, fn, &ro, mn, mx, f)) &
                         0xFFu) != 0;
            });
            u.ret = typed ? fc4_occmgr_iterate_by_bbox_typed(g_qcore, &OurIter, &ru, mn, mx, type)
                          : fc4_occmgr_iterate_by_bbox(g_qcore, &OurIter, &ru, mn, mx, ofp);
        } else if (slot == 44 || slot == 45) {
            int32_t c[2];
            WorldCell(c);
            int32_t mn[3] = {c[0], r.Range(-5, 5), c[1]};
            int32_t mx[3] = {c[0] + Extent(mcount[0]), r.Range(-5, 5), c[1] + Extent(mcount[1])};
            const int32_t* pmn = r.Chance(1, 12) ? nullptr : mn;
            const int32_t* pmx = r.Chance(1, 12) ? nullptr : mx;
            snprintf(a, sizeof(a), "%s%d,%d..%s%d,%d", pmn ? "" : "null:", mn[0], mn[2], pmx ? "" : "null:", mx[0],
                     mx[2]);
            using FF = uint32_t(__fastcall*)(const void*, void*, IterFn, void*, const int32_t*, const int32_t*,
                                             cISC4OccupantFilter*);
            using FT = uint32_t(__fastcall*)(const void*, void*, IterFn, void*, const int32_t*, const int32_t*,
                                             uint32_t);
            ok = Guarded("occmgr shadow slots 44/45", [&] {
                o.ret = ((typed ? VSlot<FT>(g_orig, slot)(g_orig, nullptr, fn, &ro, pmn, pmx, type)
                               : VSlot<FF>(g_orig, slot)(g_orig, nullptr, fn, &ro, pmn, pmx, f)) &
                         0xFFu) != 0;
            });
            u.ret = typed ? fc4_occmgr_iterate_typed(g_qcore, &OurIter, &ru, pmn, pmx, type)
                          : fc4_occmgr_iterate(g_qcore, &OurIter, &ru, pmn, pmx, ofp);
        } else {  // 46 typed, 47 filter
            int32_t mn[2], mx[2];
            StdCell(mn);
            mx[0] = mn[0] + Extent(scount[0]);
            mx[1] = mn[1] + Extent(scount[1]);
            const int32_t* pmn = r.Chance(1, 12) ? nullptr : mn;
            const int32_t* pmx = r.Chance(1, 12) ? nullptr : mx;
            snprintf(a, sizeof(a), "%s%d,%d..%s%d,%d", pmn ? "" : "null:", mn[0], mn[1], pmx ? "" : "null:", mx[0],
                     mx[1]);
            using FF = uint32_t(__fastcall*)(const void*, void*, IterFn, void*, const int32_t*, const int32_t*,
                                             cISC4OccupantFilter*);
            using FT = uint32_t(__fastcall*)(const void*, void*, IterFn, void*, const int32_t*, const int32_t*,
                                             uint32_t);
            ok = Guarded("occmgr shadow slots 46/47", [&] {
                o.ret = ((typed ? VSlot<FT>(g_orig, slot)(g_orig, nullptr, fn, &ro, pmn, pmx, type)
                               : VSlot<FF>(g_orig, slot)(g_orig, nullptr, fn, &ro, pmn, pmx, f)) &
                         0xFFu) != 0;
            });
            u.ret = typed ? fc4_occmgr_iterate_by_standard_cells_typed(g_qcore, &OurIter, &ru, pmn, pmx, type)
                          : fc4_occmgr_iterate_by_standard_cells(g_qcore, &OurIter, &ru, pmn, pmx, ofp);
        }
        if (!ok) {
            fault = true;
            return;
        }
        o.seq = ro.seq;
        u.seq = ru.seq;
        char t[40];
        snprintf(t, sizeof(t), " stop%u%s", ro.stopAfter, typed ? "" : (f != nullptr ? " f" : ""));
        Append(a, sizeof(a), t);
        if (typed) {
            snprintf(t, sizeof(t), " t%08X", type);
            Append(a, sizeof(a), t);
        }
        Judge(slot, o, u, a);
    }

    void Run(int samples) {
        for (int i = 0; i < samples && !fault; ++i) {
            for (int slot = kFirstSlot; slot <= kLastSlot && !fault; ++slot) {
                if (slot == 26) continue;  // GetBoundingCells: no grid (a pure conversion)
                if (slot == 25) {
                    S25();
                } else if (slot == 27) {
                    S27();
                } else if (slot <= 35) {
                    First(slot);
                } else if (slot <= 41) {
                    List(slot);
                } else {
                    Iterate(slot);
                }
            }
        }
    }
};

// ---- the month ------------------------------------------------------------------------------------

struct Month {
    cISC4OccupantManager* orig = nullptr;
    int32_t julian = 0;
    const char* followed = "yes";
    const char* incomplete = nullptr;
    Record rec;
    Diff d;
    uint32_t silentMoves = 0, movedBoxDiffer = 0;
    uint32_t cellsDiffer = 0, setAside = 0, headerDiffer = 0;
    std::string queries;
    uint32_t qExact = 0, qTotal = 0;
    const char* queriesOn = "followed";
    uint32_t strayFresh = 0;
    bool sampled = false;
};

// Occupants whose stored box changed without a Move message: moved in the
// core as MoveOccupant(occ, false) would.
void SyncSilentMoves(Month& mo) {
    std::vector<uint64_t> moved;
    for (const auto& e : g_fol.state) {
        if ((e.second.flags & kFlagInserted) == 0) continue;
        uint8_t real[4];
        const bool has = RealBox(e.first, real);
        if (!RealFlag(e.first)) continue;  // its Remove is still queued
        if (has != e.second.hasBox || (has && std::memcmp(real, e.second.box, 4) != 0)) moved.push_back(e.first);
    }
    for (uint64_t t : moved) {
        fc4_occmgr_move(g_fol.core, t, false);
        ++mo.silentMoves;
        uint8_t real[4];
        const bool has = RealBox(t, real);
        const OccState& s = g_fol.state[t];
        if (has != s.hasBox || (has && std::memcmp(real, s.box, 4) != 0)) {
            ++mo.movedBoxDiffer;
            mo.d.Add("silent move of %p: box orig %u,%u,%u,%u ours %u,%u,%u,%u", static_cast<void*>(Occ(t)), real[0],
                     real[1], real[2], real[3], s.box[0], s.box[1], s.box[2], s.box[3]);
        }
    }
}

// The original's cells against the core's, in list order. An occupant only
// in the original's cells whose Insert message the core has not had yet
// (flag 1 set on the occupant, not in the core's state), or only in the
// core's whose Remove message is still queued (flag 1 clear on the
// occupant), is set aside.
void CompareRecord(Month& mo) {
    const Record& rec = mo.rec;
    float sx = 0.0f, sz = 0.0f;
    fc4_occmgr_get_city_size(g_fol.core, &sx, &sz);
    int32_t c[4] = {0, 0, 0, 0};
    fc4_occmgr_get_cell_count(g_fol.core, &c[0], &c[1]);
    fc4_occmgr_get_standard_cell_count(g_fol.core, &c[2], &c[3]);
    if (sx != rec.size[0] || sz != rec.size[1] || std::memcmp(c, rec.counts, sizeof(c)) != 0) {
        ++mo.headerDiffer;
        mo.d.Add("sizes/counts orig %g,%g %d,%d,%d,%d ours %g,%g %d,%d,%d,%d", static_cast<double>(rec.size[0]),
                 static_cast<double>(rec.size[1]), rec.counts[0], rec.counts[1], rec.counts[2], rec.counts[3],
                 static_cast<double>(sx), static_cast<double>(sz), c[0], c[1], c[2], c[3]);
    }
    std::unordered_map<uint64_t, bool> realFlag;
    auto real = [&](uint64_t t) {
        auto it = realFlag.find(t);
        if (it != realFlag.end()) return it->second;
        const bool f = RealFlag(t);
        realFlag.emplace(t, f);
        return f;
    };
    const size_t rows = rec.rowStart.size() - 1;
    for (size_t x = 0; x < rows; ++x) {
        const uint32_t cells = rec.rowStart[x + 1] - rec.rowStart[x];
        for (uint32_t z = 0; z < cells; ++z) {
            const uint32_t flat = rec.rowStart[x] + z;
            std::vector<uint64_t> o(rec.refs.begin() + rec.cellStart[flat], rec.refs.begin() + rec.cellStart[flat + 1]);
            std::vector<uint64_t> u = OurCell(static_cast<int32_t>(x), static_cast<int32_t>(z));
            if (o == u) continue;
            std::vector<uint64_t> o2, u2;
            for (uint64_t t : o) {
                if (!HIsFlagSet(&g_fol, t, kFlagInserted) && real(t)) {
                    ++mo.setAside;
                    continue;
                }
                o2.push_back(t);
            }
            for (uint64_t t : u) {
                if (!real(t)) {
                    ++mo.setAside;
                    continue;
                }
                u2.push_back(t);
            }
            if (o2 == u2) continue;
            ++mo.cellsDiffer;
            mo.d.Add("cell %u,%u orig %u ours %u", static_cast<unsigned>(x), z, static_cast<unsigned>(o.size()),
                     static_cast<unsigned>(u.size()));
        }
    }
    // The core's cells beyond the record's (none for a 192 x 192 record).
    if (rows < 192) {
        for (int32_t x = static_cast<int32_t>(rows); x < 192; ++x) {
            for (int32_t z = 0; z < 192; ++z) {
                if (!OurCell(x, z).empty()) {
                    ++mo.cellsDiffer;
                    mo.d.Add("cell %d,%d only ours", x, z);
                }
            }
        }
    }
}

void MonthStep(void* ctx) {
    Month& mo = *static_cast<Month*>(ctx);
    if (g_fol.core != nullptr && g_orig == mo.orig) SyncSilentMoves(mo);
    bool answered = false;
    const bool captured = g_capture.Capture(mo.orig, &answered);
    if (captured) mo.rec = Parse(g_capture);
    if (!mo.rec.ok) {
        mo.incomplete = captured ? "the original's record could not be parsed" : "the original's record could not be captured";
        DropCore(g_fol);
        return;
    }
    if (g_fol.core == nullptr || g_orig != mo.orig) {
        mo.followed = "first";
        if (!Restart(g_fol, mo.rec, mo.orig)) {
            mo.incomplete = "the shadow core could not read the original's record";
            return;
        }
    } else {
        CompareRecord(mo);
        // Only a difference the queued messages do not explain restarts the
        // followed core.
        if (mo.cellsDiffer != 0 || mo.headerDiffer != 0) {
            mo.followed = "resynced";
            ++g_resyncs;
            if (!Restart(g_fol, mo.rec, mo.orig)) {
                mo.incomplete = "the shadow core could not read the original's record";
                return;
            }
        }
    }
    // The queries run on the followed core when it holds exactly the
    // original's cells (nothing set aside); otherwise on a fresh core read
    // from the record.
    g_qcore = g_fol.core;
    mo.queriesOn = "followed";
    if (mo.setAside != 0) {
        if (!Restart(g_fresh, mo.rec, mo.orig)) {
            mo.incomplete = "the fresh core could not read the original's record";
            return;
        }
        g_qcore = g_fresh.core;
        mo.queriesOn = "fresh";
    }
    Sampler s(mo.rec, 0x0CC0FFEEu ^ static_cast<uint32_t>(mo.julian) * 2654435761u, mo.d);
    s.Run(g_samples);
    g_qcore = nullptr;
    mo.strayFresh = g_fresh.stray;
    DropCore(g_fresh);
    g_fresh.stray = 0;
    if (s.fault) {
        mo.incomplete = "a fault in a sampled call of the original";
        return;
    }
    mo.sampled = true;
    for (int slot = kFirstSlot; slot <= kLastSlot; ++slot) {
        if (s.month[slot][1] == 0) continue;
        mo.qExact += s.month[slot][0];
        mo.qTotal += s.month[slot][1];
        if (s.month[slot][0] != s.month[slot][1]) {
            mo.queries += " s" + std::to_string(slot) + "=" + std::to_string(s.month[slot][0]) + "/" +
                          std::to_string(s.month[slot][1]);
        }
    }
}

// The message-order comparison (§5.5), defined with the message handlers below.
void CompareMessages();

void LogMonth(const Month& mo) {
    if (mo.incomplete != nullptr) {
        Log("occmgr shadow month %s: INCOMPLETE (%s); not compared", DateText(mo.julian).c_str(), mo.incomplete);
        return;
    }
    ++g_months;
    CompareMessages();
    const uint32_t stray = g_fol.stray + mo.strayFresh;
    const uint32_t msgDiffer = g_follow.msgTotal - g_follow.msgExact;
    const uint32_t mismatches = mo.d.mismatches + (g_follow.boxTotal - g_follow.boxExact) + stray + msgDiffer +
                                g_follow.destroyOther;
    if (mismatches == 0) ++g_cleanMonths;
    char rec[96];
    if (std::strcmp(mo.followed, "first") == 0) {
        snprintf(rec, sizeof(rec), "record=read");
    } else if (mo.cellsDiffer == 0 && mo.headerDiffer == 0) {
        snprintf(rec, sizeof(rec), "record=exact (%u set aside)", mo.setAside);
    } else {
        snprintf(rec, sizeof(rec), "record=%u cells differ (%u set aside)", mo.cellsDiffer, mo.setAside);
    }
    char head[512];
    snprintf(head, sizeof(head),
             "occmgr shadow month %s: followed=%s occupants=%u refs=%u %s; silent moves=%u; messages "
             "ins/rem/move=%u/%u/%u refused=%u off-thread=%u boxes=%u/%u; queries on %s core %u/%u%s",
             DateText(mo.julian).c_str(), mo.followed, static_cast<unsigned>(mo.rec.occs.size()),
             static_cast<unsigned>(mo.rec.refs.size()), rec, mo.silentMoves, g_follow.ins, g_follow.rem, g_follow.mov,
             g_follow.refused, g_follow.offThread, g_follow.boxExact, g_follow.boxTotal, mo.queriesOn, mo.qExact,
             mo.qTotal, mo.queries.empty() ? " (every slot exact)" : "");
    g_fol.stray = 0;
    char msgPart[192];
    snprintf(msgPart, sizeof(msgPart), "; msgorder=%u/%u%s destroy=%u/%u/%u", g_follow.msgExact, g_follow.msgTotal,
             msgDiffer == 0 ? " (exact)" : "", g_follow.destroyOk, g_follow.destroyOther, g_follow.destroyElse);
    std::string line = std::string(head) + (mo.queries.empty() ? "" : " differ:" + mo.queries) + msgPart + "; " +
                       ProbeText(false) + "; stray releases=" + std::to_string(stray) +
                       "; mismatches=" + std::to_string(mismatches) +
                       (mo.d.listed.empty() && g_follow.boxDiffs.empty() && g_follow.msgDiffs.empty() &&
                                g_follow.rmDiffs.empty()
                            ? std::string()
                            : "; differences: " + mo.d.listed + (g_follow.boxDiffs.empty() ? "" : " " + g_follow.boxDiffs) +
                                  (g_follow.msgDiffs.empty() ? "" : " " + g_follow.msgDiffs) +
                                  (g_follow.rmDiffs.empty() ? "" : " " + g_follow.rmDiffs));
    LogShadowLine("occmgr shadow (cont.)", reinterpret_cast<const uint8_t*>(line.data()), line.size());
    g_follow = Follow{};
}

// ---- the followed messages (§5.5) -----------------------------------------------------------------

void CompareBox(uint64_t occ, const char* what) {
    uint8_t real[4];
    const bool has = RealBox(occ, real);
    auto it = g_fol.state.find(occ);
    const bool ours = it != g_fol.state.end() && it->second.hasBox;
    ++g_follow.boxTotal;
    if (has == ours && (!has || std::memcmp(real, it->second.box, 4) == 0)) {
        ++g_follow.boxExact;
        return;
    }
    if (g_follow.boxListed++ < 4) {
        char b[160];
        const uint8_t* o = ours ? it->second.box : nullptr;
        snprintf(b, sizeof(b), "[%s box of %p orig %s%u,%u,%u,%u ours %s%u,%u,%u,%u]", what,
                 static_cast<void*>(Occ(occ)), has ? "" : "none:", real[0], real[1], real[2], real[3],
                 ours ? "" : "none:", o ? o[0] : 0u, o ? o[1] : 0u, o ? o[2] : 0u, o ? o[3] : 0u);
        g_follow.boxDiffs += b;
    }
}

struct Msg {
    uint32_t type;
    uint32_t data4;
    uint64_t occ;
};

void CallMessage(void* ctx) {
    const Msg* m = static_cast<const Msg*>(ctx);
    if (m->type == kMsgInserted) {
        ++g_follow.ins;
        if (fc4_occmgr_insert(g_fol.core, m->occ, m->data4)) {
            CompareBox(m->occ, "insert");
        } else {
            ++g_follow.refused;
        }
    } else if (m->type == kMsgRemoved) {
        ++g_follow.rem;
        if (!fc4_occmgr_remove(g_fol.core, m->occ, false, m->data4)) ++g_follow.refused;
    } else {
        ++g_follow.mov;
        // post = true: the original posted this Move, so the core must post one
        // too (§5.4 step 9, even when the box did not change). HPost drops it.
        if (fc4_occmgr_move(g_fol.core, m->occ, true)) {
            if (RealFlag(m->occ)) CompareBox(m->occ, "move");
        } else {
            ++g_follow.refused;
        }
    }
}

// The §5.3 step 4 interleaving: Remove(o1), Destroy(o1), Remove(o2),
// Destroy(o2), ... A Destroy that follows a Remove of another occupant is the
// batched the-wrong-way-round order and is counted. A Destroy after anything
// else (an occupant shut down outside the manager) is only counted.
void NoteDestroy(uint64_t occ) {
    auto it = g_follow.removed.find(occ);
    if (it == g_follow.removed.end()) {
        ++g_follow.destroyElse;  // shut down outside the manager
        return;
    }
    g_follow.removed.erase(it);
    const bool adjacent = !g_follow.origMsgs.empty() && g_follow.origMsgs.back().kind == kMsgRemoved &&
                          g_follow.origMsgs.back().occ == occ;
    if (adjacent) {
        ++g_follow.destroyOk;
        return;
    }
    ++g_follow.destroyOther;  // its own remove is outstanding, but not just before
    if (g_follow.rmListed++ < 4) {
        char b[160];
        const char* prev = g_follow.origMsgs.empty() ? "nothing" : MsgName(g_follow.origMsgs.back().kind);
        snprintf(b, sizeof(b), "[destroy of %p not after its own remove (prev %s of %p)]",
                 static_cast<void*>(Occ(occ)), prev,
                 static_cast<void*>(g_follow.origMsgs.empty() ? nullptr : Occ(g_follow.origMsgs.back().occ)));
        g_follow.rmDiffs += b;
    }
}

// The message-order comparison (§5.5): the manager's own messages, the
// original's against the followed core's, in order. A core that posts one
// message per original message and in the same order is exact.
void CompareMessages() {
    const size_t n = g_follow.origMsgs.size() < g_follow.coreMsgs.size() ? g_follow.origMsgs.size()
                                                                        : g_follow.coreMsgs.size();
    g_follow.msgTotal = static_cast<uint32_t>(g_follow.origMsgs.size() > g_follow.coreMsgs.size()
                                                  ? g_follow.origMsgs.size()
                                                  : g_follow.coreMsgs.size());
    for (size_t i = 0; i < n; ++i) {
        if (g_follow.origMsgs[i] == g_follow.coreMsgs[i]) {
            ++g_follow.msgExact;
            continue;
        }
        if (g_follow.msgListed++ < 4) {
            char b[176];
            snprintf(b, sizeof(b), "[msg %u orig %s of %p d4=%u core %s of %p d4=%u]", static_cast<unsigned>(i),
                     MsgName(g_follow.origMsgs[i].kind), static_cast<void*>(Occ(g_follow.origMsgs[i].occ)),
                     g_follow.origMsgs[i].data4, MsgName(g_follow.coreMsgs[i].kind),
                     static_cast<void*>(Occ(g_follow.coreMsgs[i].occ)), g_follow.coreMsgs[i].data4);
            g_follow.msgDiffs += b;
        }
    }
    // Trailing messages on the longer side are differences too.
    for (size_t i = n; i < g_follow.msgTotal; ++i) {
        const bool ours = i < g_follow.coreMsgs.size();
        const MsgRec& r = ours ? g_follow.coreMsgs[i] : g_follow.origMsgs[i];
        if (g_follow.msgListed++ < 4) {
            char b[176];
            snprintf(b, sizeof(b), "[msg %u %s only: %s of %p d4=%u]", static_cast<unsigned>(i),
                     ours ? "core" : "orig", MsgName(r.kind), static_cast<void*>(Occ(r.occ)), r.data4);
            g_follow.msgDiffs += b;
        }
    }
}

void OnMessage(cIGZMessage2* msg) {
    if (msg == nullptr) return;
    const uint32_t type = msg->GetType();
    const bool followed = type == kMsgInserted || type == kMsgRemoved || type == kMsgMoved;
    if (!followed && type != kMsgDestroyed) return;
    void* p = nullptr;
    if (!msg->QueryInterface(GZIID_cIGZMessage2Standard, &p) || p == nullptr) return;
    cIGZMessage2Standard* s = static_cast<cIGZMessage2Standard*>(p);
    FC4_ACQUIRED(s, "QueryInterface");
    {
        Lock lock;
        if (g_fol.core != nullptr) {
            Msg m{};
            m.type = type;
            m.data4 = static_cast<uint32_t>(s->GetData4());
            m.occ = Canonical(static_cast<cIGZUnknown*>(s->GetVoid1()));
            if (type == kMsgDestroyed) {
                NoteDestroy(m.occ);
            } else {
                g_follow.origMsgs.push_back(MsgRec{type, m.occ, m.data4});
                if (type == kMsgRemoved) {
                    g_follow.removed.insert(m.occ);
                } else if (m.occ != 0) {
                    g_follow.removed.erase(m.occ);  // live again (insert), or never left (move)
                }
                if (GetCurrentThreadId() != g_mainTid) ++g_follow.offThread;
                if (m.occ != 0) SehGuard("occmgr shadow (followed message)", &CallMessage, &m);
            }
        }
    }
    FC4_RELEASE(s);
}

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
        FC4_X87_SCOPE("occmgr shadow DoMessage", msg->GetType());  // x87 leftover guard (FC4Fpu.h)
        OnMessage(msg);
        return true;
    }

private:
    uint32_t refs_ = 0;
};

EventTarget& Events() {
    static EventTarget t;
    return t;
}

// Registered once, at the first city load, and left registered for the process.
void Listen() {
    if (g_listening) return;
    g_listening = true;
    cIGZMessageServer2Ptr ms;
    if (!ms) {
        Log("occmgr shadow: no message server; occupant messages do not reach the shadow core");
        return;
    }
    Events().AddRef();  // our own reference (static object)
    bool ok = true;
    // Destroy is subscribed for the §5.3 interleaving check only: it is
    // counted, never replayed onto the core.
    for (uint32_t id : {kMsgDestroyed, kMsgInserted, kMsgRemoved, kMsgMoved}) {
        ok = ms->AddNotification(static_cast<cIGZMessageTarget2*>(&Events()), id) && ok;
    }
    if (!ok) Log("occmgr shadow: a notification was refused; the shadow core can miss messages");
}

// After a fault: the cores are dropped without their references (they leak).
void Abandon() {
    for (CoreCtx* c : {&g_fol, &g_fresh}) {
        c->core = nullptr;
        c->holds.clear();
        c->state.clear();
    }
    g_qcore = nullptr;
    g_orig = nullptr;
}

void LoadStep(void* ctx) {
    Month& mo = *static_cast<Month*>(ctx);
    bool answered = false;
    const bool captured = g_capture.Capture(mo.orig, &answered);
    if (captured) mo.rec = Parse(g_capture);
    if (!mo.rec.ok) {
        mo.incomplete = captured ? "the original's record could not be parsed" : "the original's record could not be captured";
        return;
    }
    if (!Restart(g_fol, mo.rec, mo.orig)) mo.incomplete = "the shadow core could not read the original's record";
}

}  // namespace

void OccMgrShadowStartup() {
    g_enabled = SettingInt("occmgr_shadow", "FC4_OCCMGR_SHADOW", 0) != 0;
    if (!g_enabled) return;
    long n = SettingInt("occmgr_shadow_samples", "FC4_OCCMGR_SHADOW_SAMPLES", 4);
    g_samples = n < 1 ? 1 : (n > 64 ? 64 : static_cast<int>(n));
    g_probeWanted = SettingInt("occmgr_shadow_probe", "FC4_OCCMGR_SHADOW_PROBE", 1) != 0;
    if (!g_csReady) {
        InitializeCriticalSection(&g_cs);
        g_csReady = true;
    }
    const char* src = "";
    Setting("occmgr_shadow", "FC4_OCCMGR_SHADOW", &src);
    Log("occmgr shadow: ON (occmgr_shadow from %s): a shadow core (read-only host) starts from the ORIGINAL occupant "
        "manager's record at each city load and follows its Insert / Remove / Move messages (the core's box compared "
        "with the occupant's after each); at each SimNewMonth the record is compared cell by cell and %d samples of "
        "each of slots 25, 27 and 28-47 run on both (occupant-manager.md); probe %s",
        src, g_samples, g_probeWanted ? "on (slots 1, 2, 12, 20-22, 24, 25, 27, 28-40 even, 42, 44: calls per thread)" : "off");
}

void OccMgrShadowCityLoaded(cISC4City* city) {
    if (!g_enabled) return;
    cISC4OccupantManager* orig = OriginalOf(city);
    if (orig == nullptr) {
        Log("occmgr shadow: no original occupant manager at the city load; nothing followed");
        return;
    }
    g_mainTid = GetCurrentThreadId();
    Listen();
    Month mo;
    mo.orig = orig;
    {
        Lock lock;
        g_follow = Follow{};
        if (!SehGuard("occmgr shadow (city load)", &LoadStep, &mo)) {
            Log("occmgr shadow: a fault at the city load; the shadow core is abandoned (its references leak)");
            Abandon();
            return;
        }
    }
    if (mo.incomplete != nullptr) {
        Log("occmgr shadow: city load: %s; the shadow starts at the first month", mo.incomplete);
    } else {
        fc4_occmgr_state st{};
        st.struct_size = sizeof(st);
        fc4_occmgr_get_state(g_fol.core, &st);
        Log("occmgr shadow: following the original occupant manager %p: record %u bytes, %u occupants, %u references; "
            "core size %gx%g counts %d,%d,%d,%d holders=%u nodes=%u",
            static_cast<void*>(orig), static_cast<unsigned>(g_capture.Bytes().size()),
            static_cast<unsigned>(mo.rec.occs.size()), static_cast<unsigned>(mo.rec.refs.size()),
            static_cast<double>(st.size_x), static_cast<double>(st.size_z), st.counts[0], st.counts[1], st.counts[2],
            st.counts[3], st.holders, st.nodes);
    }
    InstallProbe(orig);
}

void OccMgrShadowMonth(cISC4City* city, int32_t julian) {
    if (!g_enabled) return;
    cISC4OccupantManager* orig = OriginalOf(city);
    if (orig == nullptr) return;
    if (g_mainTid == 0) g_mainTid = GetCurrentThreadId();
    Listen();
    Month mo;
    mo.orig = orig;
    mo.julian = julian;
    Lock lock;
    if (!SehGuard("occmgr shadow (month)", &MonthStep, &mo)) {
        Log("occmgr shadow: a fault in the month; the shadow core is abandoned (its references leak)");
        Abandon();
        mo.incomplete = "a fault in the month";
    }
    LogMonth(mo);
}

void OccMgrShadowCityShutdown() {
    if (!g_enabled) return;
    RemoveProbe();
    Lock lock;
    if (g_months > 0 || g_fol.core != nullptr) {
        std::string slots;
        for (int slot = kFirstSlot; slot <= kLastSlot; ++slot) {
            if (g_slotTotal[slot] == 0) continue;
            slots += " " + std::to_string(slot) + "=" + std::to_string(g_slotExact[slot]) + "/" +
                     std::to_string(g_slotTotal[slot]);
        }
        const std::string line = "occmgr shadow: city shutdown: " + std::to_string(g_months) + " month lines, " +
                                 std::to_string(g_cleanMonths) + " with mismatches=0; resyncs=" +
                                 std::to_string(g_resyncs) + "; per slot exact/total:" + slots + "; " +
                                 ProbeText(true);
        LogShadowLine("occmgr shadow (cont.)", reinterpret_cast<const uint8_t*>(line.data()), line.size());
    }
    DropCore(g_fresh);
    DropCore(g_fol);
}

}  // namespace fc4
