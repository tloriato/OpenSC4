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

// The airport and seaport developers (cSC4AirportDeveloper CLSID 0xAA2049A1,
// city component 13; cSC4SeaportDeveloper CLSID 0xEA230A28, component 14):
// docs/specs/port-developers.md. One shim, two classes: the behaviour lives in
// the Rust core behind fc4.h (fc4_port_developer, `kind` 0 / 1); the shared
// template below implements the four sub-objects of §1 (the primary interface
// at +0, the port sub-object cISC4PortDeveloper at +4, the message target at
// +8, the serializable at +0xC) and forwards every slot of §8 to the core;
// the host callbacks (MakePortHost) reach the game's services: the budget
// department and line 1 (§4.2), the demand simulator's occupant effects
// (§4.3, PD2), lots, the lot developer and the lot manager (§4.5), the zone
// manager (§4.6), the traffic simulator (§4.4), the terrain (§4.8), the
// history warehouse message and the advisor system.
//
// [inferred] The subscriptions of §3 are made at Init and removed at
// Shutdown (the core has no subscribe callback; the spec lists them under the
// shared base). The port lots are held (§2: "the lot (held)"): this glue keeps
// one reference per port lot the core lists, synchronised after every call on
// the main thread (CreateLot's reference for the caller becomes it), and one
// on the department.
//
// Not wired (logged once): the seaport's "no sea access" prop state switch
// (§4.4 step 3, presentation only: props with property 0xC98204B8 set to
// state 1); the advisor event of that step is pushed.
//
// Threads, as the landfill developer (cFC4Landfill.h): every call into the
// core holds this object's lock; a getter on a thread other than the main one
// waits up to kLockWaitMs and otherwise answers from a snapshot (average
// efficiency, trips, port count, sea) or with the neutral value.
//
// OFF by default: FreeCity4.ini `enable = ports` (FC4Subsystems.h).
//
// Log lines (FreeCity4.log):
//   ports <kind> Init|Read|Shutdown: ok=.. init=.. tuning=.. types=../.. ports=.. dept=.. sea=.. subscriptions=..
//     agent=.. held=.. fallbacks=..
//   ports <kind> Write -> 1 (ports=..)
//   ports <kind> month YYYY-MM-DD: ports=.. ran=.. skipped=.. history=.. event=0x.. avg=.. trips=.. fr=..
//     required=.. writes: conditions=.. effects=.. capacities=.. swaps=.. withdrawals=.. dezones=..
#pragma once

#include "FC4PortInterfaces.h"
#include "cIGZMessageTarget2.h"
#include "cIGZSerializable.h"
#include "fc4.h"

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>

#include <atomic>
#include <cstdint>
#include <map>
#include <string>
#include <vector>

class cIGZMessageServer2;
class cISC4Simulator;

namespace fc4 {

// What the port host callbacks work on. Each developer owns one; the ports
// shadow (FC4PortsShadow.h) makes read-only ones (no writes to the game).
struct PortContext {
    bool readOnly = false;
    uint32_t kind = 0;                     // 0 airport, 1 seaport
    cIGZMessageTarget2* target = nullptr;  // the developer's +8 (agent)
    cISC4Simulator* agentSim = nullptr;    // held from AddAgent to RemoveAgent
    bool agentRegistered = false;
    // CreateLot's lots: their reference for the caller is the developer's
    // hold (taken over by the hold synchronisation).
    std::vector<uint64_t> createdLots;
    // Counters (log lines).
    uint32_t conditions = 0, effects = 0, histories = 0, events = 0, capacities = 0, swaps = 0, withdrawals = 0,
             dezones = 0, departments = 0;
};

fc4_port_host MakePortHost(PortContext* ctx);

const char* PortKindName(uint32_t kind);

}  // namespace fc4

// The shared developer: Primary is cFC4ISC4AirportDeveloper or
// cFC4ISC4SeaportDeveloper (the sub-object order of §1).
template <class Primary>
class cFC4PortDeveloperT : public Primary,
                           public cFC4ISC4PortSubObject,
                           public cIGZMessageTarget2,
                           public cIGZSerializable {
public:
    explicit cFC4PortDeveloperT(uint32_t kind);
    virtual ~cFC4PortDeveloperT();

    // cIGZUnknown (§1)
    bool QueryInterface(uint32_t riid, void** ppvObj) override;
    uint32_t AddRef() override;
    uint32_t Release() override;

    // Slots 3-10 (§8.1)
    bool Init() override;
    bool Shutdown() override;
    SC4Percentage* GetAvgEfficiency(SC4Percentage* out) override;
    uint32_t GetSimulatorType() override;
    bool GetLeastEfficientPort(SC4Percentage& pct, SC4Point<int32_t>& point) override;
    void GetAllPortLots(fc4::GameVector<cISC4Lot*>& lots) override;
    uint32_t GetNumPorts(int32_t sizeFlags, int32_t stageFlags) override;
    cFC4ISC4PortSubObject* AsISC4PortDeveloper() override { return this; }

    // cIGZMessageTarget2 (§3)
    bool DoMessage(cIGZMessage2* pMessage) override;

    // cIGZSerializable (§7)
    bool Write(cIGZOStream& gzOut) override;
    bool Read(cIGZIStream& gzIn) override;
    uint32_t GetGZCLSID() override;

    // How long a getter on a thread other than the main one waits for the lock.
    static constexpr uint32_t kLockWaitMs = 20;

    fc4_port_developer* Core() const { return core_; }
    uint32_t Fallbacks() const { return fallbacks_.load(); }

protected:
    // Slots 12-14 (§4.7), shared by both kinds.
    bool UsageValues(cISC4Lot* lot, uint32_t& usage, uint32_t& capacity);
    uint32_t NumTrips();
    int64_t Maintenance(cISC4Lot* lot);
    uint32_t PortCount();  // GetNumAirports(0) / GetNumSeaports()

    // Holds the lock for one call into the core (cFC4Landfill.h).
    class Section {
    public:
        explicit Section(const cFC4PortDeveloperT* s, bool wait = false);
        ~Section();
        Section(const Section&) = delete;
        Section& operator=(const Section&) = delete;
        bool ok() const { return ok_; }

    private:
        cFC4PortDeveloperT* s_;
        bool ok_;
    };

    struct Snapshot {
        std::atomic<uint32_t> efficiencyBits{0x3F800000u};  // 1.0f (no ports)
        std::atomic<uint32_t> trips{0}, ports{0};
        std::atomic<bool> sea{false};
    };

    bool Enter(bool wait);
    void Leave();
    void AfterCall();  // the outermost section: holds (main thread) and snapshot
    void SyncHolds();
    void ReleaseHolds();
    void Subscribe(bool on);
    void LogState(const char* what, bool ok);
    void LogMonth(uint32_t julian);
    cIGZUnknown* PrimaryUnknown() { return static_cast<cIGZUnknown*>(static_cast<Primary*>(this)); }

    uint32_t kind_;
    uint32_t refCount_ = 0;
    bool destroying_ = false;
    fc4::PortContext ctx_;
    fc4_port_host host_{};
    fc4_port_developer* core_ = nullptr;
    cIGZMessageServer2* messageServer_ = nullptr;  // held while subscribed
    uint32_t subscriptions_ = 0;
    std::map<uint64_t, uint32_t> heldLots_;  // lot token -> references this object holds
    uint64_t heldDepartment_ = 0;

    mutable CRITICAL_SECTION lock_;
    DWORD mainTid_ = 0;
    int depth_ = 0;
    mutable std::atomic<uint32_t> fallbacks_{0};
    std::atomic<uint32_t> offThreadMessages_{0};
    Snapshot snap_;
    uint32_t months_ = 0;
};

class cFC4AirportDeveloper final : public cFC4PortDeveloperT<cFC4ISC4AirportDeveloper> {
public:
    cFC4AirportDeveloper() : cFC4PortDeveloperT(0) {}
    uint32_t GetNumAirports(int32_t size) override;
    bool GetAirportUsageValues(cISC4Lot* lot, uint32_t& usage, uint32_t& capacity) override {
        return UsageValues(lot, usage, capacity);
    }
    uint32_t GetNumAirportTrips() override { return NumTrips(); }
    int64_t GetMaintenanceCost(cISC4Lot* lot) override { return Maintenance(lot); }
};

class cFC4SeaportDeveloper final : public cFC4PortDeveloperT<cFC4ISC4SeaportDeveloper> {
public:
    cFC4SeaportDeveloper() : cFC4PortDeveloperT(1) {}
    uint32_t GetNumSeaports() override { return PortCount(); }
    bool GetSeaportUsageValues(cISC4Lot* lot, uint32_t& usage, uint32_t& capacity) override {
        return UsageValues(lot, usage, capacity);
    }
    uint32_t GetNumSeaportTrips() override { return NumTrips(); }
    int64_t GetMaintenanceCost(cISC4Lot* lot) override { return Maintenance(lot); }
    bool CityHasSea() override;
    uint8_t GetCellSeaAccess(int32_t x, int32_t z) override;
};

// Factories for the subsystem table (reference count 0).
cIGZUnknown* CreateFC4AirportDeveloper();
cIGZUnknown* CreateFC4SeaportDeveloper();

// Tooling (the harness script's `port` action, FC4Script.h; glue-portdev4): builds
// the stage-0 lot of port type `type` (the tuning exemplar's list 0x8A270FC3,
// the type's lot configuration list 0xCA271781 entry 0) with its top-left cell
// at (x, z), as the swap of port-developers.md §4.5 builds a stage: lot manager
// slot 23 CreateLot (size from the configuration, slot 7), SetLotConfiguration,
// UpdateZoneType, lot developer slot 7 ConstructDefaultLot(lot, cfg, 0, true).
// Works through the game's interfaces only, so the original developers adopt it
// as ours do (§3 0x99EF1142). Returns the result text for the script log.
std::string PortScriptBuild(uint32_t kind, uint32_t type, int32_t x, int32_t z, uint32_t facing);
