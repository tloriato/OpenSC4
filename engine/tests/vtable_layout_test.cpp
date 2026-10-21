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

// Slot layout of every game interface the plugin implements (foundations §16).
//
// For each interface a probe class implements every method so that it records
// the slot number the SPEC gives that method. The test then calls every slot
// N through the vtable, the way the game does (__thiscall, the method's
// number of 32-bit stack arguments), and checks that (a) the method that ran
// is the one the spec puts at slot N and (b) the stack is balanced. The
// tables below are the spec tables: simulator §5.6, budget "Verified slot
// order", and SDK order for the interfaces without overloads.

#include "FC4Interfaces.h"
#include "game_abi.h"

#include "cISC4DepartmentBudget.h"
#include "cISC4LineItem.h"

#include <cstdio>

namespace {

int g_failures = 0;
int g_hit = -1;

#define P(N, R, NAME, PARAMS, Q) \
    R NAME PARAMS Q override {   \
        g_hit = N;               \
        return static_cast<R>(0); \
    }

struct Expect {
    int slot;
    int dwords;       // 32-bit stack arguments the game pushes
    bool floatResult;  // returns in ST0 (checked, then popped); otherwise nothing may be left on the x87 stack
    const char* name;
};

template <size_t N>
void Check(const char* iface, void* probe, const Expect (&table)[N]) {
    int checked = 0;
    for (const Expect& e : table) {
        g_hit = -1;
        const uint32_t zeros[16] = {};
        const fc4test::SlotCall r = fc4test::CallSlotArray(probe, e.slot, zeros, e.dwords);
        for (int i = 0; i < r.x87Pushed; ++i) fc4test::PopX87();
        if (g_hit != e.slot || r.espDelta != 0 || r.x87Pushed != (e.floatResult ? 1 : 0)) {
            std::fprintf(stderr, "%s slot %d (%s, %d dwords): ran slot %d's method, stack off by %d bytes, %d x87 values\n",
                         iface, e.slot, e.name, e.dwords, g_hit, r.espDelta, r.x87Pushed);
            ++g_failures;
        }
        ++checked;
    }
    std::printf("%s: %d slots checked\n", iface, checked);
}

// ---- cISC4Simulator (simulator §5.6) -----------------------------------------------------------------

struct SimProbe final : cFC4ISC4Simulator {
    bool QueryInterface(uint32_t, void**) override { return false; }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    P(3, bool, Init, (), )
    P(4, bool, Shutdown, (), )
    P(5, bool, GetSimStartDate, (cFC4IGZDate&), )
    P(6, void, GetSimDateParts, (uint32_t*, uint32_t*, uint32_t*, uint32_t*, uint32_t*), )
    P(7, cFC4IGZDate*, GetSimDate, (), )
    P(8, int32_t, GetSimDateNumber, (), )
    P(9, bool, Pause, (), )
    P(10, bool, HiddenPause, (), )
    P(11, bool, EmergencyPause, (), )
    P(12, bool, Resume, (), )
    P(13, bool, HiddenResume, (), )
    P(14, bool, EmergencyResume, (), )
    P(15, bool, IsPaused, (), )
    P(16, bool, IsHiddenPaused, (), )
    P(17, bool, IsEmergencyPaused, (), )
    P(18, bool, IsAnyPaused, (), )
    P(19, bool, AddAgent, (cIGZMessageTarget2*, uint32_t, cIGZString const&, uint32_t), )
    P(20, bool, RemoveAgentAllLevels, (cIGZMessageTarget2*), )
    P(21, bool, RemoveAgent, (cIGZMessageTarget2*, uint32_t), )
    P(22, bool, RemoveAllAgentsAt, (uint32_t), )
    P(23, bool, RemoveAllAgents, (), )
    P(24, bool, EnumerateAgentsByName, (fc4::GameVector<fc4::GameString>&), )
    P(25, bool, GetAgentEnabled, (cIGZString const&), )
    P(26, bool, SetAgentEnabled, (cIGZString const&, bool), )
    P(27, int32_t, GetSimSpeed, (), )
    P(28, bool, SetSimSpeed, (int32_t), )
    P(29, int32_t, GetSimTime, (), )
    P(30, bool, SetSimTime, (int32_t), )
    P(31, bool, SetMaxMillisecondsPerTick, (uint32_t), )
    P(32, float, GetAnimationTimeDilation, (), )
    P(33, bool, SetCityEstablished, (bool), )
};

const Expect kSimulator[] = {
    {3, 0, false, "Init"}, {4, 0, false, "Shutdown"}, {5, 1, false, "GetSimStartDate"},
    {6, 5, false, "GetSimDate(5 pointers)"}, {7, 0, false, "GetSimDate()"}, {8, 0, false, "GetSimDateNumber"},
    {9, 0, false, "Pause"}, {10, 0, false, "HiddenPause"}, {11, 0, false, "EmergencyPause"},
    {12, 0, false, "Resume"}, {13, 0, false, "HiddenResume"}, {14, 0, false, "EmergencyResume"},
    {15, 0, false, "IsPaused"}, {16, 0, false, "IsHiddenPaused"}, {17, 0, false, "IsEmergencyPaused"},
    {18, 0, false, "IsAnyPaused"}, {19, 4, false, "AddAgent"}, {20, 1, false, "RemoveAgent(agent)"},
    {21, 2, false, "RemoveAgent(agent, level)"}, {22, 1, false, "RemoveAllAgents(level)"},
    {23, 0, false, "RemoveAllAgents()"}, {24, 1, false, "EnumerateAgentsByName"}, {25, 1, false, "GetAgentEnabled"},
    {26, 2, false, "SetAgentEnabled"}, {27, 0, false, "GetSimSpeed"}, {28, 1, false, "SetSimSpeed"},
    {29, 0, false, "GetSimTime"}, {30, 1, false, "SetSimTime"}, {31, 1, false, "SetMaxMillisecondsPerTick"},
    {32, 0, true, "GetAnimationTimeDilation"}, {33, 1, false, "SetCityEstablished"},
};

// ---- cISC4BudgetSimulator (budget "Verified slot order") -----------------------------------------------

struct BudgetProbe final : cFC4ISC4BudgetSimulator {
    bool QueryInterface(uint32_t, void**) override { return false; }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    P(3, bool, Init, (), )
    P(4, bool, Shutdown, (), )
    P(5, bool, SetTotalFunds, (int64_t), )
    P(6, int64_t, GetTotalFunds, (), )
    P(7, bool, DepositFunds, (int64_t), )
    P(8, bool, WithdrawFunds, (int64_t), )
    P(9, int64_t, GetMinAllowableFunds, (), )
    P(10, int64_t, GetYTDIncome, (), )
    P(11, int64_t, GetEstIncome, (), )
    P(12, int64_t, GetYTDExpenses, (), )
    P(13, int64_t, GetEstExpenses, (), )
    P(14, int64_t, GetTotalMonthlyExpense, (), )
    P(15, int64_t, GetTotalYearlyExpense, (), )
    P(16, int64_t, GetTotalMonthlyIncome, (), )
    P(17, int64_t, GetTotalYearlyIncome, (), )
    P(18, int64_t, GetTaxIncomeOfType, (int32_t), )
    P(19, int64_t, GetTaxIncome, (), )
    P(20, int32_t, SetTotalMonthlyExpense, (int64_t), )
    P(21, int32_t, SetTotalYearlyExpense, (int64_t), )
    P(22, int32_t, SetTotalMonthlyIncome, (int64_t), )
    P(23, int32_t, SetTotalYearlyIncome, (int64_t), )
    P(24, bool, ShowBudgetWindow, (), )
    P(25, cISC4DepartmentBudget*, CreateDepartmentBudget, (uint32_t, uint32_t), )
    P(26, bool, RemoveDepartmentBudget, (uint32_t), )
    P(27, cISC4DepartmentBudget*, GetDepartmentBudgetByName, (cIGZString const&), )
    P(28, cISC4DepartmentBudget*, GetDepartmentBudget, (uint32_t), )
    P(29, bool, GetAllGroups, (fc4::GameVector<fc4::GameBudgetGroupInfo>&), )
    P(30, bool, SetGroupName, (uint32_t, cIGZString&), )
    P(31, bool, GetDepartmentBudgetsInGroup, (uint32_t, fc4::GameVector<cISC4DepartmentBudget*>&), )
    P(32, void, NeededFundingChanged, (cISC4DepartmentBudget*), )
    P(33, void, FundingPercentageChanged, (cISC4DepartmentBudget*), )
    P(34, int64_t, GetFundingById, (uint32_t), )
    P(35, int64_t, GetFundingOf, (cISC4DepartmentBudget*), )
    P(36, float, GetTaxRate, (uint32_t), )
    P(37, bool, SetTaxRate, (uint32_t, float), )
    P(38, int64_t, GetWeightedAssessedTaxValue, (uint32_t), )
    P(39, int64_t, GetBondIncrement, (), )
    P(40, bool, IssueBond, (uint32_t), )
    P(41, int64_t, GetTotalBorrowed, (), )
    P(42, int64_t, GetCurrentBorrowingLimit, (), )
    P(43, int64_t, GetCurrentBondLimit, (), )
    P(44, int64_t, GetCurrentMaxOutstandingBondsLimit, (), )
    P(45, int64_t, GetTotalMonthlyBondPayments, (), )
    P(46, bool, GetAllLoans, (fc4::GameVector<LoanInfo>&), )
    P(47, int32_t, GetLoanTimeInMonths, (), )
    P(48, int64_t, GetMonthlyPaymentForLoan, (int64_t), )
    P(49, int64_t, GetFullCostOfLoan, (int64_t), )
    P(50, bool, GetBudgetItemInfo, (cISCPropertyHolder*, fc4::GameVector<BudgetItem>&), )
    P(51, bool, GetBudgetItemForPurpose, (cISCPropertyHolder*, uint32_t, BudgetItem&), )
    P(52, bool, ChangeBudgetItemLine, (cISCPropertyHolder*, uint32_t, uint32_t), )
    P(53, bool, ChangeBudgetItemCost, (cISCPropertyHolder*, uint32_t, int64_t), )
    P(54, bool, ChangeBudgetItemLocalFunding, (cISCPropertyHolder*, uint32_t, SC4Percentage const&), )
    P(55, bool, AddBudgetItemToBudget, (cISCPropertyHolder*, uint32_t), )
    P(56, bool, RemoveBudgetItemFromBudget, (cISCPropertyHolder*, uint32_t), )
    P(57, cISC4LineItem*, GetLineItemFromBudgetItem, (BudgetItem&), )
    P(58, bool, CopyBudgetItemProperties, (cISCPropertyHolder*, cISCPropertyHolder*), )
};

const Expect kBudget[] = {
    {3, 0, false, "Init"}, {4, 0, false, "Shutdown"}, {5, 2, false, "SetTotalFunds"}, {6, 0, false, "GetTotalFunds"},
    {7, 2, false, "DepositFunds"}, {8, 2, false, "WithdrawFunds"}, {9, 0, false, "GetMinAllowableFunds"},
    {10, 0, false, "GetYTDIncome"}, {11, 0, false, "GetEstIncome"}, {12, 0, false, "GetYTDExpenses"},
    {13, 0, false, "GetEstExpenses"}, {14, 0, false, "GetTotalMonthlyExpense"},
    {15, 0, false, "GetTotalYearlyExpense"}, {16, 0, false, "GetTotalMonthlyIncome"},
    {17, 0, false, "GetTotalYearlyIncome"}, {18, 1, false, "GetTaxIncome(int32)"}, {19, 0, false, "GetTaxIncome()"},
    {20, 2, false, "SetTotalMonthlyExpense"}, {21, 2, false, "SetTotalYearlyExpense"},
    {22, 2, false, "SetTotalMonthlyIncome"}, {23, 2, false, "SetTotalYearlyIncome"},
    {24, 0, false, "ShowBudgetWindow"}, {25, 2, false, "CreateDepartmentBudget"},
    {26, 1, false, "RemoveDepartmentBudget"}, {27, 1, false, "GetDepartmentBudget(name)"},
    {28, 1, false, "GetDepartmentBudget(id)"}, {29, 1, false, "GetAllGroups"}, {30, 2, false, "SetGroupName"},
    {31, 2, false, "GetDepartmentBudgetsInGroup"}, {32, 1, false, "NeededFundingChanged"},
    {33, 1, false, "FundingPercentageChanged"}, {34, 1, false, "GetFunding(uint32)"},
    {35, 1, false, "GetFunding(department)"}, {36, 1, true, "GetTaxRate"}, {37, 2, false, "SetTaxRate"},
    {38, 1, false, "GetWeightedAssessedTaxValue"}, {39, 0, false, "GetBondIncrement"}, {40, 1, false, "IssueBond"},
    {41, 0, false, "GetTotalBorrowed"}, {42, 0, false, "GetCurrentBorrowingLimit"},
    {43, 0, false, "GetCurrentBondLimit"}, {44, 0, false, "GetCurrentMaxOutstandingBondsLimit"},
    {45, 0, false, "GetTotalMonthlyBondPayments"}, {46, 1, false, "GetAllLoans"},
    {47, 0, false, "GetLoanTimeInMonths"}, {48, 2, false, "GetMonthlyPaymentForLoan"},
    {49, 2, false, "GetFullCostOfLoan"}, {50, 2, false, "GetBudgetItemInfo"},
    {51, 3, false, "GetBudgetItemForPurpose"}, {52, 3, false, "ChangeBudgetItemLine"},
    {53, 4, false, "ChangeBudgetItemCost"}, {54, 3, false, "ChangeBudgetItemLocalFunding"},
    {55, 2, false, "AddBudgetItemToBudget"}, {56, 2, false, "RemoveBudgetItemFromBudget"},
    {57, 1, false, "GetLineItemFromBudgetItem"}, {58, 2, false, "CopyBudgetItemProperties"},
};

// ---- cISC4DepartmentBudget (no overloads: SDK order; slot 26 per the budget erratum) --------------------

const SC4Percentage kProbePct{0.25f};

struct DepartmentProbe final : cFC4ISC4DepartmentBudget {
    bool QueryInterface(uint32_t, void**) override { return false; }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    P(3, uint32_t, GetDepartmentID, (), const)
    P(4, bool, GetDepartmentName, (cIGZString&), )
    P(5, bool, SetDepartmentName, (uint32_t, uint32_t), )
    P(6, uint32_t, GetBudgetGroup, (), const)
    P(7, bool, SetBudgetGroup, (uint32_t), )
    P(8, bool, GetIsFixedFunding, (), const)
    P(9, bool, SetFixedFunding, (bool), )
    P(10, SC4Percentage*, GetFundingPercentage, (), const)
    P(11, bool, SetFundingPercentage, (SC4Percentage const&, uint32_t), )
    P(12, SC4Percentage*, GetMaxAllowedFundingPercentage, (), const)
    P(13, bool, SetMaxAllowedFundingPercentage, (SC4Percentage const&), )
    P(14, int64_t, GetIdealMonthlyFunding, (), const)
    P(15, int64_t, GetTotalExpenses, (), const)
    P(16, int64_t, GetTotalIncome, (), const)
    P(17, cISC4LineItem*, CreateLineItem, (uint32_t, bool), )
    P(18, cISC4LineItem*, CreateLineItemForBuildingType, (uint32_t, bool), )
    P(19, bool, RemoveLineItem, (uint32_t), )
    P(20, cISC4LineItem*, GetLineItem, (uint32_t), )
    P(21, bool, GetAllLineItems, (fc4::GameVector<cISC4LineItem*>&), )
    P(22, bool, AddLocallyFundedObject, (cISCPropertyHolder*, uint32_t), )
    P(23, bool, RemoveLocallyFundedObject, (cISCPropertyHolder*, uint32_t), )
    P(24, bool, SetLocalFundingPercent, (cISCPropertyHolder*, SC4Percentage*, uint32_t), )
    P(25, bool, SetLocalFullFunding, (cISCPropertyHolder*, int64_t, uint32_t), )
    SC4Percentage const* GetLocalFundingPercentPtr(cISCPropertyHolder*, uint32_t) override {
        g_hit = 26;
        return &kProbePct;
    }
    P(27, int64_t, GetLocalFullFunding, (cISCPropertyHolder*, uint32_t), )
    P(28, bool, IsLocallyFundedObjectInDepartment, (cISCPropertyHolder*, uint32_t), )
    P(29, bool, RecalculateAllLineItemCurrentExpenses, (), )
    P(30, int64_t, GetTotalSpending, (), const)
    P(31, bool, SetTotalSpending, (int64_t), )
    P(32, bool, GetLocallyFundedItemsByPurpose, (uint32_t, fc4::GameList<cISCPropertyHolder*>&), )
    P(33, bool, HasLocallyFundedItemsByPurpose, (uint32_t), )
};

const Expect kDepartment[] = {
    {3, 0, false, "GetDepartmentID"}, {4, 1, false, "GetDepartmentName"}, {5, 2, false, "SetDepartmentName"},
    {6, 0, false, "GetBudgetGroup"}, {7, 1, false, "SetBudgetGroup"}, {8, 0, false, "GetIsFixedFunding"},
    {9, 1, false, "SetFixedFunding"}, {10, 0, false, "GetFundingPercentage"},
    {11, 2, false, "SetFundingPercentage"}, {12, 0, false, "GetMaxAllowedFundingPercentage"},
    {13, 1, false, "SetMaxAllowedFundingPercentage"}, {14, 0, false, "GetIdealMonthlyFunding"},
    {15, 0, false, "GetTotalExpenses"}, {16, 0, false, "GetTotalIncome"}, {17, 2, false, "CreateLineItem"},
    {18, 2, false, "CreateLineItemForBuildingType"}, {19, 1, false, "RemoveLineItem"}, {20, 1, false, "GetLineItem"},
    {21, 1, false, "GetAllLineItems"}, {22, 2, false, "AddLocallyFundedObject"},
    {23, 2, false, "RemoveLocallyFundedObject"}, {24, 3, false, "SetLocalFundingPercent"},
    {25, 4, false, "SetLocalFullFunding"}, {26, 2, false, "GetLocalFundingPercent (pointer)"},
    {27, 2, false, "GetLocalFullFunding"}, {28, 2, false, "IsLocallyFundedObjectInDepartment"},
    {29, 0, false, "RecalculateAllLineItemCurrentExpenses"}, {30, 0, false, "GetTotalSpending"},
    {31, 2, false, "SetTotalSpending"}, {32, 2, false, "GetLocallyFundedItemsByPurpose"},
    {33, 1, false, "HasLocallyFundedItemsByPurpose"},
};

// ---- cISC4LineItem (no overloads: SDK order) ------------------------------------------------------------

struct LineProbe final : cISC4LineItem {
    bool QueryInterface(uint32_t, void**) override { return false; }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    P(3, uint32_t, GetID, (), const)
    P(4, Type, GetType, (), const)
    P(5, bool, SetType, (Type), )
    P(6, bool, GetName, (intptr_t&), )
    P(7, bool, SetName, (uint32_t, uint32_t), )
    P(8, int64_t, GetSecondaryInfoField, (), const)
    P(9, bool, SetSecondaryInfoField, (int64_t), )
    P(10, int64_t, GetFullExpenses, (), const)
    P(11, int64_t, GetCurrentExpenses, (), const)
    P(12, bool, SetFullExpenses, (int64_t), )
    P(13, bool, AddToFullExpenses, (int64_t), )
    P(14, int64_t, GetIncome, (), const)
    P(15, bool, SetIncome, (int64_t), )
    P(16, bool, AddToIncome, (int64_t), )
    P(17, DisplayFlag, GetDisplayFlags, (), )
    P(18, bool, SetDisplayFlag, (DisplayFlag, bool), )
    P(19, bool, IsLocalFundingItem, (), const)
};

const Expect kLineItem[] = {
    {3, 0, false, "GetID"}, {4, 0, false, "GetType"}, {5, 1, false, "SetType"}, {6, 1, false, "GetName"},
    {7, 2, false, "SetName"}, {8, 0, false, "GetSecondaryInfoField"}, {9, 2, false, "SetSecondaryInfoField"},
    {10, 0, false, "GetFullExpenses"}, {11, 0, false, "GetCurrentExpenses"}, {12, 2, false, "SetFullExpenses"},
    {13, 2, false, "AddToFullExpenses"}, {14, 0, false, "GetIncome"}, {15, 2, false, "SetIncome"},
    {16, 2, false, "AddToIncome"}, {17, 0, false, "GetDisplayFlags"}, {18, 2, false, "SetDisplayFlag"},
    {19, 0, false, "IsLocalFundingItem"},
};


// ---- framework interfaces (foundations §17): generated from the declarations in FC4Interfaces.h, whose
// overloaded and SDK-error slots were written from the §17 tables. Each slot must run its own method,
// pop exactly the game's argument bytes and return in the right register. --------------------------------

struct DateProbe final : cFC4IGZDate {
    bool QueryInterface(uint32_t, void**) override { return false; }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    bool SetMonthNameDayYear(char const* monthName, uint32_t day, uint32_t year) override { g_hit = 3; return static_cast<bool>(0); }
    bool SetDayNumber(uint32_t dayNumber) override { g_hit = 4; return static_cast<bool>(0); }
    bool SetTime(cRZTime const& time) override { g_hit = 5; return static_cast<bool>(0); }
    bool SetMonthDayYear(uint32_t month, uint32_t day, uint32_t year) override { g_hit = 6; return static_cast<bool>(0); }
    bool SetDayOfYear(uint32_t day, uint32_t year) override { g_hit = 7; return static_cast<bool>(0); }
    bool Copy(cFC4IGZDate const& from) override { g_hit = 8; return static_cast<bool>(0); }
    bool Clone(cFC4IGZDate** out) override { g_hit = 9; return static_cast<bool>(0); }
    bool Between(cFC4IGZDate const& start, cFC4IGZDate const& end) override { g_hit = 10; return static_cast<bool>(0); }
    int32_t CompareTo(cFC4IGZDate const& other) override { g_hit = 11; return static_cast<int32_t>(0); }
    uint32_t WeekDay() override { g_hit = 12; return static_cast<uint32_t>(0); }
    uint32_t Year() override { g_hit = 13; return static_cast<uint32_t>(0); }
    uint32_t DayOfYear() override { g_hit = 14; return static_cast<uint32_t>(0); }
    uint32_t DayOfMonth() override { g_hit = 15; return static_cast<uint32_t>(0); }
    uint32_t FirstDayOfMonthOf(uint32_t month) override { g_hit = 16; return static_cast<uint32_t>(0); }
    uint32_t FirstDayOfMonth() override { g_hit = 17; return static_cast<uint32_t>(0); }
    uint32_t Hash() override { g_hit = 18; return static_cast<uint32_t>(0); }
    bool IsValid() override { g_hit = 19; return static_cast<bool>(0); }
    bool Leap() override { g_hit = 20; return static_cast<bool>(0); }
    bool MaxDate(cFC4IGZDate const& other, cFC4IGZDate& out) override { g_hit = 21; return static_cast<bool>(0); }
    bool MinDate(cFC4IGZDate const& other, cFC4IGZDate& out) override { g_hit = 22; return static_cast<bool>(0); }
    uint32_t Month() override { g_hit = 23; return static_cast<uint32_t>(0); }
    uint32_t Previous(uint32_t dayOfPriorWeek, cFC4IGZDate& out) override { g_hit = 24; return static_cast<uint32_t>(0); }
    uint32_t YearLastTwoDigits() override { g_hit = 25; return static_cast<uint32_t>(0); }
    uint32_t DayNumber() override { g_hit = 26; return static_cast<uint32_t>(0); }
    bool IsLess(cFC4IGZDate const& other) override { g_hit = 27; return static_cast<bool>(0); }
    bool IsLessOrEqual(cFC4IGZDate const& other) override { g_hit = 28; return static_cast<bool>(0); }
    bool IsGreater(cFC4IGZDate const& other) override { g_hit = 29; return static_cast<bool>(0); }
    bool IsGreaterOrEqual(cFC4IGZDate const& other) override { g_hit = 30; return static_cast<bool>(0); }
    bool IsEqual(cFC4IGZDate const& other) override { g_hit = 31; return static_cast<bool>(0); }
    bool IsNotEqual(cFC4IGZDate const& other) override { g_hit = 32; return static_cast<bool>(0); }
    uint32_t MinusDays(int32_t days) override { g_hit = 33; return static_cast<uint32_t>(0); }
    uint32_t MinusDate(cFC4IGZDate const& other) override { g_hit = 34; return static_cast<uint32_t>(0); }
    uint32_t PlusDays(int32_t days) override { g_hit = 35; return static_cast<uint32_t>(0); }
    uint32_t PlusDate(cFC4IGZDate const& other) override { g_hit = 36; return static_cast<uint32_t>(0); }
    cFC4IGZDate& Increment() override { g_hit = 37; return *this; }
    cFC4IGZDate& Decrement() override { g_hit = 38; return *this; }
    cFC4IGZDate& AddDays(int32_t days) override { g_hit = 39; return *this; }
    cFC4IGZDate& SubtractDays(int32_t days) override { g_hit = 40; return *this; }
    bool DateString(cIGZString& out, uint32_t formatID) override { g_hit = 41; return static_cast<bool>(0); }
    bool DayWithinMonth(uint32_t month, uint32_t day, uint32_t year) override { g_hit = 42; return static_cast<bool>(0); }
    uint32_t DaysInYear(uint32_t year) override { g_hit = 43; return static_cast<uint32_t>(0); }
    uint32_t Jday(uint32_t month, uint32_t day, uint32_t year) override { g_hit = 44; return static_cast<uint32_t>(0); }
    bool LeapYear(uint32_t year) override { g_hit = 45; return static_cast<bool>(0); }
};

const Expect kDate[] = {
    {3, 3, false, "SetMonthNameDayYear"}, {4, 1, false, "SetDayNumber"}, {5, 1, false, "SetTime"},
    {6, 3, false, "SetMonthDayYear"}, {7, 2, false, "SetDayOfYear"}, {8, 1, false, "Copy"}, {9, 1, false, "Clone"},
    {10, 2, false, "Between"}, {11, 1, false, "CompareTo"}, {12, 0, false, "WeekDay"}, {13, 0, false, "Year"},
    {14, 0, false, "DayOfYear"}, {15, 0, false, "DayOfMonth"}, {16, 1, false, "FirstDayOfMonthOf"},
    {17, 0, false, "FirstDayOfMonth"}, {18, 0, false, "Hash"}, {19, 0, false, "IsValid"}, {20, 0, false, "Leap"},
    {21, 2, false, "MaxDate"}, {22, 2, false, "MinDate"}, {23, 0, false, "Month"}, {24, 2, false, "Previous"},
    {25, 0, false, "YearLastTwoDigits"}, {26, 0, false, "DayNumber"}, {27, 1, false, "IsLess"},
    {28, 1, false, "IsLessOrEqual"}, {29, 1, false, "IsGreater"}, {30, 1, false, "IsGreaterOrEqual"},
    {31, 1, false, "IsEqual"}, {32, 1, false, "IsNotEqual"}, {33, 1, false, "MinusDays"},
    {34, 1, false, "MinusDate"}, {35, 1, false, "PlusDays"}, {36, 1, false, "PlusDate"},
    {37, 0, false, "Increment"}, {38, 0, false, "Decrement"}, {39, 1, false, "AddDays"},
    {40, 1, false, "SubtractDays"}, {41, 2, false, "DateString"}, {42, 3, false, "DayWithinMonth"},
    {43, 1, false, "DaysInYear"}, {44, 3, false, "Jday"}, {45, 1, false, "LeapYear"},
};

struct HolderProbe final : cFC4ISCPropertyHolder {
    bool QueryInterface(uint32_t, void**) override { return false; }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    bool HasProperty(uint32_t id) const override { g_hit = 3; return static_cast<bool>(0); }
    bool GetPropertyList(cIGZUnknownList** list) const override { g_hit = 4; return static_cast<bool>(0); }
    bool GetPropertyData(uint32_t id, void* buffer, uint32_t& length) const override { g_hit = 5; return static_cast<bool>(0); }
    bool GetPropertyInterface(uint32_t id, uint32_t riid, void** ppv) const override { g_hit = 6; return static_cast<bool>(0); }
    bool GetPropertyString(uint32_t id, cIGZString& value) const override { g_hit = 7; return static_cast<bool>(0); }
    bool GetPropertyU32(uint32_t id, uint32_t& value) const override { g_hit = 8; return static_cast<bool>(0); }
    cISCProperty* GetProperty(uint32_t id) const override { g_hit = 9; return static_cast<cISCProperty*>(0); }
    bool AddPropertyData(uint32_t id, void* data, uint32_t length, bool b) override { g_hit = 10; return static_cast<bool>(0); }
    bool AddPropertyObject(uint32_t id, cIGZUnknown* value, bool b) override { g_hit = 11; return static_cast<bool>(0); }
    bool AddPropertyString(uint32_t id, cIGZString const& value, bool b) override { g_hit = 12; return static_cast<bool>(0); }
    bool AddPropertyU32(uint32_t id, uint32_t value, bool b) override { g_hit = 13; return static_cast<bool>(0); }
    bool AddPropertyVariant(uint32_t id, cFC4IGZVariant const* value, bool b) override { g_hit = 14; return static_cast<bool>(0); }
    bool AddPropertyRecord(cISCProperty* property, bool b) override { g_hit = 15; return static_cast<bool>(0); }
    bool CopyAddProperty(cISCProperty* property, bool b) override { g_hit = 16; return static_cast<bool>(0); }
    bool RemoveProperty(uint32_t id, bool b) override { g_hit = 17; return static_cast<bool>(0); }
    bool RemoveAllProperties(bool b) override { g_hit = 18; return static_cast<bool>(0); }
    bool EnumPropertiesFiltered(FilterFn filter, EnumFn fn, void* data) const override { g_hit = 19; return static_cast<bool>(0); }
    bool EnumProperties(EnumFn fn, void* data) const override { g_hit = 20; return static_cast<bool>(0); }
    bool CompactProperties() override { g_hit = 21; return static_cast<bool>(0); }
};

const Expect kHolder[] = {
    {3, 1, false, "HasProperty"}, {4, 1, false, "GetPropertyList"}, {5, 3, false, "GetPropertyData"},
    {6, 3, false, "GetPropertyInterface"}, {7, 2, false, "GetPropertyString"}, {8, 2, false, "GetPropertyU32"},
    {9, 1, false, "GetProperty"}, {10, 4, false, "AddPropertyData"}, {11, 3, false, "AddPropertyObject"},
    {12, 3, false, "AddPropertyString"}, {13, 3, false, "AddPropertyU32"}, {14, 3, false, "AddPropertyVariant"},
    {15, 2, false, "AddPropertyRecord"}, {16, 2, false, "CopyAddProperty"}, {17, 2, false, "RemoveProperty"},
    {18, 1, false, "RemoveAllProperties"}, {19, 3, false, "EnumPropertiesFiltered"},
    {20, 2, false, "EnumProperties"}, {21, 0, false, "CompactProperties"},
};

struct LotManagerProbe final : cFC4ISC4LotManager {
    bool QueryInterface(uint32_t, void**) override { return false; }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    bool Init() override { g_hit = 3; return static_cast<bool>(0); }
    bool Shutdown() override { g_hit = 4; return static_cast<bool>(0); }
    cISC4Lot* GetLotAtPoint(cS3DVector3 const& point) override { g_hit = 5; return static_cast<cISC4Lot*>(0); }
    cISC4Lot* GetLot(int32_t cellX, int32_t cellZ, bool b) override { g_hit = 6; return static_cast<cISC4Lot*>(0); }
    uint32_t GetLotsInCellRect(fc4::GameList<cISC4Lot*>& lots, int32_t x1, int32_t z1, int32_t x2, int32_t z2, bool locationOnly) override { g_hit = 7; return 0; }
    uint32_t GetLotsSurroundingCell(int32_t cellX, int32_t cellZ, fc4::GameList<cISC4Lot*>& lots, int32_t flags) override { g_hit = 8; return 0; }
    uint32_t GetLotsSurroundingLot(cISC4Lot const* lot, fc4::GameList<cISC4Lot*>& lots, int32_t flags) override { g_hit = 9; return 0; }
    uint32_t GetLotsSurroundingRect(SC4Rect<long> const& rect, fc4::GameList<cISC4Lot*>& lots, int32_t flags) override { g_hit = 10; return 0; }
    bool GetVerticesAtEdgeOfLot(SC4Rect<long> const& cells, int32_t flags, SC4Rect<long>& vertices) override { g_hit = 11; return static_cast<bool>(0); }
    bool GetNeighborCellsAtEdgeOfLot(SC4Rect<long> const& cells, int32_t flags, SC4Rect<long>& out) override { g_hit = 12; return static_cast<bool>(0); }
    cISC4Lot* GetRandomLot() override { g_hit = 13; return static_cast<cISC4Lot*>(0); }
    cISC4Lot* GetOccupantLot(cISC4Occupant* occupant) override { g_hit = 14; return static_cast<cISC4Lot*>(0); }
    intptr_t GetSafeLotIterator() override { g_hit = 15; return static_cast<intptr_t>(0); }
    int32_t GetLotCount() override { g_hit = 16; return static_cast<int32_t>(0); }
    float GetLotSavvyTerrainVertexAltitude(int32_t x, int32_t z, cISC4Lot* exclude, bool* ok) override { g_hit = 17; return static_cast<float>(0); }
    float GetLotSavvyTerrainAltitude(float x, float z, bool* onLot) override { g_hit = 18; return static_cast<float>(0); }
    float GetLotSavvyCellAltitude(int32_t cellX, int32_t cellZ, bool* ok) override { g_hit = 19; return static_cast<float>(0); }
    int32_t CanCreateLot(int32_t x, int32_t z, int32_t w, int32_t h, bool zoneNotNeeded, bool ignoreConfig, cISC4LotConfiguration* config) override { g_hit = 20; return static_cast<int32_t>(0); }
    int32_t CanCreatePloppedLot(SC4Rect<long> bounds, int32_t n, cISC4LotConfiguration* config) override { g_hit = 21; return static_cast<int32_t>(0); }
    bool CreateLotIfPossible(int32_t x, int32_t z, int32_t w, int32_t h, int32_t n, cISC4Lot*& lot, bool zoneNotNeeded, bool ignoreConfig, cISC4LotConfiguration* config) override { g_hit = 22; return static_cast<bool>(0); }
    bool CreateLot(int32_t x, int32_t z, int32_t w, int32_t h, int32_t facing, cISC4Lot*& lot) override { g_hit = 23; return static_cast<bool>(0); }
    bool DeleteLot(cISC4Lot* lot) override { g_hit = 24; return static_cast<bool>(0); }
    bool DeleteLots(int32_t x1, int32_t z1, int32_t x2, int32_t z2) override { g_hit = 25; return static_cast<bool>(0); }
    bool GetLotVertexAltitudeAdjustmentRange(int32_t x, int32_t z, float& rangeX, float& rangeY) override { g_hit = 26; return static_cast<bool>(0); }
    cISC4SimGrid<uint16_t>* GetMaximumCapacityGrid() override { g_hit = 27; return static_cast<cISC4SimGrid<uint16_t>*>(0); }
};

const Expect kLotManager[] = {
    {3, 0, false, "Init"}, {4, 0, false, "Shutdown"}, {5, 1, false, "GetLotAtPoint"}, {6, 3, false, "GetLot"},
    {7, 6, false, "GetLotsInCellRect"}, {8, 4, false, "GetLotsSurroundingCell"},
    {9, 3, false, "GetLotsSurroundingLot"}, {10, 3, false, "GetLotsSurroundingRect"},
    {11, 3, false, "GetVerticesAtEdgeOfLot"}, {12, 3, false, "GetNeighborCellsAtEdgeOfLot"},
    {13, 0, false, "GetRandomLot"}, {14, 1, false, "GetOccupantLot"}, {15, 0, false, "GetSafeLotIterator"},
    {16, 0, false, "GetLotCount"}, {17, 4, true, "GetLotSavvyTerrainVertexAltitude"},
    {18, 3, true, "GetLotSavvyTerrainAltitude"}, {19, 3, true, "GetLotSavvyCellAltitude"},
    {20, 7, false, "CanCreateLot"}, {21, 6, false, "CanCreatePloppedLot"}, {22, 9, false, "CreateLotIfPossible"},
    {23, 6, false, "CreateLot"}, {24, 1, false, "DeleteLot"}, {25, 4, false, "DeleteLots"},
    {26, 4, false, "GetLotVertexAltitudeAdjustmentRange"}, {27, 0, false, "GetMaximumCapacityGrid"},
};

struct LotProbe final : cFC4ISC4Lot {
    bool QueryInterface(uint32_t, void**) override { return false; }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    cISCPropertyHolder* AsPropertyHolder() override { g_hit = 3; return static_cast<cISCPropertyHolder*>(0); }
    cISC4TrafficSource* AsTrafficSource() override { g_hit = 4; return static_cast<cISC4TrafficSource*>(0); }
    bool Init() override { g_hit = 5; return static_cast<bool>(0); }
    bool IsInitialized() const override { g_hit = 6; return static_cast<bool>(0); }
    bool Shutdown() override { g_hit = 7; return static_cast<bool>(0); }
    bool PostCityInit() override { g_hit = 8; return static_cast<bool>(0); }
    cISC4LotConfiguration* GetLotConfiguration() override { g_hit = 9; return static_cast<cISC4LotConfiguration*>(0); }
    bool SetLotConfiguration(cISC4LotConfiguration* config) override { g_hit = 10; return static_cast<bool>(0); }
    cSC4LotConfigurationObject* GetLotConfigurationObject(cISC4Occupant* occupant) override { g_hit = 11; return static_cast<cSC4LotConfigurationObject*>(0); }
    uint32_t GetFlags() const override { g_hit = 12; return static_cast<uint32_t>(0); }
    bool SetFlags(uint32_t flags) override { g_hit = 13; return static_cast<bool>(0); }
    bool GetBoundingRect(SC4Rect<int32_t>& rect) const override { g_hit = 14; return static_cast<bool>(0); }
    void GetCenter(float* center) const override { g_hit = 15; }
    float GetAltitude() const override { g_hit = 16; return static_cast<float>(0); }
    bool SetAltitude(float altitude) override { g_hit = 17; return static_cast<bool>(0); }
    float GetSlope() const override { g_hit = 18; return static_cast<float>(0); }
    float GetSlopeAtTimeOfConstruction() const override { g_hit = 19; return static_cast<float>(0); }
    bool UpdateSlope() override { g_hit = 20; return static_cast<bool>(0); }
    bool GetLocation(int32_t& x, int32_t& z) const override { g_hit = 21; return static_cast<bool>(0); }
    bool GetSize(uint8_t& sizeX, uint8_t& sizeZ) const override { g_hit = 22; return static_cast<bool>(0); }
    int32_t GetFacing() const override { g_hit = 23; return static_cast<int32_t>(0); }
    bool SetFacing(int32_t facing) override { g_hit = 24; return static_cast<bool>(0); }
    cISC4BusinessUnit* GetUnit() override { g_hit = 25; return static_cast<cISC4BusinessUnit*>(0); }
    bool SetUnit(cISC4BusinessUnit* unit) override { g_hit = 26; return static_cast<bool>(0); }
    bool HasJobAccess() const override { g_hit = 27; return static_cast<bool>(0); }
    bool HasRoadAccess() const override { g_hit = 28; return static_cast<bool>(0); }
    bool HasOrganicRoadAccess(uint32_t purposeType) const override { g_hit = 29; return static_cast<bool>(0); }
    bool UpdateRoadAccess() override { g_hit = 30; return static_cast<bool>(0); }
    cISC4ZoneManager::ZoneType GetZoneType() const override { g_hit = 31; return static_cast<cISC4ZoneManager::ZoneType>(0); }
    bool UpdateZoneType() override { g_hit = 32; return static_cast<bool>(0); }
    HabitationState GetState() const override { g_hit = 33; return static_cast<HabitationState>(0); }
    bool SetState(HabitationState state, WealthType wealth, VacateReason reason) override { g_hit = 34; return static_cast<bool>(0); }
    uint32_t GetCondition() const override { g_hit = 35; return static_cast<uint32_t>(0); }
    bool SetCondition(uint32_t condition) override { g_hit = 36; return static_cast<bool>(0); }
    bool IsInLivableState() const override { g_hit = 37; return static_cast<bool>(0); }
    bool HasCapacity() const override { g_hit = 38; return static_cast<bool>(0); }
    bool IsInDemolishableState() const override { g_hit = 39; return static_cast<bool>(0); }
    uint32_t GetDaysSinceLastStateChange() const override { g_hit = 40; return static_cast<uint32_t>(0); }
    uint32_t GetOccupantWealth() const override { g_hit = 41; return static_cast<uint32_t>(0); }
    uint32_t GetLandValue() const override { g_hit = 42; return static_cast<uint32_t>(0); }
    bool IsHistorical() const override { g_hit = 43; return static_cast<bool>(0); }
    bool SetHistorical(bool historical) override { g_hit = 44; return static_cast<bool>(0); }
    VacateReason GetVacateReason() const override { g_hit = 45; return static_cast<VacateReason>(0); }
    uint32_t GetBuildingType(bool forceCalculation) const override { g_hit = 46; return static_cast<uint32_t>(0); }
    bool SetBuildingType(uint32_t buildingType) override { g_hit = 47; return static_cast<bool>(0); }
    cISC4BuildingOccupant* GetBuilding() const override { g_hit = 48; return static_cast<cISC4BuildingOccupant*>(0); }
    void GetLotOccupants(SC4List<cISC4Occupant*>& occupants) const override { g_hit = 49; }
    uint16_t GetCapacityOfDeveloper(DeveloperType type, bool forceCalculation) const override { g_hit = 50; return static_cast<uint16_t>(0); }
    uint16_t GetCapacity(WealthType wealth) const override { g_hit = 51; return static_cast<uint16_t>(0); }
    uint16_t GetAvailableCapacity(DeveloperType type, bool forceCalculation) const override { g_hit = 52; return static_cast<uint16_t>(0); }
    uint16_t GetPopulation(DeveloperType type) const override { g_hit = 53; return static_cast<uint16_t>(0); }
    intptr_t GetPopulations() const override { g_hit = 54; return static_cast<intptr_t>(0); }
    bool SetPopulation(DeveloperType type, uint16_t population) override { g_hit = 55; return static_cast<bool>(0); }
    uint16_t GetTravelPopulation(DeveloperType type) const override { g_hit = 56; return static_cast<uint16_t>(0); }
    bool ResetTravelPopulation() override { g_hit = 57; return static_cast<bool>(0); }
    bool AddTravelPopulation(cISC4Lot* lot) override { g_hit = 58; return static_cast<bool>(0); }
    cISC4Lot* GetTravelDesignate() const override { g_hit = 59; return static_cast<cISC4Lot*>(0); }
    bool SetTravelDesignate(cISC4Lot* designate) override { g_hit = 60; return static_cast<bool>(0); }
    SC4Percentage GetOccupancy() const override { g_hit = 61; return SC4Percentage{0.75f}; }
    bool GetOccupancyByType(std::map<uint32_t, SC4Percentage>& out) const override { g_hit = 62; return static_cast<bool>(0); }
    float GetTravelJobs(WealthType wealth) const override { g_hit = 63; return static_cast<float>(0); }
    bool GetJobs(float jobs[]) const override { g_hit = 64; return static_cast<bool>(0); }
    bool GetNeedsPower() const override { g_hit = 65; return static_cast<bool>(0); }
    bool GetHasPower() const override { g_hit = 66; return static_cast<bool>(0); }
    bool SetHasPower(bool hasPower) override { g_hit = 67; return static_cast<bool>(0); }
    bool GetVerticesAreLocked() const override { g_hit = 68; return static_cast<bool>(0); }
    bool SetVerticesAreLocked(bool locked) override { g_hit = 69; return static_cast<bool>(0); }
};

const Expect kLot[] = {
    {3, 0, false, "AsPropertyHolder"}, {4, 0, false, "AsTrafficSource"}, {5, 0, false, "Init"},
    {6, 0, false, "IsInitialized"}, {7, 0, false, "Shutdown"}, {8, 0, false, "PostCityInit"},
    {9, 0, false, "GetLotConfiguration"}, {10, 1, false, "SetLotConfiguration"},
    {11, 1, false, "GetLotConfigurationObject"}, {12, 0, false, "GetFlags"}, {13, 1, false, "SetFlags"},
    {14, 1, false, "GetBoundingRect"}, {15, 1, false, "GetCenter"}, {16, 0, true, "GetAltitude"},
    {17, 1, false, "SetAltitude"}, {18, 0, true, "GetSlope"}, {19, 0, true, "GetSlopeAtTimeOfConstruction"},
    {20, 0, false, "UpdateSlope"}, {21, 2, false, "GetLocation"}, {22, 2, false, "GetSize"},
    {23, 0, false, "GetFacing"}, {24, 1, false, "SetFacing"}, {25, 0, false, "GetUnit"}, {26, 1, false, "SetUnit"},
    {27, 0, false, "HasJobAccess"}, {28, 0, false, "HasRoadAccess"}, {29, 1, false, "HasOrganicRoadAccess"},
    {30, 0, false, "UpdateRoadAccess"}, {31, 0, false, "GetZoneType"}, {32, 0, false, "UpdateZoneType"},
    {33, 0, false, "GetState"}, {34, 3, false, "SetState"}, {35, 0, false, "GetCondition"},
    {36, 1, false, "SetCondition"}, {37, 0, false, "IsInLivableState"}, {38, 0, false, "HasCapacity"},
    {39, 0, false, "IsInDemolishableState"}, {40, 0, false, "GetDaysSinceLastStateChange"},
    {41, 0, false, "GetOccupantWealth"}, {42, 0, false, "GetLandValue"}, {43, 0, false, "IsHistorical"},
    {44, 1, false, "SetHistorical"}, {45, 0, false, "GetVacateReason"}, {46, 1, false, "GetBuildingType"},
    {47, 1, false, "SetBuildingType"}, {48, 0, false, "GetBuilding"}, {49, 1, false, "GetLotOccupants"},
    {50, 2, false, "GetCapacityOfDeveloper"}, {51, 1, false, "GetCapacity"}, {52, 2, false, "GetAvailableCapacity"},
    {53, 1, false, "GetPopulation"}, {54, 0, false, "GetPopulations"}, {55, 2, false, "SetPopulation"},
    {56, 1, false, "GetTravelPopulation"}, {57, 0, false, "ResetTravelPopulation"},
    {58, 1, false, "AddTravelPopulation"}, {59, 0, false, "GetTravelDesignate"},
    {60, 1, false, "SetTravelDesignate"}, {62, 1, false, "GetOccupancyByType"}, {63, 1, true, "GetTravelJobs"},
    {64, 1, false, "GetJobs"}, {65, 0, false, "GetNeedsPower"}, {66, 0, false, "GetHasPower"},
    {67, 1, false, "SetHasPower"}, {68, 0, false, "GetVerticesAreLocked"}, {69, 1, false, "SetVerticesAreLocked"},
};

}  // namespace

// ---- occupant manager (occupant-manager.md §11) ----------------------------------------------------
//
// 50 slots; slot 49 is the deleting destructor (not declared). The load-bearing
// part is the overload order: 28/29 .. 44/45 are filter-first, but 46/47 are
// TYPED-first (§9.4) - the only pair the game declares the other way round.
// If either pair were laid out from the SDK's declaration order, the names here
// would not run their own slot.

#define PB(N, NAME, ...) bool NAME(__VA_ARGS__) override { g_hit = N; return false; }
#define PI(N, NAME, ...) int32_t NAME(__VA_ARGS__) override { g_hit = N; return 0; }

struct OccMgrProbe final : cFC4ISC4OccupantManager {
    bool QueryInterface(uint32_t, void**) override { return false; }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    PB(3, Init, )
    PB(4, Shutdown, )
    PB(5, SetCitySize, float, float)
    PB(6, GetOccupantManagerCellSizes, float&, float&)
    PB(7, GetWorldCellCount, int32_t&, int32_t&)
    PB(8, GetOccupantManagerCellCount, int32_t&, int32_t&)
    PB(9, GetOccupantManagerCellBounds, int32_t&, int32_t&, int32_t&, int32_t&)
    PB(10, WorldCellToOccupantManagerCell, int32_t, int32_t, int32_t&, int32_t&)
    PB(11, OccupantManagerCellToWorldCell, int32_t, int32_t, int32_t&, int32_t&)
    PB(12, WorldCellToStandardCityCell, int32_t, int32_t, int32_t&, int32_t&)
    PB(13, StandardCityCellToWorldCell, int32_t, int32_t, int32_t&, int32_t&)
    PB(14, WorldCellToPosition, int32_t, int32_t, float&, float&)
    PB(15, PositionToWorldCell, float, float, int32_t&, int32_t&)
    PB(16, OccupantManagerCellToPosition, int32_t, int32_t, float&, float&)
    PB(17, PositionToOccupantManagerCell, float, float, int32_t&, int32_t&)
    PB(18, StandardCityCellToPosition, int32_t, int32_t, float&, float&)
    PB(19, PositionToStandardCityCell, float, float, int32_t&, int32_t&)
    PB(20, InsertOccupant, cISC4Occupant*, uint32_t)
    PB(21, RemoveOccupant, cISC4Occupant*, bool, uint32_t)
    PB(22, RemoveOccupants, cISC4OccupantFilter*, bool, uint32_t)
    PB(23, RemoveOccupantsOfType, uint32_t, bool, uint32_t)
    PB(24, MoveOccupant, cISC4Occupant*, bool)
    PB(25, IsCellEmpty, int32_t, int32_t)
    PI(26, GetBoundingCells, int32_t, int32_t, int32_t (*)[2], int32_t&)
    PB(27, FindOccupant, cISC4Occupant*, bool, int32_t*, int32_t*)
    PB(28, GetFirstOccupantByPositionFilter, cISC4Occupant*&, float, float, cISC4OccupantFilter*)
    PB(29, GetFirstOccupantByPositionTyped, cISC4Occupant*&, float, float, uint32_t)
    PB(30, GetFirstOccupantFilter, cISC4Occupant*&, int32_t, int32_t, cISC4OccupantFilter*)
    PB(31, GetFirstOccupantTyped, cISC4Occupant*&, int32_t, int32_t, uint32_t)
    PB(32, GetFirstOccupantByStandardCityCellFilter, cISC4Occupant*&, int32_t, int32_t, cISC4OccupantFilter*)
    PB(33, GetFirstOccupantByStandardCityCellTyped, cISC4Occupant*&, int32_t, int32_t, uint32_t)
    PB(34, GetFirstOccupantByStandardCityCellsFilter, cISC4Occupant*&, SC4Point<int> const*, SC4Point<int> const*,
        cISC4OccupantFilter*)
    PB(35, GetFirstOccupantByStandardCityCellsTyped, cISC4Occupant*&, SC4Point<int> const*, SC4Point<int> const*,
        uint32_t)
    PB(36, GetOccupantsByBBoxFilter, SC4List<cISC4Occupant*>&, float const*, float const*, cISC4OccupantFilter*,
        uint32_t)
    PB(37, GetOccupantsByBBoxTyped, SC4List<cISC4Occupant*>&, float const*, float const*, uint32_t, uint32_t)
    PB(38, GetOccupantsByOccupantManagerCellsFilter, SC4List<cISC4Occupant*>&, int32_t const*, int32_t const*,
        cISC4OccupantFilter*, uint32_t)
    PB(39, GetOccupantsByOccupantManagerCellsTyped, SC4List<cISC4Occupant*>&, int32_t const*, int32_t const*,
        uint32_t, uint32_t)
    PB(40, GetOccupantsByStandardCityCellsFilter, SC4List<cISC4Occupant*>&, SC4Point<int> const*,
        SC4Point<int> const*, cISC4OccupantFilter*, uint32_t)
    PB(41, GetOccupantsByStandardCityCellsTyped, SC4List<cISC4Occupant*>&, SC4Point<int> const*,
        SC4Point<int> const*, uint32_t, uint32_t)
    PB(42, IterateOccupantsByBBoxFilter, bool (*)(cISC4Occupant*, void*), void*, float const*, float const*,
        cISC4OccupantFilter*)
    PB(43, IterateOccupantsByBBoxTyped, bool (*)(cISC4Occupant*, void*), void*, float const*, float const*, uint32_t)
    PB(44, IterateOccupantsFilter, bool (*)(cISC4Occupant*, void*), void*, int32_t const*, int32_t const*,
        cISC4OccupantFilter*)
    PB(45, IterateOccupantsTyped, bool (*)(cISC4Occupant*, void*), void*, int32_t const*, int32_t const*, uint32_t)
    PB(46, IterateOccupantsByStandardCityCellTyped, bool (*)(cISC4Occupant*, void*), void*, int32_t const*,
        int32_t const*, uint32_t)
    PB(47, IterateOccupantsByStandardCityCellFilter, bool (*)(cISC4Occupant*, void*), void*, int32_t const*,
        int32_t const*, cISC4OccupantFilter*)
    PB(48, ReleaseOccupantList, SC4List<cISC4Occupant*>&)
};

const Expect kOccMgr[] = {
    {3, 0, false, "Init"},
    {4, 0, false, "Shutdown"},
    {5, 2, false, "SetCitySize"},
    {6, 2, false, "GetOccupantManagerCellSizes"},
    {7, 2, false, "GetWorldCellCount"},
    {8, 2, false, "GetOccupantManagerCellCount"},
    {9, 4, false, "GetOccupantManagerCellBounds"},
    {10, 4, false, "WorldCellToOccupantManagerCell"},
    {11, 4, false, "OccupantManagerCellToWorldCell"},
    {12, 4, false, "WorldCellToStandardCityCell (bug)"},
    {13, 4, false, "StandardCityCellToWorldCell"},
    {14, 4, false, "WorldCellToPosition"},
    {15, 4, false, "PositionToWorldCell"},
    {16, 4, false, "OccupantManagerCellToPosition"},
    {17, 4, false, "PositionToOccupantManagerCell"},
    {18, 4, false, "StandardCityCellToPosition"},
    {19, 4, false, "PositionToStandardCityCell"},
    {20, 2, false, "InsertOccupant"},
    {21, 3, false, "RemoveOccupant"},
    {22, 3, false, "RemoveOccupants(filter)"},
    {23, 3, false, "RemoveOccupants(type)"},
    {24, 2, false, "MoveOccupant"},
    {25, 2, false, "IsCellEmpty"},
    {26, 4, false, "GetBoundingCells"},
    {27, 4, false, "FindOccupant"},
    {28, 4, false, "GetFirstOccupantByPosition(filter)"},
    {29, 4, false, "GetFirstOccupantByPosition(type)"},
    {30, 4, false, "GetFirstOccupant(filter)"},
    {31, 4, false, "GetFirstOccupant(type)"},
    {32, 4, false, "GetFirstOccupantByStandardCityCell(filter)"},
    {33, 4, false, "GetFirstOccupantByStandardCityCell(type)"},
    {34, 4, false, "GetFirstOccupantByStandardCityCells(filter)"},
    {35, 4, false, "GetFirstOccupantByStandardCityCells(type)"},
    {36, 5, false, "GetOccupantsByBBox(filter)"},
    {37, 5, false, "GetOccupantsByBBox(type)"},
    {38, 5, false, "GetOccupantsByOccupantManagerCells(filter)"},
    {39, 5, false, "GetOccupantsByOccupantManagerCells(type)"},
    {40, 5, false, "GetOccupantsByStandardCityCells(filter)"},
    {41, 5, false, "GetOccupantsByStandardCityCells(type)"},
    {42, 5, false, "IterateOccupantsByBBox(filter)"},
    {43, 5, false, "IterateOccupantsByBBox(type)"},
    {44, 5, false, "IterateOccupants(filter)"},
    {45, 5, false, "IterateOccupants(type)"},
    {46, 5, false, "IterateOccupantsByStandardCityCell(type first)"},
    {47, 5, false, "IterateOccupantsByStandardCityCell(filter)"},
    {48, 1, false, "ReleaseOccupantList"},
};

#undef PB
#undef PI

int main() {
    SimProbe sim;
    BudgetProbe budget;
    DepartmentProbe dept;
    LineProbe line;
    Check("cISC4Simulator", static_cast<cFC4ISC4Simulator*>(&sim), kSimulator);
    Check("cISC4BudgetSimulator", static_cast<cFC4ISC4BudgetSimulator*>(&budget), kBudget);
    Check("cISC4DepartmentBudget", static_cast<cFC4ISC4DepartmentBudget*>(&dept), kDepartment);
    {
        // Slot 26 returns the pointer in EAX, not a float in ST0 (budget erratum).
        const fc4test::SlotCall r = fc4test::CallSlot(static_cast<cFC4ISC4DepartmentBudget*>(&dept), 26, {0, 0});
        for (int i = 0; i < r.x87Pushed; ++i) fc4test::PopX87();
        if (r.eax != reinterpret_cast<uint32_t>(&kProbePct) || r.x87Pushed != 0) {
            std::fprintf(stderr, "cISC4DepartmentBudget slot 26: EAX %08X (want %p), %d x87 values\n", r.eax,
                         static_cast<const void*>(&kProbePct), r.x87Pushed);
            ++g_failures;
        }
    }
    static_assert(sizeof(SC4Percentage) == sizeof(float), "slot 26 points to a 32-bit float");

    DateProbe date;
    HolderProbe holder;
    LotManagerProbe lotManager;
    LotProbe lot;
    Check("cIGZDate", static_cast<cFC4IGZDate*>(&date), kDate);
    Check("cISCPropertyHolder", static_cast<cFC4ISCPropertyHolder*>(&holder), kHolder);
    Check("cISC4LotManager", static_cast<cFC4ISC4LotManager*>(&lotManager), kLotManager);
    Check("cISC4Lot", static_cast<cFC4ISC4Lot*>(&lot), kLot);
    {
        // cISC4Lot slot 61 returns SC4Percentage BY VALUE: the caller passes a
        // hidden result pointer (one stack dword), the callee writes the float
        // there, returns the pointer in EAX and leaves nothing on the x87 stack.
        float result = -1.0f;
        g_hit = -1;
        const fc4test::SlotCall r =
            fc4test::CallSlot(static_cast<cFC4ISC4Lot*>(&lot), 61, {reinterpret_cast<uint32_t>(&result)});
        for (int i = 0; i < r.x87Pushed; ++i) fc4test::PopX87();
        if (g_hit != 61 || r.espDelta != 0 || r.x87Pushed != 0 || r.eax != reinterpret_cast<uint32_t>(&result) ||
            result != 0.75f) {
            std::fprintf(stderr, "cISC4Lot slot 61 (GetOccupancy): ran %d, stack off by %d, %d x87, EAX %08X, %f\n",
                         g_hit, r.espDelta, r.x87Pushed, r.eax, result);
            ++g_failures;
        }
    }
    Check("cISC4LineItem", static_cast<cISC4LineItem*>(&line), kLineItem);
    OccMgrProbe occMgr;
    Check("cISC4OccupantManager", static_cast<cFC4ISC4OccupantManager*>(&occMgr), kOccMgr);
    if (g_failures == 0) std::printf("vtable layout test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}
