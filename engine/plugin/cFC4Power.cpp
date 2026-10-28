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

#include "cFC4Power.h"

#include "FC4Config.h"
#include "FC4Checked.h"
#include "FC4CoreProps.h"
#include "FC4Diag.h"
#include "FC4Fpu.h"
#include "FC4GameList.h"
#include "FC4Interfaces.h"
#include "FC4Log.h"
#include "FC4Properties.h"
#include "FC4RefAudit.h"
#include "FC4Streams.h"
#include "cFC4Message2Standard.h"
#include "compat/FC4Compat.h"
#include "cFC4PowerPoleOccupant.h"

#include "GZServPtrs.h"
#include "SC4List.h"
#include "SC4Point.h"
#include "SC4Rect.h"
#include "cIGZApp.h"
#include "cIGZFrameWork.h"
#include "cIGZMessage2Standard.h"
#include "cIGZMessageServer2.h"
#include "cISC4App.h"
#include "cISC4BuildingOccupant.h"
#include "cISC4City.h"
#include "cISC4Demolition.h"
#include "cISC4HistoryWarehouse.h"
#include "cISC4LineItem.h"
#include "cISC4NeighborConnection.h"
#include "cISC4NeighborDeal.h"
#include "cISC4NetworkManager.h"
#include "cISC4NetworkOccupant.h"
#include "cISC4Occupant.h"
#include "cISC4OccupantManager.h"
#include "cISC4OrdinanceSimulator.h"
#include "cISC4PollutionSimulator.h"
#include "cISC4PowerLineOccupant.h"
#include "cISC4SimGrid.h"
#include "cISC4ZoneManager.h"
#include "cISCPropertyHolder.h"
#include "cRZBaseString.h"
#include "cRZCOMDllDirector.h"
#include "cS3DVector3.h"

#include <cstring>
#include <string>

using fc4::FromToken;
using fc4::TokenOf;

// ---- slot 18: the consumption map as the game's vector<vector<int16>> ----------------------------

// One row: an STLport vector<int16> {begin, end, capacity_end}, buffer B.
struct FC4PowerConsumptionRow {
    int16_t* begin;
    int16_t* end;
    int16_t* capacityEnd;
};

namespace fc4 {
template <>
struct GameElement<FC4PowerConsumptionRow> {
    template <class Alloc>
    static void Destroy(FC4PowerConsumptionRow& r) {
        Alloc::Free(r.begin);
        r.begin = r.end = r.capacityEnd = nullptr;
    }
};
}  // namespace fc4

struct cFC4PowerSimulator::ConsumptionMap {
    fc4::GameVector<FC4PowerConsumptionRow> rows;
};

namespace {

constexpr uint32_t kIID_cIGZMessageTarget2 = 0x452294AA;
constexpr uint32_t kIID_cISC4Occupant = 0x26BB767E;
constexpr uint32_t kIID_cISC4PowerPoleOccupant = 0xA8C86142;
constexpr uint32_t kIID_cISC4PowerLineOccupant = 0xC98F3B24;
constexpr uint32_t kIID_cISC4NetworkOccupant = 0xA821EF94;
constexpr uint32_t kIID_cISC4BuildingOccupant = 0x87DDA3A9;
constexpr uint32_t kIID_cISC4Lot = 0xE818FCED;
constexpr uint32_t kIID_cISC4NeighborDeal = 0x0773FEF1;

// Messages (§5).
constexpr uint32_t kMsgOccupantInserted = 0x99EF1142;
constexpr uint32_t kMsgOccupantRemoved = 0x99EF1143;
constexpr uint32_t kMsgZoningChanged = 0x07699455;
constexpr uint32_t kMsgStateChanged = 0xE8BDA2E7;
constexpr uint32_t kMsgDealStatus = 0xEA418D8F;
constexpr uint32_t kMsgSimNewMonth = 0x66956816;

// Tuning exemplar (§8, "Utilities").
constexpr uint32_t kTuningType = 0x6534284A;
constexpr uint32_t kTuningGroup = 0xE7E2C2DB;
constexpr uint32_t kTuningInstance = 0xC911E35B;

// The power-line tool (§5).
constexpr int32_t kPowerLineNetwork = 5;

using GridU8 = cISC4SimGrid<uint8_t>;

// The tables' `user` is an FC4PowerGame: the simulator's own (the full
// table) or the power shadow's (the read-only table, PowerReadOnlyHost). The
// read callbacks use G(u) only; the write callbacks (full table) Self(u).
FC4PowerGame* G(void* u) { return static_cast<FC4PowerGame*>(u); }
cFC4PowerSimulator* Self(void* u) { return static_cast<cFC4PowerSimulator*>(G(u)); }

cISC4Occupant* OccOf(uint64_t t) { return reinterpret_cast<cISC4Occupant*>(FromToken(t)); }
// A pole token is the pole interface (the record reads it as 0xA8C86142), not
// its occupant: §5.6 asks the occupant (QI 0x26BB767E) for IsFlagSet and the
// properties. A token that already is an occupant answers with itself.
// Borrowed: the token's holder keeps the object alive.
cISC4Occupant* OccupantView(uint64_t t) {
    cIGZUnknown* u = FromToken(t);
    cISC4Occupant* o = nullptr;
    if (u == nullptr || !u->QueryInterface(kIID_cISC4Occupant, reinterpret_cast<void**>(&o)) || o == nullptr) {
        return reinterpret_cast<cISC4Occupant*>(u);
    }
    o->Release();
    return o;
}
cFC4ISC4Lot* LotOf(uint64_t t) { return t ? fc4::AsGame(reinterpret_cast<cISC4Lot*>(FromToken(t))) : nullptr; }
cFC4ISC4DepartmentBudget* DeptOf(uint64_t t) {
    return t ? fc4::AsGame(reinterpret_cast<cISC4DepartmentBudget*>(FromToken(t))) : nullptr;
}
cISC4LineItem* ItemOf(uint64_t t) { return reinterpret_cast<cISC4LineItem*>(FromToken(t)); }
cISC4NeighborDeal* DealOf(uint64_t t) { return reinterpret_cast<cISC4NeighborDeal*>(FromToken(t)); }
cISC4NeighborConnection* ConnOf(uint64_t t) { return reinterpret_cast<cISC4NeighborConnection*>(FromToken(t)); }

bool Gate(fc4::Sw s, const char* what) {
    if (fc4::On(s)) return true;
    fc4::NoteSkipped(s, what);
    return false;
}

void NoteOnce(bool& done, const char* text) {
    if (done) return;
    done = true;
    fc4::Log("%s", text);
}

// QueryInterface for `iid`; the result (one reference) or nullptr.
template <class T>
T* Query(cIGZUnknown* u, uint32_t iid) {
    if (u == nullptr) return nullptr;
    void* p = nullptr;
    if (!u->QueryInterface(iid, &p) || p == nullptr) return nullptr;
    T* t = static_cast<T*>(p);
    FC4_ACQUIRED(reinterpret_cast<cIGZUnknown*>(t), "QueryInterface");
    return t;
}

template <class T>
void Drop(T* p) {
    if (p != nullptr) FC4_RELEASE(reinterpret_cast<cIGZUnknown*>(p));
}

// Runs f(T*) on the occupant QI'd to `iid` (released afterwards); false if the QI fails.
template <class T, class F>
bool WithQi(uint64_t occ, uint32_t iid, F f) {
    T* p = Query<T>(FromToken(occ), iid);
    if (p == nullptr) return false;
    f(p);
    Drop(p);
    return true;
}

// ---- city, tuning, ordinance, simulator ---------------------------------------------------------

void CellCounts(void* u, int32_t* x, int32_t* z) {
    cISC4City* c = G(u)->city;
    if (x != nullptr) *x = c ? static_cast<int32_t>(c->CellCountX()) : 0;
    if (z != nullptr) *z = c ? static_cast<int32_t>(c->CellCountZ()) : 0;
}

void CellWidths(void* u, float* x, float* z) {
    cISC4City* c = G(u)->city;
    if (x != nullptr) *x = c ? c->CellWidthX() : 0.0f;
    if (z != nullptr) *z = c ? c->CellWidthZ() : 0.0f;
}

fc4_props* Tuning(void*) {
    if (!Gate(fc4::Sw::ExemplarLoads, "power tuning exemplar (none)")) return nullptr;
    fc4::props::Exemplar ex;
    if (!ex.Load(kTuningType, kTuningGroup, kTuningInstance)) {
        fc4::Log("power: tuning exemplar unavailable; the defaults stay");
        return nullptr;
    }
    return fc4::props::ToCoreAll(ex.Holder());
}

float CumulativeFactor(void* u, uint32_t id, uint32_t index) {
    cISC4City* c = G(u)->cityServices ? G(u)->city : nullptr;
    cISC4OrdinanceSimulator* o = c ? c->GetOrdinanceSimulator() : nullptr;
    return o ? o->GetCumulativeFactor(id, index) : 1.0f;
}

uint32_t SimTime(void* u) {
    cISC4City* c = G(u)->cityServices ? G(u)->city : nullptr;
    cISC4Simulator* s = c ? c->GetSimulator() : nullptr;
    return s ? static_cast<uint32_t>(fc4::AsGame(s)->GetSimTime()) : 0;  // slot 29
}

void AcquireServices(void* u) { Self(u)->AcquireServices(); }
void ReleaseServices(void* u) { Self(u)->ReleaseServices(); }

// ---- framework ----------------------------------------------------------------------------------

void AddAgent(void* u, uint32_t level, const char* name, size_t len, uint32_t flags) {
    cISC4City* c = Self(u)->City();
    cFC4ISC4Simulator* sim = c ? fc4::AsGame(c->GetSimulator()) : nullptr;
    if (sim == nullptr) {
        fc4::Log("power: no simulator; agent level %u not registered", level);
        return;
    }
    const cRZBaseString n(name, len);
    sim->AddAgent(Self(u)->Target(), level, n, flags);  // slot 19
}

void RemoveAgent(void* u) {
    cISC4City* c = Self(u)->City();
    cFC4ISC4Simulator* sim = c ? fc4::AsGame(c->GetSimulator()) : nullptr;
    if (sim != nullptr) sim->RemoveAgentAllLevels(Self(u)->Target());  // slot 20
}

void Subscribe(void* u, const uint32_t* ids, size_t n) { Self(u)->Subscribe(ids, n, true); }
void Unsubscribe(void* u, const uint32_t* ids, size_t n) { Self(u)->Subscribe(ids, n, false); }

// MessagePost(msg, false) of a standard message (§11.1).
void Post(void*, const fc4_power_post* p) {
    if (p == nullptr || !Gate(fc4::Sw::PowerPosts, "power message post")) return;
    cIGZMessageServer2Ptr ms;
    if (!ms) return;
    cFC4Message2Standard* msg = FC4MakeMessage(p->msg_type, static_cast<intptr_t>(p->data[0]),
                                               static_cast<intptr_t>(p->data[1]), static_cast<intptr_t>(p->data[2]));
    msg->SetData4(static_cast<intptr_t>(p->data[3]));
    if (p->unknown != 0) msg->SetIGZUnknown(FromToken(p->unknown));
    ms->MessagePost(msg, false);
    msg->Release();
}

void RecordHistory(void* u, uint32_t series, int32_t value) {
    if (!Gate(fc4::Sw::History, "power RecordHistoryData")) return;
    cISC4City* c = Self(u)->City();
    cISC4HistoryWarehouse* h = c ? c->GetHistoryWarehouse() : nullptr;
    if (h != nullptr) h->RecordHistoryData(series, value);
}

void SetHistoryQueueLength(void* u, uint32_t series, uint32_t length) {
    if (!Gate(fc4::Sw::History, "power SetHistoryDataQueueLength")) return;
    cISC4City* c = Self(u)->City();
    cISC4HistoryWarehouse* h = c ? c->GetHistoryWarehouse() : nullptr;
    if (h != nullptr) h->SetHistoryDataQueueLength(series, length);
}

// ---- references ---------------------------------------------------------------------------------

void AddRefToken(void*, uint64_t t) {
    cIGZUnknown* u = FromToken(t);
    if (u == nullptr) return;
    if (fc4::IsOurObject(u)) {
        u->AddRef();
    } else {
        FC4_ADDREF(u);
    }
}

void ReleaseToken(void*, uint64_t t) {
    cIGZUnknown* u = FromToken(t);
    if (u == nullptr) return;
    if (fc4::IsOurObject(u)) {
        u->Release();
    } else {
        FC4_RELEASE(u);
    }
}

// A saved pole comes back as its cISC4PowerPoleOccupant; the core knows
// occupants by their cISC4Occupant identity (the message's void1, §5.1).
// The core owns one reference per resolved token (it releases it at Shutdown
// or when a later Read replaces it). A host object arrives from the stream
// with that reference; one of ours (our department, when the budget subsystem
// is on) arrives without one (FC4Streams.h, OurObjectRef), so it is taken here.
uint64_t ResolveRef(void*, uint64_t t, uint32_t iid) {
    cIGZUnknown* u = FromToken(t);
    if (u == nullptr) return 0;
    if (fc4::IsOurObject(u)) {
        u->AddRef();
        return t;
    }
    if (iid != kIID_cISC4PowerPoleOccupant) return t;
    cISC4Occupant* occ = Query<cISC4Occupant>(u, kIID_cISC4Occupant);
    FC4_RELEASE(u);
    return TokenOf(occ);
}

// ---- occupants ----------------------------------------------------------------------------------

uint32_t OccupantType(void*, uint64_t occ) {
    if (occ == 0 || !Gate(fc4::Sw::PowerOccupants, "power occupant GetType (0)")) return 0;
    return static_cast<uint32_t>(OccOf(occ)->GetType());
}

bool OccupantProperty(void*, uint64_t occ, uint32_t id, fc4_props* out) {
    if (occ == 0 || !Gate(fc4::Sw::PowerOccupants, "power occupant property read (absent)")) return false;
    cISCPropertyHolder* h = OccupantView(occ)->AsPropertyHolder();  // borrowed
    return h != nullptr && fc4::props::CopyToCore(h, id, out);
}

void SetOccupantProperty(void*, uint64_t occ, uint32_t id, uint16_t memType, bool isArray, const void* data,
                         size_t count) {
    if (occ == 0 || !Gate(fc4::Sw::PowerOccupants, "power occupant property write")) return;
    if (!Gate(fc4::Sw::PropertyWrites, "power occupant property write")) return;
    cISCPropertyHolder* h = OccupantView(occ)->AsPropertyHolder();
    if (h != nullptr && !fc4::props::SetTyped(h, id, memType, isArray, data, count)) {
        FC4_LOGV("power: property 0x%08X (type 0x%X) not written to occupant %p", id, memType, FromToken(occ));
    }
}

void OccupantPosition(void*, uint64_t occ, float* xyz) {
    if (xyz == nullptr || occ == 0 || !Gate(fc4::Sw::PowerOccupants, "power occupant GetPosition")) return;
    cS3DVector3 v;
    OccOf(occ)->GetPosition(&v);
    xyz[0] = v.fX;
    xyz[1] = v.fY;
    xyz[2] = v.fZ;
}

void OccupantBoundingBox(void*, uint64_t occ, float* mn, float* mx) {
    if (mn == nullptr || mx == nullptr || occ == 0 ||
        !Gate(fc4::Sw::PowerOccupants, "power occupant GetBoundingBox")) {
        return;
    }
    cS3DVector3 a, b;
    OccOf(occ)->GetBoundingBox(&a, &b);
    mn[0] = a.fX;
    mn[1] = a.fY;
    mn[2] = a.fZ;
    mx[0] = b.fX;
    mx[1] = b.fY;
    mx[2] = b.fZ;
}

void OccupantBoundingCells(void*, uint64_t occ, fc4_cell_rect* out) {
    if (out == nullptr || occ == 0 || !Gate(fc4::Sw::PowerOccupants, "power occupant GetBoundingCityCells")) return;
    SC4Rect<long> r;
    OccOf(occ)->GetBoundingCityCells(r);
    out->x1 = static_cast<int32_t>(r.topLeftX);
    out->z1 = static_cast<int32_t>(r.topLeftY);
    out->x2 = static_cast<int32_t>(r.bottomRightX);
    out->z2 = static_cast<int32_t>(r.bottomRightY);
}

bool OccupantInManager(void*, uint64_t occ) {
    if (occ == 0 || !Gate(fc4::Sw::PowerOccupants, "power occupant IsFlagSet (true)")) return occ != 0;
    return OccupantView(occ)->IsFlagSet(1);
}

bool BuildingType(void*, uint64_t occ, uint32_t* out) {
    if (!Gate(fc4::Sw::PowerOccupants, "power building type (none)")) return false;
    return WithQi<cISC4BuildingOccupant>(occ, kIID_cISC4BuildingOccupant, [&](cISC4BuildingOccupant* b) {
        if (out != nullptr) *out = b->GetBuildingType();
    });
}

bool PowerPole(void*, uint64_t occ, fc4_power_pole* out) {
    if (!Gate(fc4::Sw::PowerPoles, "power pole QueryInterface (not a pole)")) return false;
    return WithQi<cFC4ISC4PowerPoleOccupant>(occ, kIID_cISC4PowerPoleOccupant, [&](cFC4ISC4PowerPoleOccupant* p) {
        int32_t x = 0, z = 0;
        const uint32_t ignored = p->IsIgnoredByPower() ? 1u : 0u;  // slot 13
        p->GetCell(x, z);                                          // slot 8
        if (out != nullptr) {
            out->ignored = ignored;
            out->cell_x = x;
            out->cell_z = z;
        }
    });
}

void BreakAllConnections(void*, uint64_t pole, bool flag) {
    if (!Gate(fc4::Sw::PowerPoles, "power pole BreakAllConnections")) return;
    WithQi<cFC4ISC4PowerPoleOccupant>(pole, kIID_cISC4PowerPoleOccupant,
                                      [&](cFC4ISC4PowerPoleOccupant* p) { p->BreakAllConnections(flag); });
}

bool PoleIsDistressed(void*, uint64_t pole) {
    if (!Gate(fc4::Sw::PowerPoles, "power pole IsDistressed (false)")) return false;
    bool on = false;
    WithQi<cFC4ISC4PowerPoleOccupant>(pole, kIID_cISC4PowerPoleOccupant,
                                      [&](cFC4ISC4PowerPoleOccupant* p) { on = p->IsDistressed(); });
    return on;
}

void SetPoleDistressed(void*, uint64_t pole, bool on) {
    if (!Gate(fc4::Sw::PowerPoles, "power pole SetDistressed")) return;
    WithQi<cFC4ISC4PowerPoleOccupant>(pole, kIID_cISC4PowerPoleOccupant,
                                      [&](cFC4ISC4PowerPoleOccupant* p) { p->SetDistressed(on); });
}

bool PowerLine(void*, uint64_t occ, fc4_power_line* out) {
    if (!Gate(fc4::Sw::PowerPoles, "power line QueryInterface (not a line)")) return false;
    return WithQi<cISC4PowerLineOccupant>(occ, kIID_cISC4PowerLineOccupant, [&](cISC4PowerLineOccupant* l) {
        uint32_t x = 0, z = 0;
        const bool preview = l->IsPreview();  // slot 6
        l->GetCell(x, z);                     // slot 5
        if (out != nullptr) {
            out->preview = preview ? 1u : 0u;
            out->cell_x = static_cast<int32_t>(x);
            out->cell_z = static_cast<int32_t>(z);
        }
    });
}

// network-occupants §5.1: both outputs are counted pole interfaces (the SDK
// misdeclares the first as a line). functions.tsv gives slot 3 a void return.
class PowerLineConnections : public cIGZUnknown {
public:
    virtual void GetConnectedPoles(cIGZUnknown*& first, cIGZUnknown*& second, bool& flag) = 0;
};

bool LineConnectedPoles(void* u, uint64_t line, uint64_t* first, uint64_t* second) {
    if (first != nullptr) *first = 0;
    if (second != nullptr) *second = 0;
    cIGZUnknown* poles[2] = {};
    const bool found = WithQi<PowerLineConnections>(line, kIID_cISC4PowerLineOccupant,
        [&](PowerLineConnections* p) {
            bool flag = false;
            p->GetConnectedPoles(poles[0], poles[1], flag);
        });
    // The core retains the tokens across the three terrain-change passes.
    // Hold each returned reference until the outermost core call ends, even
    // when both outputs name the same pole or only one output is non-null.
    for (cIGZUnknown* pole : poles) {
        if (pole == nullptr) continue;
        FC4_ACQUIRED(pole, "power GetConnectedPoles");
        G(u)->Hold(std::shared_ptr<void>(pole, [](void* p) { FC4_RELEASE(static_cast<cIGZUnknown*>(p)); }));
    }
    if (!found || poles[0] == nullptr || poles[1] == nullptr) return false;
    if (first != nullptr) *first = TokenOf(poles[0]);
    if (second != nullptr) *second = TokenOf(poles[1]);
    return true;
}

// first.RemoveConnection(second, 0): docs/abi/direct-calls.md. `this` and the
// argument are both pole bases, the pointer QueryInterface(0xA8C86142)
// returns (this also checks that the objects are the game's pole class).
using RemoveConnectionFn = void(__fastcall*)(void* self, void* edx, void* other, uint32_t flag);

void RemovePoleConnection(void*, uint64_t first, uint64_t second) {
    // A FreeCity4 pole (enable = netocc) takes the call through its core.
    if (FC4NetOccRemovePoleConnection(FromToken(first), FromToken(second), false)) return;
    if (!Gate(fc4::Sw::PowerDirectCalls, "power pole RemoveConnection (direct call)")) return;
    const uintptr_t va = fc4::compat::DirectCall(fc4::compat::Direct::PoleRemoveConnection);
    if (va == 0) return;  // logged once by compat
    cFC4ISC4PowerPoleOccupant* a = Query<cFC4ISC4PowerPoleOccupant>(FromToken(first), kIID_cISC4PowerPoleOccupant);
    cFC4ISC4PowerPoleOccupant* b = Query<cFC4ISC4PowerPoleOccupant>(FromToken(second), kIID_cISC4PowerPoleOccupant);
    if (a != nullptr && b != nullptr) {
        // __thiscall through __fastcall: ECX = this, EDX unused, stack (other, flag), callee pops 8.
        reinterpret_cast<RemoveConnectionFn>(va)(a, nullptr, b, 0);
    } else {
        fc4::Log("power: RemoveConnection skipped: %p / %p is not a game pole", FromToken(first), FromToken(second));
    }
    Drop(b);
    Drop(a);
}

bool NetworkOccupantCell(void*, uint64_t occ, int32_t* x, int32_t* z) {
    if (!Gate(fc4::Sw::PowerOccupants, "power network occupant QueryInterface (none)")) return false;
    return WithQi<cISC4NetworkOccupant>(occ, kIID_cISC4NetworkOccupant, [&](cISC4NetworkOccupant* n) {
        uint32_t cx = 0, cz = 0;
        n->GetOccupiedCell(cx, cz);  // slot 59
        if (x != nullptr) *x = static_cast<int32_t>(cx);
        if (z != nullptr) *z = static_cast<int32_t>(cz);
    });
}

bool HasNetworkFlag(void*, uint64_t occ, uint32_t flag) {
    if (!Gate(fc4::Sw::PowerOccupants, "power HasNetworkFlag (false)")) return false;
    bool on = false;
    WithQi<cISC4NetworkOccupant>(occ, kIID_cISC4NetworkOccupant,
                                 [&](cISC4NetworkOccupant* n) { on = n->HasNetworkFlag(flag); });
    return on;
}

uint32_t NetworkFlags(void*, uint64_t occ) {
    if (!Gate(fc4::Sw::PowerOccupants, "power GetNetworkFlag (0)")) return 0;
    uint32_t f = 0;
    WithQi<cISC4NetworkOccupant>(occ, kIID_cISC4NetworkOccupant,
                                 [&](cISC4NetworkOccupant* n) { f = n->GetNetworkFlag(); });
    return f;
}

// GetOccupantsByStandardCityCells(list, &min, &max, 0, -1) (§5.3, §5.4).
// [abi] Occupant manager slots 40 (filter) and 41 (typed) both fill an A list
// (SC4List), appending with one reference each; type 0 forwards to slot 40
// with no filter, so the two overloads are the same call here. The list is
// kept until the outermost call into the core returns: §5.4 removes poles
// between its passes and the list's references keep them alive, as in the
// original.
void OccupantsInCells(void* u, const fc4_cell_rect* r, fc4_token_callback cb, void* ctx) {
    if (r == nullptr || cb == nullptr) return;
    if (!Gate(fc4::Sw::PowerOccupantQuery, "power occupant query (no occupants)")) return;
    cISC4OccupantManager* om = G(u)->occupantManager;
    if (om == nullptr) return;
    auto list = std::make_shared<SC4List<cISC4Occupant*>>();
    const SC4Point<int> mn(r->x1, r->z1);
    const SC4Point<int> mx(r->x2, r->z2);
    om->GetOccupantsByStandardCityCells(*list, &mn, &mx, static_cast<uint32_t>(0), 0xFFFFFFFFu);
    for (cISC4Occupant* o : *list) {
        if (o != nullptr) cb(ctx, TokenOf(o));
    }
    G(u)->Hold(std::move(list));
}

void RemoveOccupant(void* u, uint64_t occ) {
    if (occ == 0 || !Gate(fc4::Sw::PowerTerrain, "power RemoveOccupant")) return;
    cISC4OccupantManager* om = Self(u)->OccupantManager();
    if (om != nullptr) om->RemoveOccupant(OccOf(occ), true, 0);  // slot 21
}

// ---- zones, tools -------------------------------------------------------------------------------

uint32_t ZoneType(void* u, int32_t x, int32_t z) {
    cISC4ZoneManager* zm = G(u)->zoneManager;
    return zm ? static_cast<uint32_t>(zm->GetZoneType(x, z)) : 0;
}

// §5: GetNetworkTool(5, false): AddRef, Init, UpdateOnZoneChange(rect, data3), Shutdown, Release.
void PowerLineToolZoneChange(void* u, const fc4_cell_rect* r, uint32_t data3) {
    if (r == nullptr || !Gate(fc4::Sw::PowerZoneTool, "power-line tool UpdateOnZoneChange")) return;
    cISC4City* c = Self(u)->City();
    cISC4NetworkManager* nm = c ? c->GetNetworkManager() : nullptr;
    cISC4NetworkTool* t = nm ? nm->GetNetworkTool(kPowerLineNetwork, false) : nullptr;
    if (t == nullptr) return;
    cFC4ISC4NetworkTool* tool = reinterpret_cast<cFC4ISC4NetworkTool*>(t);
    FC4_ADDREF(reinterpret_cast<cIGZUnknown*>(tool));
    tool->Init();
    FC4Int32Quad q{{r->x1, r->z1, r->x2, r->z2}};
    tool->UpdateOnZoneChange(q, data3);
    tool->Shutdown();
    FC4_RELEASE(reinterpret_cast<cIGZUnknown*>(tool));
}

// ---- lots, demolition, pollution, view ----------------------------------------------------------

cFC4ISC4LotManager* LotManager(void* u) {
    cISC4City* c = G(u)->cityServices ? G(u)->city : nullptr;
    cISC4LotManager* lm = c ? c->GetLotManager() : nullptr;
    return lm ? fc4::AsGame(lm) : nullptr;
}

uint64_t LotAt(void* u, int32_t x, int32_t z) {
    if (!Gate(fc4::Sw::PowerLots, "power GetLot (none)")) return 0;
    cFC4ISC4LotManager* lm = LotManager(u);
    return lm ? TokenOf(lm->GetLot(x, z, false)) : 0;  // slot 6, borrowed
}

uint64_t OccupantLot(void* u, uint64_t occ) {
    if (occ == 0 || !Gate(fc4::Sw::PowerLots, "power GetOccupantLot (none)")) return 0;
    cFC4ISC4LotManager* lm = LotManager(u);
    return lm ? TokenOf(lm->GetOccupantLot(OccOf(occ))) : 0;  // slot 14, borrowed
}

// [choice] The lot of a message's IGZUnknown is borrowed: the message holds
// it for the call.
uint64_t AsLot(void*, uint64_t t) {
    if (!Gate(fc4::Sw::PowerLots, "power lot QueryInterface (none)")) return 0;
    cISC4Lot* l = Query<cISC4Lot>(FromToken(t), kIID_cISC4Lot);
    Drop(l);
    return TokenOf(l);
}

void LotBoundingRect(void*, uint64_t lot, fc4_cell_rect* out) {
    cFC4ISC4Lot* l = LotOf(lot);
    if (l == nullptr || out == nullptr) return;
    SC4Rect<int32_t> r;
    l->GetBoundingRect(r);
    out->x1 = r.topLeftX;
    out->z1 = r.topLeftY;
    out->x2 = r.bottomRightX;
    out->z2 = r.bottomRightY;
}

uint32_t LotState(void*, uint64_t lot) {
    cFC4ISC4Lot* l = LotOf(lot);
    return l ? static_cast<uint32_t>(l->GetState()) : 0;
}

bool LotNeedsPower(void*, uint64_t lot) {
    cFC4ISC4Lot* l = LotOf(lot);
    return l != nullptr && l->GetNeedsPower();
}

bool LotHasPower(void*, uint64_t lot) {
    cFC4ISC4Lot* l = LotOf(lot);
    return l != nullptr && l->GetHasPower();
}

void SetLotHasPower(void*, uint64_t lot, bool on) {
    if (!Gate(fc4::Sw::PowerLots, "power SetHasPower")) return;
    if (cFC4ISC4Lot* l = LotOf(lot)) l->SetHasPower(on);
}

void SetLotCondition(void*, uint64_t lot, uint32_t condition) {
    if (!Gate(fc4::Sw::PowerLots, "power SetCondition")) return;
    if (cFC4ISC4Lot* l = LotOf(lot)) l->SetCondition(condition);
}

// DemolishLots(true, lots, 0, flags, false, null, null, 0) (§5.5, §7.4).
// [abi] The game only reads the list (allocators.md): an SC4List holding one
// reference per lot, released when it is destroyed after the call.
void DemolishLots(void* u, const uint64_t* lots, size_t count, uint32_t flags) {
    if (!Gate(fc4::Sw::PowerDemolish, "power DemolishLots")) return;
    cISC4Demolition* d = Self(u)->Demolition();
    if (d == nullptr) {
        fc4::Log("power: no demolition utility; %u lots not demolished", static_cast<unsigned>(count));
        return;
    }
    SC4List<cISC4Lot*> list;
    for (size_t i = 0; i < count; ++i) {
        cISC4Lot* l = reinterpret_cast<cISC4Lot*>(FromToken(lots[i]));
        if (l == nullptr) continue;
        l->AddRef();  // the list's reference, released by its destructor
        list.push_back(l);
    }
    fc4::Log("power: DemolishLots(%u lots, flags 0x%X)", static_cast<unsigned>(count), flags);
    d->DemolishLots(true, list, 0, flags, false, nullptr, nullptr, 0);  // slot 7
}

void AddRadiation(void* u, const float* xyz, float strength, float radius) {
    if (xyz == nullptr || !Gate(fc4::Sw::PowerRadiation, "power AddRadiationToCity")) return;
    cISC4City* c = Self(u)->City();
    cISC4PollutionSimulator* p = c ? c->GetPollutionSimulator() : nullptr;
    if (p == nullptr) return;
    cS3DVector3 v;
    v.fX = xyz[0];
    v.fY = xyz[1];
    v.fZ = xyz[2];
    p->AddRadiationToCity(v, strength, radius);  // slot 66
}

bool g_viewNoted = false;

// [choice] Refused: the original reaches the 3D view through a game global;
// the interface path (cISC4App -> cIGZWin -> cISC4View3DWin) uses cIGZWin,
// which docs/abi/interfaces.tsv does not cover (spec question). The Nuclear
// Free Zone therefore never removes the current view input control.
bool CurrentViewInputControl(void*, uint32_t*) {
    NoteOnce(g_viewNoted,
             "power: the 3D view's input control is not read (cIGZWin not in docs/abi); the Nuclear Free Zone "
             "does not close the placement tool (§5.5)");
    return false;
}

void RemoveCurrentViewInputControl(void*) {}

// ---- neighbours ---------------------------------------------------------------------------------

cFC4ISC4NeighborsSimulator* Neighbors(void* u) {
    cISC4NeighborsSimulator* n = G(u)->neighbors;
    return n ? fc4::AsGame(n) : nullptr;
}

// FindDeals(type, list&) (slot 30). [abi] A list, cleared first, one
// reference each (allocators.md): an SC4List, held until the outermost call
// into the core returns.
void FindDeals(void* u, uint32_t type, fc4_power_deal_callback cb, void* ctx) {
    if (cb == nullptr || !Gate(fc4::Sw::PowerNeighbors, "power FindDeals (none)")) return;
    cFC4ISC4NeighborsSimulator* n = Neighbors(u);
    if (n == nullptr) return;
    auto list = std::make_shared<SC4List<cISC4NeighborDeal*>>();
    n->FindDealsOfType(static_cast<int32_t>(type), *list);
    for (cISC4NeighborDeal* d : *list) {
        if (d == nullptr) continue;
        fc4_power_deal pd{};
        pd.token = TokenOf(d);
        pd.city = TokenOf(d->GetCityOnOtherSideOfConnection());
        pd.amount = d->GetAmount();
        cb(ctx, &pd);
    }
    G(u)->Hold(std::move(list));
}

int32_t DealAmountUsed(void*, uint64_t deal) { return deal ? DealOf(deal)->GetAmountUsed() : 0; }

void SetDealAmountUsed(void*, uint64_t deal, int32_t used) {
    if (deal != 0 && Gate(fc4::Sw::PowerNeighbors, "power SetAmountUsed")) DealOf(deal)->SetAmountUsed(used);
}

void SetDealFlags(void*, uint64_t deal, uint32_t flags, bool on) {
    if (deal != 0 && Gate(fc4::Sw::PowerNeighbors, "power deal SetFlags")) {
        DealOf(deal)->SetFlags(static_cast<int32_t>(flags), on);
    }
}

bool DealType(void*, uint64_t t, uint32_t* out) {
    cISC4NeighborDeal* d = Query<cISC4NeighborDeal>(FromToken(t), kIID_cISC4NeighborDeal);
    if (d == nullptr) return false;
    if (out != nullptr) *out = static_cast<uint32_t>(d->GetType());
    Drop(d);
    return true;
}

void MarkDealsDirty(void* u) {
    if (!Gate(fc4::Sw::PowerNeighbors, "power MarkDealsDirty")) return;
    if (cFC4ISC4NeighborsSimulator* n = Neighbors(u)) n->MarkDealsDirty();
}

template <class Fill>
void Connections(void* u, fc4_token_callback cb, void* ctx, Fill fill) {
    if (cb == nullptr || !Gate(fc4::Sw::PowerNeighbors, "power neighbour connections (none)")) return;
    cFC4ISC4NeighborsSimulator* n = Neighbors(u);
    if (n == nullptr) return;
    auto list = std::make_shared<SC4List<cISC4NeighborConnection*>>();
    fill(n, *list);
    for (cISC4NeighborConnection* c : *list) {
        if (c != nullptr) cb(ctx, TokenOf(c));
    }
    G(u)->Hold(std::move(list));
}

using ConnType = cISC4NeighborsSimulator::ConnectionType;

// [abi] Slots 17 / 18: A lists, cleared first, one reference each.
void ConnectionsOfType(void* u, uint32_t type, fc4_token_callback cb, void* ctx) {
    Connections(u, cb, ctx, [&](cFC4ISC4NeighborsSimulator* n, SC4List<cISC4NeighborConnection*>& l) {
        n->GetConnectionsOfType(static_cast<ConnType>(type), l);
    });
}

void ConnectionsToCity(void* u, uint32_t type, uint64_t city, fc4_token_callback cb, void* ctx) {
    Connections(u, cb, ctx, [&](cFC4ISC4NeighborsSimulator* n, SC4List<cISC4NeighborConnection*>& l) {
        n->GetConnectionsOfTypeToCity(static_cast<ConnType>(type),
                                      reinterpret_cast<cISC4RegionalCity const*>(FromToken(city)), l);
    });
}

uint64_t FindUnvisitedConnection(void* u, uint32_t type, uint64_t city) {
    if (!Gate(fc4::Sw::PowerNeighbors, "power FindUnvisitedConnection (none)")) return 0;
    cFC4ISC4NeighborsSimulator* n = Neighbors(u);
    if (n == nullptr) return 0;
    return TokenOf(n->FindUnvisitedConnection(static_cast<ConnType>(type),
                                              reinterpret_cast<cISC4RegionalCity const*>(FromToken(city))));
}

uint64_t ConnectionOnOtherSide(void* u, uint64_t conn) {
    if (conn == 0 || !Gate(fc4::Sw::PowerNeighbors, "power FindConnectionOnOtherSide (none)")) return 0;
    cFC4ISC4NeighborsSimulator* n = Neighbors(u);
    return n ? TokenOf(n->FindConnectionOnOtherSideOf(ConnOf(conn))) : 0;
}

void ConnectionCell(void*, uint64_t conn, int32_t* x, int32_t* z) {
    int32_t cx = 0, cz = 0;
    if (conn != 0) ConnOf(conn)->GetConnectionCell(cx, cz);
    if (x != nullptr) *x = cx;
    if (z != nullptr) *z = cz;
}

void SetConnectionVisited(void*, uint64_t conn, bool on) {
    if (conn != 0 && Gate(fc4::Sw::PowerNeighbors, "power SetVisitedThisCycle")) ConnOf(conn)->SetVisitedThisCycle(on);
}

bool ConnectionIsActive(void*, uint64_t conn) { return conn != 0 && ConnOf(conn)->GetIsActive(); }

void SetConnectionActive(void*, uint64_t conn, bool on) {
    if (conn != 0 && Gate(fc4::Sw::PowerNeighbors, "power SetIsActive")) ConnOf(conn)->SetIsActive(on);
}

// ---- budget -------------------------------------------------------------------------------------

cFC4ISC4BudgetSimulator* Budget(void* u) {
    cISC4BudgetSimulator* b = G(u)->budget;
    return b ? fc4::AsGame(b) : nullptr;
}

uint64_t Department(void* u, uint32_t id) {
    if (!Gate(fc4::Sw::PowerBudget, "power GetDepartmentBudget (none)")) return 0;
    cFC4ISC4BudgetSimulator* b = Budget(u);
    return b ? TokenOf(b->GetDepartmentBudget(id)) : 0;  // slot 28, borrowed
}

uint64_t CreateDepartment(void* u, uint32_t id, uint32_t group) {
    if (!Gate(fc4::Sw::PowerBudget, "power CreateDepartmentBudget (none)")) return 0;
    cFC4ISC4BudgetSimulator* b = Budget(u);
    return b ? TokenOf(b->CreateDepartmentBudget(id, group)) : 0;  // slot 25, borrowed
}

void SetDepartmentName(void*, uint64_t dept, uint32_t group, uint32_t instance) {
    if (cFC4ISC4DepartmentBudget* d = DeptOf(dept)) d->SetDepartmentName(group, instance);
}

void SetDepartmentMaxFunding(void*, uint64_t dept, float fraction) {
    if (cFC4ISC4DepartmentBudget* d = DeptOf(dept)) {
        SC4Percentage p;
        p.percentage = fraction;
        d->SetMaxAllowedFundingPercentage(p);
    }
}

float DepartmentFunding(void*, uint64_t dept) {
    cFC4ISC4DepartmentBudget* d = DeptOf(dept);
    SC4Percentage* p = d ? d->GetFundingPercentage() : nullptr;  // slot 10
    return p ? p->percentage : 1.0f;
}

int64_t BudgetFunding(void* u, uint64_t dept) {
    cFC4ISC4BudgetSimulator* b = Budget(u);
    return b && dept ? b->GetFundingOf(reinterpret_cast<cISC4DepartmentBudget*>(FromToken(dept))) : 0;  // slot 35
}

uint64_t CreateLineItem(void*, uint64_t dept, uint32_t line, bool locallyFunded) {
    cFC4ISC4DepartmentBudget* d = DeptOf(dept);
    return d ? TokenOf(d->CreateLineItem(line, locallyFunded)) : 0;
}

uint64_t CreateLineItemForBuildingType(void*, uint64_t dept, uint32_t line, bool flag) {
    cFC4ISC4DepartmentBudget* d = DeptOf(dept);
    return d ? TokenOf(d->CreateLineItemForBuildingType(line, flag)) : 0;
}

uint64_t LineItem(void*, uint64_t dept, uint32_t line) {
    cFC4ISC4DepartmentBudget* d = DeptOf(dept);
    return d ? TokenOf(d->GetLineItem(line)) : 0;
}

void SetLineItemName(void*, uint64_t item, uint32_t group, uint32_t instance) {
    if (item != 0) ItemOf(item)->SetName(group, instance);
}

void SetLineItemDisplayFlag(void*, uint64_t item, uint32_t flag, bool on) {
    if (item != 0) ItemOf(item)->SetDisplayFlag(static_cast<cISC4LineItem::DisplayFlag>(flag), on);
}

int64_t LineItemSecondaryInfo(void*, uint64_t item) { return item ? ItemOf(item)->GetSecondaryInfoField() : 0; }

void SetLineItemSecondaryInfo(void*, uint64_t item, int64_t v) {
    if (item != 0) ItemOf(item)->SetSecondaryInfoField(v);
}

void SetLineItemFullExpenses(void*, uint64_t item, int64_t v) {
    if (item != 0) ItemOf(item)->SetFullExpenses(v);
}

cISCPropertyHolder* HolderOf(uint64_t occ) { return occ ? OccupantView(occ)->AsPropertyHolder() : nullptr; }

bool BudgetItemForPurpose(void* u, uint64_t occ, uint32_t purpose, fc4_power_budget_item* out) {
    if (!Gate(fc4::Sw::PowerBudget, "power GetBudgetItemForPurpose (none)")) return false;
    cFC4ISC4BudgetSimulator* b = Budget(u);
    cISCPropertyHolder* h = HolderOf(occ);
    if (b == nullptr || h == nullptr) return false;
    cFC4ISC4BudgetSimulator::BudgetItem item{};
    if (!b->GetBudgetItemForPurpose(h, purpose, item)) return false;  // slot 51
    if (out != nullptr) {
        out->department = item.departmentID;
        out->line = item.lineItem;
        out->cost = item.cost;
        out->index = static_cast<uint32_t>(item.index);
        out->local_funding = item.localFundingPercentage;
    }
    return true;
}

void ChangeBudgetItemLine(void* u, uint64_t occ, uint32_t index, uint32_t line) {
    cFC4ISC4BudgetSimulator* b = Budget(u);
    cISCPropertyHolder* h = HolderOf(occ);
    if (b != nullptr && h != nullptr) b->ChangeBudgetItemLine(h, index, line);
}

void ChangeBudgetItemCost(void* u, uint64_t occ, uint32_t index, int64_t cost) {
    cFC4ISC4BudgetSimulator* b = Budget(u);
    cISCPropertyHolder* h = HolderOf(occ);
    if (b != nullptr && h != nullptr) b->ChangeBudgetItemCost(h, index, cost);
}

void AddLocallyFundedObject(void*, uint64_t dept, uint64_t occ, uint32_t purpose) {
    cFC4ISC4DepartmentBudget* d = DeptOf(dept);
    cISCPropertyHolder* h = HolderOf(occ);
    if (d != nullptr && h != nullptr) d->AddLocallyFundedObject(h, purpose);
}

void RemoveLocallyFundedObject(void*, uint64_t dept, uint64_t occ, uint32_t purpose) {
    cFC4ISC4DepartmentBudget* d = DeptOf(dept);
    cISCPropertyHolder* h = HolderOf(occ);
    if (d != nullptr && h != nullptr) d->RemoveLocallyFundedObject(h, purpose);
}

bool IsLocallyFundedObject(void*, uint64_t dept, uint64_t occ, uint32_t purpose) {
    cFC4ISC4DepartmentBudget* d = DeptOf(dept);
    cISCPropertyHolder* h = HolderOf(occ);
    return d != nullptr && h != nullptr && d->IsLocallyFundedObjectInDepartment(h, purpose);
}

bool SetLocalFundingPercent(void*, uint64_t dept, uint64_t occ, float fraction, uint32_t purpose) {
    cFC4ISC4DepartmentBudget* d = DeptOf(dept);
    cISCPropertyHolder* h = HolderOf(occ);
    if (d == nullptr || h == nullptr) return false;
    SC4Percentage p;
    p.percentage = fraction;
    return d->SetLocalFundingPercent(h, &p, purpose);
}

void SetLocalFullFunding(void*, uint64_t dept, uint64_t occ, int64_t amount, uint32_t purpose) {
    cFC4ISC4DepartmentBudget* d = DeptOf(dept);
    cISCPropertyHolder* h = HolderOf(occ);
    if (d != nullptr && h != nullptr) d->SetLocalFullFunding(h, amount, purpose);
}

fc4_power_host MakeHost(cFC4PowerSimulator* self, const fc4::HostGridContext* grids) {
    fc4_power_host h{};
    h.struct_size = sizeof(h);
    h.user = static_cast<FC4PowerGame*>(self);
    h.grids = fc4::MakeGridApi(grids);
    h.cell_counts = &CellCounts;
    h.cell_widths = &CellWidths;
    h.tuning = &Tuning;
    h.cumulative_factor = &CumulativeFactor;
    h.sim_time = &SimTime;
    h.acquire_services = &AcquireServices;
    h.release_services = &ReleaseServices;
    h.add_agent = &AddAgent;
    h.remove_agent = &RemoveAgent;
    h.subscribe = &Subscribe;
    h.unsubscribe = &Unsubscribe;
    h.post = &Post;
    h.record_history = &RecordHistory;
    h.set_history_queue_length = &SetHistoryQueueLength;
    h.add_ref = &AddRefToken;
    h.release = &ReleaseToken;
    h.resolve_ref = &ResolveRef;
    h.occupant_type = &OccupantType;
    h.occupant_property = &OccupantProperty;
    h.set_occupant_property = &SetOccupantProperty;
    h.occupant_position = &OccupantPosition;
    h.occupant_bounding_box = &OccupantBoundingBox;
    h.occupant_bounding_cells = &OccupantBoundingCells;
    h.occupant_in_manager = &OccupantInManager;
    h.building_type = &BuildingType;
    h.power_pole = &PowerPole;
    h.break_all_connections = &BreakAllConnections;
    h.pole_is_distressed = &PoleIsDistressed;
    h.set_pole_distressed = &SetPoleDistressed;
    h.power_line = &PowerLine;
    h.line_connected_poles = &LineConnectedPoles;
    h.remove_pole_connection = &RemovePoleConnection;
    h.network_occupant_cell = &NetworkOccupantCell;
    h.has_network_flag = &HasNetworkFlag;
    h.network_flags = &NetworkFlags;
    h.occupants_in_cells = &OccupantsInCells;
    h.remove_occupant = &RemoveOccupant;
    h.zone_type = &ZoneType;
    h.power_line_tool_zone_change = &PowerLineToolZoneChange;
    h.lot_at = &LotAt;
    h.occupant_lot = &OccupantLot;
    h.as_lot = &AsLot;
    h.lot_bounding_rect = &LotBoundingRect;
    h.lot_state = &LotState;
    h.lot_needs_power = &LotNeedsPower;
    h.lot_has_power = &LotHasPower;
    h.set_lot_has_power = &SetLotHasPower;
    h.set_lot_condition = &SetLotCondition;
    h.demolish_lots = &DemolishLots;
    h.add_radiation = &AddRadiation;
    h.current_view_input_control = &CurrentViewInputControl;
    h.remove_current_view_input_control = &RemoveCurrentViewInputControl;
    h.find_deals = &FindDeals;
    h.deal_amount_used = &DealAmountUsed;
    h.set_deal_amount_used = &SetDealAmountUsed;
    h.set_deal_flags = &SetDealFlags;
    h.deal_type = &DealType;
    h.mark_deals_dirty = &MarkDealsDirty;
    h.connections_of_type = &ConnectionsOfType;
    h.connections_to_city = &ConnectionsToCity;
    h.find_unvisited_connection = &FindUnvisitedConnection;
    h.connection_on_other_side = &ConnectionOnOtherSide;
    h.connection_cell = &ConnectionCell;
    h.set_connection_visited = &SetConnectionVisited;
    h.connection_is_active = &ConnectionIsActive;
    h.set_connection_active = &SetConnectionActive;
    h.department = &Department;
    h.create_department = &CreateDepartment;
    h.set_department_name = &SetDepartmentName;
    h.set_department_max_funding = &SetDepartmentMaxFunding;
    h.department_funding = &DepartmentFunding;
    h.budget_funding = &BudgetFunding;
    h.create_line_item = &CreateLineItem;
    h.create_line_item_for_building_type = &CreateLineItemForBuildingType;
    h.line_item = &LineItem;
    h.set_line_item_name = &SetLineItemName;
    h.set_line_item_display_flag = &SetLineItemDisplayFlag;
    h.line_item_secondary_info = &LineItemSecondaryInfo;
    h.set_line_item_secondary_info = &SetLineItemSecondaryInfo;
    h.set_line_item_full_expenses = &SetLineItemFullExpenses;
    h.budget_item_for_purpose = &BudgetItemForPurpose;
    h.change_budget_item_line = &ChangeBudgetItemLine;
    h.change_budget_item_cost = &ChangeBudgetItemCost;
    h.add_locally_funded_object = &AddLocallyFundedObject;
    h.remove_locally_funded_object = &RemoveLocallyFundedObject;
    h.is_locally_funded_object = &IsLocallyFundedObject;
    h.set_local_funding_percent = &SetLocalFundingPercent;
    h.set_local_full_funding = &SetLocalFullFunding;
    return h;
}

// Slot 25's callback: append with the reference the core hands over.
void AppendPlant(void* ctx, uint64_t t) {
    auto* list = static_cast<SC4List<cISC4Occupant*>*>(ctx);
    cISC4Occupant* o = OccOf(t);
    if (o != nullptr) list->push_back(o);
}

}  // namespace

// ---- the power shadow's read-only table ---------------------------------------------------------

fc4_power_host fc4::PowerReadOnlyHost(FC4PowerGame* game, const HostGridContext* grids) {
    fc4_power_host h{};
    h.struct_size = sizeof(h);
    h.user = game;
    h.grids = MakeGridApi(grids);
    // Geometry and reads only: the shadow copies the live grid and never
    // creates, writes, shuts down or releases one.
    h.grids.create = nullptr;
    h.grids.set = nullptr;
    h.grids.fill = nullptr;
    h.grids.shutdown = nullptr;
    h.grids.release = nullptr;
    h.cell_counts = &CellCounts;
    h.cell_widths = &CellWidths;
    h.tuning = &Tuning;
    h.cumulative_factor = &CumulativeFactor;
    h.sim_time = &SimTime;
    h.occupant_type = &OccupantType;
    h.occupant_property = &OccupantProperty;
    h.occupant_position = &OccupantPosition;
    h.occupant_bounding_box = &OccupantBoundingBox;
    h.occupant_bounding_cells = &OccupantBoundingCells;
    h.occupant_in_manager = &OccupantInManager;
    h.building_type = &BuildingType;
    h.power_pole = &PowerPole;
    h.pole_is_distressed = &PoleIsDistressed;
    h.power_line = &PowerLine;
    h.line_connected_poles = &LineConnectedPoles;
    h.network_occupant_cell = &NetworkOccupantCell;
    h.has_network_flag = &HasNetworkFlag;
    h.network_flags = &NetworkFlags;
    h.occupants_in_cells = &OccupantsInCells;
    h.zone_type = &ZoneType;
    h.lot_at = &LotAt;
    h.occupant_lot = &OccupantLot;
    h.as_lot = &AsLot;
    h.lot_bounding_rect = &LotBoundingRect;
    h.lot_state = &LotState;
    h.lot_needs_power = &LotNeedsPower;
    h.lot_has_power = &LotHasPower;
    h.current_view_input_control = &CurrentViewInputControl;
    h.find_deals = &FindDeals;
    h.deal_amount_used = &DealAmountUsed;
    h.deal_type = &DealType;
    h.connections_of_type = &ConnectionsOfType;
    h.connections_to_city = &ConnectionsToCity;
    h.find_unvisited_connection = &FindUnvisitedConnection;
    h.connection_on_other_side = &ConnectionOnOtherSide;
    h.connection_cell = &ConnectionCell;
    h.connection_is_active = &ConnectionIsActive;
    h.department = &Department;
    h.department_funding = &DepartmentFunding;
    h.budget_funding = &BudgetFunding;
    h.line_item = &LineItem;
    h.line_item_secondary_info = &LineItemSecondaryInfo;
    h.budget_item_for_purpose = &BudgetItemForPurpose;
    h.is_locally_funded_object = &IsLocallyFundedObject;
    return h;
}

// ---- object -------------------------------------------------------------------------------------

cIGZUnknown* CreateFC4PowerSimulator() {
    return static_cast<cIGZUnknown*>(static_cast<cFC4ISC4PowerSimulator*>(new cFC4PowerSimulator()));
}

// Test seam (FC4TestSeam.h, op 6): the power glue's department and line-item
// host callbacks against `budget` (in the tests, our cFC4 budget classes),
// including a department reference read back from a save. Returns 0, or the
// number of the first failed step.
intptr_t PowerTestBudgetPath(cISC4BudgetSimulator* budget) {
    constexpr uint32_t kDept = 0x8910BC8A;   // the power department (power spec §4.1)
    constexpr uint32_t kGroup = 0x4A357EAF;  // its budget group
    constexpr uint32_t kLinePoles = 1;
    constexpr uint32_t kIID_cISC4DepartmentBudget = 0x223E2B67;
    if (budget == nullptr) return 1;
    auto* sim = new cFC4PowerSimulator();
    sim->AddRef();
    sim->TestSetBudget(budget);  // borrowed; cleared before the release
    void* u = static_cast<FC4PowerGame*>(sim);
    intptr_t failed = 0;
    auto fail = [&](intptr_t step, const char* what) {
        if (failed == 0) {
            failed = step;
            fc4::Log("power budget path test: step %d FAILED: %s", static_cast<int>(step), what);
        }
    };
    const uint64_t dept = CreateDepartment(u, kDept, kGroup);  // borrowed
    if (dept == 0 || Department(u, kDept) != dept) fail(2, "CreateDepartmentBudget / GetDepartmentBudget");
    const uint64_t created = failed ? 0 : CreateLineItem(u, dept, kLinePoles, false);
    if (failed == 0 && (created == 0 || LineItem(u, dept, kLinePoles) != created)) fail(3, "CreateLineItem / GetLineItem");
    const SC4Percentage* own = failed ? nullptr : DeptOf(dept)->GetFundingPercentage();
    const float funding = own ? own->percentage : -1.0f;
    if (failed == 0 && (own == nullptr || DepartmentFunding(u, dept) != funding)) fail(4, "GetFundingPercentage");
    if (failed == 0) {
        // A save's department reference, as the stream adapter hands it to
        // the core, then resolved by the power host.
        cIGZUnknown* d = FromToken(dept);
        cIGZSerializable* ser = nullptr;
        fc4_ref ref{};
        if (!d->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) || ser == nullptr ||
            !fc4::OurObjectRef(ser, kIID_cISC4DepartmentBudget, &ref)) {
            fail(5, "department reference read");
        } else {
            const uint32_t before = d->AddRef() - 1;
            d->Release();
            const uint64_t t = ResolveRef(u, ref.token, kIID_cISC4DepartmentBudget);
            if (t != dept) fail(6, "the resolved department token is not the department interface");
            const uint32_t held = d->AddRef() - 1;
            d->Release();
            if (held != before + 1) fail(7, "the resolved department carries no reference for the core");
            if (failed == 0) {
                const uint64_t li = LineItem(u, t, kLinePoles);
                if (li != created) fail(8, "GetLineItem through the resolved department");
                if (failed == 0) {
                    SetLineItemFullExpenses(u, li, 12345);
                    if (ItemOf(li)->GetFullExpenses() != 12345) fail(9, "SetFullExpenses / GetFullExpenses");
                    if (DepartmentFunding(u, t) != funding) fail(10, "GetFundingPercentage through the resolved department");
                }
            }
            if (t != 0 && held == before + 1) FromToken(t)->Release();  // the core's reference
        }
    }
    sim->TestSetBudget(nullptr);
    sim->Release();
    fc4::Log("power budget path test: %s", failed ? "FAILED" : "passed");
    return failed;
}

cFC4PowerSimulator::cFC4PowerSimulator() {
    host_ = MakeHost(this, &gridCtx_);
    core_ = fc4_power_create(&host_, TokenOf(static_cast<cFC4ISC4PowerSimulator*>(this)));
    if (core_ == nullptr) fc4::Log("cFC4PowerSimulator: core creation failed");
    noDepartment_.percentage = 1.0f;
    fc4::Log("cFC4PowerSimulator(%p) created, core %p", static_cast<void*>(this), static_cast<void*>(core_));
}

cFC4PowerSimulator::~cFC4PowerSimulator() {
    held.clear();
    fc4_power_destroy(core_);  // releases what a never-initialised (loaded) object still holds
    core_ = nullptr;
    delete map_;
    map_ = nullptr;
    fc4::Log("cFC4PowerSimulator destroyed");
}

bool cFC4PowerSimulator::QueryInterface(uint32_t riid, void** ppvObj) {
    if (ppvObj == nullptr) return false;
    switch (riid) {
        case kFC4_IID_cISC4PowerSimulator:
        case GZIID_cIGZUnknown:
            *ppvObj = static_cast<cFC4ISC4PowerSimulator*>(this);
            break;
        case kIID_cIGZMessageTarget2:
            *ppvObj = static_cast<cIGZMessageTarget2*>(this);
            break;
        case GZIID_cIGZSerializable:
            *ppvObj = static_cast<cIGZSerializable*>(this);
            break;
        default:
            return false;  // §1: the out pointer is left untouched
    }
    AddRef();
    return true;
}

uint32_t cFC4PowerSimulator::AddRef() { return ++refCount_; }

// §1: at a count of 1 (or below) Shutdown, destroy, return 0.
uint32_t cFC4PowerSimulator::Release() {
    if (refCount_ > 1) return --refCount_;
    if (destroying_) return 0;  // a release from inside the final Shutdown
    destroying_ = true;
    refCount_ = 0;
    Shutdown();
    delete this;
    return 0;
}

uint64_t cFC4PowerSimulator::Occ(cISC4Occupant* occupant) const { return TokenOf(occupant); }

// §4.1 step 1: each service only while its member is still null.
void cFC4PowerSimulator::AcquireServices() {
    if (city == nullptr) {
        cIGZFrameWork* fw = RZGetFrameWork();
        cIGZApp* app = fw ? fw->Application() : nullptr;
        cISC4App* sc4 = nullptr;
        if (app != nullptr && app->QueryInterface(GZIID_cISC4App, reinterpret_cast<void**>(&sc4)) && sc4 != nullptr) {
            FC4_ACQUIRED(sc4, "QueryInterface");
            city = sc4->GetCity();
            FC4_ADDREF(city);
            FC4_RELEASE(sc4);
        }
    }
    if (city == nullptr) {
        fc4::Log("power: no city; services not acquired");
        return;
    }
    auto take = [](auto*& member, auto* value) {
        if (member == nullptr && value != nullptr) {
            member = value;
            FC4_ADDREF(member);
        }
    };
    take(occupantManager, city->GetOccupantManager());
    take(zoneManager, city->GetZoneManager());
    take(neighbors, city->GetNeighborsSimulator());
    take(budget, city->GetBudgetSimulator());
    take(demolition, city->GetDemolitionUtility());
}

// §4.2.
void cFC4PowerSimulator::ReleaseServices() {
    auto drop = [](auto*& member) {
        if (member != nullptr) FC4_RELEASE(member);
        member = nullptr;
    };
    drop(demolition);
    drop(budget);
    drop(neighbors);
    drop(zoneManager);
    drop(occupantManager);
    drop(city);
}

void cFC4PowerSimulator::Subscribe(const uint32_t* ids, size_t count, bool add) {
    if (ids == nullptr) return;
    if (add && msgServer_ == nullptr) {
        cIGZMessageServer2Ptr ms;
        if (ms) {
            msgServer_ = ms;
            FC4_ADDREF(msgServer_);
        }
    }
    if (msgServer_ == nullptr) return;
    for (size_t i = 0; i < count; ++i) {
        if (add) {
            msgServer_->AddNotification(Target(), ids[i]);
        } else {
            msgServer_->RemoveNotification(Target(), ids[i]);
        }
    }
    if (!add) {
        FC4_RELEASE(msgServer_);
        msgServer_ = nullptr;
    }
}

void cFC4PowerSimulator::LogState(const char* what) {
    fc4_power_state st{};
    st.struct_size = sizeof(st);
    if (!fc4_power_get_state(core_, &st)) return;
    fc4::Log("power %s: init=%u running=%u enabled=%u state=%u plants=%d poles=%u consumers=%u powered=%u "
             "unpowered=%u requirement=%u production=%u consumption=%u import=%u export=%u demand=%u deficit=%.3f "
             "department=%p grid=%p reentrant=%u",
             what, st.initialized, st.running, st.enabled, st.state, st.plant_count, st.pole_count, st.consumers,
             st.powered, st.unpowered, st.requirement, st.last_production, st.last_consumption, st.last_import,
             st.last_export, st.last_demand, st.deficit, static_cast<void*>(FromToken(st.department)),
             static_cast<void*>(FromToken(st.grid)), st.reentrant_calls);
    lastState_ = st.state;
}

// ---- lifecycle (§4) -----------------------------------------------------------------------------

bool cFC4PowerSimulator::Init() {
    FC4_TRACE("PowerSimulator::Init");
    CoreCall call(this);
    fc4_power_init(core_);
    LogState("Init");
    // Tooling: one report per recompute (plants, floods, consumers).
    report_ = fc4::SettingInt("power_report", "FC4_POWER_REPORT", fc4::kCheckedBuild ? 1 : 0) != 0;
    fc4_power_set_report(core_, report_);
    return true;
}

// The first kReportsInFull cycles are logged, then the first after each
// month.
void cFC4PowerSimulator::LogReport() {
    constexpr uint32_t kReportsInFull = 40;
    if (!report_) return;
    std::string text;
    auto sink = [](void* ctx, const uint8_t* data, size_t len) {
        static_cast<std::string*>(ctx)->append(reinterpret_cast<const char*>(data), len);
    };
    if (!fc4_power_take_report(core_, sink, &text)) return;
    if (reports_ >= kReportsInFull && !reportAfterMonth_) return;
    reportAfterMonth_ = false;
    ++reports_;
    size_t at = 0;
    while (at <= text.size()) {
        size_t nl = text.find('\n', at);
        if (nl == std::string::npos) nl = text.size();
        fc4::Log("%s", text.substr(at, nl - at).c_str());
        at = nl + 1;
    }
    if (reports_ == kReportsInFull) {
        fc4::Log("power: %u recompute reports logged; from now on the first after each month", reports_);
    }
}

bool cFC4PowerSimulator::Shutdown() {
    FC4_TRACE("PowerSimulator::Shutdown");
    // Keep this object alive while the simulator and the message server drop
    // their references (unless this already is the final Release).
    if (!destroying_) AddRef();
    {
        CoreCall call(this);
        fc4_power_shutdown(core_);
    }
    held.clear();
    fc4::Log("power Shutdown (%u months)", cycles_);
    if (!destroying_) Release();
    return true;
}

// ---- queries (§10.1) ----------------------------------------------------------------------------

void cFC4PowerSimulator::PositionToCell(float x, float z, int32_t& cellX, int32_t& cellZ) {
    fc4_power_position_to_cell(core_, x, z, &cellX, &cellZ);
}

void cFC4PowerSimulator::CellToPosition(int32_t cellX, int32_t cellZ, float& x, float& z) {
    fc4_power_cell_to_position(core_, cellX, cellZ, &x, &z);
}

// Slots 7 / 8: identity copies.
void cFC4PowerSimulator::CellToStandardCityCell(int32_t x, int32_t z, int32_t& outX, int32_t& outZ) {
    outX = x;
    outZ = z;
}

void cFC4PowerSimulator::StandardCityCellToCell(int32_t x, int32_t z, int32_t& outX, int32_t& outZ) {
    outX = x;
    outZ = z;
}

uint32_t cFC4PowerSimulator::GetAnnualConsumption() { return static_cast<uint32_t>(fc4_power_slot_value(core_, 9)); }
uint32_t cFC4PowerSimulator::GetAnnualConsumptionByPowerType(int32_t t) {
    return static_cast<uint32_t>(fc4_power_annual_consumption_by_type(core_, t));
}
int32_t cFC4PowerSimulator::GetMonthlyRequirement() { return static_cast<int32_t>(fc4_power_slot_value(core_, 11)); }
int32_t cFC4PowerSimulator::GetMonthlyProduction() { return static_cast<int32_t>(fc4_power_slot_value(core_, 12)); }
int32_t cFC4PowerSimulator::GetMonthlyImport() { return static_cast<int32_t>(fc4_power_slot_value(core_, 13)); }
int32_t cFC4PowerSimulator::GetMonthlyExport() { return static_cast<int32_t>(fc4_power_slot_value(core_, 14)); }
int32_t cFC4PowerSimulator::GetMonthlyDemand() { return static_cast<int32_t>(fc4_power_slot_value(core_, 15)); }
int32_t cFC4PowerSimulator::GetMonthlyConsumptionAt(int32_t x, int32_t z) {
    return fc4_power_consumption_at(core_, x, z);
}
int32_t cFC4PowerSimulator::GetMonthlyConsumptionTotal() {
    return static_cast<int32_t>(fc4_power_slot_value(core_, 17));
}

// Slot 18. [choice] No game caller exists (allocators.md, Unresolved); the
// spec keeps a vector<vector<int16>> for plugins. Ours is a snapshot of the
// core's map, rebuilt at each call in the game's layout (rows x outer, z
// inner, B buffers) and owned by this object. Without the game allocator
// (unknown executable) it writes null.
void cFC4PowerSimulator::GetMonthlyConsumptionMapPtr(void*& map) {
    map = nullptr;
    if (!fc4::StlNodeAllocator::Available()) return;
    if (map_ == nullptr) map_ = new ConsumptionMap();
    map_->rows.clear();
    const int32_t cx = city ? static_cast<int32_t>(city->CellCountX()) : 0;
    const int32_t cz = city ? static_cast<int32_t>(city->CellCountZ()) : 0;
    for (int32_t x = 0; x < cx; ++x) {
        FC4PowerConsumptionRow row{};
        if (cz > 0) {
            row.begin = static_cast<int16_t*>(fc4::StlNodeAllocator::Allocate(static_cast<uint32_t>(cz) * 2u));
            if (row.begin == nullptr) break;
            for (int32_t z = 0; z < cz; ++z) row.begin[z] = static_cast<int16_t>(fc4_power_consumption_at(core_, x, z));
            row.end = row.capacityEnd = row.begin + cz;
        }
        if (!map_->rows.push_back(row)) {
            fc4::StlNodeAllocator::Free(row.begin);
            break;
        }
    }
    map = &map_->rows;
}

uint32_t cFC4PowerSimulator::GetSupplyValue(int32_t x, int32_t z) { return fc4_power_is_powered(core_, x, z); }

void cFC4PowerSimulator::GetSupplyValueMapPtr(cISC4SimGrid<uint8_t>*& grid) {
    grid = reinterpret_cast<GridU8*>(FromToken(fc4_power_powered_grid(core_)));  // no AddRef
}

int32_t cFC4PowerSimulator::GetUnpoweredConsumerCount() {
    return static_cast<int32_t>(fc4_power_slot_value(core_, 21));
}
int32_t cFC4PowerSimulator::GetPoweredConsumerCount() { return static_cast<int32_t>(fc4_power_slot_value(core_, 22)); }
int32_t cFC4PowerSimulator::GetPowerPlantCountOfType(int32_t t) { return fc4_power_plant_count_of_type(core_, t); }
int32_t cFC4PowerSimulator::GetPowerPlantCountAll() { return static_cast<int32_t>(fc4_power_slot_value(core_, 24)); }

// Slot 25: appends every plant with one reference (allocators.md table (2):
// A nodes, the caller releases with occupant manager slot 48).
void cFC4PowerSimulator::GetPowerPlants(SC4List<cISC4Occupant*>& list) {
    fc4_power_plants(core_, &AppendPlant, &list);
}

uint32_t cFC4PowerSimulator::GetPowerPoleCount() { return static_cast<uint32_t>(fc4_power_slot_value(core_, 26)); }
int32_t cFC4PowerSimulator::GetOverworkedPowerPlantCount() {
    return static_cast<int32_t>(fc4_power_slot_value(core_, 27));
}
int32_t cFC4PowerSimulator::GetAgedPowerPlantCount() { return static_cast<int32_t>(fc4_power_slot_value(core_, 28)); }
int32_t cFC4PowerSimulator::GetDistressedPowerPlantCount() {
    return static_cast<int32_t>(fc4_power_slot_value(core_, 29));
}

// Slot 30: each pair only if found and its out pointer is non-null; the
// occupant comes with one reference (given back when nobody takes it).
void cFC4PowerSimulator::FindPowerPlants(cISC4Occupant** lowestFunded, SC4Percentage& funding,
                                         cISC4Occupant** mostWorn, int32_t& wearPercent) {
    CoreCall call(this);
    fc4_power_found f{};
    if (!fc4_power_find_plants(core_, &f)) return;
    if (f.has_lowest_funded) {
        if (lowestFunded != nullptr) {
            *lowestFunded = OccOf(f.lowest_funded);
            funding.percentage = f.lowest_funding;
        } else {
            ReleaseToken(this, f.lowest_funded);
        }
    }
    if (f.has_most_worn) {
        if (mostWorn != nullptr) {
            *mostWorn = OccOf(f.most_worn);
            wearPercent = f.wear_percent;
        } else {
            ReleaseToken(this, f.most_worn);
        }
    }
}

uint32_t cFC4PowerSimulator::IsOccupantPowered(cISC4Occupant* o) {
    return fc4_power_is_occupant_powered(core_, Occ(o));
}

bool cFC4PowerSimulator::GetOccupantPowerConsumed(cISC4Occupant* o, uint32_t& consumed) {
    return fc4_power_occupant_consumed(core_, Occ(o), &consumed);
}

bool cFC4PowerSimulator::GetOccupantPowerProduced(cISC4Occupant* o, uint32_t& produced, uint32_t& used) {
    return fc4_power_occupant_produced(core_, Occ(o), &produced, &used);
}

bool cFC4PowerSimulator::GetOccupantMaxPowerProduced(cISC4Occupant* o, uint32_t& maxProduced) {
    return fc4_power_occupant_max_produced(core_, Occ(o), &maxProduced);
}

cISC4SimGrid<uint8_t>* cFC4PowerSimulator::GetPoweredGridConst() const {
    return reinterpret_cast<GridU8*>(FromToken(fc4_power_powered_grid(core_)));
}

cISC4SimGrid<uint8_t>* cFC4PowerSimulator::GetPoweredGrid() {
    return reinterpret_cast<GridU8*>(FromToken(fc4_power_powered_grid(core_)));
}

uint32_t cFC4PowerSimulator::IsPowered(int32_t x, int32_t z) { return fc4_power_is_powered(core_, x, z); }
uint32_t cFC4PowerSimulator::IsWired(int32_t x, int32_t z) { return fc4_power_is_wired(core_, x, z); }
bool cFC4PowerSimulator::IsOnStrike() { return fc4_power_slot_value(core_, 39) != 0; }

// Slot 40 returns its argument's address (§10.1 SDK check 4).
SC4Percentage* cFC4PowerSimulator::GetChanceOfStrike(SC4Percentage& chance) {
    chance.percentage = fc4_power_chance_of_strike(core_);
    return &chance;
}

bool cFC4PowerSimulator::BeginStrike() { return fc4_power_slot_value(core_, 41) != 0; }
bool cFC4PowerSimulator::EndStrike() { return fc4_power_slot_value(core_, 42) != 0; }
int32_t cFC4PowerSimulator::GetStrikeStartDate() { return static_cast<int32_t>(fc4_power_slot_value(core_, 43)); }
int32_t cFC4PowerSimulator::GetStrikeLength() { return static_cast<int32_t>(fc4_power_slot_value(core_, 44)); }

SC4Percentage* cFC4PowerSimulator::GetCurrentPlantEfficiencyPercent(SC4Percentage* result, cISC4Occupant* o) {
    if (result != nullptr) result->percentage = fc4_power_plant_efficiency(core_, Occ(o));
    return result;
}

SC4Percentage* cFC4PowerSimulator::GetCurrentPlantConditionPercent(SC4Percentage* result, cISC4Occupant* o) {
    if (result != nullptr) result->percentage = fc4_power_plant_condition(core_, Occ(o));
    return result;
}

SC4Percentage* cFC4PowerSimulator::GetStrikeEfficiencyPercent() {
    strikeEfficiency_.percentage = fc4_power_strike_efficiency(core_);
    return &strikeEfficiency_;
}

// Slot 48: the department's GetFundingPercentage. [choice] Without a
// department (the original would crash) a member holding 1.0.
SC4Percentage* cFC4PowerSimulator::GetFundingPercent() {
    cFC4ISC4DepartmentBudget* d = DeptOf(fc4_power_department(core_));
    SC4Percentage* p = d ? d->GetFundingPercentage() : nullptr;
    return p ? p : &noDepartment_;
}

bool cFC4PowerSimulator::GetMonthlyPowerGenerationCost(cISC4Occupant* o, int64_t& cost) {
    return fc4_power_generation_cost(core_, Occ(o), &cost);
}

bool cFC4PowerSimulator::GetPowerPlantFundingPercentage(cISC4Occupant* o, SC4Percentage& funding) {
    return fc4_power_plant_funding(core_, Occ(o), &funding.percentage);
}

bool cFC4PowerSimulator::SetPowerPlantFundingPercentage(cISC4Occupant* o, SC4Percentage const& funding) {
    CoreCall call(this);
    return fc4_power_set_plant_funding(core_, Occ(o), funding.percentage);
}

// ---- messages (§4.3, §5) ------------------------------------------------------------------------

bool cFC4PowerSimulator::DoMessage(cIGZMessage2* pMessage) {
    if (pMessage == nullptr || core_ == nullptr) return true;
    FC4_X87_SCOPE("power DoMessage", pMessage->GetType());  // x87 leftover guard (FC4Fpu.h)
    CoreCall call(this);
    fc4_power_message m{};
    m.msg_type = pMessage->GetType();
    cIGZMessage2Standard* s = Query<cIGZMessage2Standard>(pMessage, GZIID_cIGZMessage2Standard);
    cISC4Occupant* occ = nullptr;
    if (s != nullptr) {
        m.data1 = static_cast<uint32_t>(s->GetData1());
        m.data2 = static_cast<uint32_t>(s->GetData2());
        m.data3 = static_cast<uint32_t>(s->GetData3());
        m.data4 = static_cast<uint32_t>(s->GetData4());
        if (m.msg_type == kMsgOccupantInserted || m.msg_type == kMsgOccupantRemoved) {
            // §5.1: the occupant is void1, known by its cISC4Occupant identity.
            occ = Query<cISC4Occupant>(static_cast<cIGZUnknown*>(s->GetVoid1()), kIID_cISC4Occupant);
            m.void1 = TokenOf(occ);
        } else if (m.msg_type == kMsgStateChanged || m.msg_type == kMsgDealStatus) {
            m.void1 = TokenOf(s->GetIGZUnknown());  // §5: the message's IGZUnknown
        }
        if (m.msg_type == kMsgZoningChanged) {
            // [abi] void2 is the message's SC4CellRegion<long>* (allocators.md;
            // owned by the message, C): its first member is the SC4Rect<long>
            // bounds {x1, z1, x2, z2}, read here and not kept.
            const int32_t* r = static_cast<const int32_t*>(s->GetVoid2());
            if (r != nullptr) {
                m.rect.x1 = r[0];
                m.rect.z1 = r[1];
                m.rect.x2 = r[2];
                m.rect.z2 = r[3];
            }
        }
    }
    // A message delivered while another call into the core is running is
    // queued by the core and handled when that call returns: keep its
    // objects alive until then.
    if (depth_ > 1) {
        cIGZUnknown* keep[] = {reinterpret_cast<cIGZUnknown*>(occ), FromToken(m.void1)};
        for (cIGZUnknown* k : keep) {
            if (k == nullptr) continue;
            k->AddRef();
            Hold(std::shared_ptr<void>(k, [](void* p) { static_cast<cIGZUnknown*>(p)->Release(); }));
        }
    }
    fc4_power_do_message(core_, &m);
    Drop(occ);
    Drop(s);
    LogReport();
    if (m.msg_type == kMsgSimNewMonth) {
        ++cycles_;
        LogState("month");
        reportAfterMonth_ = true;
    } else {
        fc4_power_state st{};
        st.struct_size = sizeof(st);
        if (fc4_power_get_state(core_, &st) && st.state != lastState_) {
            FC4_LOGV("power: state %u -> %u", lastState_, st.state);
            lastState_ = st.state;
        }
    }
    return true;  // §4.3: always
}

// ---- persistence (§9) ---------------------------------------------------------------------------

bool cFC4PowerSimulator::Write(cIGZOStream& s) {
    FC4_TRACE("PowerSimulator::Write");
    CoreCall call(this);
    fc4::OStreamAdapter a(s);
    const bool ok = fc4_power_write(core_, a.Get());
    fc4::Log("power Write -> %d", ok ? 1 : 0);
    return ok;
}

bool cFC4PowerSimulator::Read(cIGZIStream& s) {
    FC4_TRACE("PowerSimulator::Read");
    CoreCall call(this);
    fc4::IStreamAdapter a(s);
    const bool ok = fc4_power_read(core_, a.Get());
    fc4::Log("power Read -> %d", ok ? 1 : 0);
    LogState("Read");
    return ok;
}

uint32_t cFC4PowerSimulator::GetGZCLSID() { return kFC4_CLSID_cSC4PowerSimulator; }

intptr_t PowerTestLineConnections(cIGZUnknown* line, uint64_t* pair) {
    if (pair == nullptr) return -1;
    FC4PowerGame game;
    return LineConnectedPoles(&game, TokenOf(line), &pair[0], &pair[1]) ? 1 : 0;
}
