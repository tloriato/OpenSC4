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

// The fc4_budget_host callbacks: everything the budget core asks of the game
// (docs/specs/budget.md: city queries §4.1/§5, lots §5.2, history §5.1/§5.7,
// LTEXT names §2/§7/§8, holder properties §2, disasters §4.3, objects §1).
// One table serves the budget simulator, its departments and line items, so
// objects the game's class factory creates while loading a save work too.
//
// Bisection tooling (FC4Config.h): each behaviour that touches game objects
// has a switch; when it is off the callback is a no-op with a neutral result.
// References on game objects go through the audit (FC4RefAudit.h), and small
// buffers the game writes into are canary-guarded in checked builds
// (FC4Checked.h).

#include "FC4Checked.h"
#include "FC4Config.h"
#include "FC4Diag.h"
#include "FC4GameList.h"
#include "FC4SafeLotIterator.h"
#include "FC4Interfaces.h"
#include "FC4Properties.h"
#include "FC4Log.h"
#include "FC4Seed.h"
#include "FC4RefAudit.h"
#include "FC4Streams.h"
#include "FC4TestSeam.h"
#include "cFC4Budget.h"
#include "cFC4Message2Standard.h"
#include "FC4BudgetShadow.h"
#include "cFC4ZoneDeveloper.h"

#include "GZServPtrs.h"
#include "SC4List.h"
#include "StringResourceKey.h"
#include "StringResourceManager.h"
#include "cGZPersistResourceKey.h"
#include "cIGZApp.h"
#include "cIGZFrameWork.h"
#include "cIGZMessageServer2.h"
#include "cIGZString.h"
#include "cIGZVariant.h"
#include "cISC4App.h"
#include "cISC4BuildingDevelopmentSimulator.h"
#include "cISC4BuildingOccupant.h"
#include "cISC4City.h"
#include "cISC4DisasterLayer.h"
#include "cISC4DisasterManager.h"
#include "cISC4HistoryWarehouse.h"
#include "cISC4LandValueSimulator.h"
#include "cISC4Lot.h"
#include "cISC4LotManager.h"
#include "cISC4NeighborsSimulator.h"
#include "cISC4Occupant.h"
#include "cISC4OrdinanceSimulator.h"
#include "cISC4Simulator.h"
#include "cISCPropertyHolder.h"
#include "cRZAutoRefCount.h"
#include "cRZCOMDllDirector.h"

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>

#include <unordered_map>
#include <unordered_set>
#include <vector>

namespace fc4 {
namespace {

constexpr uint32_t kPropBuildingValue = 0xE91A0B5F;  // spec §5.2

cISC4City* g_city = nullptr;

// Test seam (FC4TestSeam.h): sees every posted message.
FC4PostSinkFn g_postSink = nullptr;
void* g_postSinkContext = nullptr;

// Game objects the budget core holds references on through add_ref (holder_refs).
std::unordered_map<cIGZUnknown*, long> g_coreRefs;

cISC4City* City() {
    if (g_city != nullptr) return g_city;
    // Before the budget's Init (e.g. objects created while a save loads).
    cIGZFrameWork* fw = RZGetFrameWork();
    cIGZApp* app = fw ? fw->Application() : nullptr;
    FC4_OUT(cISC4App*, sc4, "cISC4App* (QueryInterface out pointer)");
    const bool ok = app != nullptr && app->QueryInterface(GZIID_cISC4App, reinterpret_cast<void**>(sc4.ptr()));
    sc4.Check();
    if (!ok || sc4.get() == nullptr) return nullptr;
    FC4_ACQUIRED(sc4.get(), "QueryInterface");
    cISC4City* city = sc4.get()->GetCity();
    FC4_RELEASE(sc4.get());
    return city;
}

cISCPropertyHolder* Holder(uint64_t token) { return reinterpret_cast<cISCPropertyHolder*>(FromToken(token)); }

template <typename T>
int32_t CopyOut(bool ok, const std::vector<T>& v, T* out, size_t capacity) {
    if (!ok) return -1;
    for (size_t i = 0; i < v.size() && i < capacity; ++i) out[i] = v[i];
    return static_cast<int32_t>(v.size());
}

void Sink(const cIGZString* s, fc4_bytes_sink sink, void* ctx) {
    if (s == nullptr || sink == nullptr) return;
    const char* p = s->ToChar();
    if (p != nullptr) sink(ctx, reinterpret_cast<const uint8_t*>(p), s->Strlen());
}

// ---- callbacks -----------------------------------------------------------------------------------

void Post(void*, const fc4_budget_message* m) {
    if (m == nullptr) return;
    if (!On(Sw::MessagePosting)) {
        NoteSkipped(Sw::MessagePosting, "budget message post");
        return;
    }
    cIGZMessageServer2Ptr ms;
    if (!ms && g_postSink == nullptr) {
        FC4_LOGV("budget post 0x%08X skipped: no message server", m->type);
        return;
    }
    // Budget §10.1/§10.2 [compat]: void1 = IGZUnknown = the sender's primary
    // interface (the budget simulator, or the department as
    // cISC4DepartmentBudget*). void1 and data1 share one slot, so data1 reads
    // back as that pointer.
    cFC4Message2Standard* msg = FC4MakeMessage(m->type, static_cast<intptr_t>(m->data1),
                                               static_cast<intptr_t>(m->data2), static_cast<intptr_t>(m->data3));
    if (m->void1 != 0) msg->SetVoid1(FromToken(m->void1));
    if (m->unknown != 0) msg->SetIGZUnknown(FromToken(m->unknown));
    if (g_postSink != nullptr) g_postSink(g_postSinkContext, msg);
    const bool ok = ms ? ms->MessagePost(msg, false) : false;
    // What the receivers read (the message's own fields, not the core's request).
    FC4_LOGV("budget post 0x%08X d1=0x%08lX d2=%ld d3=%ld void1=%p unknown=%p -> %s", m->type,
             static_cast<unsigned long>(msg->GetData1()), static_cast<long>(msg->GetData2()),
             static_cast<long>(msg->GetData3()), msg->GetVoid1(), static_cast<void*>(msg->GetIGZUnknown()),
             ms ? (ok ? "true" : "FALSE") : "not posted (no message server; test sink only)");
    msg->Release();
}

void ShowBudgetWindow(void*, uint32_t panel) {
    // [choice] The UI is outside the simulation (spec §6).
    Log("budget: ShowBudgetWindow(panel %u) requested; the budget UI is not replaced", panel);
}

uint32_t Difficulty(void*) {
    cISC4City* c = City();
    return c ? static_cast<uint32_t>(c->GetDifficultyLevel()) : 0;
}

void SimDate(void*, int32_t* year, int32_t* month) {
    cISC4City* c = City();
    cFC4ISC4Simulator* s = c ? fc4::AsGame(c->GetSimulator()) : nullptr;
    if (s == nullptr) return;
    FC4_OUT(uint32_t, y, "uint32 year (GetSimDateParts)");
    FC4_OUT(uint32_t, m, "uint32 month (GetSimDateParts)");
    FC4_OUT(uint32_t, d, "uint32 day (GetSimDateParts)");
    FC4_OUT(uint32_t, doy, "uint32 day of year (GetSimDateParts)");
    FC4_OUT(uint32_t, wd, "uint32 weekday (GetSimDateParts)");
    s->GetSimDateParts(y.ptr(), m.ptr(), d.ptr(), doy.ptr(), wd.ptr());
    y.Check();
    m.Check();
    d.Check();
    doy.Check();
    wd.Check();
    *year = static_cast<int32_t>(y.get());
    *month = static_cast<int32_t>(m.get());
}

bool Established(void*) {
    cISC4City* c = City();
    return c != nullptr && c->GetEstablished();
}

void CitySize(void*, float* x, float* z) {
    if (cISC4City* c = City()) {
        *x = c->SizeX();
        *z = c->SizeZ();
    }
}

int64_t OrdinanceIncome(void*) {
    cISC4City* c = City();
    cISC4OrdinanceSimulator* o = c ? c->GetOrdinanceSimulator() : nullptr;
    return o ? o->GetOrdinanceMonthlyIncome() : 0;
}

int64_t OrdinanceExpense(void*) {
    cISC4City* c = City();
    cISC4OrdinanceSimulator* o = c ? c->GetOrdinanceSimulator() : nullptr;
    return o ? o->GetOrdinanceMonthlyExpense() : 0;
}

int64_t NeighborIncome(void*) {
    cISC4City* c = City();
    cFC4ISC4NeighborsSimulator* n = c ? fc4::AsGame(c->GetNeighborsSimulator()) : nullptr;  // int64, edx:eax (NG11)
    return n ? n->GetTotalMonthlyNeighborDealIncome() : 0;
}

int64_t NeighborExpense(void*) {
    cISC4City* c = City();
    cFC4ISC4NeighborsSimulator* n = c ? fc4::AsGame(c->GetNeighborsSimulator()) : nullptr;  // int64, edx:eax (NG11)
    return n ? n->GetTotalMonthlyNeighborDealExpense() : 0;
}

// The budget shadow's lot tap (SetBudgetLotTap): sees each lot just before
// its visit. Main thread only.
BudgetLotTapFn g_lotTap = nullptr;
void* g_lotTapContext = nullptr;
// Diagnostic tallies of the lots one scan visited (reset at each master-list scan).
uint32_t g_scanSeen = 0, g_scanBuilding = 0, g_scanLivable = 0, g_scanLand = 0, g_scanValue = 0;

// One lot of ForEachLot (§5.2), each read behind its lot_* sub-switch; an
// off switch leaves the neutral value in `info`.
void VisitLot(cFC4ISC4Lot* lot, cISC4LandValueSimulator* lv, fc4_lot_visitor visit, void* ctx) {
    if (g_lotTap != nullptr) g_lotTap(g_lotTapContext, lot);
    fc4_lot_info info{};
    cISC4BuildingOccupant* b = nullptr;
    if (On(Sw::LotBuilding)) {
        b = lot->GetBuilding();  // borrowed
    } else {
        NoteSkipped(Sw::LotBuilding, "cISC4Lot::GetBuilding (no building)");
    }
    info.has_building = b != nullptr;
    if (On(Sw::LotState)) {
        info.livable = lot->IsInLivableState();
        info.lot_wealth = lot->GetOccupantWealth();
    } else {
        NoteSkipped(Sw::LotState, "IsInLivableState / GetOccupantWealth (livable, wealth 0)");
        info.livable = true;  // [choice] so the rest of the scan still runs
    }
    if (On(Sw::LotSize)) {
        FC4_OUT(uint8_t, sx, "uint8 sizeX (cISC4Lot::GetSize)");
        FC4_OUT(uint8_t, sz, "uint8 sizeZ (cISC4Lot::GetSize)");
        sx.get() = 1;
        sz.get() = 1;
        lot->GetSize(sx.get(), sz.get());
        sx.Check();
        sz.Check();
        info.size_x = sx.get();
        info.size_z = sz.get();
    } else {
        NoteSkipped(Sw::LotSize, "cISC4Lot::GetSize (1 x 1)");
        info.size_x = 1;
        info.size_z = 1;
    }
    if (lv != nullptr && On(Sw::LotLandValue)) {
        FC4_OUT(int32_t, x, "int32 x (cISC4Lot::GetLocation)");
        FC4_OUT(int32_t, z, "int32 z (cISC4Lot::GetLocation)");
        const bool located = lot->GetLocation(x.get(), z.get());
        x.Check();
        z.Check();
        if (located) info.land_value = lv->GetLandValue(x.get(), z.get());
    } else if (lv != nullptr) {
        NoteSkipped(Sw::LotLandValue, "GetLocation / GetLandValue (land value 0)");
    }
    if (b != nullptr) {
        if (On(Sw::LotProfile)) {
            const cISC4BuildingOccupant::BuildingProfile& p = b->GetBuildingProfile();
            info.purpose = static_cast<uint32_t>(p.purpose);
            info.building_wealth = static_cast<uint32_t>(p.wealth);
        } else {
            NoteSkipped(Sw::LotProfile, "GetBuildingProfile (purpose 0, wealth 0)");
        }
        if (On(Sw::LotBuildingValue)) {
            FC4_OUT(cISCPropertyHolder*, h, "cISCPropertyHolder* (QueryInterface out pointer)");
            const bool has = b->QueryInterface(GZIID_cISCPropertyHolder, reinterpret_cast<void**>(h.ptr()));
            h.Check();
            if (has && h.get() != nullptr) {
                FC4_ACQUIRED(h.get(), "QueryInterface");
                std::vector<int64_t> v;
                if (props::GetI64s(h.get(), kPropBuildingValue, v) && !v.empty()) info.building_value = v[0];
                FC4_RELEASE(h.get());
            }
        } else {
            NoteSkipped(Sw::LotBuildingValue, "building QI + property 0xE91A0B5F (value 0)");
        }
    }
    // Per-scan summary counters (diagnostic only; logged with the scan line).
    ++g_scanSeen;
    if (info.has_building) ++g_scanBuilding;
    if (info.livable) ++g_scanLivable;
    if (info.land_value != 0) ++g_scanLand;
    if (info.building_value != 0) ++g_scanValue;
    visit(ctx, &info);
}

volatile LONG g_lotScans = 0;  // ForEachLot list scans so far

static_assert(sizeof(SC4ListNode<cISC4Lot*>) == 3 * sizeof(void*), "list node: next, prev, value");

// Every lot through GetLot(x, z, true) (borrowed, demand §6.3), each once; no
// game list, no references taken or released.
void WalkLotCells(cFC4ISC4LotManager* lm, int32_t maxX, int32_t maxZ, cISC4LandValueSimulator* lv,
                  fc4_lot_visitor visit, void* ctx) {
    std::unordered_set<cISC4Lot*> seen;
    for (int32_t x = 0; x <= maxX; ++x) {
        for (int32_t z = 0; z <= maxZ; ++z) {
            cISC4Lot* sdkLot = lm->GetLot(x, z, true);  // slot 6
            if (sdkLot == nullptr || !seen.insert(sdkLot).second) continue;
            VisitLot(AsGame(sdkLot), lv, visit, ctx);
        }
    }
}

// The GetLotsInCellRect path of ForEachLot on `list` (owned by the caller).
// [abi] The game allocates the list's nodes from its STL node allocator
// (docs/abi/allocators.md), so the list is a GameList: its head and nodes are
// allocated and freed there too, never through the allocator service.
void ScanLotList(cFC4ISC4LotManager* lm, GameList<cISC4Lot*>& list, int32_t maxX, int32_t maxZ,
                 cISC4LandValueSimulator* lv, fc4_lot_visitor visit, void* ctx, long scan) {
    const bool detail = kCheckedBuild && scan <= 3;
    SC4ListNode<cISC4Lot*>* const headBefore = list.head();
    const uint32_t returned = lm->GetLotsInCellRect(list, 0, 0, maxX, maxZ, false);  // slot 7
    if (detail) {
        SC4ListNode<cISC4Lot*>* head = list.head();
        char nodes[256] = "";
        size_t used = 0;
        SC4ListNode<cISC4Lot*>* n = head->next;
        for (int i = 0; i < 3 && n != head; ++i, n = n->next) {
            const int w = _snprintf_s(nodes + used, sizeof(nodes) - used, _TRUNCATE, "%s%p{next %p, prev %p, value %p}",
                                      i ? " " : "", static_cast<void*>(n), static_cast<void*>(n->next),
                                      static_cast<void*>(n->prev), static_cast<void*>(n->value));
            if (w < 0) break;
            used += static_cast<size_t>(w);
        }
        Log("budget lot scan #%ld: list %p, head %p (before the call %p; head next %p, prev %p), first nodes: %s", scan,
            static_cast<void*>(&list), static_cast<void*>(head), static_cast<void*>(headBefore),
            static_cast<void*>(head->next), static_cast<void*>(head->prev), used ? nodes : "-");
    }
    uint32_t iterated = 0, nulls = 0;
    for (cISC4Lot*& sdkLot : list) {
        ++iterated;
        if (sdkLot == nullptr) {
            ++nulls;
            continue;
        }
        FC4_ACQUIRED(sdkLot, "GetLotsInCellRect list entry");
        VisitLot(AsGame(sdkLot), lv, visit, ctx);
    }
    const bool release = On(Sw::LotRelease);
    if (!release) NoteSkipped(Sw::LotRelease, "Release of the list's lots (one reference leaked per lot)");
    for (cISC4Lot*& sdkLot : list) {
        if (sdkLot == nullptr) continue;
        if (release) FC4_RELEASE(sdkLot);
        sdkLot = nullptr;  // the list's destructor then only frees its nodes
    }
    if (detail || iterated != returned) {
        Log("budget lot scan #%ld: GetLotsInCellRect returned %u, ForEachLot iterated %u node(s) (%u null)%s", scan,
            returned, iterated, nulls, iterated != returned ? " MISMATCH" : "");
    } else {
        Log("budget lot scan #%ld: %u lots", scan, iterated);  // one line per scan (expected monthly)
    }
}

// ---- the master lot list (budget.md §5.2, BQ1 / BQ3) ------------------------------------------

// The walk ForEachLot used last (the budget shadow's report names it).
const char* g_lastLotWalk = "none";

// Registered safe iterators are few (one per aura simulator, lot developer,
// a caller's walk); a longer iterator ring is not trusted.
constexpr uint32_t kMaxRegisteredIterators = 4096;

struct MasterCopy {
    safelots::Block* block;
    const void* expectedList;
    void** out;
    uint32_t cap;
    uint32_t count;
    safelots::Status status;
    bool wrongList;
};

// Plain reads of the game's list (no game call): under SehGuard.
void MasterCopyFn(void* c) {
    auto* m = static_cast<MasterCopy*>(c);
    if (m->block != nullptr && m->block->listObject != m->expectedList) {
        m->wrongList = true;
        return;
    }
    m->status = safelots::CopyLots(m->block, m->out, m->cap, &m->count);
}

struct MasterRelease {
    safelots::Block* block;
    safelots::FreeFn freeNode;
    safelots::FreeFn freeBlock;
    int removed;
};

// Unlinks the block from the iterator ring, frees the nodes with B and the
// block with C (BQ4: whether a node matched or not): under SehGuard.
void MasterReleaseFn(void* c) {
    auto* m = static_cast<MasterRelease*>(c);
    m->removed = safelots::Release(m->block, m->freeNode, m->freeBlock, kMaxRegisteredIterators);
}

// The first few refusals are logged; ForEachLot then uses the rect query.
void MasterRefused(long scan, const char* why) {
    static volatile LONG refusals = 0;
    if (InterlockedIncrement(&refusals) <= 3) {
        Log("budget lot scan #%ld: master lot list not used (%s); GetLotsInCellRect instead (lot order: budget §5.2 "
            "[choice], the tax total within ±1 per 1000 lots)",
            scan, why);
    }
}

// Every lot of the original lot manager's master list, in list order, through
// its safe lot iterator (slot 15), as the original ComputeTax walks it
// (budget.md §5.2 BQ1: the same lots as the rect query; BQ3: the float32
// running total matches only in this order). Only on the identified
// executable (allocators B and C) and only for the original lot manager (its
// vtable in the executable's image; the block's list object must be lm +
// 0x5C, aura.md §6.5) or FreeCity4's (enable = zonedev: the block's list
// object must be its MasterListObject(); cFC4ZoneDeveloper::
// SafeIteratorListObject): any other replacement's slot 15 is never asked.
// The whole list is checked and copied before any lot is visited; any failure
// visits nothing and returns false, and the caller falls back. The lots are
// borrowed: the master list holds the manager's reference, and nothing here
// removes a lot. The block is always unregistered (the equivalent of
// std::list::remove, FC4SafeLotIterator.h) from its own list object, as the
// original does whatever that object is, and then freed with C whether a node
// matched or not, and also when the list object is null (budget.md 5.2,
// BQ4). [choice] Only after a faulted walk or release, or a ring that fails
// the checks, is it leaked, never freed (the original would fault there).
bool WalkMasterList(cFC4ISC4LotManager* lm, cISC4LandValueSimulator* lv, fc4_lot_visitor visit, void* ctx,
                    long scan) {
    const compat::ExecutableVersion* v = compat::IdentifiedVersion();
    compat::StlNodeFreeFn freeNode = nullptr;
    const uintptr_t freeBlockVa = compat::DirectCall(compat::Direct::GlobalHeapFree);
    if (v == nullptr || freeBlockVa == 0 || !compat::StlNodeAllocator(nullptr, &freeNode)) {
        MasterRefused(scan, "executable not identified, or allocator B / C unavailable");
        return false;
    }
    // The original lot manager's list object (+0x5C), or FreeCity4's mirror
    // of the master list in the same raw layout (enable = zonedev; its slot 15
    // hands out the same blocks, registered in its own iterator list, B nodes
    // and C blocks, so the release below is the same).
    const char* why = nullptr;
    const void* expectedList = cFC4ZoneDeveloper::SafeIteratorListObject(lm, &why);
    if (expectedList == nullptr) {
        MasterRefused(scan, why);
        return false;
    }
    const int32_t lotCount = lm->GetLotCount();  // slot 16
    if (lotCount < 0) {
        MasterRefused(scan, "negative GetLotCount");
        return false;
    }
    auto* block = reinterpret_cast<safelots::Block*>(lm->GetSafeLotIterator());  // slot 15
    if (block == nullptr) {
        MasterRefused(scan, "GetSafeLotIterator returned null");
        return false;
    }
    std::vector<void*> lots(static_cast<size_t>(lotCount) + 1);
    MasterCopy copy{block,
                    expectedList,
                    lots.data(),
                    static_cast<uint32_t>(lotCount),
                    0,
                    safelots::Status::NoBlock,
                    false};
    const bool copied = SehGuard("budget master lot list walk", &MasterCopyFn, &copy);
    // BQ4: the release does not depend on the walk's list check (ComputeTax
    // reads no lot manager field; it removes the block from the block's own
    // owner, or skips that for a null owner, and frees the block with C).
    const bool nullOwner = copied && block->listObject == nullptr;  // read without fault by the walk
    MasterRelease release{block, reinterpret_cast<safelots::FreeFn>(freeNode),
                          reinterpret_cast<safelots::FreeFn>(freeBlockVa), -1};
    const bool released = copied && SehGuard("budget safe lot iterator release", &MasterReleaseFn, &release);
    if (!released || release.removed < 0) {
        static volatile LONG leaks = 0;
        if (InterlockedIncrement(&leaks) <= 3) {
            Log("budget lot scan #%ld: WARNING the safe lot iterator %p could not be unregistered (%s); its block is "
                "leaked, not freed",
                scan, static_cast<void*>(block),
                !copied     ? "the walk faulted"
                : !released ? "the release faulted"
                            : "broken iterator ring");
        }
    } else if (release.removed == 0) {
        static volatile LONG unregistered = 0;
        if (InterlockedIncrement(&unregistered) <= 3) {
            Log("budget lot scan #%ld: the safe lot iterator %p was not in an iterator list (%s); its block was "
                "freed with C all the same (BQ4)",
                scan, static_cast<void*>(block), nullOwner ? "null list object" : "no node matched");
        }
    }
    if (!copied || copy.wrongList || copy.status != safelots::Status::Ok) {
        MasterRefused(scan, !copied ? "the walk faulted"
                            : copy.wrongList ? "unexpected list object"
                                             : safelots::StatusName(copy.status));
        return false;
    }
    uint32_t nulls = 0;
    g_scanSeen = g_scanBuilding = g_scanLivable = g_scanLand = g_scanValue = 0;
    for (uint32_t i = 0; i < copy.count; ++i) {
        cISC4Lot* sdkLot = static_cast<cISC4Lot*>(lots[i]);
        if (sdkLot == nullptr) {
            ++nulls;
            continue;
        }
        VisitLot(AsGame(sdkLot), lv, visit, ctx);
    }
    g_lastLotWalk = "master list";
    if (copy.count != static_cast<uint32_t>(lotCount) || nulls != 0) {
        Log("budget lot scan #%ld: master list %u lots (%u null), GetLotCount %d", scan, copy.count, nulls,
            static_cast<int>(lotCount));
    } else {
        Log("budget lot scan #%ld: %u lots (master list)", scan, copy.count);  // one line per scan
    }
    Log("budget lot scan #%ld: visited %u, with building %u, livable %u, land value>0 %u, building value>0 %u", scan,
        g_scanSeen, g_scanBuilding, g_scanLivable, g_scanLand, g_scanValue);
    return true;
}

void ForEachLot(void*, fc4_lot_visitor visit, void* ctx) {
    if (!On(Sw::LotScan)) {
        NoteSkipped(Sw::LotScan, "ForEachLot (no lots: property tax 0)");
        return;
    }
    cISC4City* c = City();
    cFC4ISC4LotManager* lm = c ? AsGame(c->GetLotManager()) : nullptr;
    if (lm == nullptr || visit == nullptr) return;
    cISC4LandValueSimulator* lv = c->GetLandValueSimulator();
    const int32_t maxX = static_cast<int32_t>(c->CellCountX()) - 1;
    const int32_t maxZ = static_cast<int32_t>(c->CellCountZ()) - 1;
    if (!On(Sw::LotList)) {
        // Bisection: no game list (neither the master list nor the rect query).
        NoteSkipped(Sw::LotList, "master lot list / GetLotsInCellRect (lots walked cell by cell instead)");
        g_lastLotWalk = "cells";
        WalkLotCells(lm, maxX, maxZ, lv, visit, ctx);
        return;
    }
    if (!StlNodeAllocator::Available()) {
        // [choice] Without the game's STL node allocator (unknown exe) the game
        // lists are refused; the cell walk visits the same lots.
        static LONG logged = 0;
        if (InterlockedExchange(&logged, 1) == 0) {
            Log("budget: WARNING the master lot list and GetLotsInCellRect refused (STL node allocator unavailable); "
                "lots walked cell by cell");
        }
        g_lastLotWalk = "cells";
        WalkLotCells(lm, maxX, maxZ, lv, visit, ctx);
        return;
    }
    const long scan = InterlockedIncrement(&g_lotScans);
    // BQ3 [compat]: the original's order, the master list (logged and refused
    // on anything unexpected, before any lot is visited).
    if (WalkMasterList(lm, lv, visit, ctx, scan)) return;
    // [choice] Every lot of the city through one rectangle query; the game
    // appends one reference per lot (budget Errata round 2, item 5), which we
    // release below (audited) before the list frees its nodes.
    g_lastLotWalk = "rect query";
    if (On(Sw::LotListFree)) {
        GameList<cISC4Lot*> lots;
        if (lots.valid()) ScanLotList(lm, lots, maxX, maxZ, lv, visit, ctx, scan);
    } else {
        // Bisection: the list (head and nodes) is never freed.
        NoteSkipped(Sw::LotListFree, "lot list destruction (list leaked)");
        auto* leaked = new GameList<cISC4Lot*>();
        if (leaked->valid()) ScanLotList(lm, *leaked, maxX, maxZ, lv, visit, ctx, scan);
    }
}

void RecordHistory(void*, uint32_t series, int32_t value) {
    if (!On(Sw::History)) {
        NoteSkipped(Sw::History, "RecordHistoryData");
        return;
    }
    cISC4City* c = City();
    if (cISC4HistoryWarehouse* h = c ? c->GetHistoryWarehouse() : nullptr) h->RecordHistoryData(series, value);
}

void SetHistoryQueueLength(void*, uint32_t series, uint32_t length) {
    if (!On(Sw::History)) {
        NoteSkipped(Sw::History, "SetHistoryDataQueueLength");
        return;
    }
    cISC4City* c = City();
    if (cISC4HistoryWarehouse* h = c ? c->GetHistoryWarehouse() : nullptr) h->SetHistoryDataQueueLength(series, length);
}

bool TryStartDisaster(void*, uint32_t id, float x, float z) {
    if (!On(Sw::Disaster)) {
        NoteSkipped(Sw::Disaster, "AttemptDisasterStart (not started)");
        return false;
    }
    cISC4City* c = City();
    cISC4DisasterLayer* layer = c ? c->GetDisasterLayer() : nullptr;
    cISC4DisasterManager* dm = layer ? layer->GetDisasterManager(id) : nullptr;
    if (dm == nullptr) return false;
    // Budget §4.3: integer WORLD coordinates, truncated toward zero (the
    // earthquake converts to cells itself); compiled slot 6, foundations §17.8.
    const bool started = dm->AttemptDisasterStart(static_cast<int32_t>(x), static_cast<int32_t>(z)) != nullptr;
    Log("budget: disaster 0x%08X at (%.0f, %.0f): %s", id, x, z, started ? "started" : "not started");
    return started;
}

// The session seed (foundations §6.3, FC4Seed.h): the RiskyMoney generator
// takes derive(S, 0x6C09C3D7).
uint32_t SessionSeedCb(void*) { return fc4::SessionSeed(); }

bool Ltext(void*, uint32_t group, uint32_t instance, fc4_bytes_sink sink, void* ctx) {
    if (!On(Sw::Ltext)) {
        NoteSkipped(Sw::Ltext, "LTEXT lookup (not found)");
        return false;
    }
    // The SDK helper owns the reference (cRZAutoRefCount; not audited).
    cRZAutoRefCount<cIGZString> s;
    if (!StringResourceManager::GetLocalizedString(StringResourceKey(group, instance), s)) return false;
    Sink(s, sink, ctx);
    return true;
}

bool BuildingTypeName(void*, uint32_t type, fc4_bytes_sink sink, void* ctx) {
    if (!On(Sw::BuildingName)) {
        NoteSkipped(Sw::BuildingName, "GetBuildingName (empty name)");
        return true;
    }
    // Budget Errata round 2, item 2: the building-development simulator's
    // user-visible name of the type (compiled slot 38). The string belongs to
    // the simulator: copied, not released. Never-loaded types give "".
    cISC4City* c = City();
    cISC4BuildingDevelopmentSimulator* bd = c ? c->GetBuildingDevelopmentSimulator() : nullptr;
    if (bd == nullptr) return false;
    Sink(bd->GetBuildingName(type), sink, ctx);
    return true;
}

// Property reads and writes: the switches live in FC4Properties.cpp.
int32_t GetU32s(void*, uint64_t h, uint32_t p, uint32_t* out, size_t cap) {
    std::vector<uint32_t> v;
    return CopyOut(props::GetU32s(Holder(h), p, v), v, out, cap);
}

int32_t GetI64s(void*, uint64_t h, uint32_t p, int64_t* out, size_t cap) {
    std::vector<int64_t> v;
    return CopyOut(props::GetI64s(Holder(h), p, v), v, out, cap);
}

int32_t GetF32s(void*, uint64_t h, uint32_t p, float* out, size_t cap) {
    std::vector<float> v;
    return CopyOut(props::GetF32s(Holder(h), p, v), v, out, cap);
}

bool SetU32s(void*, uint64_t h, uint32_t p, const uint32_t* v, size_t n) {
    return props::SetU32s(Holder(h), p, std::vector<uint32_t>(v, v + n));
}

bool SetI64s(void*, uint64_t h, uint32_t p, const int64_t* v, size_t n) {
    return props::SetI64s(Holder(h), p, std::vector<int64_t>(v, v + n));
}

bool SetF32s(void*, uint64_t h, uint32_t p, const float* v, size_t n) {
    return props::SetF32s(Holder(h), p, std::vector<float>(v, v + n));
}

bool CopyProperty(void*, uint64_t src, uint64_t dst, uint32_t p) { return props::Copy(Holder(src), Holder(dst), p); }

// ABI 50 (data-formats §5.2, EX39): whether a property is an array (1), a
// scalar (0) or absent (-1). The core reads the budget item arrays as the
// original's vector helpers do, a scalar (count 0) giving an empty vector;
// GetI64s and friends keep converting a scalar to one value for the tuning.
int32_t PropertyIsArray(void*, uint64_t h, uint32_t p) {
    const uint16_t type = props::TypeOf(Holder(h), p);
    if (type == 0) return -1;
    return (type & cIGZVariant::TypeArray) != 0 ? 1 : 0;
}

uint64_t OccupantOf(void*, uint64_t h) {
    cIGZUnknown* u = FromToken(h);
    if (u == nullptr) return 0;
    FC4_OUT(void*, occ, "cISC4Occupant* (QueryInterface out pointer)");
    const bool ok = u->QueryInterface(GZIID_cISC4Occupant, occ.ptr());
    occ.Check();
    if (!ok || occ.get() == nullptr) return 0;
    cISC4Occupant* o = static_cast<cISC4Occupant*>(occ.get());
    FC4_ACQUIRED(o, "QueryInterface");
    const uint64_t t = TokenOf(static_cast<cIGZUnknown*>(o));
    FC4_RELEASE(o);  // borrowed: the holder keeps the occupant alive
    return t;
}

uint32_t ClassOf(void*, uint64_t token) {
    cIGZUnknown* u = FromToken(token);
    if (u == nullptr) return 0;
    FC4_OUT(cIGZSerializable*, s, "cIGZSerializable* (QueryInterface out pointer)");
    const bool ok = u->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(s.ptr()));
    s.Check();
    if (!ok || s.get() == nullptr) return 0;
    FC4_ACQUIRED(s.get(), "QueryInterface");
    const uint32_t c = s.get()->GetGZCLSID();
    FC4_RELEASE(s.get());
    return c;
}

// The core's own references. Our objects always pass through. Game objects
// (locally funded holders, objects read from the save) obey holder_refs; a
// reference taken while the switch was on is always given back.
void AddRefToken(void*, uint64_t t) {
    cIGZUnknown* u = FromToken(t);
    if (u == nullptr) return;
    if (IsOurObject(u)) {
        u->AddRef();
        return;
    }
    if (!On(Sw::HolderRefs)) {
        NoteSkipped(Sw::HolderRefs, "AddRef of a game object for the budget core");
        return;
    }
    ++g_coreRefs[u];
    FC4_ADDREF(u);
}

void ReleaseToken(void*, uint64_t t) {
    cIGZUnknown* u = FromToken(t);
    if (u == nullptr) return;
    if (IsOurObject(u)) {
        u->Release();
        return;
    }
    auto it = g_coreRefs.find(u);
    if (it != g_coreRefs.end()) {
        if (--it->second == 0) g_coreRefs.erase(it);
    } else if (!On(Sw::HolderRefs)) {
        NoteSkipped(Sw::HolderRefs, "Release of a game object for the budget core");
        return;
    }
    FC4_RELEASE(u);
}

void* CreateObject(void*, uint32_t classId) {
    // [choice] Our own classes directly (the class registry would route these
    // CLSIDs to us anyway while the budget subsystem is enabled).
    if (classId == kFC4_CLSID_cSC4DepartmentBudget) {
        cFC4DepartmentBudget* d = new cFC4DepartmentBudget();
        d->AddRef();  // owned by the core
        return d->CoreHandle();
    }
    if (classId == kFC4_CLSID_cSC4LineItem) {
        cFC4LineItem* l = new cFC4LineItem();
        l->AddRef();
        return l->CoreHandle();
    }
    Log("budget: create_object(0x%08X) refused", classId);
    return nullptr;
}

fc4_budget_host MakeHost() {
    fc4_budget_host h{};
    h.struct_size = sizeof(h);
    h.user = nullptr;
    h.post = &Post;
    h.show_budget_window = &ShowBudgetWindow;
    h.difficulty = &Difficulty;
    h.sim_date = &SimDate;
    h.city_established = &Established;
    h.city_size = &CitySize;
    h.ordinance_income = &OrdinanceIncome;
    h.ordinance_expense = &OrdinanceExpense;
    h.neighbor_income = &NeighborIncome;
    h.neighbor_expense = &NeighborExpense;
    h.for_each_lot = &ForEachLot;
    h.record_history = &RecordHistory;
    h.set_history_queue_length = &SetHistoryQueueLength;
    h.try_start_disaster = &TryStartDisaster;
    h.session_seed = &SessionSeedCb;
    h.ltext = &Ltext;
    h.building_type_name = &BuildingTypeName;
    h.get_u32s = &GetU32s;
    h.get_i64s = &GetI64s;
    h.get_f32s = &GetF32s;
    h.set_u32s = &SetU32s;
    h.set_i64s = &SetI64s;
    h.set_f32s = &SetF32s;
    h.copy_property = &CopyProperty;
    h.occupant_of = &OccupantOf;
    h.class_of = &ClassOf;
    h.add_ref = &AddRefToken;
    h.release = &ReleaseToken;
    h.create_object = &CreateObject;
    h.property_is_array = &PropertyIsArray;
    return h;
}

// The budget shadow's table: the read callbacks only. Every callback that
// could change game state (posts, history, property writes and copies,
// disasters, references, object creation) is NULL, which the core reads as
// "nothing there" (FC4BudgetShadow.h).
fc4_budget_host MakeReadOnlyHost() {
    fc4_budget_host h{};
    h.struct_size = sizeof(h);
    h.user = nullptr;
    h.difficulty = &Difficulty;
    h.sim_date = &SimDate;
    h.city_established = &Established;
    h.city_size = &CitySize;
    h.ordinance_income = &OrdinanceIncome;
    h.ordinance_expense = &OrdinanceExpense;
    h.neighbor_income = &NeighborIncome;
    h.neighbor_expense = &NeighborExpense;
    h.for_each_lot = &ForEachLot;
    h.ltext = &Ltext;
    h.building_type_name = &BuildingTypeName;
    h.get_u32s = &GetU32s;
    h.get_i64s = &GetI64s;
    h.get_f32s = &GetF32s;
    h.occupant_of = &OccupantOf;
    h.class_of = &ClassOf;
    h.property_is_array = &PropertyIsArray;
    return h;
}

}  // namespace

const fc4_budget_host* BudgetHost() {
    static const fc4_budget_host host = MakeHost();
    return &host;
}

const fc4_budget_host* BudgetReadOnlyHost() {
    static const fc4_budget_host host = MakeReadOnlyHost();
    return &host;
}

void SetBudgetLotTap(BudgetLotTapFn fn, void* context) {
    g_lotTapContext = context;
    g_lotTap = fn;
}

void SetBudgetCity(cISC4City* city) { g_city = city; }

const char* BudgetLastLotWalk() { return g_lastLotWalk; }

void SetBudgetPostSink(FC4PostSinkFn fn, void* context) {
    g_postSinkContext = context;
    g_postSink = fn;
}

void LogBudgetCoreRefs(const char* why) {
    long total = 0;
    for (const auto& kv : g_coreRefs) total += kv.second;
    Log("budget core (%s): holds %ld reference(s) on %u game object(s) taken through add_ref", why, total,
        static_cast<unsigned>(g_coreRefs.size()));
}

}  // namespace fc4
