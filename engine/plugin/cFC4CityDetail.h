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

// The city detail manager (cSC4CityDetailManager, CLSID 0xEAD4BA36, city
// component 51): docs/specs/city-detail.md. The behaviour lives in the Rust
// core behind fc4.h (fc4_city_detail); this class implements the interface
// (§6: IID 0x6AD4B9F1, 7 slots, no SDK header) at +0 and cIGZSerializable at
// +4, and the host callbacks over the game's services (the preferences, the
// lot, prop, flora, occupant, lot configuration and network lot managers and
// the lot developer).
//
// The increase pass's generator (§3.5 [choice]) is seeded from the session
// seed S: `citydetail_seed` / FC4_CITYDETAIL_SEED, else session_seed / the
// clock (FC4Seed.h); the core derives derive(0xEAD4BA36 XOR n) from it.
//
// OFF by default: FreeCity4.ini `enable = citydetail` (FC4Subsystems.h).
//
// Log lines (FreeCity4.log):
//   citydetail Init: setting=.. level a->b pass=none|reduce|increase removed=.. created=.. draws=.. network-lots=..
//   citydetail Read|Write -> 0|1 (level ..)
//   citydetail Shutdown -> 1 (level .., inits=.., QueryInterface own=.. serializable=.. refused=.., GetCityDetail=..)
#pragma once

#include "FC4PropCityCommon.h"
#include "cIGZSerializable.h"
#include "cIGZUnknown.h"
#include "fc4.h"

#include <atomic>
#include <cstdint>

static const uint32_t kFC4_CLSID_cSC4CityDetailManager = 0xEAD4BA36;
static const uint32_t kFC4_IID_cISC4CityDetailManager = 0x6AD4B9F1;

// cISC4CityDetailManager (city-detail.md §6; no SDK header).
class cFC4ISC4CityDetailManager : public cIGZUnknown {
public:
    virtual bool Init() = 0;                  // 3
    virtual bool Shutdown() = 0;              // 4
    virtual uint32_t GetCityDetail() = 0;     // 5 [inferred name]
    virtual ~cFC4ISC4CityDetailManager() {}  // 6 deleting destructor
};

namespace fc4 {

// What the city detail host callbacks work on. The manager owns one; the
// shadow (FC4CityDetailShadow.h) makes a read-only one that records instead
// of changing the city.
struct CityDetailContext {
    bool readOnly = false;
    propcity::CallPool pool;  // list entries kept for one core call
    // Counters (log lines).
    uint32_t removes = 0, creates = 0, tags = 0, redevelops = 0;
    bool lotsOrdered = true;
    // The detail setting the host reported last (0: not asked).
    uint32_t settingRead = 0;
};

fc4_city_detail_host MakeCityDetailHost(CityDetailContext* ctx);

// The city detail setting (city-detail.md §2): the preferences byte +0xF16
// (cISC4App slot 38, SC4Preferences videoPreferences.cityDetail), or 3 (the
// original's default) without preferences.
uint32_t CityDetailSetting();

}  // namespace fc4

class cFC4CityDetailManager final : public cFC4ISC4CityDetailManager, public cIGZSerializable {
public:
    cFC4CityDetailManager();
    ~cFC4CityDetailManager() override;

    // cIGZUnknown (§1)
    bool QueryInterface(uint32_t riid, void** ppvObj) override;
    uint32_t AddRef() override;
    uint32_t Release() override;

    // cISC4CityDetailManager (§6)
    bool Init() override;
    bool Shutdown() override;
    uint32_t GetCityDetail() override;

    // cIGZSerializable (§4)
    bool Write(cIGZOStream& gzOut) override;
    bool Read(cIGZIStream& gzIn) override;
    uint32_t GetGZCLSID() override;

    fc4_city_detail* Core() const { return core_; }

private:
    bool destroying_ = false;
    fc4::CityDetailContext ctx_;
    fc4_city_detail_host host_{};
    fc4_city_detail* core_ = nullptr;
    uint32_t seed_ = 0;
    // Trace counters for the Shutdown log line (atomic: §5.1, the game may
    // query from its second thread).
    std::atomic<uint32_t> qiOwn_{0}, qiSer_{0}, qiOther_{0}, getDetail_{0}, inits_{0};
};

// Factory for the subsystem table (reference count 0).
cIGZUnknown* CreateFC4CityDetailManager();
