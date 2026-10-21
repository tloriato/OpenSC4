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

// The occupant manager's GZCOM class: see cFC4OccMgr.h and
// docs/specs/occupant-manager.md.

#include "cFC4OccMgr.h"

#include "FC4Diag.h"
#include "FC4Fpu.h"
#include "FC4Log.h"
#include "FC4RefAudit.h"
#include "FC4Streams.h"

#include "cIGZMessage2.h"
#include "cISC4Occupant.h"
#include "cISC4OccupantFilter.h"
#include "cS3DVector3.h"
#include "SC4List.h"
#include "SC4Point.h"

#include <cstdio>
#include <cstring>
#include <vector>

namespace {

cISC4Occupant* Occ(uint64_t t) {
    return reinterpret_cast<cISC4Occupant*>(static_cast<uintptr_t>(t));
}
uint64_t TokenOf(cISC4Occupant* o) { return static_cast<uint64_t>(reinterpret_cast<uintptr_t>(o)); }

// The manager box the occupant stores (§4.3): 4 used bytes of an 8-byte
// buffer; the base version's "no box" mark is a first byte of 0xFF.
bool ReadOccBox(cISC4Occupant* o, fc4_occ_cell_box* out) {
    uint8_t b[8] = {0, 0, 0, 0, 0, 0, 0, 0};
    if (!o->GetOccupantManagerBBox(b)) return false;  // occupant slot 25
    out->x0 = b[0];
    out->z0 = b[1];
    out->x1 = b[2];
    out->z1 = b[3];
    return true;
}

void WriteOccBox(cISC4Occupant* o, const fc4_occ_cell_box* box) {
    uint8_t b[8] = {0, 0, 0, 0, 0, 0, 0, 0};
    b[0] = box->x0;
    b[1] = box->z0;
    b[2] = box->x1;
    b[3] = box->z1;
    o->SetOccupantManagerBBox(b);  // occupant slot 26
}

// A filter argument as the core takes it (fc4.h "fc4_occ_filter").
fc4_occ_filter FilterOf(cISC4OccupantFilter* f) {
    fc4_occ_filter d{};
    if (f == nullptr) {
        d.kind = FC4_OCC_FILTER_ALL;
    } else {
        d.kind = FC4_OCC_FILTER_HOST;
        d.token = static_cast<uint64_t>(reinterpret_cast<uintptr_t>(f));
    }
    return d;
}
fc4_occ_filter TypeOf(uint32_t occType) {
    fc4_occ_filter d{};
    d.kind = FC4_OCC_FILTER_TYPE;
    d.occ_type = occType;
    return d;
}

}  // namespace

// ---- host callbacks (fc4_occmgr_host) -------------------------------------------------------------
//
// Every occupant the core holds arrives as its cISC4Occupant pointer; the
// reference the core takes is ours to account for (HANDOFF §5.1b), so each
// AddRef is noted and each Release is balanced. The host does not create or
// Release anything the core did not ask for.
namespace {

// Diagnostics (glue25fix3): query tallies, and a per-occupant trace of the
// callbacks made while the manager shuts down.
bool g_traceShutdown = false;
uint32_t g_traceN = 0;
uint64_t g_firstCalls = 0, g_firstHits = 0, g_listCalls = 0, g_listItems = 0, g_iterItems = 0;
uint32_t g_samples = 0, g_sampleCalls = 0;
fc4_occmgr* g_sampleCore = nullptr;
void SampleSink(void* ctx, uint64_t occ) { static_cast<std::vector<uint64_t>*>(ctx)->push_back(occ); }
// The first-query sampler found the slot 28-35 miss (fdd2a5ef); kept, off by
// default (kSampleFirstQueries), for the next diagnosis.
constexpr bool kSampleFirstQueries = false;
bool Sample(const char* slot, double a, double b, bool hit, uint32_t type = 0, int32_t c = 0, int32_t d = 0) {
    if (kSampleFirstQueries && g_samples < 40 && (g_sampleCalls++ % 397) == 0) {
        ++g_samples;
        char extra[512] = "";
        // For a standard-cell miss: what a null-filter list query of that cell holds.
        if (!hit && g_sampleCore != nullptr && (slot[1] == '2' || slot[1] == '3') && slot[0] == '3') {
            const int32_t p[2] = {static_cast<int32_t>(a), static_cast<int32_t>(b)};
            fc4_occ_filter all{};
            all.kind = FC4_OCC_FILTER_ALL;
            std::vector<uint64_t> got;
            fc4_occmgr_occupants_by_standard_cells(g_sampleCore, p, p, &all, -1, &SampleSink, &got);
            uint64_t f1 = 0, f2 = 0;
            const bool r1 = fc4_occmgr_first_by_standard_cell(g_sampleCore, p[0], p[1], &all, &f1);
            int32_t wx = 0, wz = 0;
            fc4_occ_position_to_world_cell(p[0] * 16.0f + 8.0f, p[1] * 16.0f + 8.0f, &wx, &wz);
            const bool r2 = fc4_occmgr_first_in_cell(g_sampleCore, wx, wz, &all, &f2);
            if (type != 0) {
                uint64_t f3 = 0, f4 = 0;
                fc4_occ_filter tf{};
                tf.kind = FC4_OCC_FILTER_TYPE;
                tf.occ_type = type;
                const bool r3 = fc4_occmgr_first_by_standard_cell_typed(g_sampleCore, p[0], p[1], type, &f3);
                const bool r4 = fc4_occmgr_first_by_standard_cell(g_sampleCore, p[0], p[1], &tf, &f4);
                fc4::Log("occmgr first-query recheck typed %d (%p) type-filter %d (%p) first-all %p", r3 ? 1 : 0,
                         reinterpret_cast<void*>(static_cast<uintptr_t>(f3)), r4 ? 1 : 0,
                         reinterpret_cast<void*>(static_cast<uintptr_t>(f4)),
                         reinterpret_cast<void*>(static_cast<uintptr_t>(f1)));
                if (f3 != 0) FC4_RELEASE(reinterpret_cast<cISC4Occupant*>(static_cast<uintptr_t>(f3)));
                if (f4 != 0) FC4_RELEASE(reinterpret_cast<cISC4Occupant*>(static_cast<uintptr_t>(f4)));
            }
            if (f1 != 0) FC4_RELEASE(reinterpret_cast<cISC4Occupant*>(static_cast<uintptr_t>(f1)));
            if (f2 != 0) FC4_RELEASE(reinterpret_cast<cISC4Occupant*>(static_cast<uintptr_t>(f2)));
            int n = std::snprintf(extra, sizeof(extra), " | all-filter slot32 %d, slot30(%d,%d) %d, empty %d | cell holds %u:",
                                  r1 ? 1 : 0, wx, wz, r2 ? 1 : 0, fc4_occmgr_is_cell_empty(g_sampleCore, wx, wz) ? 1 : 0,
                                  static_cast<unsigned>(got.size()));
            for (size_t i = 0; i < got.size(); ++i) {
                cISC4Occupant* o = reinterpret_cast<cISC4Occupant*>(static_cast<uintptr_t>(got[i]));
                cS3DVector3 mn(0, 0, 0), mx(0, 0, 0);
                o->GetBoundingBox(&mn, &mx);
                if (i < 4 && n > 0 && n < 400)
                    n += std::snprintf(extra + n, sizeof(extra) - n, " %08X [%.1f,%.1f..%.1f,%.1f]",
                                       static_cast<uint32_t>(o->GetType()), mn.fX, mn.fZ, mx.fX, mx.fZ);
                FC4_RELEASE(o);
            }
        }
        fc4::Log("occmgr first-query sample #%u slot %s args %.2f %.2f type %08X max %d %d -> %s%s", g_samples, slot,
                 a, b, type, c, d, hit ? "hit" : "miss", extra);
    }
    return hit;
}
void TraceStep(const char* step, uint64_t t) {
    if (!g_traceShutdown) return;
    FC4_LOGV("occmgr shutdown trace #%u %s occ %p", g_traceN, step, reinterpret_cast<void*>(static_cast<uintptr_t>(t)));
}

void HAddRef(void*, uint64_t t) {
    if (t == 0) return;
    cISC4Occupant* o = Occ(t);
    o->AddRef();  // occupant slot 1
    FC4_ACQUIRED(o, "occmgr core add_ref");
}
void HRelease(void*, uint64_t t) {
    if (t == 0) return;
    cISC4Occupant* o = Occ(t);
    TraceStep("release", t);
    FC4_RELEASE(o);  // occupant slot 2, the core's reference
}
void HInit(void*, uint64_t t) {
    if (t != 0) Occ(t)->Init();  // occupant slot 3: the base posts Create (§5.6)
}
void HShutdown(void*, uint64_t t) {
    TraceStep("shutdown", t);
    if (t != 0) Occ(t)->Shutdown();  // occupant slot 4: the base posts Destroy
}
// AsPropertyHolder is a borrowed pointer (no reference of ours).
bool HHasHolder(void*, uint64_t t) {
    return t != 0 && Occ(t)->AsPropertyHolder() != nullptr;  // occupant slot 6
}
uint32_t HType(void*, uint64_t t) { return t == 0 ? 0u : static_cast<uint32_t>(Occ(t)->GetType()); }

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

void HPost(void*, uint64_t t, uint32_t msg_type, uint32_t data4) {
    if (g_traceShutdown && t != 0) {
        ++g_traceN;
        FC4_LOGV("occmgr shutdown trace #%u post %08X occ %p type %08X", g_traceN, msg_type,
                 reinterpret_cast<void*>(static_cast<uintptr_t>(t)), static_cast<uint32_t>(Occ(t)->GetType()));
    }
    if (t != 0) Occ(t)->PostOccupantMessage(msg_type, data4);  // occupant slot 15
}

bool HGetBox(void*, uint64_t t, fc4_occ_cell_box* out) {
    return t != 0 && ReadOccBox(Occ(t), out);
}
void HSetBox(void*, uint64_t t, const fc4_occ_cell_box* b) {
    if (t != 0 && b != nullptr) WriteOccBox(Occ(t), b);
}
void HSetFlag(void*, uint64_t t, uint32_t f) {
    if (t != 0) Occ(t)->SetFlag(f);  // occupant slot 29
}
void HClearFlag(void*, uint64_t t, uint32_t f) {
    if (t != 0) Occ(t)->ClearFlag(f);  // occupant slot 31
}
bool HIsFlagSet(void*, uint64_t t, uint32_t f) {
    return t != 0 && Occ(t)->IsFlagSet(f);  // occupant slot 32
}
bool HFilter(void*, uint64_t filter, uint64_t occ) {
    if (filter == 0) return true;
    cISC4OccupantFilter* f = reinterpret_cast<cISC4OccupantFilter*>(static_cast<uintptr_t>(filter));
    return f->IsOccupantIncluded(Occ(occ));  // filter slot 3
}

// Read (§13 step 4.1): the reference the stream handed over is the manager's
// one (IStreamAdapter::ReadRef already QI'd to cISC4Occupant).
uint64_t HResolve(void*, uint64_t token, uint32_t) { return token; }

fc4_occmgr_host MakeHost(cFC4OccMgr* self) {
    fc4_occmgr_host h{};
    h.struct_size = sizeof(h);
    h.user = self;
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

}  // namespace

// ---- the class ----------------------------------------------------------------------------------

cFC4OccMgr::cFC4OccMgr() {
    host_ = MakeHost(this);
    core_ = fc4_occmgr_create(&host_);
}

cFC4OccMgr::~cFC4OccMgr() {
    fc4::Log("occmgr destroyed (core %p)", static_cast<void*>(core_));
    if (core_ != nullptr) {
        fc4_occmgr_destroy(core_);
        core_ = nullptr;
    }
}

void cFC4OccMgr::SetCity(cISC4City* c) { city_ = c; }

// §1: 1 and 0xC6BB8334 at +0, 0xE4FDA3D4 (cIGZSerializable) at +4 and
// 0x452294AA (cIGZMessageTarget2) at +8. The core names the offset.
bool cFC4OccMgr::QueryInterface(uint32_t riid, void** ppvObj) {
    if (ppvObj == nullptr) return false;
    *ppvObj = nullptr;
    const int32_t off = fc4_occmgr_query_interface(riid);
    switch (off) {
        case 0:
            *ppvObj = Primary();
            break;
        case 4:
            *ppvObj = Serializable();
            break;
        case 8:
            *ppvObj = Target();
            break;
        default:
            return false;
    }
    AddRef();
    return true;
}

uint32_t cFC4OccMgr::AddRef() {
    refCount_ = fc4_occmgr_add_ref(core_);
    return refCount_;
}

// §2.4: at the last reference the core shuts down; the deleting destructor
// (slot 49) then frees this object.
uint32_t cFC4OccMgr::Release() {
    const uint32_t n = fc4_occmgr_release(core_);
    refCount_ = n;
    if (n == 0) delete this;
    return n;
}

// ---- slots 3-19: lifecycle and conversions -------------------------------------------------------

bool cFC4OccMgr::Init() {
    g_sampleCore = core_;
    const bool ok = fc4_occmgr_init(core_);
    fc4::Log("occmgr Init -> %d", ok ? 1 : 0);
    return ok;
}
bool cFC4OccMgr::Shutdown() {
    fc4::Log("occmgr Shutdown: enter (queries: first %llu calls %llu hits, list %llu calls %llu items, iterate %llu items)",
             static_cast<unsigned long long>(g_firstCalls), static_cast<unsigned long long>(g_firstHits),
             static_cast<unsigned long long>(g_listCalls), static_cast<unsigned long long>(g_listItems),
             static_cast<unsigned long long>(g_iterItems));
    g_traceShutdown = true;
    g_traceN = 0;
    const bool ok = fc4_occmgr_shutdown(core_);
    g_traceShutdown = false;
    fc4::Log("occmgr Shutdown -> %d (%u occupants traced)", ok ? 1 : 0, g_traceN);
    return ok;
}
bool cFC4OccMgr::SetCitySize(float fx, float fz) { return fc4_occmgr_set_city_size(core_, fx, fz); }

// Slots 6-19 do not set AL in the original (§3.2 quirk). [choice] We return
// true; no shipped caller tests it.
bool cFC4OccMgr::GetOccupantManagerCellSizes(float& x, float& z) {
    fc4_occmgr_get_cell_sizes(core_, &x, &z);
    return true;
}
bool cFC4OccMgr::GetWorldCellCount(int32_t& x, int32_t& z) {
    fc4_occmgr_get_world_cell_count(core_, &x, &z);
    return true;
}
bool cFC4OccMgr::GetOccupantManagerCellCount(int32_t& x, int32_t& z) {
    fc4_occmgr_get_cell_count(core_, &x, &z);
    return true;
}
bool cFC4OccMgr::GetOccupantManagerCellBounds(int32_t& x0, int32_t& z0, int32_t& x1, int32_t& z1) {
    int32_t out[4] = {0, 0, 0, 0};
    fc4_occmgr_get_cell_bounds(core_, out);
    x0 = out[0];
    z0 = out[1];
    x1 = out[2];
    z1 = out[3];
    return true;
}
bool cFC4OccMgr::WorldCellToOccupantManagerCell(int32_t x, int32_t z, int32_t& ox, int32_t& oz) {
    fc4_occ_world_cell_to_manager_cell(x, z, &ox, &oz);
    return true;
}
bool cFC4OccMgr::OccupantManagerCellToWorldCell(int32_t x, int32_t z, int32_t& ox, int32_t& oz) {
    fc4_occ_manager_cell_to_world_cell(x, z, &ox, &oz);
    return true;
}
// Slot 12 is wrong in the original (§3.2, §15 quirk 1) and nothing calls it
// (OM3). Reproduced exactly: outX = 4*z - 256 from the z input, outZ = the
// value already in outZ, times 4. The core keeps that bug.
bool cFC4OccMgr::WorldCellToStandardCityCell(int32_t x, int32_t z, int32_t& ox, int32_t& oz) {
    fc4_occ_world_cell_to_standard_city_cell(x, z, &ox, &oz);
    return true;
}
bool cFC4OccMgr::StandardCityCellToWorldCell(int32_t x, int32_t z, int32_t& ox, int32_t& oz) {
    fc4_occ_standard_city_cell_to_world_cell(x, z, &ox, &oz);
    return true;
}
bool cFC4OccMgr::WorldCellToPosition(int32_t x, int32_t z, float& fx, float& fz) {
    fc4_occ_world_cell_to_position(x, z, &fx, &fz);
    return true;
}
bool cFC4OccMgr::PositionToWorldCell(float fx, float fz, int32_t& x, int32_t& z) {
    fc4_occ_position_to_world_cell(fx, fz, &x, &z);
    return true;
}
bool cFC4OccMgr::OccupantManagerCellToPosition(int32_t x, int32_t z, float& fx, float& fz) {
    fc4_occ_manager_cell_to_position(x, z, &fx, &fz);
    return true;
}
bool cFC4OccMgr::PositionToOccupantManagerCell(float fx, float fz, int32_t& x, int32_t& z) {
    fc4_occ_position_to_manager_cell(fx, fz, &x, &z);
    return true;
}
bool cFC4OccMgr::StandardCityCellToPosition(int32_t x, int32_t z, float& fx, float& fz) {
    fc4_occ_standard_city_cell_to_position(x, z, &fx, &fz);
    return true;
}
bool cFC4OccMgr::PositionToStandardCityCell(float fx, float fz, int32_t& x, int32_t& z) {
    fc4_occ_position_to_standard_city_cell(fx, fz, &x, &z);
    return true;
}

// ---- slots 20-27: insert, remove, move, cell helpers ---------------------------------------------

bool cFC4OccMgr::InsertOccupant(cISC4Occupant* occ, uint32_t data4) {
    return fc4_occmgr_insert(core_, TokenOf(occ), data4);
}
bool cFC4OccMgr::RemoveOccupant(cISC4Occupant* occ, bool shutdown, uint32_t data4) {
    return fc4_occmgr_remove(core_, TokenOf(occ), shutdown, data4);
}
bool cFC4OccMgr::RemoveOccupants(cISC4OccupantFilter* filter, bool shutdown, uint32_t data4) {
    const fc4_occ_filter f = FilterOf(filter);
    return fc4_occmgr_remove_occupants(core_, &f, shutdown, data4);
}
// Slot 23: the typed overload, which tail-jumps to slot 22 with the shared
// filter (§5.3). The core's shared filter is null before Init, where the
// original writes to address 0xC (OM6 [choice]: nothing is removed, true).
bool cFC4OccMgr::RemoveOccupantsOfType(uint32_t occType, bool shutdown, uint32_t data4) {
    return fc4_occmgr_remove_occupants_of_type(core_, occType, shutdown, data4);
}
bool cFC4OccMgr::MoveOccupant(cISC4Occupant* occ, bool post) {
    return fc4_occmgr_move(core_, TokenOf(occ), post);
}
bool cFC4OccMgr::IsCellEmpty(int32_t x, int32_t z) { return fc4_occmgr_is_cell_empty(core_, x, z); }

// Slot 26 (§6.2) does not touch the grid: eight neighbours, x-major, skipping
// those outside 0..191. n is stored and returned in EAX.
int32_t cFC4OccMgr::GetBoundingCells(int32_t x, int32_t z, int32_t (*cells)[2], int32_t& n) {
    int32_t flat[16] = {0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0};
    const uint32_t got = fc4_occ_bounding_cells(x, z, flat);
    if (cells != nullptr) {
        for (uint32_t i = 0; i < got && i < 8; ++i) {
            cells[i][0] = flat[i * 2];
            cells[i][1] = flat[i * 2 + 1];
        }
    }
    n = static_cast<int32_t>(got);
    return n;
}

// §6.3: x and z are written only by the full scan (§15 quirk 7).
bool cFC4OccMgr::FindOccupant(cISC4Occupant* occ, bool searchAll, int32_t* x, int32_t* z) {
    return fc4_occmgr_find_occupant(core_, TokenOf(occ), searchAll, x, z);
}

// ---- slots 28-35: first-occupant queries ---------------------------------------------------------

bool cFC4OccMgr::FirstOut(bool ok, const uint64_t& occ, cISC4Occupant*& out) {
    ++g_firstCalls;
    if (!ok || occ == 0) return false;
    ++g_firstHits;
    out = Occ(occ);  // the core's one reference is now the caller's
    return true;
}

bool cFC4OccMgr::GetFirstOccupantByPositionFilter(cISC4Occupant*& out, float fx, float fz,
                                                  cISC4OccupantFilter* filter) {
    const fc4_occ_filter f = FilterOf(filter);
    uint64_t occ = 0;
    return Sample("28", fx, fz, FirstOut(fc4_occmgr_first_by_position(core_, fx, fz, &f, &occ), occ, out));
}
bool cFC4OccMgr::GetFirstOccupantByPositionTyped(cISC4Occupant*& out, float fx, float fz, uint32_t occType) {
    uint64_t occ = 0;
    return Sample("29", fx, fz, FirstOut(fc4_occmgr_first_by_position_typed(core_, fx, fz, occType, &occ), occ, out), occType);
}
bool cFC4OccMgr::GetFirstOccupantFilter(cISC4Occupant*& out, int32_t x, int32_t z, cISC4OccupantFilter* filter) {
    const fc4_occ_filter f = FilterOf(filter);
    uint64_t occ = 0;
    return Sample("30", x, z, FirstOut(fc4_occmgr_first_in_cell(core_, x, z, &f, &occ), occ, out));
}
bool cFC4OccMgr::GetFirstOccupantTyped(cISC4Occupant*& out, int32_t x, int32_t z, uint32_t occType) {
    uint64_t occ = 0;
    return Sample("31", x, z, FirstOut(fc4_occmgr_first_in_cell_typed(core_, x, z, occType, &occ), occ, out), occType);
}
bool cFC4OccMgr::GetFirstOccupantByStandardCityCellFilter(cISC4Occupant*& out, int32_t x, int32_t z,
                                                          cISC4OccupantFilter* filter) {
    const fc4_occ_filter f = FilterOf(filter);
    uint64_t occ = 0;
    return Sample("32", x, z, FirstOut(fc4_occmgr_first_by_standard_cell(core_, x, z, &f, &occ), occ, out));
}
bool cFC4OccMgr::GetFirstOccupantByStandardCityCellTyped(cISC4Occupant*& out, int32_t x, int32_t z,
                                                         uint32_t occType) {
    uint64_t occ = 0;
    return Sample("33", x, z, FirstOut(fc4_occmgr_first_by_standard_cell_typed(core_, x, z, occType, &occ), occ, out), occType);
}

namespace {
// SC4Point<int> is two ints, x then z (the SDK's north-west / south-east cell).
void PairOf(SC4Point<int> const* p, int32_t* out) {
    if (p == nullptr) {
        out[0] = 0;
        out[1] = 0;
        return;
    }
    const int* v = reinterpret_cast<const int*>(p);
    out[0] = v[0];
    out[1] = v[1];
}
}  // namespace

bool cFC4OccMgr::GetFirstOccupantByStandardCityCellsFilter(cISC4Occupant*& out, SC4Point<int> const* min,
                                                           SC4Point<int> const* max,
                                                           cISC4OccupantFilter* filter) {
    int32_t lo[2], hi[2];
    PairOf(min, lo);
    PairOf(max, hi);
    const fc4_occ_filter f = FilterOf(filter);
    uint64_t occ = 0;
    // A null corner is left as 0; the core turns a null max into (count, count)
    // and clamps a given one to 0..count-1 (OM5, §7 slot 34). The wrapper
    // cannot see "null" once it has been copied, so both are passed through
    // unclamped here and the core's own defaults apply only when the pointer
    // was null - which is why the two are resolved below.
    const int32_t* loP = min != nullptr ? lo : nullptr;
    const int32_t* hiP = max != nullptr ? hi : nullptr;
    return Sample("34", lo[0], lo[1], FirstOut(fc4_occmgr_first_by_standard_cells(core_, loP, hiP, &f, &occ), occ, out), 0, hi[0], hi[1]);
}
bool cFC4OccMgr::GetFirstOccupantByStandardCityCellsTyped(cISC4Occupant*& out, SC4Point<int> const* min,
                                                          SC4Point<int> const* max, uint32_t occType) {
    int32_t lo[2], hi[2];
    PairOf(min, lo);
    PairOf(max, hi);
    const int32_t* loP = min != nullptr ? lo : nullptr;
    const int32_t* hiP = max != nullptr ? hi : nullptr;
    uint64_t occ = 0;
    return Sample("35", lo[0], lo[1], FirstOut(fc4_occmgr_first_by_standard_cells_typed(core_, loP, hiP, occType, &occ), occ, out), occType, hi[0], hi[1]);
}

// ---- slots 36-41: list queries -------------------------------------------------------------------

void cFC4OccMgr::ListSink(void* ctx, uint64_t occ) {
    static_cast<ListOut*>(ctx)->occs.push_back(occ);
}

// §8.1: the call does not clear the list; each match is appended at the back
// with one reference; the result is true whenever it ran (initialised and
// maxCount != 0), even with nothing appended.
bool cFC4OccMgr::FillList(bool ok, const ListOut& out, SC4List<cISC4Occupant*>& list) {
    ++g_listCalls;
    if (!ok) return false;
    g_listItems += out.occs.size();
    for (uint64_t t : out.occs) {
        cISC4Occupant* o = Occ(t);
        list.push_back(o);  // the core's reference is now the list's
    }
    return true;
}

bool cFC4OccMgr::GetOccupantsByBBoxFilter(SC4List<cISC4Occupant*>& list, float const* min3, float const* max3,
                                          cISC4OccupantFilter* filter, uint32_t maxCount) {
    const fc4_occ_filter f = FilterOf(filter);
    ListOut out;
    const bool ok = fc4_occmgr_occupants_by_bbox(core_, min3, max3, &f, static_cast<int32_t>(maxCount), &ListSink,
                                                 &out);
    return FillList(ok, out, list);
}
bool cFC4OccMgr::GetOccupantsByBBoxTyped(SC4List<cISC4Occupant*>& list, float const* min3, float const* max3,
                                         uint32_t occType, uint32_t maxCount) {
    ListOut out;
    const bool ok = fc4_occmgr_occupants_by_bbox_typed(core_, min3, max3, occType, static_cast<int32_t>(maxCount),
                                                       &ListSink, &out);
    return FillList(ok, out, list);
}
bool cFC4OccMgr::GetOccupantsByOccupantManagerCellsFilter(SC4List<cISC4Occupant*>& list, int32_t const* min,
                                                          int32_t const* max, cISC4OccupantFilter* filter,
                                                          uint32_t maxCount) {
    const fc4_occ_filter f = FilterOf(filter);
    ListOut out;
    const bool ok = fc4_occmgr_occupants_by_cells(core_, min, max, &f, static_cast<int32_t>(maxCount), &ListSink,
                                                  &out);
    return FillList(ok, out, list);
}
bool cFC4OccMgr::GetOccupantsByOccupantManagerCellsTyped(SC4List<cISC4Occupant*>& list, int32_t const* min,
                                                         int32_t const* max, uint32_t occType, uint32_t maxCount) {
    ListOut out;
    const bool ok = fc4_occmgr_occupants_by_cells_typed(core_, min, max, occType, static_cast<int32_t>(maxCount),
                                                        &ListSink, &out);
    return FillList(ok, out, list);
}
bool cFC4OccMgr::GetOccupantsByStandardCityCellsFilter(SC4List<cISC4Occupant*>& list, SC4Point<int> const* min,
                                                       SC4Point<int> const* max, cISC4OccupantFilter* filter,
                                                       uint32_t maxCount) {
    int32_t lo[2], hi[2];
    PairOf(min, lo);
    PairOf(max, hi);
    const fc4_occ_filter f = FilterOf(filter);
    ListOut out;
    const bool ok = fc4_occmgr_occupants_by_standard_cells(core_, min != nullptr ? lo : nullptr,
                                                           max != nullptr ? hi : nullptr, &f,
                                                           static_cast<int32_t>(maxCount), &ListSink, &out);
    return FillList(ok, out, list);
}
bool cFC4OccMgr::GetOccupantsByStandardCityCellsTyped(SC4List<cISC4Occupant*>& list, SC4Point<int> const* min,
                                                      SC4Point<int> const* max, uint32_t occType,
                                                      uint32_t maxCount) {
    int32_t lo[2], hi[2];
    PairOf(min, lo);
    PairOf(max, hi);
    ListOut out;
    const bool ok = fc4_occmgr_occupants_by_standard_cells_typed(
        core_, min != nullptr ? lo : nullptr, max != nullptr ? hi : nullptr, occType,
        static_cast<int32_t>(maxCount), &ListSink, &out);
    return FillList(ok, out, list);
}

// ---- slots 42-47: iteration ----------------------------------------------------------------------

bool cFC4OccMgr::IterThunk(void* ctx, uint64_t occ) {
    IterIn* in = static_cast<IterIn*>(ctx);
    ++g_iterItems;
    return in->fn(Occ(occ), in->data);
}

bool cFC4OccMgr::IterateOccupantsByBBoxFilter(bool (*fn)(cISC4Occupant*, void*), void* data, float const* min3,
                                              float const* max3, cISC4OccupantFilter* filter) {
    if (fn == nullptr) return false;
    IterIn in{fn, data};
    const fc4_occ_filter f = FilterOf(filter);
    return fc4_occmgr_iterate_by_bbox(core_, &IterThunk, &in, min3, max3, &f);
}
bool cFC4OccMgr::IterateOccupantsByBBoxTyped(bool (*fn)(cISC4Occupant*, void*), void* data, float const* min3,
                                             float const* max3, uint32_t occType) {
    if (fn == nullptr) return false;
    IterIn in{fn, data};
    return fc4_occmgr_iterate_by_bbox_typed(core_, &IterThunk, &in, min3, max3, occType);
}
bool cFC4OccMgr::IterateOccupantsFilter(bool (*fn)(cISC4Occupant*, void*), void* data, int32_t const* min,
                                        int32_t const* max, cISC4OccupantFilter* filter) {
    if (fn == nullptr) return false;
    IterIn in{fn, data};
    const fc4_occ_filter f = FilterOf(filter);
    return fc4_occmgr_iterate(core_, &IterThunk, &in, min, max, &f);
}
bool cFC4OccMgr::IterateOccupantsTyped(bool (*fn)(cISC4Occupant*, void*), void* data, int32_t const* min,
                                       int32_t const* max, uint32_t occType) {
    if (fn == nullptr) return false;
    IterIn in{fn, data};
    return fc4_occmgr_iterate_typed(core_, &IterThunk, &in, min, max, occType);
}
// 46/47 (§9.4): the typed overload is slot 46, the filter one slot 47 - the
// only pair the game declares the other way round from the SDK.
bool cFC4OccMgr::IterateOccupantsByStandardCityCellTyped(bool (*fn)(cISC4Occupant*, void*), void* data,
                                                         int32_t const* min, int32_t const* max,
                                                         uint32_t occType) {
    if (fn == nullptr) return false;
    IterIn in{fn, data};
    return fc4_occmgr_iterate_by_standard_cells_typed(core_, &IterThunk, &in, min, max, occType);
}
bool cFC4OccMgr::IterateOccupantsByStandardCityCellFilter(bool (*fn)(cISC4Occupant*, void*), void* data,
                                                          int32_t const* min, int32_t const* max,
                                                          cISC4OccupantFilter* filter) {
    if (fn == nullptr) return false;
    IterIn in{fn, data};
    const fc4_occ_filter f = FilterOf(filter);
    return fc4_occmgr_iterate_by_standard_cells(core_, &IterThunk, &in, min, max, &f);
}

// ---- slot 48: ReleaseOccupantList ----------------------------------------------------------------
//
// §9.5: release each element once and free every node, leaving the head
// self-linked (SC4List::erase does exactly that). No lock, and the head stays
// the caller's.
bool cFC4OccMgr::ReleaseOccupantList(SC4List<cISC4Occupant*>& list) {
    while (!list.empty()) list.erase(list.begin());
    return true;
}

// ---- cIGZMessageTarget2 (§1) ---------------------------------------------------------------------

// The interface exists but its DoMessage is a shared stub: the manager
// subscribes to nothing and handles no message.
bool cFC4OccMgr::DoMessage(cIGZMessage2*) { return true; }

// ---- cIGZSerializable (§13) ----------------------------------------------------------------------

namespace {
// The record's shape (§13) as it goes out and comes back: version 1, the two
// sizes and four counts, 192 rows of 192 cells. Logged so a game run shows the
// v1 record was written and read.
void LogRecord(const char* what, bool ok, fc4_occmgr* core) {
    fc4_occmgr_state st{};
    st.struct_size = sizeof(st);
    fc4_occmgr_get_state(core, &st);
    fc4::Log("occmgr %s -> %d (version 1, size %.0fx%.0f counts %d,%d,%d,%d rows=%u holders=%u nodes=%u)", what,
             ok ? 1 : 0, st.size_x, st.size_z, st.counts[0], st.counts[1], st.counts[2], st.counts[3], st.rows,
             st.holders, st.nodes);
}
}  // namespace

bool cFC4OccMgr::Write(cIGZOStream& gzOut) {
    FC4_TRACE("OccMgr::Write");
    fc4::OStreamAdapter a(gzOut);
    const bool ok = fc4_occmgr_write(core_, a.Get());
    LogRecord("Write", ok, core_);
    return ok;
}

// Read runs before Init (HANDOFF §5.1); nothing here is sized from the city.
bool cFC4OccMgr::Read(cIGZIStream& gzIn) {
    FC4_TRACE("OccMgr::Read");
    fc4::IStreamAdapter a(gzIn);
    const bool ok = fc4_occmgr_read(core_, a.Get());
    LogRecord("Read", ok, core_);
    return ok;
}

uint32_t cFC4OccMgr::GetGZCLSID() { return kFC4_CLSID_cSC4OccupantManager; }

// ---- factory -------------------------------------------------------------------------------------

cIGZUnknown* CreateFC4OccupantManager() {
    cFC4OccMgr* m = new cFC4OccMgr();
    return static_cast<cIGZUnknown*>(static_cast<cFC4ISC4OccupantManager*>(m));
}
