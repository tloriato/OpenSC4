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

// The tract developer's GZCOM class (docs/specs/tract-developer.md §1, §7;
// CLSID 0x2990C142, city component 17). The model lives in the Rust core
// behind fc4.h (fc4_tract); this class implements the three interfaces of §7
// in the compiled slot order and return channels of §7.1
// (FC4DevelopmentInterfaces.h cFC4ISC4TractDeveloper) and the host callbacks over
// the game's services (§8, FC4TractHost.cpp). The BDS, the growth
// developers, the lot developer, the lot manager and the lot configuration
// manager stay the game's (or whoever the city runs): they are reached only
// through their interfaces.
//
// One object, three interfaces, one reference count: the primary interface
// (cISC4TractDeveloper 0xA818ED1B, also cIGZUnknown), cIGZMessageTarget2 (the
// agent, §3) and cIGZSerializable (§6). Which sub-object answers an IID is
// the core's decision (fc4_tract_query_interface).
//
// The configuration filter (§7.4) is cFC4TractFilter: allocated per (wealth,
// purpose) when the core asks, registered with the game's lot configuration
// manager (slot 18), unregistered (slot 19) at Shutdown and only then freed.
// It is not reference counted.
//
// Threads. Grow, Decline and UpdatePopulation come from the main thread (the
// BDS's day agent and the growth developers); the style getters may come from
// the UI. The core is single-threaded, so every call into it holds this
// object's lock. The main thread waits for it; any other thread tries for up
// to kLockWaitMs and otherwise answers from a snapshot (slots 8, 11, 13), the
// member vector (slot 9) or the neutral value, and counts a fallback.
// Write and Read always wait.
//
// The subsystem is OFF by default: FreeCity4.ini `enable = tract`; `disable`
// still wins. The tract_* switches (FC4Config.h) turn groups of game calls off
// for bisecting.
#pragma once

#include "FC4Config.h"
#include "FC4TestSeam.h"
#include "FC4TractInterfaces.h"
#include "cIGZMessageTarget2.h"
#include "cIGZSerializable.h"
#include "cISC4LotConfigurationFilter.h"
#include "fc4.h"

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>

#include <atomic>
#include <cstdint>
#include <vector>

class cIGZMessageServer2;
class cISC4BuildingDevelopmentSimulator;
class cISC4City;
class cISC4LandValueSimulator;
class cISC4Lot;
class cISC4LotConfiguration;
class cISC4LotConfigurationManager;
class cISC4LotDeveloper;
class cISC4LotManager;
class cISC4PlumbingSimulator;
class cISC4PowerSimulator;
class cISC4TrafficSimulator;
class cISC4ZoneManager;

class cFC4TractFilter;

namespace fc4 {

// What the tract developer's host callbacks work on. The tract developer
// owns one; the tract shadow (FC4TractShadow.h) fills one with a city's
// borrowed services. Main thread, or another thread holding the tract
// developer's lock.
struct TractContext {
    bool readOnly = false;  // the shadow: borrowed services, read callbacks only
    bool gated = true;      // false: the tract_* switches are ignored (the shadow reads as it is)
    // The services of §1.3 step 3: held from Init to Shutdown by the tract
    // developer (one reference each), borrowed by the shadow.
    cISC4City* city = nullptr;
    cISC4BuildingDevelopmentSimulator* bds = nullptr;
    cISC4LandValueSimulator* landValue = nullptr;
    cISC4LotDeveloper* lotDeveloper = nullptr;
    cISC4LotConfigurationManager* configs = nullptr;
    cISC4LotManager* lots = nullptr;
    cISC4PlumbingSimulator* plumbing = nullptr;
    cISC4PowerSimulator* power = nullptr;
    cISC4TrafficSimulator* traffic = nullptr;
    cISC4ZoneManager* zones = nullptr;
    cIGZMessageServer2* msgServer = nullptr;  // tract developer only (§1.3 step 5)
    cIGZMessageTarget2* target = nullptr;     // tract developer only: the agent (§1.3 step 4)
    uint32_t agentLevels = 0;                 // AddAgent calls that succeeded since the last RemoveAgent
    // The configuration filters allocated (§4.6, §7.4), live until unregistered.
    std::vector<cFC4TractFilter*> filters;
    const fc4_tract_host* filterHost = nullptr;  // what the filters read configurations through
    // Test seam op 45: a table whose non-NULL callbacks answer instead of the
    // game (its own `user`), and a lot configuration manager (borrowed) the
    // filters are registered with while no service manager is held.
    const fc4_tract_host* test = nullptr;
    cISC4LotConfigurationManager* testConfigs = nullptr;
    // References taken for one top-level call into the core (the lots and
    // configurations its lists gave, a created lot), released when it
    // returns; the lots it deleted (DeleteLot), kept alive until then and
    // released as objects whose last reference may be ours; BuildFarm's lot
    // lists (§4.15.2: one list appended across calls, begun at each
    // GetLotsSurroundingRect with flags 1).
    std::vector<cIGZUnknown*> held;
    // The configurations GetLotConfigurationsByFilter gave (one reference
    // each), released with `held` but outside the reference audit.
    std::vector<cIGZUnknown*> heldConfigs;
    uint64_t configRefs = 0;  // references taken into heldConfigs since construction
    std::vector<cIGZUnknown*> deleted;
    std::vector<fc4::GameList<cISC4Lot*, fc4::TractNodeAllocator>*> farmLists;
    // Observability (the status lines): calls made since construction.
    uint32_t filtersRegistered = 0, filtersUnregistered = 0, filtersRefused = 0;
    uint32_t listsRefused = 0;     // game lists / sets not made (no allocator B)
    uint32_t roadsRefused = 0;     // GetRequiredRoads results not read (unexpected layout)
    uint32_t mapsRefused = 0;      // style maps not read (inconsistent)
    uint32_t exemplarMisses = 0;   // building exemplars not found (occupant groups, monopoly, fields)
};

// The host table over `ctx` (the tract developer's).
fc4_tract_host MakeTractHost(TractContext* ctx);
// The read-only table for the shadow: every callback that would change the
// game is NULL; the shadow core swallows those writes before they reach the
// table anyway.
fc4_tract_host MakeTractReadOnlyHost(TractContext* ctx);
// Releases the references and lists `ctx` keeps for one top-level call.
void TractReleaseHeld(TractContext* ctx);
// The services `ctx` holds (the tract developer's Shutdown path, or a
// destructor after a failed Init); a read-only context holds none.
void TractReleaseServices(TractContext* ctx);
// Unregisters (slot 19) and frees every filter still allocated.
void TractFreeFilters(TractContext* ctx);

// The tract developer the city runs, if it is FreeCity4's (null otherwise);
// main thread. The observer's month hook writes its month line.
void TractMonthLine(int32_t julian);

}  // namespace fc4

// The configuration filter (§7.4): {vtable, wealth, purpose}; not reference
// counted (AddRef / Release return 1), QueryInterface answers nothing.
class cFC4TractFilter final : public cISC4LotConfigurationFilter {
public:
    cFC4TractFilter(const fc4_tract_host* host, uint32_t wealth, uint32_t purpose);
    ~cFC4TractFilter();
    cFC4TractFilter(const cFC4TractFilter&) = delete;
    cFC4TractFilter& operator=(const cFC4TractFilter&) = delete;
    bool QueryInterface(uint32_t riid, void** ppvObj) override;
    uint32_t AddRef() override;
    uint32_t Release() override;
    bool IsLotConfigurationIncluded(cISC4LotConfiguration* config) override;
    uint32_t Wealth() const { return wealth_; }
    uint32_t Purpose() const { return purpose_; }
    static long Live();  // filters allocated and not yet freed (tests)

private:
    // {vtable, wealth, purpose} as the original's (§4.6), then the table the
    // filter reads configurations through.
    uint32_t wealth_;
    uint32_t purpose_;
    const fc4_tract_host* host_;
};

class cFC4TractDeveloper final : public cFC4ISC4TractDeveloper, public cIGZMessageTarget2, public cIGZSerializable {
public:
    cFC4TractDeveloper();
    ~cFC4TractDeveloper();

    // cIGZUnknown (all three interfaces)
    bool QueryInterface(uint32_t riid, void** ppvObj) override;
    uint32_t AddRef() override;
    uint32_t Release() override;

    // cISC4TractDeveloper as compiled (§7.1)
    bool Init() override;
    bool Shutdown() override;
    int32_t Grow(FC4GrowthProfile const& profile, fc4::GameList<int32_t>& outcomes, bool& outFlag) override;
    int32_t Decline(FC4GrowthProfile const& profile) override;
    int32_t UpdatePopulation(FC4GrowthProfile const& profile) override;
    int32_t GetCurrentStyle() const override;
    fc4::GameVector<uint32_t>& GetActiveStyles() override;
    intptr_t SetActiveStyles(fc4::GameVector<uint32_t> const& styles) override;
    bool IsUsingAllStylesAtOnce() const override;
    cFC4ISC4TractDeveloper* UseAllStylesAtOnce(bool all) override;
    int32_t GetYearsBetweenStyleChanges() const override;
    cFC4ISC4TractDeveloper* SetYearsBetweenStyleChanges(int32_t years) override;

    // cIGZMessageTarget2 (§3)
    bool DoMessage(cIGZMessage2* pMessage) override;

    // cIGZSerializable (§6)
    bool Write(cIGZOStream& gzOut) override;
    bool Read(cIGZIStream& gzIn) override;
    uint32_t GetGZCLSID() override;

    // How long a thread other than the main one waits for the lock.
    static constexpr uint32_t kLockWaitMs = 20;

    // Observability: calls answered from the snapshot / neutral value.
    uint32_t Fallbacks() const { return fallbacks_.load(); }
    // The month status line (the observer's SimNewMonth; main thread).
    void LogMonth(int32_t julian);
    // Test seam op 45 (FC4TestSeam.h): the instance one of whose interfaces
    // is `p`, or null.
    static cFC4TractDeveloper* FromPrimary(const void* p);
    void SetTestHooks(const fc4_tract_host* test, cISC4LotConfigurationManager* configs);
    // The context's counters, for the test seam.
    const fc4::TractContext& Context() const { return ctx_; }

private:
    // Holds the lock for one call into the core (see the file comment). On a
    // thread other than the main one ok() may be false (unless `wait`):
    // answer from the snapshot or with the neutral value.
    class Section {
    public:
        explicit Section(const cFC4TractDeveloper* s, bool wait = false);
        ~Section();
        Section(const Section&) = delete;
        Section& operator=(const Section&) = delete;
        bool ok() const { return ok_; }

    private:
        cFC4TractDeveloper* s_;
        bool ok_;
    };

    // The values a getter on another thread falls back to.
    struct Snapshot {
        std::atomic<uint32_t> currentStyle{0};
        std::atomic<bool> allStyles{false};
        std::atomic<int32_t> yearsBetween{0};
    };

    // The month line's running totals at the previous line.
    struct MonthBase {
        uint32_t grows = 0, declines = 0, populations = 0;
        int64_t built = 0, declined = 0;
        uint32_t outcomes[28] = {};
        uint32_t constructions = 0, reoccupations = 0, abandonments = 0, demolitions = 0, lotsCreated = 0,
                 lotsDeleted = 0;
    };

    bool Enter(bool wait);
    void Leave();
    // The snapshot is rewritten only when something could have changed it
    // (MarkStylesChanged): Grow, Decline, UpdatePopulation and the getters
    // cannot, and `UpdatePopulation` runs once per desirability tract of every
    // growth-developer map update (12288 calls a month in a Konradshohe run),
    // so refreshing on those was the tract developer's largest per-call cost.
    void RefreshSnapshot();
    void MarkStylesChanged() { snapDirty_ = true; }
    void LogState(const char* what);
    void CreateCore();

    cIGZUnknown* Primary() { return static_cast<cIGZUnknown*>(static_cast<cFC4ISC4TractDeveloper*>(this)); }

    // Slot 9's vector (B): the core's active styles, rewritten after every
    // call that holds the lock. The first data member, so its address (slot
    // 10's EAX) is 4 mod 8 in an 8-aligned object: AL is never 0.
    fc4::TractStyleVector styles_;
    uint32_t refCount_ = 0;
    bool destroying_ = false;
    bool growLog_ = false;
    bool noteStyles_ = false;
    bool snapDirty_ = true;  // the snapshot may be stale; see RefreshSnapshot
    fc4::TractContext ctx_;
    fc4_tract_host host_{};
    fc4_tract* core_ = nullptr;
    uint32_t seed_ = 0;
    bool seedFixed_ = false;

    mutable CRITICAL_SECTION lock_;
    DWORD mainTid_ = 0;
    int depth_ = 0;  // nesting of sections held (by the lock holder)
    mutable std::atomic<uint32_t> fallbacks_{0};
    Snapshot snap_;
    MonthBase month_;
};

// Factory for the subsystem table (reference count 0).
cIGZUnknown* CreateFC4TractDeveloper();

// Test seam op 45 (FC4TestSeam.h, kFC4SeamTract): `a` is one of these.
struct FC4TractTestInput {
    uint32_t op;  // kFC4TractTestHooks, kFC4TractTestShadow or kFC4TractTestFilters
    // kFC4TractTestHooks: on `tractDeveloper` (FreeCity4's, any of its
    // interfaces), every non-NULL callback of `host` answers instead of the
    // game (with host->user); `configManager` (borrowed, may be null) takes
    // the filter registrations while no city manager is held; the game
    // containers (outcome nodes, the style vector, sets, lists) come from
    // `allocate` / `free` instead of allocator B. The core is created anew
    // (call it before Init / Read). A null `tractDeveloper` sets the
    // allocator only (null: B again).
    cIGZUnknown* tractDeveloper;
    const fc4_tract_host* host;
    cISC4LotConfigurationManager* configManager;
    void* (*allocate)(uint32_t size);
    void (*free)(void* p);
    // kFC4TractTestShadow: one shadow comparison (FC4TractShadow.h) on `city`,
    // whose tract developer is the "original": event 0 = PostCityInit, 1 = a
    // month; `host` (may be null) answers the shadow core's host calls.
    cISC4City* city;
    int32_t julian;
    uint32_t event;
};
enum : uint32_t { kFC4TractTestHooks = 1, kFC4TractTestShadow = 2, kFC4TractTestFilters = 3 };
// kFC4TractTestFilters's answer: the filters of `tractDeveloper` (may be null).
struct FC4TractFilterCounts {
    long live;  // cFC4TractFilter objects not yet freed (all instances)
    uint32_t allocated, registered, unregistered, refused;
};
// Op 45's body: kFC4TractTestHooks returns 1 (installed), 0 (the allocator
// only) or -1 (not a FreeCity4 tract developer); kFC4TractTestShadow returns
// what TractShadowTestRun returns (FC4TractShadow.h); kFC4TractTestFilters
// fills `*out` (b) and returns 1 (-1 without `out`).
intptr_t TractTestSeam(const FC4TractTestInput* in, void* out);
