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

// The power simulator's GZCOM class (docs/specs/power.md §1, CLSID
// 0x0990C005). The simulation lives in the Rust core behind fc4.h
// (fc4_power); this class implements cISC4PowerSimulator in the game's
// compiled slot order (cFC4ISC4PowerSimulator, §10.1) and the host callbacks
// (fc4_power_host) over the game's services (§11.1).
//
// The powered grid is the GAME's own cSC4SimGrid<uint8> (§11.2), created
// through GZCOM (FC4HostGrid.h).
//
// The subsystem is OFF by default: FreeCity4.ini `enable = power` turns it on
// (FC4Subsystems.h); `disable` still wins. The power_* switches (FC4Config.h)
// turn groups of game calls off for bisecting.
//
// Containers and direct calls (docs/abi): slot 25 appends to the caller's
// ilist (A: SC4List); the occupant query (occupant manager slot 40) fills an
// SC4List (A, one reference each) that is kept until the outermost call into
// the core returns; DemolishLots reads an SC4List of lots; the pole's
// non-virtual RemoveConnection is called by address on the identified
// executable only (docs/abi/direct-calls.md). Still refused (logged): the
// line's GetConnectedPoles (its outputs are not in docs/abi) and the 3D
// view's input control (cIGZWin is not in docs/abi/interfaces.tsv).
#pragma once

#include "FC4HostGrid.h"
#include "FC4Interfaces.h"
#include "cIGZMessageTarget2.h"
#include "cIGZSerializable.h"
#include "fc4.h"

#include <cstdint>
#include <memory>
#include <vector>

class cIGZMessageServer2;
class cISC4City;
class cISC4Demolition;
class cISC4NeighborsSimulator;
class cISC4OccupantManager;
class cISC4ZoneManager;

static const uint32_t kFC4_CLSID_cSC4PowerSimulator = 0x0990C005;
static const uint32_t kFC4_IID_cISC4PowerSimulator = 0xE9E7BFF5;

// The game services the power host callbacks read (§11.1). The simulator
// fills it with references of its own (AcquireServices); the power shadow
// (FC4PowerShadow.h) with the city's borrowed pointers, for one delivery.
struct FC4PowerGame {
    cISC4City* city = nullptr;
    cISC4OccupantManager* occupantManager = nullptr;
    cISC4ZoneManager* zoneManager = nullptr;
    cISC4NeighborsSimulator* neighbors = nullptr;
    cISC4BudgetSimulator* budget = nullptr;
    cISC4Demolition* demolition = nullptr;
    // False only for the power shadow's test seam: the city answers its cell
    // geometry and nothing else (the ordinance factor reads 1, the sim time 0,
    // there are no lots).
    bool cityServices = true;
    // Game lists (and the references they hold) kept until the outermost call
    // into the core returns (ours) or the shadowed delivery ends (shadow).
    std::vector<std::shared_ptr<void>> held;
    void Hold(std::shared_ptr<void> list) { held.push_back(std::move(list)); }
};

namespace fc4 {
// A host table with the read callbacks only (city geometry, tuning, ordinance
// factor, sim time, occupants and their properties, poles, lines, network
// occupants, the occupant query, zones, lots, deals, neighbour connections,
// departments, line items, budget items) over `game`, and a grid table with
// geometry and reads only (`grids` must outlive the table). Every write
// callback is NULL. For the power shadow.
fc4_power_host PowerReadOnlyHost(FC4PowerGame* game, const HostGridContext* grids);
}  // namespace fc4

class cFC4PowerSimulator final : public cFC4ISC4PowerSimulator,
                                 public cIGZMessageTarget2,
                                 public cIGZSerializable,
                                 public FC4PowerGame {
public:
    cFC4PowerSimulator();
    ~cFC4PowerSimulator();

    // cIGZUnknown
    bool QueryInterface(uint32_t riid, void** ppvObj) override;
    uint32_t AddRef() override;
    uint32_t Release() override;

    // cISC4PowerSimulator (§10.1, compiled order)
    bool Init() override;
    bool Shutdown() override;
    void PositionToCell(float x, float z, int32_t& cellX, int32_t& cellZ) override;
    void CellToPosition(int32_t cellX, int32_t cellZ, float& x, float& z) override;
    void CellToStandardCityCell(int32_t x, int32_t z, int32_t& outX, int32_t& outZ) override;
    void StandardCityCellToCell(int32_t x, int32_t z, int32_t& outX, int32_t& outZ) override;
    uint32_t GetAnnualConsumption() override;
    uint32_t GetAnnualConsumptionByPowerType(int32_t powerType) override;
    int32_t GetMonthlyRequirement() override;
    int32_t GetMonthlyProduction() override;
    int32_t GetMonthlyImport() override;
    int32_t GetMonthlyExport() override;
    int32_t GetMonthlyDemand() override;
    int32_t GetMonthlyConsumptionAt(int32_t x, int32_t z) override;
    int32_t GetMonthlyConsumptionTotal() override;
    void GetMonthlyConsumptionMapPtr(void*& map) override;
    uint32_t GetSupplyValue(int32_t x, int32_t z) override;
    void GetSupplyValueMapPtr(cISC4SimGrid<uint8_t>*& grid) override;
    int32_t GetUnpoweredConsumerCount() override;
    int32_t GetPoweredConsumerCount() override;
    int32_t GetPowerPlantCountOfType(int32_t powerType) override;
    int32_t GetPowerPlantCountAll() override;
    void GetPowerPlants(SC4List<cISC4Occupant*>& list) override;
    uint32_t GetPowerPoleCount() override;
    int32_t GetOverworkedPowerPlantCount() override;
    int32_t GetAgedPowerPlantCount() override;
    int32_t GetDistressedPowerPlantCount() override;
    void FindPowerPlants(cISC4Occupant** lowestFunded, SC4Percentage& funding, cISC4Occupant** mostWorn,
                         int32_t& wearPercent) override;
    uint32_t IsOccupantPowered(cISC4Occupant* occupant) override;
    bool GetOccupantPowerConsumed(cISC4Occupant* occupant, uint32_t& consumed) override;
    bool GetOccupantPowerProduced(cISC4Occupant* occupant, uint32_t& produced, uint32_t& used) override;
    bool GetOccupantMaxPowerProduced(cISC4Occupant* occupant, uint32_t& maxProduced) override;
    cISC4SimGrid<uint8_t>* GetPoweredGridConst() const override;
    cISC4SimGrid<uint8_t>* GetPoweredGrid() override;
    uint32_t IsPowered(int32_t x, int32_t z) override;
    uint32_t IsWired(int32_t x, int32_t z) override;
    bool IsOnStrike() override;
    SC4Percentage* GetChanceOfStrike(SC4Percentage& chance) override;
    bool BeginStrike() override;
    bool EndStrike() override;
    int32_t GetStrikeStartDate() override;
    int32_t GetStrikeLength() override;
    SC4Percentage* GetCurrentPlantEfficiencyPercent(SC4Percentage* result, cISC4Occupant* occupant) override;
    SC4Percentage* GetCurrentPlantConditionPercent(SC4Percentage* result, cISC4Occupant* occupant) override;
    SC4Percentage* GetStrikeEfficiencyPercent() override;
    SC4Percentage* GetFundingPercent() override;
    bool GetMonthlyPowerGenerationCost(cISC4Occupant* occupant, int64_t& cost) override;
    bool GetPowerPlantFundingPercentage(cISC4Occupant* occupant, SC4Percentage& funding) override;
    bool SetPowerPlantFundingPercentage(cISC4Occupant* occupant, SC4Percentage const& funding) override;

    // cIGZMessageTarget2 (§4.3, §5)
    bool DoMessage(cIGZMessage2* pMessage) override;

    // cIGZSerializable (§9)
    bool Write(cIGZOStream& gzOut) override;
    bool Read(cIGZIStream& gzIn) override;
    uint32_t GetGZCLSID() override;

    // For the host callbacks (cFC4Power.cpp).
    cISC4City* City() const { return city; }
    cISC4OccupantManager* OccupantManager() const { return occupantManager; }
    cISC4ZoneManager* ZoneManager() const { return zoneManager; }
    cISC4NeighborsSimulator* Neighbors() const { return neighbors; }
    cISC4BudgetSimulator* Budget() const { return budget; }
    // Test seam only (PowerTestBudgetPath): a borrowed budget, cleared before release.
    void TestSetBudget(cISC4BudgetSimulator* b) { budget = b; }
    cISC4Demolition* Demolition() const { return demolition; }
    cIGZMessageTarget2* Target() { return static_cast<cIGZMessageTarget2*>(this); }
    void AcquireServices();
    void ReleaseServices();
    void Subscribe(const uint32_t* ids, size_t count, bool add);
    // Hold (FC4PowerGame) keeps a game list (and the references it holds)
    // until the outermost call into the core returns.

    struct ConsumptionMap;  // slot 18 (cFC4Power.cpp)

    // Counts the calls into the core; the outermost one drops the held lists.
    class CoreCall {
    public:
        explicit CoreCall(cFC4PowerSimulator* s) : s_(s) { ++s_->depth_; }
        ~CoreCall() {
            if (--s_->depth_ == 0) s_->held.clear();
        }
        CoreCall(const CoreCall&) = delete;
        CoreCall& operator=(const CoreCall&) = delete;

    private:
        cFC4PowerSimulator* s_;
    };

private:
    uint64_t Occ(cISC4Occupant* occupant) const;
    void LogState(const char* what);
    // The recompute report (power_report): logs the last complete cycle's.
    void LogReport();

    uint32_t refCount_ = 0;
    bool destroying_ = false;
    fc4::HostGridContext gridCtx_{"power", fc4::Sw::PowerGridWrites};
    fc4_power_host host_{};
    fc4_power* core_ = nullptr;
    cIGZMessageServer2* msgServer_ = nullptr;
    int depth_ = 0;
    ConsumptionMap* map_ = nullptr;
    SC4Percentage strikeEfficiency_{};  // slot 47 returns its address
    SC4Percentage noDepartment_{};      // slot 48 without a department [choice]
    uint32_t lastState_ = 4;
    uint32_t cycles_ = 0;
    bool report_ = false;
    uint32_t reports_ = 0;        // reports logged
    bool reportAfterMonth_ = false;  // log the first report after a month
};

// Factory for the subsystem table (reference count 0).
cIGZUnknown* CreateFC4PowerSimulator();

// Test seam op 6 (FC4TestSeam.h): 0 = the department / line-item path passed.
intptr_t PowerTestBudgetPath(cISC4BudgetSimulator* budget);

// Counted line-slot outputs, including partial and aliased pairs.
intptr_t PowerTestLineConnections(cIGZUnknown* line, uint64_t* pair);
