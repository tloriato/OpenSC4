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

// The demolition utility's GZCOM class: see cFC4Demolition.h.

#include "cFC4Demolition.h"

#include "FC4Diag.h"
#include "FC4Log.h"
#include "FC4RefAudit.h"
#include "FC4Streams.h"

#include "GZServPtrs.h"
#include "SC4CellRegion.h"
#include "SC4List.h"
#include "SC4Rect.h"
#include "cIGZMessage2.h"
#include "cIGZMessage2Standard.h"
#include "cIGZMessageServer2.h"
#include "cISC4City.h"
#include "cISC4Occupant.h"
#include "cISC4OccupantFilter.h"

#include <cstring>
#include <vector>

namespace fc4 {

// The game's SC4CellRegion<long> as fc4_zone_region (zones.md, 0x24 bytes:
// x1, z1, x2, z2, then the cell map; bit (dx, dz) is bit dz % 32 of
// rows[dx][dz / 32]). The mask is rebuilt here in the C ABI's bit order.
bool DemolitionRegionToC(const SC4CellRegion<int32_t>& r, fc4_zone_region& out, std::vector<uint8_t>& mask) {
    const auto* raw = reinterpret_cast<const uint8_t*>(&r);
    int32_t x0, z0, x1, z1;
    std::memcpy(&x0, raw + 0, 4);
    std::memcpy(&z0, raw + 4, 4);
    std::memcpy(&x1, raw + 8, 4);
    std::memcpy(&z1, raw + 12, 4);
    const int64_t w = int64_t{x1} - x0 + 1;
    const int64_t h = int64_t{z1} - z0 + 1;
    if (w <= 0 || h <= 0 || w * h > (int64_t{1} << 24)) return false;
    uint32_t mapW = 0, mapH = 0, words = 0;
    uint32_t** rows = nullptr;
    // SDK cRZCellMap has a virtual destructor: +0x10 is its vptr.
    std::memcpy(&mapW, raw + 20, 4);
    std::memcpy(&mapH, raw + 24, 4);
    std::memcpy(&words, raw + 28, 4);
    std::memcpy(&rows, raw + 32, sizeof(rows));
    if (rows == nullptr || mapW < static_cast<uint32_t>(w) || mapH < static_cast<uint32_t>(h) ||
        words < static_cast<uint32_t>((h + 31) / 32)) {
        return false;
    }
    mask.assign(static_cast<size_t>((w * h + 7) / 8), 0);
    for (int64_t dx = 0; dx < w; ++dx) {
        const uint32_t* row = rows[dx];
        if (row == nullptr) return false;
        for (int64_t dz = 0; dz < h; ++dz) {
            if ((row[dz / 32] >> (dz % 32)) & 1u) {
                const size_t i = static_cast<size_t>(dx * h + dz);
                mask[i / 8] |= static_cast<uint8_t>(1u << (i % 8));
            }
        }
    }
    out.x0 = x0;
    out.z0 = z0;
    out.x1 = x1;
    out.z1 = z1;
    out.mask = mask.data();
    out.len = mask.size();
    return true;
}

}  // namespace fc4

cIGZUnknown* CreateFC4Demolition() {
    return static_cast<cIGZUnknown*>(static_cast<cFC4ISC4Demolition*>(new cFC4Demolition()));
}

cFC4Demolition::cFC4Demolition() {
    host_ = fc4::MakeDemolitionHost(&ctx_);
    core_ = fc4_demolition_create(&host_, 0);
    if (core_ == nullptr) fc4::Log("cFC4Demolition: core creation failed");
    fc4::Log("cFC4Demolition(%p) created, core %p", static_cast<void*>(this), static_cast<void*>(core_));
}

cFC4Demolition::~cFC4Demolition() {
    if (core_ != nullptr) {
        fc4_demolition_destroy(core_);
        core_ = nullptr;
    }
    fc4::DemolitionHostReleaseAll(&ctx_);
}

void cFC4Demolition::SetCity(cISC4City* city) {
    ctx_.city = city;
    ctx_.messageTarget = static_cast<cIGZMessageTarget2*>(this);
}

bool cFC4Demolition::QueryInterface(uint32_t riid, void** ppvObj) {
    if (ppvObj == nullptr) return false;
    *ppvObj = nullptr;
    uint32_t offset = 0;
    if (fc4_demolition_query_interface(riid, &offset)) {
        *ppvObj = reinterpret_cast<uint8_t*>(static_cast<cFC4ISC4Demolition*>(this)) + offset;
        AddRef();
        return true;
    }
    return false;
}

uint32_t cFC4Demolition::AddRef() { return ++refCount_; }

uint32_t cFC4Demolition::Release() {
    const uint32_t n = --refCount_;
    if (n == 0 && !destroying_) {
        destroying_ = true;
        if (core_ != nullptr) {
            fc4_demolition_state st{};
            if (fc4_demolition_get_state(core_, &st) && st.initialized != 0) Shutdown();
        }
        fc4::DemolitionHostReleaseAll(&ctx_);
        delete this;
    }
    return n;
}

bool cFC4Demolition::Init() {
    FC4_TRACE("Demolition::Init");
    return core_ != nullptr && fc4_demolition_init(core_);
}

bool cFC4Demolition::Shutdown() {
    FC4_TRACE("Demolition::Shutdown");
    const bool ok = core_ != nullptr && fc4_demolition_shutdown(core_);
    fc4::DemolitionHostReleaseHeld(&ctx_);
    return ok;
}

// Slot 5 does nothing (§8).
void cFC4Demolition::SetDefaultOccupantFilter(cISC4OccupantFilter*) {}

bool cFC4Demolition::DemolishRegion(bool doIt, SC4CellRegion<int32_t> const& region, int32_t privilege,
                                    uint32_t flags, bool dezone, cISC4OccupantFilter* filter, int64_t* cost,
                                    intptr_t demolished, cISC4Occupant* clicked, long x, long y) {
    if (core_ == nullptr) return false;
    fc4::DemolitionHostCall call(ctx_);
    fc4_zone_region r{};
    std::vector<uint8_t> mask;
    if (!fc4::DemolitionRegionToC(region, r, mask)) return false;
    return fc4_demolition_demolish_region(
        core_, doIt, &r, static_cast<uint32_t>(privilege), flags, dezone, fc4::TokenOf(filter),
        static_cast<uint64_t>(demolished), fc4::TokenOf(clicked), static_cast<int32_t>(x), static_cast<int32_t>(y),
        cost);
}

bool cFC4Demolition::DemolishLots(bool doIt, SC4List<cISC4Lot*> const& lots, int32_t privilege, uint32_t flags,
                                  bool dezone, cISC4OccupantFilter* filter, int64_t* cost, intptr_t demolished) {
    if (core_ == nullptr) return false;
    fc4::DemolitionHostCall call(ctx_);
    // §4.5: an empty list returns true at once and writes nothing; null
    // entries are skipped [choice].
    std::vector<uint64_t> toks;
    for (cISC4Lot* l : lots) toks.push_back(fc4::TokenOf(l));
    return fc4_demolition_demolish_lots(core_, doIt, toks.data(), toks.size(), static_cast<uint32_t>(privilege),
                                        flags, dezone, fc4::TokenOf(filter), static_cast<uint64_t>(demolished),
                                        cost);
}

bool cFC4Demolition::DemolishOccupant(bool doIt, cISC4Occupant* occ, int32_t privilege, uint32_t flags,
                                      int64_t* cost, bool effects, SC4List<cISC4Occupant*>* orphans) {
    if (core_ == nullptr) return false;
    fc4::DemolitionHostCall call(ctx_);
    return fc4_demolition_demolish_occupant(core_, doIt, fc4::TokenOf(occ), static_cast<uint32_t>(privilege),
                                            flags, effects, fc4::TokenOf(orphans), cost);
}

int32_t cFC4Demolition::ModifyTerrainHeight(bool doIt, bool perVertex, int32_t x0, int32_t z0, int32_t x1,
                                            int32_t z1, float h, cISC4Lot* exempt, uint32_t reason, int64_t*) {
    // [abi-ret] slot 9 returns the differing vertex count in EAX; `cost` is
    // never written (demolition.md §4.7, DM2).
    if (core_ == nullptr) return 0;
    fc4::DemolitionHostCall call(ctx_);
    return fc4_demolition_modify_terrain_height(core_, doIt, perVertex, x0, z0, x1, z1, h, fc4::TokenOf(exempt),
                                                reason);
}

void cFC4Demolition::DoDemolishOccupantEffect(cISC4Occupant*, uint32_t, cISC4Lot*, bool, long, long) {
    // §4.8 [choice]: presentation only; a replacement may do nothing.
}

void cFC4Demolition::DoDemolishGroupEffect(uint32_t, int32_t, int32_t, SC4Rect<int32_t> const&) {
    // §4.8 [choice]: presentation only.
}

bool cFC4Demolition::DoMessage(cIGZMessage2* pMessage) {
    if (core_ == nullptr || pMessage == nullptr) return true;
    fc4::DemolitionHostCall call(ctx_);
    const uint32_t type = pMessage->GetType();
    uint32_t d1 = 0, d2 = 0;
    uint64_t lot = 0;
    bool dezone = false;
    cIGZMessage2Standard* m = nullptr;
    if (pMessage->QueryInterface(GZIID_cIGZMessage2Standard, reinterpret_cast<void**>(&m)) && m != nullptr) {
        FC4_ACQUIRED(m, "QueryInterface");
        d1 = static_cast<uint32_t>(m->GetData1());
        d2 = static_cast<uint32_t>(m->GetData2());
        // 0xA9DED179 (§3): data1 points at a heap record
        // {cISC4Lot* lot, bool dezone}; the record's lot reference is the one
        // the core releases.
        if (type == 0xA9DED179u) {
            const uint8_t* rec = reinterpret_cast<const uint8_t*>(static_cast<uintptr_t>(m->GetData1()));
            if (rec != nullptr) {
                void* lp = nullptr;
                std::memcpy(&lp, rec, sizeof(lp));
                std::memcpy(&dezone, rec + sizeof(lp), 1);
                lot = fc4::TokenOf(lp);
            }
        }
        FC4_RELEASE(m);
    }
    return fc4_demolition_do_message(core_, type, d1, d2, lot, dezone);
}

bool cFC4Demolition::Write(cIGZOStream& gzOut) {
    if (core_ == nullptr) return false;
    fc4::OStreamAdapter s(gzOut);
    return fc4_demolition_write(core_, s.Get());
}

bool cFC4Demolition::Read(cIGZIStream& gzIn) {
    if (core_ == nullptr) return false;
    fc4::IStreamAdapter s(gzIn);
    return fc4_demolition_read(core_, s.Get());
}

uint32_t cFC4Demolition::GetGZCLSID() { return kFC4_CLSID_cSC4Demolition; }
