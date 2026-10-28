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

// Game interfaces with overloaded methods, declared in the game's verified
// slot order (foundations §16, simulator §5.6, budget "Verified slot order").
//
// Rule: these declarations contain NO overloads. MSVC lays out overloaded
// virtual functions in its own order (it groups them and reverses each
// group), so with overloads the slot order depends on the compiler rather
// than on the declaration. Every method here has a unique name and the vtable
// follows declaration order exactly: slot N is the N-th virtual function,
// counting QueryInterface, AddRef and Release as slots 0-2.
// tests/vtable_layout_test.cpp checks every slot against the spec tables.
//
// Use these types - never the SDK's cISC4Simulator / cISC4BudgetSimulator /
// cISC4DepartmentBudget / cIGZDate / cISCPropertyHolder / cISC4LotManager /
// cISC4Lot / cIGZVariant / cISC4NeighborsSimulator / cISC4Ordinance /
// cISC4Demand / cISC4DemandSimulator / cISC4ResidentialSimulator /
// cISC4RegionalCity / cISC4PowerSimulator / cISC4NetworkTool - to call the
// game's objects or to implement ours.
// tests/abi_conformance_test.cpp checks every declaration against
// docs/abi/interfaces.tsv (callee-pop bytes and return channel per slot).
// Besides overload order, they also fix return types the SDK gets wrong
// (cISC4DepartmentBudget slot 26).
#pragma once

#include "SC4Percentage.h"
#include "SC4Point.h"   // glue29: cFC4ISC4NetworkToolFull usa SC4Point<uint32_t>
#include "SC4Vector.h"  // glue29: idem, SC4Vector<SC4Point<uint32_t>>
#include "cIGZUnknown.h"
#include "SC4Rect.h"
#include "cISC4BudgetSimulator.h"
#include "cISC4BuildingDevelopmentSimulator.h"
#include "cISC4BuildingOccupant.h"
#include "cISC4City.h"
#include "cISC4Lot.h"
#include "cISC4NeighborsSimulator.h"
#include "cISC4SimGrid.h"
#include "cISC4ZoneManager.h"
#include "FC4GameList.h"
// glue29: cFC4ISC4NetworkToolFull usa cISC4NetworkOccupant::eNetworkType
#include "cISC4NetworkOccupant.h"

#include <cstdint>
#include <map>

class cFC4IGZDate;
class cIGZDate;
class cISC4LotManager;
class cIGZMessageTarget2;
class cIGZString;
class cIGZVariant;
class cISC4DepartmentBudget;
class cISC4LineItem;
class cISCPropertyHolder;
class SC4String;
class cISC4Demand;
class cISC4DemandSimulator;
class cISC4NeighborDeal;
class SC4NewCityPreferences;
template <typename T> class SC4List;
template <typename T> class SC4Point;
template <typename T> class SC4Vector;
template <typename T> class cISC4SimGrid;
class cISC4OccupantManager;

// cISC4Simulator (IID 0x8695664E), 34 slots.
class cFC4ISC4Simulator : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                  // 3
    virtual bool Shutdown() = 0;                              // 4
    virtual bool GetSimStartDate(cFC4IGZDate& date) = 0;      // 5
    virtual void GetSimDateParts(uint32_t* year, uint32_t* month, uint32_t* day, uint32_t* dayOfYear,
                                 uint32_t* weekDay) = 0;      // 6  GetSimDate(5 pointers)
    virtual cFC4IGZDate* GetSimDate() = 0;                    // 7  GetSimDate()
    virtual int32_t GetSimDateNumber() = 0;                   // 8
    virtual bool Pause() = 0;                                 // 9
    virtual bool HiddenPause() = 0;                           // 10
    virtual bool EmergencyPause() = 0;                        // 11
    virtual bool Resume() = 0;                                // 12
    virtual bool HiddenResume() = 0;                          // 13
    virtual bool EmergencyResume() = 0;                       // 14
    virtual bool IsPaused() = 0;                              // 15
    virtual bool IsHiddenPaused() = 0;                        // 16
    virtual bool IsEmergencyPaused() = 0;                     // 17
    virtual bool IsAnyPaused() = 0;                           // 18
    virtual bool AddAgent(cIGZMessageTarget2* agent, uint32_t level, cIGZString const& name,
                          uint32_t flags) = 0;                // 19
    virtual bool RemoveAgentAllLevels(cIGZMessageTarget2* agent) = 0;          // 20  RemoveAgent(agent)
    virtual bool RemoveAgent(cIGZMessageTarget2* agent, uint32_t level) = 0;   // 21  RemoveAgent(agent, level)
    virtual bool RemoveAllAgentsAt(uint32_t level) = 0;       // 22  RemoveAllAgents(level)
    virtual bool RemoveAllAgents() = 0;                       // 23  RemoveAllAgents()
    // [abi] std::vector<cRZString>: buffer and strings are B (allocators.md).
    virtual bool EnumerateAgentsByName(fc4::GameVector<fc4::GameString>& names) = 0;  // 24
    virtual bool GetAgentEnabled(cIGZString const& name) = 0;  // 25
    virtual bool SetAgentEnabled(cIGZString const& name, bool enabled) = 0;  // 26
    virtual int32_t GetSimSpeed() = 0;                        // 27
    virtual bool SetSimSpeed(int32_t speed) = 0;              // 28
    virtual int32_t GetSimTime() = 0;                         // 29
    virtual bool SetSimTime(int32_t time) = 0;                // 30
    virtual bool SetMaxMillisecondsPerTick(uint32_t ms) = 0;  // 31
    virtual float GetAnimationTimeDilation() = 0;             // 32
    virtual bool SetCityEstablished(bool established) = 0;    // 33
};

// cISC4BudgetSimulator (IID 0x6772BD23), 59 slots. GetTotalMonthlyExpense
// and GetTotalMonthlyIncome return 64-bit values (budget §6 [compat]): in
// EDX:EAX, so callers using a 32-bit declaration still see the low 32 bits.
class cFC4ISC4BudgetSimulator : public cIGZUnknown {
public:
    using BudgetGroupInfo = cISC4BudgetSimulator::BudgetGroupInfo;
    using BudgetItem = cISC4BudgetSimulator::BudgetItem;
    using LoanInfo = cISC4BudgetSimulator::LoanInfo;

    virtual bool Init() = 0;                                  // 3
    virtual bool Shutdown() = 0;                              // 4
    virtual bool SetTotalFunds(int64_t funds) = 0;            // 5
    virtual int64_t GetTotalFunds() = 0;                      // 6
    virtual bool DepositFunds(int64_t funds) = 0;             // 7
    virtual bool WithdrawFunds(int64_t funds) = 0;            // 8
    virtual int64_t GetMinAllowableFunds() = 0;               // 9
    virtual int64_t GetYTDIncome() = 0;                       // 10
    virtual int64_t GetEstIncome() = 0;                       // 11
    virtual int64_t GetYTDExpenses() = 0;                     // 12
    virtual int64_t GetEstExpenses() = 0;                     // 13
    virtual int64_t GetTotalMonthlyExpense() = 0;             // 14  (SDK: uint32_t)
    virtual int64_t GetTotalYearlyExpense() = 0;              // 15
    virtual int64_t GetTotalMonthlyIncome() = 0;              // 16  (SDK: int32_t)
    virtual int64_t GetTotalYearlyIncome() = 0;               // 17
    virtual int64_t GetTaxIncomeOfType(int32_t taxType) = 0;  // 18  GetTaxIncome(int32)
    virtual int64_t GetTaxIncome() = 0;                       // 19  GetTaxIncome()
    // 20-23: the SDK returns int64; docs/abi says the game's methods return 32
    // bits in eax. [choice] ours return the low 32 bits of the stored value
    // (budget spec: "return v"), which is the eax an int64 return gave too.
    virtual int32_t SetTotalMonthlyExpense(int64_t v) = 0;    // 20
    virtual int32_t SetTotalYearlyExpense(int64_t v) = 0;     // 21
    virtual int32_t SetTotalMonthlyIncome(int64_t v) = 0;     // 22
    virtual int32_t SetTotalYearlyIncome(int64_t v) = 0;      // 23
    virtual bool ShowBudgetWindow() = 0;                      // 24
    virtual cISC4DepartmentBudget* CreateDepartmentBudget(uint32_t id, uint32_t group) = 0;  // 25
    virtual bool RemoveDepartmentBudget(uint32_t id) = 0;     // 26
    virtual cISC4DepartmentBudget* GetDepartmentBudgetByName(cIGZString const& name) = 0;  // 27
    virtual cISC4DepartmentBudget* GetDepartmentBudget(uint32_t id) = 0;  // 28
    // [abi] 29, 31, 46, 50: std::vector with the default allocator (B); the
    // group names are cRZStrings with B buffers (allocators.md, table (2)).
    virtual bool GetAllGroups(fc4::GameVector<fc4::GameBudgetGroupInfo>& groups) = 0;  // 29
    virtual bool SetGroupName(uint32_t id, cIGZString& name) = 0;         // 30
    virtual bool GetDepartmentBudgetsInGroup(uint32_t id, fc4::GameVector<cISC4DepartmentBudget*>& out) = 0;  // 31
    virtual void NeededFundingChanged(cISC4DepartmentBudget* dept) = 0;      // 32
    virtual void FundingPercentageChanged(cISC4DepartmentBudget* dept) = 0;  // 33
    virtual int64_t GetFundingById(uint32_t id) = 0;          // 34  GetFunding(uint32) (same body as 35)
    virtual int64_t GetFundingOf(cISC4DepartmentBudget* dept) = 0;  // 35  GetFunding(dept)
    virtual float GetTaxRate(uint32_t group) = 0;             // 36
    virtual bool SetTaxRate(uint32_t group, float rate) = 0;  // 37
    virtual int64_t GetWeightedAssessedTaxValue(uint32_t group) = 0;  // 38
    virtual int64_t GetBondIncrement() = 0;                   // 39
    virtual bool IssueBond(uint32_t amount) = 0;              // 40
    virtual int64_t GetTotalBorrowed() = 0;                   // 41
    virtual int64_t GetCurrentBorrowingLimit() = 0;           // 42
    virtual int64_t GetCurrentBondLimit() = 0;                // 43
    virtual int64_t GetCurrentMaxOutstandingBondsLimit() = 0;  // 44
    virtual int64_t GetTotalMonthlyBondPayments() = 0;        // 45
    virtual bool GetAllLoans(fc4::GameVector<LoanInfo>& loans) = 0;  // 46
    virtual int32_t GetLoanTimeInMonths() = 0;                // 47
    virtual int64_t GetMonthlyPaymentForLoan(int64_t loan) = 0;  // 48
    virtual int64_t GetFullCostOfLoan(int64_t loan) = 0;      // 49
    virtual bool GetBudgetItemInfo(cISCPropertyHolder* holder, fc4::GameVector<BudgetItem>& out) = 0;  // 50
    virtual bool GetBudgetItemForPurpose(cISCPropertyHolder* holder, uint32_t purpose, BudgetItem& item) = 0;  // 51
    virtual bool ChangeBudgetItemLine(cISCPropertyHolder* holder, uint32_t index, uint32_t line) = 0;  // 52
    virtual bool ChangeBudgetItemCost(cISCPropertyHolder* holder, uint32_t index, int64_t cost) = 0;  // 53
    virtual bool ChangeBudgetItemLocalFunding(cISCPropertyHolder* holder, uint32_t index,
                                              SC4Percentage const& funding) = 0;  // 54
    virtual bool AddBudgetItemToBudget(cISCPropertyHolder* holder, uint32_t index) = 0;  // 55
    virtual bool RemoveBudgetItemFromBudget(cISCPropertyHolder* holder, uint32_t index) = 0;  // 56
    virtual cISC4LineItem* GetLineItemFromBudgetItem(BudgetItem& item) = 0;  // 57
    virtual bool CopyBudgetItemProperties(cISCPropertyHolder* src, cISCPropertyHolder* dst) = 0;  // 58
};

// cISC4DepartmentBudget (IID 0x223E2B67), 34 slots. No overloads; this
// declaration exists because the SDK declares slot 26 with the wrong return
// type (budget "Erratum: GetLocalFundingPercent returns a pointer").
class cFC4ISC4DepartmentBudget : public cIGZUnknown {
public:
    virtual uint32_t GetDepartmentID() const = 0;             // 3
    virtual bool GetDepartmentName(cIGZString& name) = 0;     // 4
    virtual bool SetDepartmentName(uint32_t ltextGroupID, uint32_t ltextInstanceID) = 0;  // 5
    virtual uint32_t GetBudgetGroup() const = 0;              // 6
    virtual bool SetBudgetGroup(uint32_t budgetGroup) = 0;    // 7
    virtual bool GetIsFixedFunding() const = 0;               // 8
    virtual bool SetFixedFunding(bool value) = 0;             // 9
    virtual SC4Percentage* GetFundingPercentage() const = 0;  // 10
    virtual bool SetFundingPercentage(SC4Percentage const& percentage, uint32_t lineItem) = 0;  // 11
    virtual SC4Percentage* GetMaxAllowedFundingPercentage() const = 0;  // 12
    virtual bool SetMaxAllowedFundingPercentage(SC4Percentage const& percentage) = 0;  // 13
    virtual int64_t GetIdealMonthlyFunding() const = 0;       // 14
    virtual int64_t GetTotalExpenses() const = 0;             // 15
    virtual int64_t GetTotalIncome() const = 0;               // 16
    virtual cISC4LineItem* CreateLineItem(uint32_t lineNumber, bool isLocallyFunded) = 0;  // 17
    virtual cISC4LineItem* CreateLineItemForBuildingType(uint32_t buildingIID, bool isLocallyFunded) = 0;  // 18
    virtual bool RemoveLineItem(uint32_t lineNumber) = 0;     // 19
    virtual cISC4LineItem* GetLineItem(uint32_t lineNumber) = 0;  // 20
    // [abi] 21, 32: std::vector / std::list with the default allocator (B).
    virtual bool GetAllLineItems(fc4::GameVector<cISC4LineItem*>& destination) = 0;  // 21
    virtual bool AddLocallyFundedObject(cISCPropertyHolder* holder, uint32_t purpose) = 0;  // 22
    virtual bool RemoveLocallyFundedObject(cISCPropertyHolder* holder, uint32_t purpose) = 0;  // 23
    virtual bool SetLocalFundingPercent(cISCPropertyHolder* holder, SC4Percentage* percentage,
                                        uint32_t purpose) = 0;  // 24
    virtual bool SetLocalFullFunding(cISCPropertyHolder* holder, int64_t cost, uint32_t purpose) = 0;  // 25
    // 26: returns a POINTER in EAX (SDK: float). Never null.
    virtual SC4Percentage const* GetLocalFundingPercentPtr(cISCPropertyHolder* holder, uint32_t purpose) = 0;
    virtual int64_t GetLocalFullFunding(cISCPropertyHolder* holder, uint32_t purpose) = 0;  // 27
    virtual bool IsLocallyFundedObjectInDepartment(cISCPropertyHolder* holder, uint32_t purpose) = 0;  // 28
    virtual bool RecalculateAllLineItemCurrentExpenses() = 0;  // 29
    virtual int64_t GetTotalSpending() const = 0;             // 30
    virtual bool SetTotalSpending(int64_t value) = 0;         // 31
    virtual bool GetLocallyFundedItemsByPurpose(uint32_t purpose, fc4::GameList<cISCPropertyHolder*>& list) = 0;  // 32
    virtual bool HasLocallyFundedItemsByPurpose(uint32_t purpose) = 0;  // 33
};

// ---- foundations §17: framework interfaces we implement or call ------------------------------------

class cFC4ISCPropertyHolder;
class cIGZUnknownList;
class cIGZVariant;
class cISCProperty;
class cRZTime;
class cS3DVector3;
class cISC4BusinessUnit;
class cISC4LotConfiguration;
class cISC4Occupant;
class cISC4TrafficSource;
class cSC4LotConfigurationObject;
template <typename T> class cISC4SimGrid;

// cIGZDate (cGZDate vtable, 46 slots; foundations §17.4). Implemented by
// cFC4Date. SDK error: slots 33-36 (operator- / operator+) return a uint32
// day number and do not modify the date.
class cFC4IGZDate : public cIGZUnknown {
public:
    // dates.md §3.2: full EAX, even when the SDK declares bool.
    virtual uint32_t SetMonthNameDayYear(char const* monthName, uint32_t day, uint32_t year) = 0;  // 3
    virtual uint32_t SetDayNumber(uint32_t dayNumber) = 0;                     // 4
    virtual uint32_t SetTime(cRZTime const& time) = 0;                         // 5
    virtual uint32_t SetMonthDayYear(uint32_t month, uint32_t day, uint32_t year) = 0;  // 6
    virtual uint32_t SetDayOfYear(uint32_t day, uint32_t year) = 0;            // 7
    virtual uint32_t Copy(cFC4IGZDate const& from) = 0;                        // 8
    virtual uint32_t Clone(cFC4IGZDate** out) = 0;                             // 9: AddRef = 1
    virtual bool Between(cFC4IGZDate const& start, cFC4IGZDate const& end) = 0;  // 10
    virtual int32_t CompareTo(cFC4IGZDate const& other) = 0;                   // 11
    virtual uint32_t WeekDay() = 0;                                            // 12
    virtual uint32_t Year() = 0;                                               // 13
    virtual uint32_t DayOfYear() = 0;                                          // 14
    virtual uint32_t DayOfMonth() = 0;                                         // 15
    virtual uint32_t FirstDayOfMonthOf(uint32_t month) = 0;                    // 16  FirstDayOfMonth(uint32)
    virtual uint32_t FirstDayOfMonth() = 0;                                    // 17  FirstDayOfMonth()
    virtual uint32_t Hash() = 0;                                               // 18
    virtual bool IsValid() = 0;                                                // 19
    virtual bool Leap() = 0;                                                   // 20
    virtual uint32_t MaxDate(cFC4IGZDate const& other, cFC4IGZDate& out) = 0;  // 21: out.Copy EAX
    virtual uint32_t MinDate(cFC4IGZDate const& other, cFC4IGZDate& out) = 0;  // 22
    virtual uint32_t Month() = 0;                                              // 23
    virtual uint32_t Previous(uint32_t dayOfPriorWeek, cFC4IGZDate& out) = 0;  // 24
    virtual uint32_t YearLastTwoDigits() = 0;                                  // 25
    virtual uint32_t DayNumber() = 0;                                          // 26
    virtual bool IsLess(cFC4IGZDate const& other) = 0;                         // 27  operator<
    virtual bool IsLessOrEqual(cFC4IGZDate const& other) = 0;                  // 28  operator<=
    virtual bool IsGreater(cFC4IGZDate const& other) = 0;                      // 29  operator>
    virtual bool IsGreaterOrEqual(cFC4IGZDate const& other) = 0;               // 30  operator>=
    virtual bool IsEqual(cFC4IGZDate const& other) = 0;                        // 31  operator==
    virtual bool IsNotEqual(cFC4IGZDate const& other) = 0;                     // 32  operator!=
    virtual uint32_t MinusDays(int32_t days) = 0;                              // 33  operator-(int32): DayNumber() - n
    virtual uint32_t MinusDate(cFC4IGZDate const& other) = 0;                  // 34  operator-(date): DayNumber() - other
    virtual uint32_t PlusDays(int32_t days) = 0;                               // 35  operator+(int32): DayNumber() + n
    virtual uint32_t PlusDate(cFC4IGZDate const& other) = 0;                   // 36  operator+(date): DayNumber() + other
    virtual cFC4IGZDate& Increment() = 0;                                      // 37  operator++
    virtual cFC4IGZDate& Decrement() = 0;                                      // 38  operator--
    virtual cFC4IGZDate& AddDays(int32_t days) = 0;                            // 39  operator+=
    virtual cFC4IGZDate& SubtractDays(int32_t days) = 0;                       // 40  operator-=
    virtual uint32_t DateString(cIGZString& out, uint32_t formatID) = 0;       // 41: cleanup depth
    virtual bool DayWithinMonth(uint32_t month, uint32_t day, uint32_t year) = 0;  // 42
    virtual uint32_t DaysInYear(uint32_t year) = 0;                            // 43
    virtual uint32_t Jday(uint32_t month, uint32_t day, uint32_t year) = 0;    // 44
    virtual bool LeapYear(uint32_t year) = 0;                                  // 45
};

// cIGZVariant (IID 0x60FD4D2B, 127 slots; foundations §17.1),
// in compiled order with unique names. SDK errors fixed: slot 8 takes the
// type, slot 60 takes (riid, void**). Overload groups as compiled: the
// value getter comes before GetValXTo(X&), the const AsX() before AsXRef(),
// and the SetRefX(X*, count) setter before the RefX() getter.
class cFC4IGZVariant : public cIGZUnknown {
public:
    virtual uint16_t GetType() const = 0;                                                  // 3
    virtual uint32_t GetCount() const = 0;                                                 // 4
    virtual bool Erase() = 0;                                                              // 5
    virtual bool CopyFrom(cFC4IGZVariant const& value) = 0;                                // 6
    virtual bool CreateCopy(cFC4IGZVariant** value) const = 0;                             // 7
    virtual bool CanGetVal(uint16_t type) const = 0;                                       // 8
    virtual bool GetValBool() const = 0;                                                   // 9
    virtual bool GetValBoolTo(bool& value) const = 0;                                      // 10
    virtual void SetValBool(bool value) = 0;                                               // 11
    virtual uint8_t GetValUint8() const = 0;                                               // 12
    virtual bool GetValUint8To(uint8_t& value) const = 0;                                  // 13
    virtual void SetValUint8(uint8_t value) = 0;                                           // 14
    virtual int8_t GetValSint8() const = 0;                                                // 15
    virtual bool GetValSint8To(int8_t& value) const = 0;                                   // 16
    virtual void SetValSint8(int8_t value) = 0;                                            // 17
    virtual uint16_t GetValUint16() const = 0;                                             // 18
    virtual bool GetValUint16To(uint16_t& value) const = 0;                                // 19
    virtual void SetValUint16(uint16_t value) = 0;                                         // 20
    virtual int16_t GetValSint16() const = 0;                                              // 21
    virtual bool GetValSint16To(int16_t& value) const = 0;                                 // 22
    virtual void SetValSint16(int16_t value) = 0;                                          // 23
    virtual uint32_t GetValUint32() const = 0;                                             // 24
    virtual bool GetValUint32To(uint32_t& value) const = 0;                                // 25
    virtual void SetValUint32(uint32_t value) = 0;                                         // 26
    virtual int32_t GetValSint32() const = 0;                                              // 27
    virtual bool GetValSint32To(int32_t& value) const = 0;                                 // 28
    virtual void SetValSint32(int32_t value) = 0;                                          // 29
    virtual uint64_t GetValUint64() const = 0;                                             // 30
    virtual bool GetValUint64To(uint64_t& value) const = 0;                                // 31
    virtual void SetValUint64(uint64_t value) = 0;                                         // 32
    virtual int64_t GetValSint64() const = 0;                                              // 33
    virtual bool GetValSint64To(int64_t& value) const = 0;                                 // 34
    virtual void SetValSint64(int64_t value) = 0;                                          // 35
    virtual float GetValFloat32() const = 0;                                               // 36
    virtual bool GetValFloat32To(float& value) const = 0;                                  // 37
    virtual void SetValFloat32(float value) = 0;                                           // 38
    virtual double GetValFloat64() const = 0;                                              // 39
    virtual bool GetValFloat64To(double& value) const = 0;                                 // 40
    // docs/abi (corrected 2026-10-14): the game's SetValFloat64 returns
    // nothing (eax-or-void; it stores and pops). A double declaration here
    // would leave one value on the caller's x87 stack per call.
    virtual void SetValFloat64(double value) = 0;                                          // 41
    virtual char GetValChar() const = 0;                                                   // 42
    virtual bool GetValCharTo(char& value) const = 0;                                      // 43
    virtual void SetValChar(char value) = 0;                                               // 44
    virtual uint16_t GetValRZUnicodeChar() const = 0;                                      // 45
    virtual bool GetValRZUnicodeCharTo(uint16_t& value) const = 0;                         // 46
    virtual void SetValRZUnicodeChar(uint16_t value) = 0;                                  // 47
    virtual char GetValRZChar() const = 0;                                                 // 48
    virtual bool GetValRZCharTo(char& value) const = 0;                                    // 49
    virtual void SetValRZChar(char value) = 0;                                             // 50
    virtual void* GetValVoidPtr() const = 0;                                               // 51
    virtual bool GetValVoidPtrTo(void** value) const = 0;                                  // 52
    virtual void SetValVoidPtr(void* value) = 0;                                           // 53
    virtual bool CreateValString(cIGZString** value) const = 0;                            // 54
    virtual bool GetValString(cIGZString& value) const = 0;                                // 55
    virtual void SetValString(cIGZString const& value) = 0;                                // 56
    virtual bool GetValVoid(void* value, uint32_t& length) const = 0;                      // 57
    virtual void SetValVoid(void* value, uint32_t length) = 0;                             // 58
    virtual cIGZUnknown* GetValUnknown() = 0;                                              // 59
    virtual bool GetValUnknownAs(uint32_t riid, void** ppvObj) = 0;                        // 60
    virtual void SetValUnknown(cIGZUnknown* value) = 0;                                    // 61
    virtual bool AsBool() const = 0;                                                       // 62
    virtual bool& AsBoolRef() = 0;                                                         // 63
    virtual uint8_t AsUint8() const = 0;                                                   // 64
    virtual uint8_t& AsUint8Ref() = 0;                                                     // 65
    virtual int8_t AsSint8() const = 0;                                                    // 66
    virtual int8_t& AsSint8Ref() = 0;                                                      // 67
    virtual uint16_t AsUint16() const = 0;                                                 // 68
    virtual uint16_t& AsUint16Ref() = 0;                                                   // 69
    virtual int16_t AsSint16() const = 0;                                                  // 70
    virtual int16_t& AsSint16Ref() = 0;                                                    // 71
    virtual uint32_t AsUint32() const = 0;                                                 // 72
    virtual uint32_t& AsUint32Ref() = 0;                                                   // 73
    virtual int32_t AsSint32() const = 0;                                                  // 74
    virtual int32_t& AsSint32Ref() = 0;                                                    // 75
    virtual uint64_t AsUint64() const = 0;                                                 // 76
    virtual uint64_t& AsUint64Ref() = 0;                                                   // 77
    virtual int64_t AsSint64() const = 0;                                                  // 78
    virtual int64_t& AsSint64Ref() = 0;                                                    // 79
    virtual float AsFloat32() const = 0;                                                   // 80
    virtual float& AsFloat32Ref() = 0;                                                     // 81
    virtual double AsFloat64() const = 0;                                                  // 82
    virtual double& AsFloat64Ref() = 0;                                                    // 83
    virtual char AsChar() const = 0;                                                       // 84
    virtual char& AsCharRef() = 0;                                                         // 85
    virtual uint16_t AsRZUnicodeChar() const = 0;                                          // 86
    virtual uint16_t& AsRZUnicodeCharRef() = 0;                                            // 87
    virtual char AsRZChar() const = 0;                                                     // 88
    virtual char& AsRZCharRef() = 0;                                                       // 89
    virtual void* AsVoidPtr() const = 0;                                                   // 90
    virtual void*& AsVoidPtrRef() = 0;                                                     // 91
    virtual void SetRefBool(bool* value, uint32_t length) = 0;                             // 92
    virtual bool* RefBool() const = 0;                                                     // 93
    virtual void SetRefUint8(uint8_t* value, uint32_t length) = 0;                         // 94
    virtual uint8_t* RefUint8() const = 0;                                                 // 95
    virtual void SetRefSint8(int8_t* value, uint32_t length) = 0;                          // 96
    virtual int8_t* RefSint8() const = 0;                                                  // 97
    virtual void SetRefUint16(uint16_t* value, uint32_t length) = 0;                       // 98
    virtual uint16_t* RefUint16() const = 0;                                               // 99
    virtual void SetRefSint16(int16_t* value, uint32_t length) = 0;                        // 100
    virtual int16_t* RefSint16() const = 0;                                                // 101
    virtual void SetRefUint32(uint32_t* value, uint32_t length) = 0;                       // 102
    virtual uint32_t* RefUint32() const = 0;                                               // 103
    virtual void SetRefSint32(int32_t* value, uint32_t length) = 0;                        // 104
    virtual int32_t* RefSint32() const = 0;                                                // 105
    virtual void SetRefUint64(uint64_t* value, uint32_t length) = 0;                       // 106
    virtual uint64_t* RefUint64() const = 0;                                               // 107
    virtual void SetRefSint64(int64_t* value, uint32_t length) = 0;                        // 108
    virtual int64_t* RefSint64() const = 0;                                                // 109
    virtual void SetRefFloat32(float* value, uint32_t length) = 0;                         // 110
    virtual float* RefFloat32() const = 0;                                                 // 111
    virtual void SetRefFloat64(double* value, uint32_t length) = 0;                        // 112
    virtual double* RefFloat64() const = 0;                                                // 113
    virtual void SetRefChar(char* value, uint32_t length) = 0;                             // 114
    virtual char* RefChar() const = 0;                                                     // 115
    virtual void SetRefRZUnicodeChar(uint16_t* value, uint32_t length) = 0;                // 116
    virtual uint16_t* RefRZUnicodeChar() const = 0;                                        // 117
    virtual void SetRefRZChar(char* value, uint32_t length) = 0;                           // 118
    virtual char* RefRZChar() const = 0;                                                   // 119
    virtual void SetRefVoidPtr(void** value, uint32_t length) = 0;                         // 120
    virtual void** RefVoidPtr() const = 0;                                                 // 121
    virtual void SetRefVoid(void* value, uint32_t length) = 0;                             // 122
    virtual void* RefVoid() const = 0;                                                     // 123
    virtual void SetRefIGZUnknown(cIGZUnknown* value) = 0;                                 // 124
    virtual bool RefIGZUnknownAs(uint32_t riid, void** ppvObj) = 0;                        // 125
    virtual cIGZUnknown* RefIGZUnknown() const = 0;                                        // 126
};

// cISCPropertyHolder (IID 0x25216283, 22 slots; foundations §17.2). The
// game's holders (buildings, exemplars) are only ever called through this.
// SDK errors fixed: slots 11, 12, 17, 18, 19. The bools' meaning is unknown;
// callers pass false.
class cFC4ISCPropertyHolder : public cIGZUnknown {
public:
    using EnumFn = void (*)(cISCProperty*, void*);
    using FilterFn = bool (*)(cISCProperty*, void*);

    virtual bool HasProperty(uint32_t id) const = 0;                                         // 3
    virtual bool GetPropertyList(cIGZUnknownList** list) const = 0;                          // 4
    virtual bool GetPropertyData(uint32_t id, void* buffer, uint32_t& length) const = 0;     // 5
    virtual bool GetPropertyInterface(uint32_t id, uint32_t riid, void** ppv) const = 0;     // 6
    virtual bool GetPropertyString(uint32_t id, cIGZString& value) const = 0;                // 7
    virtual bool GetPropertyU32(uint32_t id, uint32_t& value) const = 0;                     // 8
    virtual cISCProperty* GetProperty(uint32_t id) const = 0;                                // 9
    virtual bool AddPropertyData(uint32_t id, void* data, uint32_t length, bool b) = 0;      // 10
    virtual bool AddPropertyObject(uint32_t id, cIGZUnknown* value, bool b) = 0;            // 11  (SDK: int32)
    virtual bool AddPropertyString(uint32_t id, cIGZString const& value, bool b) = 0;        // 12  (SDK: no bool)
    virtual bool AddPropertyU32(uint32_t id, uint32_t value, bool b) = 0;                    // 13
    virtual bool AddPropertyVariant(uint32_t id, cFC4IGZVariant const* value, bool b) = 0;   // 14
    virtual bool AddPropertyRecord(cISCProperty* property, bool b) = 0;                      // 15
    virtual bool CopyAddProperty(cISCProperty* property, bool b) = 0;                        // 16
    virtual bool RemoveProperty(uint32_t id, bool b) = 0;                                    // 17  (SDK: no bool)
    virtual bool RemoveAllProperties(bool b) = 0;                                            // 18  (SDK: no bool)
    virtual bool EnumPropertiesFiltered(FilterFn filter, EnumFn fn, void* data) const = 0;   // 19  (SDK: 2 args)
    virtual bool EnumProperties(EnumFn fn, void* data) const = 0;                            // 20
    virtual bool CompactProperties() = 0;                                                    // 21
};

// cISC4LotManager (28 slots; foundations §17.8; zones.md §11.3). SDK errors
// fixed: slots 7-10 return a count in EAX, slots 17-19 a float in ST0
// (flora.md §12, zones.md §8.7), slot 22 has the lot out-parameter, slot 27's
// grid is the uint16 grid.
class cFC4ISC4LotManager : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                     // 3
    virtual bool Shutdown() = 0;                                                 // 4
    virtual cISC4Lot* GetLotAtPoint(cS3DVector3 const& point) = 0;               // 5  GetLot(cS3DVector3 const&)
    virtual cISC4Lot* GetLot(int32_t cellX, int32_t cellZ, bool b) = 0;          // 6  GetLot(x, z, bool)
    // Returns the number of lots appended (budget Errata round 2, item 5; SDK: bool).
    // [abi] 7-10: the nodes are B (allocators.md); 8-10 append without clearing.
    virtual uint32_t GetLotsInCellRect(fc4::GameList<cISC4Lot*>& lots, int32_t x1, int32_t z1, int32_t x2, int32_t z2,
                                       bool locationOnly) = 0;                   // 7
    // zones.md §11.3: 8-10 return the number of lots appended in EAX (SDK: bool).
    virtual uint32_t GetLotsSurroundingCell(int32_t cellX, int32_t cellZ, fc4::GameList<cISC4Lot*>& lots,
                                            int32_t flags) = 0;                  // 8  GetLotsSurroundingLot(x, z, ...)
    virtual uint32_t GetLotsSurroundingLot(cISC4Lot const* lot, fc4::GameList<cISC4Lot*>& lots,
                                           int32_t flags) = 0;                   // 9
    virtual uint32_t GetLotsSurroundingRect(SC4Rect<long> const& rect, fc4::GameList<cISC4Lot*>& lots,
                                            int32_t flags) = 0;                  // 10
    virtual bool GetVerticesAtEdgeOfLot(SC4Rect<long> const& cells, int32_t flags, SC4Rect<long>& vertices) = 0;  // 11
    virtual bool GetNeighborCellsAtEdgeOfLot(SC4Rect<long> const& cells, int32_t flags, SC4Rect<long>& out) = 0;  // 12
    virtual cISC4Lot* GetRandomLot() = 0;                                        // 13
    virtual cISC4Lot* GetOccupantLot(cISC4Occupant* occupant) = 0;              // 14
    // [abi] 15: EAX = an 8-byte global-heap (C) block {current node, lot list
    // object}, registered with the manager (aura.md §6.5; FC4SafeLotIterator.h).
    virtual intptr_t GetSafeLotIterator() = 0;                                   // 15
    virtual int32_t GetLotCount() = 0;                                           // 16
    // zones.md §11.3: 17 and 19 return a float in ST0 (SDK: int32); the
    // bool out-parameters are pointers (pop 16 / 12).
    virtual float GetLotSavvyTerrainVertexAltitude(int32_t x, int32_t z, cISC4Lot* exclude, bool* ok) = 0;  // 17
    // flora.md §12, §13.2: returns a float in ST0 (SDK: int32), pop 12.
    virtual float GetLotSavvyTerrainAltitude(float x, float z, bool* onLot) = 0;  // 18
    virtual float GetLotSavvyCellAltitude(int32_t cellX, int32_t cellZ, bool* ok) = 0;  // 19
    virtual int32_t CanCreateLot(int32_t x, int32_t z, int32_t w, int32_t h, bool zoneNotNeeded, bool ignoreConfig,
                                 cISC4LotConfiguration* config) = 0;             // 20
    virtual int32_t CanCreatePloppedLot(SC4Rect<long> bounds, int32_t n, cISC4LotConfiguration* config) = 0;  // 21
    virtual bool CreateLotIfPossible(int32_t x, int32_t z, int32_t w, int32_t h, int32_t n, cISC4Lot*& lot,
                                     bool zoneNotNeeded, bool ignoreConfig,
                                     cISC4LotConfiguration* config) = 0;         // 22  (SDK: no lot out-parameter)
    virtual bool CreateLot(int32_t x, int32_t z, int32_t w, int32_t h, int32_t facing, cISC4Lot*& lot) = 0;  // 23
    virtual bool DeleteLot(cISC4Lot* lot) = 0;                                   // 24
    virtual bool DeleteLots(int32_t x1, int32_t z1, int32_t x2, int32_t z2) = 0;  // 25
    virtual bool GetLotVertexAltitudeAdjustmentRange(int32_t x, int32_t z, float& rangeX, float& rangeY) = 0;  // 26
    // zones.md §11.3: the uint16 grid (class 0x49B9E604; SDK: <short>), no AddRef.
    virtual cISC4SimGrid<uint16_t>* GetMaximumCapacityGrid() = 0;                // 27
};

// cISC4Lot (70 slots; foundations §17.8). SDK error fixed: slot 61 returns
// SC4Percentage BY VALUE (hidden result pointer under MSVC).
class cFC4ISC4Lot : public cIGZUnknown {
public:
    using DeveloperType = cISC4BuildingDevelopmentSimulator::DeveloperType;
    using WealthType = cISC4BuildingOccupant::WealthType;
    using HabitationState = cISC4Lot::HabitationState;
    using VacateReason = cISC4Lot::VacateReason;

    virtual cISCPropertyHolder* AsPropertyHolder() = 0;                          // 3
    virtual cISC4TrafficSource* AsTrafficSource() = 0;                           // 4
    virtual bool Init() = 0;                                                     // 5
    virtual bool IsInitialized() const = 0;                                      // 6
    virtual bool Shutdown() = 0;                                                 // 7
    virtual bool PostCityInit() = 0;                                             // 8
    virtual cISC4LotConfiguration* GetLotConfiguration() = 0;                    // 9
    virtual bool SetLotConfiguration(cISC4LotConfiguration* config) = 0;         // 10
    virtual cSC4LotConfigurationObject* GetLotConfigurationObject(cISC4Occupant* occupant) = 0;  // 11
    virtual uint32_t GetFlags() const = 0;                                       // 12
    virtual bool SetFlags(uint32_t flags) = 0;                                   // 13
    virtual bool GetBoundingRect(SC4Rect<int32_t>& rect) const = 0;              // 14
    // docs/abi and crime.md §13 (CQ3): the result goes only through the out
    // pointer (EAX = &out) and nothing is left on the x87 stack (the SDK says
    // bool). The crime glue calls it through LotCenterRaw, which reads the out
    // parameter and counts any x87 stack depth change (never compensated).
    virtual void GetCenter(float* center) const = 0;                             // 15
    virtual float GetAltitude() const = 0;                                       // 16
    virtual bool SetAltitude(float altitude) = 0;                                // 17
    virtual float GetSlope() const = 0;                                          // 18
    virtual float GetSlopeAtTimeOfConstruction() const = 0;                      // 19
    virtual bool UpdateSlope() = 0;                                              // 20
    virtual bool GetLocation(int32_t& x, int32_t& z) const = 0;                  // 21
    virtual bool GetSize(uint8_t& sizeX, uint8_t& sizeZ) const = 0;              // 22
    virtual int32_t GetFacing() const = 0;                                       // 23
    virtual bool SetFacing(int32_t facing) = 0;                                  // 24
    virtual cISC4BusinessUnit* GetUnit() = 0;                                    // 25
    virtual bool SetUnit(cISC4BusinessUnit* unit) = 0;                           // 26
    virtual bool HasJobAccess() const = 0;                                       // 27
    virtual bool HasRoadAccess() const = 0;                                      // 28
    virtual bool HasOrganicRoadAccess(uint32_t purposeType) const = 0;           // 29
    virtual bool UpdateRoadAccess() = 0;                                         // 30
    virtual cISC4ZoneManager::ZoneType GetZoneType() const = 0;                  // 31
    virtual bool UpdateZoneType() = 0;                                           // 32
    virtual HabitationState GetState() const = 0;                                // 33
    virtual bool SetState(HabitationState state, WealthType wealth, VacateReason reason) = 0;  // 34
    virtual uint32_t GetCondition() const = 0;                                   // 35
    virtual bool SetCondition(uint32_t condition) = 0;                           // 36
    virtual bool IsInLivableState() const = 0;                                   // 37
    virtual bool HasCapacity() const = 0;                                        // 38
    virtual bool IsInDemolishableState() const = 0;                              // 39
    virtual uint32_t GetDaysSinceLastStateChange() const = 0;                    // 40
    virtual uint32_t GetOccupantWealth() const = 0;                              // 41
    virtual uint32_t GetLandValue() const = 0;                                   // 42
    virtual bool IsHistorical() const = 0;                                       // 43
    virtual bool SetHistorical(bool historical) = 0;                             // 44
    virtual VacateReason GetVacateReason() const = 0;                            // 45
    virtual uint32_t GetBuildingType(bool forceCalculation) const = 0;           // 46
    virtual bool SetBuildingType(uint32_t buildingType) = 0;                     // 47
    virtual cISC4BuildingOccupant* GetBuilding() const = 0;                      // 48
    virtual void GetLotOccupants(SC4List<cISC4Occupant*>& occupants) const = 0;  // 49
    virtual uint16_t GetCapacityOfDeveloper(DeveloperType type, bool forceCalculation) const = 0;  // 50  GetCapacity(dev, bool)
    virtual uint16_t GetCapacity(WealthType wealth) const = 0;                   // 51  GetCapacity(wealth)
    virtual uint16_t GetAvailableCapacity(DeveloperType type, bool forceCalculation) const = 0;  // 52
    virtual uint16_t GetPopulation(DeveloperType type) const = 0;                // 53
    virtual intptr_t GetPopulations() const = 0;                                 // 54
    virtual bool SetPopulation(DeveloperType type, uint16_t population) = 0;     // 55
    virtual uint16_t GetTravelPopulation(DeveloperType type) const = 0;          // 56
    virtual bool ResetTravelPopulation() = 0;                                    // 57
    virtual bool AddTravelPopulation(cISC4Lot* lot) = 0;                         // 58
    virtual cISC4Lot* GetTravelDesignate() const = 0;                            // 59
    virtual bool SetTravelDesignate(cISC4Lot* designate) = 0;                    // 60
    virtual SC4Percentage GetOccupancy() const = 0;                              // 61  (SDK: float)
    virtual bool GetOccupancyByType(std::map<uint32_t, SC4Percentage>& out) const = 0;  // 62
    virtual float GetTravelJobs(WealthType wealth) const = 0;                    // 63
    virtual bool GetJobs(float jobs[]) const = 0;                                // 64
    virtual bool GetNeedsPower() const = 0;                                      // 65
    virtual bool GetHasPower() const = 0;                                        // 66
    virtual bool SetHasPower(bool hasPower) = 0;                                 // 67
    virtual bool GetVerticesAreLocked() const = 0;                               // 68
    virtual bool SetVerticesAreLocked(bool locked) = 0;                          // 69
};

// cISC4NeighborsSimulator (38 slots), from the SDK declaration with the
// return channels of docs/abi/interfaces.tsv. Slots 32/33 return an int64 in
// EDX:EAX, as the SDK says (neighbors.md §15.4 NG11: every game caller reads
// EDX); slot 35 returns in ST0 (SDK error: int64). Overload groups as compiled
// (13/14 and 29/30, checked by abi_conformance_test).
class cFC4ISC4NeighborsSimulator : public cIGZUnknown {
public:
    using ConnectionType = cISC4NeighborsSimulator::ConnectionType;

    virtual bool Init() = 0;                                                     // 3
    virtual bool Shutdown() = 0;                                                 // 4
    virtual bool RequestCreateConnection(ConnectionType type, int32_t x, int32_t z, bool flag) = 0;  // 5
    virtual cISC4NeighborConnection* CreateConnection(ConnectionType type, int32_t x, int32_t z, bool flag) = 0;  // 6
    virtual void DeleteConnection(ConnectionType type, int32_t x, int32_t z) = 0;  // 7
    virtual void DeleteOccupantConnection(cISC4Occupant* occupant) = 0;          // 8
    virtual void DeleteAllConnectionsOfType(ConnectionType type) = 0;            // 9
    virtual bool HasConnection(ConnectionType type, int32_t x, int32_t z) const = 0;  // 10
    virtual cISC4NeighborConnection* FindConnection(ConnectionType type, int32_t x, int32_t z) = 0;  // 11
    virtual cISC4RegionalCity* FindCityOnOtherSideOfConnection(int32_t x, int32_t z) = 0;  // 12
    virtual cISC4NeighborConnection* FindConnectionOnOtherSideOf(cISC4NeighborConnection* connection) = 0;  // 13
    virtual cISC4NeighborConnection* FindConnectionOnOtherSideAt(ConnectionType type, int32_t x,
                                                                 int32_t z) = 0;  // 14
    virtual cISC4NeighborConnection* FindUnvisitedConnection(ConnectionType type,
                                                             cISC4RegionalCity const* city) = 0;  // 15
    virtual void GetAllConnections(SC4List<cISC4NeighborConnection*>& list) const = 0;  // 16
    virtual void GetConnectionsOfType(ConnectionType type, SC4List<cISC4NeighborConnection*>& list) const = 0;  // 17
    virtual void GetConnectionsOfTypeToCity(ConnectionType type, cISC4RegionalCity const* city,
                                            SC4List<cISC4NeighborConnection*>& list) const = 0;  // 18
    virtual void GetConnectionsToCity(cISC4RegionalCity const* city, SC4List<cISC4NeighborConnection*>& list) = 0;  // 19
    virtual void GetAdjacentCities(SC4List<cISC4RegionalCity*>& list, bool includeUnestablished) = 0;  // 20
    virtual void GetConnectedCities(SC4List<cISC4RegionalCity*>& list, bool includeUnestablished) = 0;  // 21
    virtual void GetAllConnectedCities(SC4List<cISC4RegionalCity*>& list, bool includeUnestablished) = 0;  // 22
    virtual cISC4NeighborConnection* FindConnectionSuitableForDeal(int32_t dealType,
                                                                   cISC4RegionalCity const* city) = 0;  // 23
    virtual bool ProposeDeal(int32_t dealType, cISC4RegionalCity* otherCity, cISC4NeighborDeal** outDeal) = 0;  // 24
    // [abi] 25, 28-30 hold neighbour deals (allocators.md); A: SC4List is compatible.
    virtual bool ProposeDeals(SC4List<cISC4NeighborDeal*>& deals, bool flag) = 0;  // 25
    virtual bool AddDeal(cISC4NeighborDeal* deal) = 0;                           // 26
    virtual void SetUIDisableFlag(bool flag) = 0;                                // 27
    virtual void FindDealsAll(SC4List<cISC4NeighborDeal*>& deals) = 0;     // 28
    virtual void FindDealsWithCity(int32_t dealType, cISC4RegionalCity* city,
                                   SC4List<cISC4NeighborDeal*>& deals) = 0;  // 29  FindDeals(3 args)
    virtual void FindDealsOfType(int32_t dealType, SC4List<cISC4NeighborDeal*>& deals) = 0;  // 30  FindDeals(2 args)
    virtual void MarkDealsDirty() = 0;                                           // 31
    // NG11 (neighbors.md §15.4): a signed 64-bit sum in EDX:EAX; the finance
    // advisor, the budget simulator and the budget windows read both halves.
    virtual int64_t GetTotalMonthlyNeighborDealIncome() = 0;                     // 32  edx:eax
    virtual int64_t GetTotalMonthlyNeighborDealExpense() = 0;                    // 33  edx:eax
    virtual void UpdateCommodityStats(cISC4City* city) = 0;                      // 34
    virtual double GetDealCost(int32_t dealType, double amount, cISC4RegionalCity const* city) = 0;  // 35  (SDK: int64)
    // [choice] the table says x87|eax (low confidence); not called.
    virtual double GetDealMaxAmount(int32_t dealType, cISC4RegionalCity const* city) = 0;  // 36  (SDK: int64)
    virtual cISCResExemplar* GetTuningExemplar() = 0;                            // 37
};

// cISC4WeatherSimulator (IID 0xC6C66245), 24 slots (weather spec "Compiled
// slot tables": the SDK order). Implemented by cFC4WeatherSimulator. One
// return channel differs from the SDK (docs/abi: StandardCityCellToCell
// returns in EDX:EAX); no game code calls it (weather §11.2), but a
// replacement keeps the channel. CellToPosition returns nothing and leaves
// nothing on the x87 stack (weather §4, corrected 2026-10-14; it was declared
// float). GetMoistureValueMapPtr takes the caller's pointer variable (§11.2
// [compat]; the SDK says a vector reference).
struct FC4ByteGridVector;  // SC4Vector<SC4Vector<uint8_t>>-compatible (cFC4Weather.h)
class cS3DVector2;
class cFC4ISC4WeatherSimulator : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                      // 3
    virtual bool Shutdown() = 0;                                                  // 4
    virtual bool DoPostCityInit() = 0;                                            // 5
    virtual float CellWidthX() = 0;                                               // 6
    virtual float CellWidthZ() = 0;                                               // 7
    virtual uint32_t CellCountX() = 0;                                            // 8
    virtual uint32_t CellCountZ() = 0;                                            // 9
    virtual int32_t PositionToCell(float x, float z, int32_t& cx, int32_t& cz) = 0;     // 10
    virtual void CellToPosition(int32_t cx, int32_t cz, float& x, float& z) = 0;        // 11
    virtual void CellToStandardCityCell(int32_t cx, int32_t cz, int32_t& sx, int32_t& sz) = 0;  // 12
    virtual int64_t StandardCityCellToCell(int32_t sx, int32_t sz, int32_t& cx, int32_t& cz) = 0;  // 13  (SDK: void)
    virtual int32_t GetCurrentSeason() = 0;                                       // 14  (SDK: enum Season)
    virtual int32_t GetCurrentSeasonMonth() = 0;                                  // 15
    virtual void UpdateMoisture(SC4Rect<int32_t> const& rect) = 0;                // 16
    virtual uint8_t GetMoistureValue(float x, float z) = 0;                       // 17
    virtual void GetMoistureValueMapPtr(FC4ByteGridVector*& out) = 0;             // 18
    virtual uint32_t GetMoistureType(float x, float z) = 0;  // 19 (flora.md §6.2, FQ5: 0..15, all of EAX)
    virtual void SetManualMoistureAddition(float x0, float z0, float x1, float z1, float v) = 0;  // 20
    virtual float GetHumidity(float x, float z) = 0;                              // 21
    virtual float GetAmbientWind(cS3DVector2& wind) = 0;                          // 22
    virtual float GetWindAtCell(int32_t x, int32_t z, cS3DVector2& wind) = 0;     // 23
};

// cISC4Ordinance (IID 0xAA5E2247), 30 slots (ordinance spec "Compiled slot
// tables": the SDK order). Implemented by cFC4Ordinance and called on foreign
// ordinances (DLL plugins). One return channel differs from the SDK: slot 10
// returns the SC4Percentage's f32 bits in EAX and pops nothing (docs/abi); the
// SDK's by-value SC4Percentage would be a hidden result pointer under MSVC.
class cFC4ISC4Ordinance : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                      // 3
    virtual bool Shutdown() = 0;                                                  // 4
    virtual int64_t GetCurrentMonthlyIncome() = 0;                                // 5
    virtual uint32_t GetID() const = 0;                                           // 6
    virtual cIGZString* GetName() = 0;                                            // 7
    virtual cIGZString* GetDescription() = 0;                                     // 8
    virtual uint32_t GetYearFirstAvailable() = 0;                                 // 9
    virtual uint32_t GetChanceAvailability() = 0;                                 // 10  (SDK: SC4Percentage)
    virtual int64_t GetEnactmentIncome() = 0;                                     // 11
    virtual int64_t GetRetracmentIncome() = 0;                                    // 12
    virtual int64_t GetMonthlyConstantIncome() = 0;                               // 13
    virtual float GetMonthlyIncomeFactor() = 0;                                   // 14
    virtual uint32_t GetAdvisorID() = 0;                                          // 15
    virtual cISCPropertyHolder* GetMiscProperties() = 0;                          // 16
    virtual bool IsAvailable() = 0;                                               // 17
    virtual bool IsOn() = 0;                                                      // 18
    virtual bool IsEnabled() = 0;                                                 // 19
    virtual int64_t GetMonthlyAdjustedIncome() = 0;                               // 20
    virtual bool CheckConditions() = 0;                                           // 21
    virtual bool IsIncomeOrdinance() = 0;                                         // 22
    virtual bool Simulate() = 0;                                                  // 23
    virtual bool SetAvailable(bool isAvailable) = 0;                              // 24
    virtual bool SetOn(bool isOn) = 0;                                            // 25
    virtual bool SetEnabled(bool isEnabled) = 0;                                  // 26
    virtual bool ForceAvailable(bool isAvailable) = 0;                            // 27
    virtual bool ForceOn(bool isOn) = 0;                                          // 28
    virtual bool ForceMonthlyAdjustedIncome(int64_t monthlyAdjustedIncome) = 0;   // 29
};

// cISC4OrdinanceSimulator (IID 0x41193C3A), 26 slots (docs/abi; the SDK
// order), called on whatever ordinance simulator the city has (the aura glue:
// aura.md section 6.3). One return channel differs from the SDK: slot 24
// returns the 32-bit sum in EAX and leaves EDX unset (SDK: int64). It is named
// apart from the SDK's GetCumulativeOffset so that abi_conformance_test's
// waiver scan, which finds calls by name, never takes it for the SDK one.
class cISC4Ordinance;
class cISC4OrdinanceSimulator;
class cFC4ISC4OrdinanceSimulator : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                             // 3
    virtual bool Shutdown() = 0;                                                         // 4
    virtual bool AddOrdinance(cISC4Ordinance& ordinance) = 0;                            // 5
    virtual bool RemoveOrdinance(cISC4Ordinance& ordinance) = 0;                         // 6
    virtual bool IsOrdinanceAvailable(uint32_t clsid) = 0;                               // 7
    virtual bool IsOrdinanceOn(uint32_t clsid) = 0;                                      // 8
    virtual bool IsOrdinanceAvailableButOff(uint32_t clsid) = 0;                         // 9
    virtual bool SetOrdinanceAvailable(uint32_t clsid, bool available) = 0;              // 10
    virtual uint32_t GetOrdinanceAvailableCount() = 0;                                   // 11
    virtual uint32_t GetOrdinanceAvailableArray(uint32_t* clsids, uint32_t& count) = 0;  // 12
    virtual bool SetOrdinanceOn(uint32_t clsid, bool on) = 0;                            // 13
    virtual uint32_t GetOrdinanceOnCount() = 0;                                          // 14
    virtual uint32_t GetOrdinanceOnArray(uint32_t* clsids, uint32_t& count) = 0;         // 15
    virtual bool IsOrdinanceEnabled(uint32_t clsid) = 0;                                 // 16
    virtual bool SetOrdinanceEnabled(uint32_t clsid, bool enabled) = 0;                  // 17
    virtual uint32_t GetOrdinanceIDArray(uint32_t* clsids, uint32_t& count) = 0;         // 18
    virtual uint32_t GetOrdinanceIDByName(cIGZString const& name) = 0;                  // 19
    virtual cISC4Ordinance* GetOrdinanceByID(uint32_t clsid) = 0;                        // 20
    virtual cISC4Ordinance* GetOrdinanceByName(cIGZString const& name) = 0;              // 21
    virtual int64_t GetOrdinanceMonthlyExpense() = 0;                                    // 22
    virtual int64_t GetOrdinanceMonthlyIncome() = 0;                                     // 23
    virtual int32_t GetCumulativeOffset32(uint32_t prop, uint32_t index) = 0;            // 24 (SDK: int64)
    virtual float GetCumulativeFactor(uint32_t prop, uint32_t index) = 0;                // 25
};

// cISC4Demand (IID 0x0797A4E8), 34 slots (demand spec §3.2, "Compiled slot
// tables": the SDK order). Implemented by cFC4Demand. Return channels as the
// game has them (docs/abi): AddToSupplyValue / AddToDemandValue return true
// as a bool in AL with nothing on the x87 stack (§3.2, corrected 2026-10-14;
// they were declared float, which left one value on the game's x87 stack per
// call), EndOfCycle returns a bool in EAX (SDK: float; §3.2 [compat]).
class cFC4ISC4Demand : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                      // 3
    virtual bool Shutdown() = 0;                                                  // 4
    virtual bool SimulationBegin() = 0;                                           // 5
    virtual uint32_t GetId() const = 0;                                           // 6
    virtual bool SetId(uint32_t id) = 0;                                          // 7
    virtual float QuerySupplyValue() const = 0;                                   // 8
    virtual float QueryDemandValue() const = 0;                                   // 9
    virtual float QueryNewSupply() const = 0;                                     // 10
    virtual float QueryNewDemand() const = 0;                                     // 11
    virtual float QueryActiveDemandValue() const = 0;                             // 12
    virtual float QueryEconomyModifier() const = 0;                               // 13
    virtual float QueryActiveDemandMax() const = 0;                               // 14
    virtual float QueryActiveDemandMin() const = 0;                               // 15
    virtual bool AddToSupplyValue(float value) = 0;                               // 16
    virtual bool AddToDemandValue(float value) = 0;                               // 17
    virtual bool SetSupplyValue(float value) = 0;                                 // 18
    virtual bool SetDemandValue(float value) = 0;                                 // 19
    virtual bool SetActiveDemandMax(float value) = 0;                             // 20
    virtual bool SetActiveDemandMin(float value) = 0;                             // 21
    virtual float GetEconomyModifier() const = 0;                                 // 22
    virtual bool SetEconomyModifier(float value) = 0;                             // 23
    virtual float GetTaxModifier() const = 0;                                     // 24
    virtual bool SetTaxModifier(float value) = 0;                                 // 25
    virtual SC4Percentage* GetDemandCap() const = 0;                              // 26
    virtual bool SetDemandCap(SC4Percentage const& demandCap) = 0;                // 27
    virtual uint32_t GetRegionUse() = 0;                                          // 28
    virtual bool SetRegionUse(uint32_t regionUse) = 0;                            // 29
    virtual bool EndOfCycle() = 0;                                                // 30  (SDK: float)
    virtual void DebugLockValue(float value) = 0;                                 // 31
    virtual void DebugUnlockValue() = 0;                                          // 32
    virtual bool DebugIsValueLocked() = 0;                                        // 33
};

// cISC4DemandSimulator (IID 0x47967E16), 12 slots (demand spec §5, "Compiled
// slot tables"). Implemented by cFC4DemandSimulator. SDK errors fixed: slot 7
// takes the building type (uint32) first (§5 [compat]). Slot 8 returns
// nothing and leaves the x87 stack as it found it (§5, corrected 2026-10-14;
// it was declared float); `jobs` is a four-float record whose jobs[1..3] it
// writes (never jobs[0]).
class cFC4ISC4DemandSimulator : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                      // 3
    virtual bool Shutdown() = 0;                                                  // 4
    virtual uint32_t GetSimulatorType() = 0;                                      // 5
    virtual cISC4Demand* GetDemand(uint32_t demandID, uint32_t demandIndex) = 0;  // 6
    virtual void UpdateOccupantEffects(uint32_t buildingType, SC4Percentage const& oldPct,
                                       SC4Percentage const& newPct) = 0;          // 7
    virtual void CalculateJobsPerUnitOfDemand(float* jobs, uint32_t demandID, uint32_t demandIndex) = 0;  // 8
    // 9: EAX = the low 32 bits of _ftol, no clamp (a negative product is a
    // negative int32); EDX is not meaningful (demand.md §5, DQ10, DQ12).
    virtual uint32_t GetJobsBySensus(uint32_t type) = 0;                          // 9
    virtual float GetNeutralTaxRate() = 0;                                        // 10
    virtual void GetLocalPopulationSummary(std::map<uint32_t, int32_t>& summary) const = 0;  // 11
};

/// The query record of residential slots 14, 15, 26 and 27 (REH §4.11,
// "tQueryData", 0x3C bytes): a leading cRZString the simulator never touches,
// then the fields below. The costs are 32-bit in the record.
struct FC4ResidentialQueryData {
    uint8_t name[0x14];          // +0x00 cRZString (not read or written here)
    uint32_t capacity;           // +0x14
    uint32_t effectiveCapacity;  // +0x18
    uint32_t capacityPerStaff;   // +0x1C
    uint32_t effectivePerStaff;  // +0x20
    uint32_t served;             // +0x24
    float grade;                 // +0x28
    float staffFunding;          // +0x2C
    uint32_t staffCost;          // +0x30
    float vehicleFunding;        // +0x34
    uint32_t vehicleCost;        // +0x38
};
static_assert(sizeof(void*) != 4 || sizeof(FC4ResidentialQueryData) == 0x3C, "tQueryData layout");

// cISC4ResidentialSimulator (60 slots), from the SDK declaration with the
// argument sizes and return channels of docs/abi/vtables.tsv
// (cSC4ResidentialSimulator::vftable), residential.md §10.1 and
// residential-education-health.md §7. Implemented by
// cFC4ResidentialSimulator; the demand census calls GetCellEQ and
// GetCellWorkforcePercent, crime slots 23 / 25, aura slots 39 / 40. SDK
// errors fixed:
//   - slots 10-13, 30 and 31 return an SC4Percentage BY VALUE (hidden result
//     pointer: the callee pops 4 more bytes and returns the pointer in EAX);
//   - slots 14 / 15 return the building count (EAX) and take the query
//     record and the group vector;
//   - slots 23 / 24 / 25: all three parameters are out-references, each
//     AddRef'd for the caller;
//   - slots 26 / 27 take the query record;
//   - slots 29, 34 and 42 return a uint8 in AL only (an implementation
//     returns the value zero-extended in EAX, §10.1);
//   - slots 39 / 40 return a cISC4SimGrid<float>*, slot 41 a
//     cISC4SimGrid<uint8>* (borrowed);
//   - slot 56 takes the occupant, slot 57 the occupant and the flag.
// The slots whose original return value is garbage (19, 20, 21-25, 28, 47,
// 49) are declared bool (§10.1: a replacement returns true).
class cFC4ISC4ResidentialSimulator : public cIGZUnknown {
public:
    using WealthType = cISC4BuildingOccupant::WealthType;

    virtual bool Init() = 0;                                                      // 3
    virtual bool Shutdown() = 0;                                                  // 4
    virtual cISC4SimGrid<uint8_t>* GetProximityMap(WealthType wealth) = 0;        // 5
    virtual bool SchoolIsOnStrike() = 0;                                          // 6
    virtual bool HealthIsOnStrike() = 0;                                          // 7
    virtual bool EndSchoolStrike() = 0;                                           // 8
    virtual bool EndHealthStrike() = 0;                                           // 9
    virtual SC4Percentage ChanceOfSchoolStrike() = 0;                             // 10  (SDK: float)
    virtual SC4Percentage ChanceOfHealthStrike() = 0;                             // 11  (SDK: float)
    virtual SC4Percentage GetSchoolSystemRating() = 0;                            // 12  (SDK: float)
    virtual SC4Percentage GetHealthSystemRating() = 0;                            // 13  (SDK: float)
    virtual uint32_t GetSchoolSystemTotals(FC4ResidentialQueryData* totals,
                                           SC4Vector<uint32_t> const* groups) = 0;  // 14  (SDK: bool)
    virtual uint32_t GetHospitalSystemTotals(FC4ResidentialQueryData* totals,
                                             SC4Vector<uint32_t> const* groups) = 0;  // 15  (SDK: bool)
    virtual int32_t GetPopulation() = 0;                                          // 16
    virtual int32_t GetTotalCityEducationUpkeepCost() = 0;                        // 17
    virtual int32_t GetTotalCityHealthUpkeepCost() = 0;                           // 18
    virtual bool SetOccupantFundingPercentages(cISC4Occupant* occupant, SC4Percentage const& staff,
                                               SC4Percentage const& vehicle, bool isSchool) = 0;  // 19
    virtual bool GetOccupantFundingPercentages(cISC4Occupant* occupant, SC4Percentage& staff,
                                               SC4Percentage& vehicle, bool isSchool) = 0;  // 20
    virtual bool GetAverageEQGrid(cISC4SimGrid<float>*& grid, float* min, float* max) = 0;  // 21
    virtual bool GetAverageHQGrid(cISC4SimGrid<float>*& grid, float* min, float* max) = 0;  // 22
    virtual bool GetEQGrids(cISC4SimGrid<float>*& g0, cISC4SimGrid<float>*& g1, cISC4SimGrid<float>*& g2) = 0;  // 23
    virtual bool GetHQGrids(cISC4SimGrid<float>*& g0, cISC4SimGrid<float>*& g1, cISC4SimGrid<float>*& g2) = 0;  // 24
    virtual bool GetPopulationGrids(cISC4SimGrid<uint16_t>*& g0, cISC4SimGrid<uint16_t>*& g1,
                                    cISC4SimGrid<uint16_t>*& g2) = 0;             // 25
    virtual bool GetSchoolQueryData(cISC4Occupant* occupant, FC4ResidentialQueryData* data) = 0;    // 26
    virtual bool GetHospitalQueryData(cISC4Occupant* occupant, FC4ResidentialQueryData* data) = 0;  // 27
    virtual bool EstimateCurrentOccupantCapacity(cISC4Occupant* occupant, uint32_t& capacity,
                                                 uint32_t& staff) = 0;            // 28
    virtual uint8_t GetCellLifeExpectancy(uint32_t cellX, uint32_t cellZ) = 0;    // 29  (SDK: int32)
    virtual SC4Percentage GetCellWorkforcePercent(uint32_t cellX, uint32_t cellZ) = 0;  // 30  (SDK: float)
    virtual SC4Percentage GetGlobalWorkforcePercent() = 0;                        // 31  (SDK: float)
    virtual float GetGlobalEQ() = 0;                                              // 32
    virtual float GetGlobalHQ() = 0;                                              // 33
    virtual uint8_t GetGlobalLE() = 0;                                            // 34  (SDK: float)
    virtual float GetCellEQ(uint32_t cellX, uint32_t cellZ) = 0;                  // 35
    virtual float GetCellHQ(uint32_t cellX, uint32_t cellZ) = 0;                  // 36
    // 37/38 return in x87 ST0 (re-answers7 RG22: docs/abi interfaces.tsv x87, high; they end in the float
    // grid's GetTractValue): declared float, so abi_conformance_test checks the channel.
    virtual float GetCellEQByWealth(uint32_t cellX, uint32_t cellZ, WealthType wealth) = 0;  // 37
    virtual float GetCellHQByWealth(uint32_t cellX, uint32_t cellZ, WealthType wealth) = 0;  // 38
    virtual cISC4SimGrid<float>* GetSchoolAverageGradeMap() = 0;                 // 39  (SDK: int32)
    virtual cISC4SimGrid<float>* GetHospitalAverageGradeMap() = 0;               // 40  (SDK: int32)
    virtual cISC4SimGrid<uint8_t>* GetAverageAgeMap() = 0;                        // 41  (SDK: int32)
    virtual uint8_t GetAverageNewAgeByWealth(WealthType wealth) = 0;              // 42  (SDK: int32)
    virtual bool GetEQMinAndMaxCellCoords(uint32_t& minX, uint32_t& minZ, uint32_t& maxX, uint32_t& maxZ, float& min,
                                          float& max) = 0;                        // 43
    virtual bool GetHQMinAndMaxCellCoords(uint32_t& minX, uint32_t& minZ, uint32_t& maxX, uint32_t& maxZ, float& min,
                                          float& max) = 0;                        // 44
    virtual bool GetOccupantCoverage(cISC4Occupant* occupant, SC4Percentage const& level, float& distance,
                                     float& radius) = 0;                          // 45
    virtual int32_t GetSchoolBuildingCount() = 0;                                 // 46
    virtual bool GetSchoolBuildings(SC4List<cISC4Occupant*>& buildings, SC4Vector<uint32_t> const* groups) = 0;  // 47
    virtual int32_t GetHospitalBuildingCount() = 0;                               // 48
    virtual bool GetHospitalBuildings(SC4List<cISC4Occupant*>& buildings,
                                      SC4Vector<uint32_t> const* groups) = 0;     // 49
    virtual int32_t GetMaxEQ() = 0;                                               // 50
    virtual int32_t GetMaxHQ() = 0;                                               // 51
    virtual bool GetGlobalAutoBudgetForSchools() = 0;                             // 52
    virtual bool SetGlobalAutoBudgetForSchools(bool enable) = 0;                  // 53
    virtual bool GetGlobalAutoBudgetForHospitals() = 0;                           // 54
    virtual bool SetGlobalAutoBudgetForHospitals(bool enable) = 0;                // 55
    virtual bool GetAutoBudget(cISC4Occupant* occupant) = 0;                      // 56  (SDK: no argument)
    virtual bool SetAutoBudget(cISC4Occupant* occupant, bool enable) = 0;         // 57  (SDK: one argument)
    virtual bool EstimateIdealFunding(cISC4Occupant* occupant, SC4Percentage& funding) = 0;  // 58
    virtual void ToggleTractTracking(int32_t a, int32_t b) = 0;                   // 59
};

// cISC4RegionalCity (73 slots), from the SDK declaration with the argument
// sizes and return channels of docs/abi/interfaces.tsv. The demand simulator
// calls the neighbour methods of §9 (slots 9, 13 and 16-19). Overload groups
// as compiled (MSVC puts each group at its first declaration, reversed):
// GetPopulation(type) is slot 9 and GetPopulation() slot 10, the two
// UpdateCityCache overloads slots 24 (city) / 25 (preferences). SDK errors
// fixed: slot 13 returns 32 bits in EAX (SDK: float) [choice: the f32 bits,
// see the demand host]; slot 48 is the elevation SETTER (the SDK names it
// GetWorldBaseElevation); the setters 51, 53, 55 ... 63 take a double (SDK:
// float) and return nothing (docs/abi, corrected 2026-10-14: they were
// declared float, which would pop a value the game never pushed).
// An element of GetPointsOfInterest's vector: 12 bytes (allocators.md); the
// fields are not specified.
struct FC4PointOfInterest {
    uint32_t raw[3];
};
static_assert(sizeof(FC4PointOfInterest) == 12, "12-byte elements");

class cFC4ISC4RegionalCity : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                      // 3
    virtual bool Shutdown() = 0;                                                  // 4
    virtual void GetPosition(int32_t& x, int32_t& z) const = 0;                   // 5
    virtual void SetPosition(int32_t x, int32_t z, bool rearrange) = 0;           // 6
    virtual void GetCitySize(int32_t& x, int32_t& z) const = 0;                   // 7
    virtual void SetCitySize(int32_t x, int32_t z) = 0;                           // 8
    virtual int32_t GetPopulationOfType(uint32_t populationType) const = 0;       // 9   GetPopulation(type)
    virtual int32_t GetPopulation() const = 0;                                    // 10  GetPopulation()
    virtual int32_t GetCommercialJobs() const = 0;                                // 11
    virtual int32_t GetIndustrialJobs() const = 0;                                // 12
    virtual uint32_t GetWorkforcePercentage() const = 0;                          // 13  (SDK: float)
    virtual int8_t GetMayorRating() const = 0;                                    // 14
    virtual cISC4City::DifficultyLevel GetDifficultyLevel() const = 0;            // 15
    virtual float GetTaxRate(uint32_t taxType) const = 0;                         // 16
    virtual int32_t GetExtrapolatedPopulation(uint32_t populationType) const = 0;  // 17
    virtual int32_t GetAllowableExtrapolation(uint32_t populationType) const = 0;  // 18
    virtual void ExtrapolateGrowth(uint32_t populationType, float addedPopulation) = 0;  // 19
    virtual cISC4RegionalCity* FindConnection(int32_t a, int32_t b, int32_t c) const = 0;  // 20
    virtual void GetAllConnections(SC4List<cISC4NeighborConnection*>& list) const = 0;  // 21
    virtual void ChangeSymmetricConnection(cISC4NeighborConnection* connection, bool flag) = 0;  // 22
    virtual void SetupPreferences(SC4NewCityPreferences* preferences) = 0;        // 23
    virtual void UpdateCityCacheFromCity(cISC4City* city) = 0;                    // 24  UpdateCityCache(city)
    virtual void UpdateCityCacheFromPreferences(SC4NewCityPreferences* preferences) = 0;  // 25  UpdateCityCache(prefs)
    virtual void SetupCity(cISC4City* city) = 0;                                  // 26
    virtual uint32_t GetCitySerialNumber() const = 0;                             // 27
    virtual void SetCitySerialNumber(uint32_t serialNumber) = 0;                  // 28
    virtual bool GetOriginalLanguageAndCountry(int32_t& language, int32_t& country) const = 0;  // 29
    virtual bool GetLastLanguageAndCountry(int32_t& language, int32_t& country) = 0;  // 30
    virtual void GetCitySaveFilePath(cIGZString& path) const = 0;                 // 31
    virtual void SetCitySaveFilePath(cIGZString const& path) = 0;                 // 32
    virtual void GetCityName(cIGZString& name) const = 0;                         // 33
    virtual void SetCityName(cIGZString const& name) = 0;                         // 34
    virtual void GetMayorName(cIGZString& name) const = 0;                        // 35
    virtual void SetMayorName(cIGZString const& name) = 0;                        // 36
    virtual void GetUtilityAdvisorName(cIGZString& name) const = 0;               // 37
    virtual void SetUtilityAdvisorName(cIGZString const& name) = 0;               // 38
    virtual void GetCityDescription(cIGZString& description) const = 0;           // 39
    virtual void SetCityDescription(cIGZString const& description) = 0;          // 40
    virtual uint32_t GetBirthDate() const = 0;                                    // 41
    virtual void SetBirthDate(uint32_t birthDate) = 0;                            // 42
    virtual bool GetEstablished() const = 0;                                      // 43
    virtual void SetEstablished(bool established) = 0;                           // 44
    virtual void GetWorldPosition(float& longitude, float& latitude) const = 0;   // 45
    virtual void SetWorldPosition(float longitude, float latitude) = 0;           // 46
    virtual float GetWorldBaseElevation() const = 0;                              // 47
    virtual void SetWorldBaseElevation(float elevation) = 0;                      // 48  (SDK: GetWorldBaseElevation)
    virtual cISC4City::WorldHemisphere GetWorldHemisphere() const = 0;            // 49
    virtual double GetBudget() const = 0;                                         // 50
    virtual void SetBudget(double budget) = 0;                                    // 51
    virtual double GetIncome() const = 0;                                         // 52
    virtual void SetIncome(double income) = 0;                                    // 53
    virtual double GetExported(int32_t commodity) const = 0;                      // 54
    virtual void SetExported(int32_t commodity, double value) = 0;                // 55
    virtual double GetImported(int32_t commodity) const = 0;                      // 56
    virtual void SetImported(int32_t commodity, double value) = 0;                // 57
    virtual double GetProduced(int32_t commodity) const = 0;                      // 58
    virtual void SetProduced(int32_t commodity, double value) = 0;                // 59
    virtual double GetDemanded(int32_t commodity) const = 0;                      // 60
    virtual void SetDemanded(int32_t commodity, double value) = 0;                // 61
    virtual double GetCostPerUnit(int32_t commodity) const = 0;                   // 62
    virtual void SetCostPerUnit(int32_t commodity, double value) = 0;             // 63
    virtual double GetCommodityBalance(int32_t commodity) const = 0;              // 64
    virtual uint32_t GetTutorialGUID() const = 0;                                 // 65
    virtual void SetTutorialGUID(uint32_t guid) = 0;                              // 66
    virtual bool IsTutorial() const = 0;                                          // 67
    virtual bool UpdateLocalDeals() = 0;                                          // 68
    virtual bool SetLocalDeals(SC4List<cISC4NeighborDeal*>& list) = 0;            // 69
    virtual bool GetLocalDeals(SC4List<cISC4NeighborDeal*>& list) = 0;            // 70
    virtual bool UpdateImportExport() = 0;                                        // 71
    // [abi] std::vector (B) of 12-byte elements (allocators.md); left untouched
    // when the type is absent.
    virtual void GetPointsOfInterest(uint32_t type, fc4::GameVector<FC4PointOfInterest>& list) = 0;  // 72
};

// ---- power (docs/specs/power.md §10, §11.1) -----------------------------------------------------

class cISC4Occupant;

// cISC4PowerSimulator (IID 0xE9E7BFF5), 52 slots in compiled order (power
// §10.1). Implemented by cFC4PowerSimulator. SDK errors fixed: the overloads
// are in compiled order with unique names (16 = (x, z), 17 = (); 23 = (type),
// 24 = ()); 19/31/37/38 return full EAX 0/1, 32-34 return bool in AL;
// 40 returns its argument's address; 45/46
// return an SC4Percentage through a hidden pointer (pop 8, EAX = the pointer).
class cFC4ISC4PowerSimulator : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                                // 3
    virtual bool Shutdown() = 0;                                                            // 4
    virtual void PositionToCell(float x, float z, int32_t& cellX, int32_t& cellZ) = 0;       // 5
    virtual void CellToPosition(int32_t cellX, int32_t cellZ, float& x, float& z) = 0;       // 6
    virtual void CellToStandardCityCell(int32_t x, int32_t z, int32_t& outX, int32_t& outZ) = 0;  // 7
    virtual void StandardCityCellToCell(int32_t x, int32_t z, int32_t& outX, int32_t& outZ) = 0;  // 8
    virtual uint32_t GetAnnualConsumption() = 0;                                            // 9
    virtual uint32_t GetAnnualConsumptionByPowerType(int32_t powerType) = 0;                // 10
    virtual int32_t GetMonthlyRequirement() = 0;                                            // 11
    virtual int32_t GetMonthlyProduction() = 0;                                             // 12
    virtual int32_t GetMonthlyImport() = 0;                                                 // 13
    virtual int32_t GetMonthlyExport() = 0;                                                 // 14
    virtual int32_t GetMonthlyDemand() = 0;                                                 // 15
    virtual int32_t GetMonthlyConsumptionAt(int32_t x, int32_t z) = 0;                      // 16  (x, z)
    virtual int32_t GetMonthlyConsumptionTotal() = 0;                                       // 17  ()
    // 18: the game writes the address of its raw vector<vector<int16>> (power §11.2).
    virtual void GetMonthlyConsumptionMapPtr(void*& map) = 0;                               // 18
    virtual uint32_t GetSupplyValue(int32_t x, int32_t z) = 0;                              // 19  full EAX 0/1
    virtual void GetSupplyValueMapPtr(cISC4SimGrid<uint8_t>*& grid) = 0;                    // 20
    virtual int32_t GetUnpoweredConsumerCount() = 0;                                        // 21
    virtual int32_t GetPoweredConsumerCount() = 0;                                          // 22
    virtual int32_t GetPowerPlantCountOfType(int32_t powerType) = 0;                        // 23  (type)
    virtual int32_t GetPowerPlantCountAll() = 0;                                            // 24  ()
    // [abi] 25: ilist<cISC4Occupant*> with the framework allocator (A, allocators.md
    // table (2)): SC4List is compatible; appends, one reference each.
    virtual void GetPowerPlants(SC4List<cISC4Occupant*>& list) = 0;                        // 25
    virtual uint32_t GetPowerPoleCount() = 0;                                               // 26
    virtual int32_t GetOverworkedPowerPlantCount() = 0;                                     // 27
    virtual int32_t GetAgedPowerPlantCount() = 0;                                           // 28
    virtual int32_t GetDistressedPowerPlantCount() = 0;                                     // 29
    virtual void FindPowerPlants(cISC4Occupant** lowestFunded, SC4Percentage& funding, cISC4Occupant** mostWorn,
                                 int32_t& wearPercent) = 0;                                 // 30
    virtual uint32_t IsOccupantPowered(cISC4Occupant* occupant) = 0;                        // 31  full EAX 0/1
    virtual bool GetOccupantPowerConsumed(cISC4Occupant* occupant, uint32_t& consumed) = 0;  // 32  (SDK: void)
    virtual bool GetOccupantPowerProduced(cISC4Occupant* occupant, uint32_t& produced,
                                          uint32_t& used) = 0;                              // 33  (SDK: void)
    virtual bool GetOccupantMaxPowerProduced(cISC4Occupant* occupant, uint32_t& maxProduced) = 0;  // 34  (SDK: void)
    virtual cISC4SimGrid<uint8_t>* GetPoweredGridConst() const = 0;                         // 35  GetPoweredGrid() const
    virtual cISC4SimGrid<uint8_t>* GetPoweredGrid() = 0;                                    // 36
    virtual uint32_t IsPowered(int32_t x, int32_t z) = 0;                                   // 37  full EAX 0/1
    virtual uint32_t IsWired(int32_t x, int32_t z) = 0;                                     // 38  full EAX 0/1
    virtual bool IsOnStrike() = 0;                                                          // 39
    virtual SC4Percentage* GetChanceOfStrike(SC4Percentage& chance) = 0;                    // 40  (SDK: void)
    virtual bool BeginStrike() = 0;                                                         // 41
    virtual bool EndStrike() = 0;                                                           // 42
    virtual int32_t GetStrikeStartDate() = 0;                                               // 43
    virtual int32_t GetStrikeLength() = 0;                                                  // 44
    virtual SC4Percentage* GetCurrentPlantEfficiencyPercent(SC4Percentage* result,
                                                            cISC4Occupant* occupant) = 0;  // 45  (SDK: float(occ))
    virtual SC4Percentage* GetCurrentPlantConditionPercent(SC4Percentage* result,
                                                           cISC4Occupant* occupant) = 0;   // 46  (SDK: float(occ))
    virtual SC4Percentage* GetStrikeEfficiencyPercent() = 0;                                // 47
    virtual SC4Percentage* GetFundingPercent() = 0;                                         // 48
    virtual bool GetMonthlyPowerGenerationCost(cISC4Occupant* occupant, int64_t& cost) = 0;  // 49
    virtual bool GetPowerPlantFundingPercentage(cISC4Occupant* occupant, SC4Percentage& funding) = 0;  // 50
    virtual bool SetPowerPlantFundingPercentage(cISC4Occupant* occupant, SC4Percentage const& funding) = 0;  // 51
};

// cISC4PowerPoleOccupant (IID 0xA8C86142), 19 slots. There is no SDK
// declaration: the slots the power simulator calls are named after power
// §11.1, the others are placeholders with the table's argument bytes and
// return channel (never called).
class cFC4ISC4PowerPoleOccupant : public cIGZUnknown {
public:
    virtual cISC4Occupant* AsOccupant() = 0;                      // 3
    virtual uint32_t PoleSlot04() = 0;                            // 4
    virtual uint32_t PoleSlot05(uint32_t a, uint32_t b) = 0;      // 5
    virtual bool BreakAllConnections(bool flag) = 0;              // 6
    virtual uint32_t PoleSlot07(uint32_t a) = 0;                  // 7
    virtual bool GetCell(int32_t& x, int32_t& z) = 0;             // 8
    virtual uint32_t PoleSlot09() = 0;                            // 9
    virtual int64_t PoleSlot10() = 0;                             // 10  (edx:eax)
    virtual bool IsDistressed() = 0;                              // 11  (EAX 0/1)
    virtual bool SetDistressed(bool on) = 0;                      // 12
    virtual bool IsIgnoredByPower() = 0;                          // 13  (power §13 question 1)
    virtual uint32_t PoleSlot14(uint32_t a) = 0;                  // 14
    virtual uint32_t PoleSlot15(uint32_t a, uint32_t b) = 0;      // 15  Init
    virtual uint32_t PoleSlot16(uint32_t a, uint32_t b) = 0;      // 16
    virtual uint32_t PoleSlot17(uint32_t a, uint32_t b, uint32_t c, uint32_t d, uint32_t e) = 0;  // 17
    virtual uint32_t PoleSlot18(uint32_t a) = 0;                  // 18  ChangeZoomLevel
};

// The rectangle UpdateOnZoneChange takes by value: the four int32 of the
// zoning message's void2, passed through in their order.
struct FC4Int32Quad {
    int32_t v[4];
};

// cISC4NetworkTool as the power-line tool's vtable has it (power §11.1), slots
// 3-10 only (the table has 83; we call 3, 4 and 10). SDK error fixed: slot 10
// UpdateOnZoneChange takes a rectangle by value and a uint32 (pops 20; SDK: none).
class cFC4ISC4NetworkTool : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                     // 3
    virtual bool Shutdown() = 0;                                                 // 4
    virtual int64_t ToolSlot05() = 0;                                            // 5  Reset
    virtual uint32_t ToolSlot06(uint32_t a) = 0;                                 // 6
    virtual uint32_t ToolSlot07(uint32_t a, uint32_t b, uint32_t c, uint32_t d, uint32_t e, uint32_t f, uint32_t g,
                                uint32_t h) = 0;                                 // 7  PlaceNetworkOccupants
    virtual uint32_t ToolSlot08(uint32_t a, uint32_t b, uint32_t c, uint32_t d, uint32_t e) = 0;  // 8
    virtual uint32_t ToolSlot09(uint32_t a, uint32_t b, uint32_t c, uint32_t d) = 0;  // 9
    virtual void UpdateOnZoneChange(FC4Int32Quad rect, uint32_t data3) = 0;      // 10
};

// cISC4NetworkTool in full compiled order (interfaces.tsv, base vtable
// 0x00AA8748, slots 0-50; glue29 implements it as cFC4NetworkTool). The SDK's
// declaration matches the compiled order slot for slot except:
//   - slot 10 UpdateOnZoneChange: the game takes (SC4Rect by value, uint32) and
//     pops 20 (foundations.md M3 errata; the power-line override 0x006522F0
//     pops 20 too). The SDK declares no parameters — a game caller pushing the
//     rectangle through an SDK-typed pointer would unbalance its stack.
//   - slot 33 GetBridgeAltitudeOverride returns a float in ST0
//     (network-bridges.md 1.1; SDK: void).
//   - slot 35 GetBridgeName takes (index, string) (network-bridges.md 1.1; the
//     SDK declares (string, index); pop 8 either way).
// Slots 16/17: the SDK header says the Windows binary has ClearOccupant-
// Highlights before ClearCellPreviews; network-geometry.md §1.1's Mac-name note
// puts them the other way round (question NT15). Both pop 0 and the replacement
// no-ops them, so the layout question only names the bodies.
// The compiled GetFailureReasonString of network-tools.md §14 (slot 52, pop 8)
// is the tool's own class method; the interface's slot 15 is the SDK's
// GetFailureReasonString(cIGZString&) (pop 4, unnamed in the table). Slots
// 51-80 are cSC4NetworkTool's own class methods past the interface
// (network-geometry.md §1.1): not declared here — the concrete replacement is
// final and nothing outside the tool's own class calls them in the specs (a
// kReviewedSlotCounts entry covers the shorter declaration), [choice] like the
// lot developer's class slots.
class NeighborConnectionInfo;
struct tPlacementParams;
class cISC4NetworkToolUI;
class cFC4ISC4NetworkToolFull : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                     // 3
    virtual bool Shutdown() = 0;                                                 // 4
    virtual void Reset() = 0;                                                    // 5
    virtual uint32_t GetUITool(cISC4NetworkToolUI** outUi) = 0;                  // 6  (4)
    virtual uint32_t PlaceNetworkOccupants(uint32_t startX, uint32_t startZ, uint32_t endX, uint32_t endZ,
                                           unsigned long networkMask, cFC4ISC4NetworkToolFull* toolCallback,
                                           bool bUseWorldCache,
                                           cISC4NetworkOccupant::eNetworkType networkType) = 0;  // 7  (32)
    virtual void PlaceNetworkOccupantById(int x, int z, unsigned long occupantId, bool bSelect,
                                          cISC4NetworkOccupant::eNetworkType networkType) = 0;  // 8  (20)
    virtual uint32_t OccupantExists(uint32_t x, uint32_t z, unsigned long networkMask,
                                    unsigned long& outOccupantId) = 0;           // 9  (16)
    virtual void UpdateOnZoneChange(FC4Int32Quad rect, uint32_t data3) = 0;      // 10 (20; SDK: ())
    virtual void ForceReload() = 0;                                              // 11 (0)
    virtual void SetTraceFinal(bool enable) = 0;                                 // 12 (4)
    virtual void ClearFailureReason() = 0;                                       // 13 (0)
    virtual uint32_t GetFailureState() const = 0;                                // 14 (0)
    virtual void GetFailureReasonString(cIGZString& outStr) = 0;                 // 15 (4; unnamed in the table)
    virtual void ClearOccupantHighlights() = 0;                                  // 16 (0)
    virtual void ClearCellPreviews() = 0;                                        // 17 (0)
    virtual uint32_t DrawNetworkLine(const SC4Point<uint32_t>& start, const SC4Point<uint32_t>& end, bool useWorldCache,
                                     cISC4NetworkOccupant::eNetworkType networkType) = 0;       // 18 (16)
    virtual void PlaceAllSegments(bool commit, bool previewOnly, bool allowDemolition,
                                  cISC4NetworkOccupant::eNetworkType networkType) = 0;          // 19 (16)
    virtual int GetCostOfSolution() const = 0;                                   // 20 (0)
    virtual void EnablePreviewUpdate(bool enable) = 0;                           // 21 (4)
    virtual void SetAnchorPoints(const SC4Vector<SC4Point<uint32_t>>& points) = 0;  // 22 (4)
    virtual void GetDraggedCells(SC4Vector<SC4Point<uint32_t>>& outCells) const = 0;  // 23 (4)
    virtual void SetValidStepCount(uint32_t count) = 0;                          // 24 (4)
    virtual uint32_t GetValidStepCount() const = 0;                              // 25 (0)
    virtual uint32_t GetStraightStepCount() const = 0;                           // 26 (0)
    virtual uint32_t GetTotalStepCount() const = 0;                              // 27 (0)
    virtual bool IsRegularStep(uint32_t stepIndex) const = 0;                    // 28 (4)
    virtual void GetLastValidPoint(SC4Point<uint32_t>& outPoint) const = 0;      // 29 (4)
    virtual bool GetPossibleBridges(SC4Vector<int32_t>& outBridgeInstanceIds) const = 0;  // 30 (4)
    virtual int GetPreferredBridge() const = 0;                                  // 31 (0)
    virtual void SetPreferredBridge(int bridgeId) = 0;                           // 32 (4)
    virtual float GetBridgeAltitudeOverride() = 0;                               // 33 (0, ST0; SDK: void)
    virtual void SetBridgeAltitudeOverride(float altitude) = 0;                  // 34 (4)
    virtual void GetBridgeName(int bridgeIndex, cIGZString& outStr) = 0;         // 35 (8; SDK: (string, index))
    virtual int GetMajorBridge() const = 0;                                      // 36 (0)
    virtual bool GetMajorBridgeAltitude(float& outAltitude) const = 0;           // 37 (4)
    virtual bool GetMajorBridgeReferenceAltitude(float& outAltitude) const = 0;  // 38 (4)
    virtual bool IsMajorBridgeLandBridge() const = 0;                            // 39 (0)
    virtual bool GetBridgeBitmapKey(long bridgeId, unsigned long& outGroup, unsigned long& outInstance) const = 0;  // 40 (12)
    virtual bool GetPossibleNeighborConnection(NeighborConnectionInfo& outInfo) const = 0;  // 41 (4)
    virtual uint32_t ViewInputControlId() const = 0;                             // 42 (0)
    virtual uint32_t CursorId() const = 0;                                       // 43 (0)
    virtual uint32_t DirectionalCursorBase() const = 0;                          // 44 (0)
    virtual void PenWidth(int& outWidthX, int& outWidthZ) const = 0;             // 45 (8)
    virtual uint32_t StopPickAtWater() const = 0;                                // 46 (0)
    virtual uint32_t GetToolViewId() const = 0;                                  // 47 (0)
    virtual void GetPlacementParams(tPlacementParams& outParams) const = 0;      // 48 (4)
    virtual void QueryUserForPotentialIntersections() = 0;                       // 49 (0)
    virtual void DestroyOccupantsAndResolve(const SC4Vector<cISC4NetworkOccupant*>& occupants) = 0;  // 50 (4)
};

class cISC4Simulator;
class cISC4BudgetSimulator;
class cISC4Ordinance;
class cISC4ResidentialSimulator;
class cISC4RegionalCity;
class cISC4PlumbingSimulator;
class cISC4PowerSimulator;

// ---- plumbing (docs/specs/plumbing.md §10.1) -----------------------------------------------------

// cISC4PlumbingSimulator (IID 0x47782CCC), 29 interface slots + destructor.
// There are no overloads, so the SDK's declaration order is already the
// compiled one; this declaration exists for the five return types the SDK gets
// wrong (§10.4) and so abi_conformance_test checks the implemented slots.
// Slots 14-17 and 23 return only the low 32 bits in eax (EDX is not set), the
// same [choice] as the budget's slots 20-23.
class cFC4ISC4PlumbingSimulator : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                               // 3
    virtual bool Shutdown() = 0;                                                           // 4
    virtual uint8_t GetSupplyValue(int32_t x, int32_t z) = 0;                              // 5
    // 6: no meaningful return (EAX = the grid pointer); a replacement returns true.
    virtual bool GetSupplyValueMapPtr(cISC4SimGrid<uint8_t>*& grid) = 0;                   // 6
    virtual cISC4SimGrid<uint8_t>* GetWateredGrid() = 0;                                   // 7
    virtual bool AreWateredBuildingsInCell(int32_t x, int32_t z) = 0;                       // 8  full EAX
    virtual bool IsBuildingWatered(cISC4Occupant* occupant) = 0;                            // 9
    virtual uint32_t GetWaterProduction(cISC4Occupant* occupant) = 0;                       // 10
    virtual bool IsCellWatered(int32_t x, int32_t z) = 0;                                   // 11
    virtual uint32_t GetNumWaterSources() = 0;                                              // 12
    virtual uint32_t GetNumPollutedWaterSources() = 0;                                      // 13
    virtual int32_t GetWaterProductionCapacity() = 0;                                       // 14 (SDK: int64)
    virtual int32_t GetWaterConsumed() = 0;                                                 // 15 (SDK: int64)
    virtual int32_t GetWaterImported() = 0;                                                 // 16 (SDK: int64)
    virtual int32_t GetWaterExported() = 0;                                                 // 17 (SDK: int64)
    virtual uint32_t GetNumPipes() = 0;                                                     // 18
    virtual uint32_t GetNumDistressedPipes() = 0;                                           // 19
    virtual uint32_t GetNumBurstPipes() = 0;                                                // 20
    virtual uint32_t GetNumWateredBuildings() = 0;                                          // 21
    virtual uint32_t GetNumUnwateredBuildings() = 0;                                        // 22
    virtual int32_t GetWaterDemand() = 0;                                                   // 23 (SDK: int64)
    virtual bool GetWaterProducerInfo(cISC4Occupant* producer, uint32_t* currentCapacity, uint32_t* maxCapacity,
                                      SC4Percentage* efficiency, bool* shutDownByPollution, uint32_t* waterUsed) = 0;  // 24
    virtual bool GetWaterProducerConditionPercent(cISC4Occupant* producer, SC4Percentage& conditionPercent) = 0;  // 25
    virtual bool GetOccupantMonthlyUpkeepCost(cISC4Occupant* occupant, int64_t& monthlyUpkeepCost) = 0;  // 26
    // 27: no meaningful return; a replacement returns true. Occupants written are AddRef'd.
    virtual bool FindWaterBuildings(cISC4Occupant** lowestFunded, SC4Percentage& funding, cISC4Occupant** worstCondition,
                                    long& conditionPercent) = 0;                            // 27
    // [abi] 28: ilist<cISC4Occupant*> with the framework allocator (A, allocators.md
    // table (2)): SC4List is compatible; appends, one reference each. No meaningful
    // return; a replacement returns true.
    virtual bool GetWaterProducers(SC4List<cISC4Occupant*>& list) = 0;                        // 28
};

// ---- pollution (docs/specs/pollution.md §8) -------------------------------------------------------

class cISC4PollutionSimulator;
class cS3DVector3;

// cISC4PollutionSimulator (IID 0x877181EC), 71 slots (§8.1). There are no
// overloads, so the SDK's declaration order is the compiled one; this
// declaration fixes the return types §8.1 marks "SDK": 29 / 30 return the
// applied delta / the argument in EAX (SDK: bool), 33 a pointer to an
// SC4Percentage (SDK: int32), 34-41 64-bit values in EDX:EAX (SDK: int32),
// 51 the new landfilled total (SDK: int64, "value meaningless"; docs/abi says
// eax: waived in abi_conformance_test), 65 a signed int64 (SDK: uint64&), and
// 66's arguments are (position, centre strength, radius in tracts). Slots 7-9,
// 17-19, 25-28, 46 and 50 return nothing meaningful in the original (the SDK
// says bool); a replacement returns true. Slot 55 returns AL (§8.1; docs/abi
// says x87: waived, spec question).
class cFC4ISC4PollutionSimulator : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                                // 3
    virtual bool Shutdown() = 0;                                                            // 4
    virtual cISC4SimGrid<int16_t>* GetPollutionGrid(uint32_t type) = 0;                    // 5
    virtual uint16_t GetPollutionValue(uint32_t type, uint32_t cellX, uint32_t cellZ) = 0;  // 6  (AX only)
    virtual bool GetAirValue(uint32_t cellX, uint32_t cellZ, int32_t& value) = 0;           // 7
    virtual bool GetWaterValue(uint32_t cellX, uint32_t cellZ, int32_t& value) = 0;         // 8
    virtual bool GetGarbageValue(uint32_t cellX, uint32_t cellZ, int32_t& value) = 0;       // 9
    virtual int32_t GetAverageAirValue() = 0;                                               // 10
    virtual int32_t GetAverageWaterValue() = 0;                                             // 11
    virtual int32_t GetAverageGarbageValue() = 0;                                           // 12
    virtual bool IsAirPolluted(uint32_t cellX, uint32_t cellZ) = 0;                         // 13
    virtual bool IsWaterPolluted(uint32_t cellX, uint32_t cellZ) = 0;                       // 14
    virtual bool IsGarbageAccumulated(uint32_t cellX, uint32_t cellZ) = 0;                  // 15
    virtual bool IsRadioactive(uint32_t cellX, uint32_t cellZ) = 0;                         // 16
    virtual bool GetWorstAirTract(uint32_t& cellX, uint32_t& cellZ, int32_t& value) = 0;    // 17
    virtual bool GetWorstWaterTract(uint32_t& cellX, uint32_t& cellZ, int32_t& value) = 0;  // 18
    virtual bool GetWorstGarbageTract(uint32_t& cellX, uint32_t& cellZ, int32_t& value) = 0;  // 19
    virtual uint32_t GetGarbageSupply() = 0;                                                // 20
    virtual int32_t GetLandfillTileCapacity() = 0;                                          // 21
    virtual int32_t GetGarbageCapacity() = 0;                                               // 22
    virtual bool IsGarbageUncollected() = 0;                                                // 23
    virtual int32_t GetTotalGarbageUncollected() = 0;                                       // 24
    virtual bool SetIsRadioactive(uint32_t cellX, uint32_t cellZ, bool radioactive) = 0;    // 25
    virtual bool SetAirValue(uint32_t cellX, uint32_t cellZ, int16_t value) = 0;            // 26
    virtual bool SetWaterValue(uint32_t cellX, uint32_t cellZ, int16_t value) = 0;          // 27
    virtual bool SetGarbageValue(uint32_t cellX, uint32_t cellZ, int16_t value) = 0;        // 28
    virtual int32_t AddToGarbageSupply(int32_t amount) = 0;                                 // 29  (SDK: bool)
    virtual int32_t SetGarbageCapacity(int32_t capacity) = 0;                               // 30  (SDK: bool)
    virtual int32_t GetMonthlyIncineratorCapacity() = 0;                                    // 31
    virtual int32_t GetMonthlyWasteToEnergyCapacity() = 0;                                  // 32
    virtual SC4Percentage* GetRecyclingCenterEffect() = 0;                                  // 33  (SDK: int32)
    virtual int64_t GetGarbageAbductedPrevMonth() = 0;                                      // 34  (SDK: int32)
    virtual int64_t GetGarbageProducedLastMonth() = 0;                                      // 35  (SDK: int32)
    virtual int64_t GetGarbageImportedLastMonth() = 0;                                      // 36  (SDK: int32)
    virtual int64_t GetGarbageExportedLastMonth() = 0;                                      // 37  (SDK: int32)
    virtual int64_t GetGarbageRecycledLastMonth() = 0;                                      // 38  (SDK: int32)
    virtual int64_t GetGarbageIncineratedLastMonth() = 0;                                   // 39  (SDK: int32)
    virtual int64_t GetGarbageConvertedToEnergyLastMonth() = 0;                             // 40  (SDK: int32)
    virtual int64_t GetGarbageLandfilledLastMonth() = 0;                                    // 41  (SDK: int32)
    virtual uint32_t GetIncineratorCount() = 0;                                             // 42
    virtual uint32_t GetWasteToEnergyBuildingCount() = 0;                                   // 43
    virtual uint32_t GetRecyclingCenterCount() = 0;                                         // 44
    virtual uint32_t GetWaterTreatmentPlantCount() = 0;                                     // 45
    // [abi] 46: ilist<cISC4Occupant*> with the framework allocator (A): SC4List is
    // compatible; appends, one reference each (as GetWaterProducers).
    virtual bool GetGarbageBuildings(SC4List<cISC4Occupant*>& list) = 0;                    // 46
    virtual uint32_t GetBuildingGarbageCapacity(cISC4Occupant* occupant) = 0;               // 47
    virtual float GetGarbageScalingFactor() = 0;                                            // 48
    virtual bool IsActiveLandfill(uint32_t cellX, uint32_t cellZ) = 0;                      // 49
    virtual bool SetIsActiveLandfill(uint32_t cellX, uint32_t cellZ, bool active) = 0;      // 50
    virtual int64_t GarbagePutInLandfill(int32_t amount) = 0;                               // 51
    virtual bool GetMaxCapacity(uint32_t property, cISC4Occupant* occupant, uint32_t& result) = 0;      // 52
    virtual bool GetCurrentCapacity(uint32_t property, cISC4Occupant* occupant, uint32_t& result) = 0;  // 53
    virtual bool GetRecyclingCenterMaxCapacity(cISC4Occupant* occupant, SC4Percentage& capacity) = 0;      // 54
    virtual bool GetRecyclingCenterCurrentCapacity(cISC4Occupant* occupant, SC4Percentage& capacity) = 0;  // 55
    virtual bool GetCurrentOccupantCondition(cISC4Occupant* occupant, SC4Percentage& condition) = 0;       // 56
    virtual bool GetCurrentOccupantEfficiency(cISC4Occupant* occupant, SC4Percentage& efficiency) = 0;     // 57
    virtual bool GetIncineratorUsedCapacity(cISC4Occupant* occupant, uint32_t& capacity) = 0;              // 58
    virtual bool GetWasteToEnergyUsedCapacity(cISC4Occupant* occupant, uint32_t& capacity) = 0;            // 59
    virtual bool GetIncineratorCurrentCapacity(cISC4Occupant* occupant, uint32_t& capacity) = 0;           // 60
    virtual bool GetWasteToEnergyCurrentCapacity(cISC4Occupant* occupant, uint32_t& capacity) = 0;         // 61
    virtual bool GetWaterTreatmentMaxCapacity(cISC4Occupant* occupant, uint32_t& capacity) = 0;            // 62
    virtual bool GetWaterTreatmentCurrentCapacity(cISC4Occupant* occupant, uint32_t& capacity) = 0;        // 63
    virtual bool GetWaterTreatmentUsedCapacity(cISC4Occupant* occupant, uint32_t& capacity) = 0;           // 64
    virtual bool CalcOccupantMaintenanceCost(cISC4Occupant* occupant, int64_t& cost) = 0;  // 65  (SDK: uint64&)
    virtual void AddRadiationToCity(cS3DVector3 const& position, float strength, float radius) = 0;  // 66
    virtual int32_t GetMaxAirPollutionLevelForUI() = 0;                                     // 67
    virtual int32_t GetMaxWaterPollutionLevelForUI() = 0;                                   // 68
    virtual int32_t GetMaxGarbagePollutionLevelForUI() = 0;                                 // 69
    virtual int32_t GetMaxRadiationPollutionLevelForUI() = 0;                               // 70
};

#ifndef FC4_HAVE_PROP_REQUESTER
#define FC4_HAVE_PROP_REQUESTER
// cISC4PropRequester (IID 0x89C498BE), 7 slots (pollution §8.4: there is no
// SDK header). Implemented by simulators that ask the prop developer for
// props (the pollution and crime simulators). SelectProp's vector is not
// read by the implementations here: a std::vector (B) as the game passes it.
class cFC4ISC4PropRequester : public cIGZUnknown {
public:
    virtual int32_t GetRequestedShare(SC4Rect<int32_t> const& cells) = 0;                   // 3
    virtual uint32_t GetRequesterSatisfactionId() = 0;                                      // 4
    virtual bool UsesCustomPropSelector() = 0;                                              // 5
    virtual uint32_t SelectProp(fc4::GameVector<uint32_t> const& candidates, int32_t a, int32_t b) = 0;  // 6
};
#endif

// ---- protection (docs/specs/protection.md §15) ---------------------------------------------------

class cISC4Automaton;
class cISC4DispatchManager;
class cISC4DispatchSource;
class cISC4FireProtectionSimulator;
class cISC4PoliceSimulator;
class cISC4PylonOccupant;

// cISC4FireProtectionSimulator (IID 0x88F54C7B), 26 slots in compiled order
// (§15.1). Implemented by cFC4FireProtectionSimulator. SDK errors fixed: slot 5
// is a uint8 grid; 10 and 12 return an SC4Percentage by value (written here as
// the hidden result pointer: pop 8 / 4, EAX = that pointer); 17 returns the
// number appended; 20 returns a pointer. [abi] The lists are ilist with the
// framework allocator (A, allocators.md): SC4List is compatible.
class cFC4ISC4FireProtectionSimulator : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                                // 3
    virtual bool Shutdown() = 0;                                                            // 4
    virtual cISC4SimGrid<uint8_t>* GetFireProtectionGrid() = 0;                             // 5  (SDK: int16)
    virtual cISC4DepartmentBudget* GetDepartmentBudget() = 0;                               // 6
    virtual bool IsFireStation(cISC4Occupant* occupant) = 0;                                // 7
    virtual bool GetStationFunding(cISC4Occupant* occupant, int32_t* cost, SC4Percentage* funding) = 0;  // 8
    virtual bool SetStationFunding(cISC4Occupant* occupant, SC4Percentage const& funding) = 0;  // 9
    virtual SC4Percentage* GetStationEfficiency(SC4Percentage* result, cISC4Occupant* occupant) = 0;  // 10 (SDK: float())
    virtual bool IsOnStrike() = 0;                                                          // 11
    virtual SC4Percentage* GetChanceOfStrike(SC4Percentage* result) = 0;                    // 12 (SDK: float)
    virtual bool BeginStrike(cISC4Occupant* station) = 0;                                   // 13
    virtual bool EndStrike() = 0;                                                           // 14
    virtual uint32_t GetStrikeStartDate() = 0;                                              // 15
    virtual uint32_t GetStrikeLength() = 0;                                                 // 16
    virtual int32_t GetStrikingStations(SC4List<cISC4Occupant*>& list) = 0;                 // 17 (SDK: bool)
    virtual bool IsStationOnStrike(cISC4Occupant* station) = 0;                             // 18
    virtual float GetAverageCoverage() = 0;                                                 // 19
    virtual SC4Percentage const* GetPercentCoverage() = 0;                                  // 20 (SDK: float)
    virtual bool GetOccupantCoverage(cISC4Occupant* station, SC4Percentage const& p, float& coverRadius,
                                     float& stationRadius) = 0;                             // 21
    virtual int32_t GetFireStations(SC4List<cISC4Occupant*>& list) = 0;                     // 22
    virtual uint32_t GetFireStationCount() = 0;                                             // 23
    virtual float GetStationDispatchRadius(cISC4Occupant* station) = 0;                     // 24
    virtual int32_t GetTotalDispatchCount() = 0;                                            // 25
};

// cISC4PoliceSimulator (IID 0x67659895), 45 slots in compiled order (§15.2).
// Implemented by cFC4PoliceSimulator. SDK errors fixed as for fire, plus: the
// two GetJailCapacity overloads are compiled at the first one's position in
// reverse order (28 = (jail), 29 = ()), which shifts the SDK's 28-34 by one;
// 27, 32 and 42 return int32 counts; 36 is an SC4Percentage by value; 37 takes
// the jail and returns int64; 39 and 40 take the jail.
class cFC4ISC4PoliceSimulator : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                                // 3
    virtual bool Shutdown() = 0;                                                            // 4
    virtual cISC4SimGrid<uint8_t>* GetPolicePowerGrid() = 0;                                // 5  (SDK: short)
    virtual cISC4DepartmentBudget* GetDepartmentBudget() = 0;                               // 6
    virtual bool GetStationFunding(cISC4Occupant* occupant, int32_t* cost, SC4Percentage* funding) = 0;  // 7
    virtual bool SetStationFunding(cISC4Occupant* occupant, SC4Percentage const& funding) = 0;  // 8
    virtual cISC4DepartmentBudget* GetJailDepartmentBudget() = 0;                           // 9
    virtual bool IsOnStrike() = 0;                                                          // 10
    virtual SC4Percentage* GetChanceOfStrike(SC4Percentage* result) = 0;                    // 11 (SDK: float)
    virtual bool BeginStrike(cISC4Occupant* station) = 0;                                   // 12
    virtual bool EndStrike() = 0;                                                           // 13
    virtual uint32_t GetStrikeStartDate() = 0;                                              // 14
    virtual uint32_t GetStrikeLength() = 0;                                                 // 15
    virtual int32_t GetStrikingStations(SC4List<cISC4Occupant*>& list) = 0;                 // 16 (SDK: bool)
    virtual bool IsStationOnStrike(cISC4Occupant* station) = 0;                             // 17
    virtual float GetAverageCoverage() = 0;                                                 // 18
    virtual SC4Percentage const* GetPercentCoverage() = 0;                                  // 19 (SDK: float)
    virtual bool GetOccupantCoverage(cISC4Occupant* station, SC4Percentage const& p, float& coverRadius,
                                     float& stationRadius) = 0;                             // 20
    virtual int32_t GetPoliceStations(SC4List<cISC4Occupant*>& list) = 0;                   // 21 (SDK: bool)
    virtual bool IsPoliceStation(cISC4Occupant* occupant) = 0;                              // 22
    virtual float GetStationDispatchRadius(cISC4Occupant* station) = 0;                     // 23
    virtual uint32_t GetPoliceStationCount() = 0;                                           // 24
    virtual SC4Percentage* GetStationEfficiency(SC4Percentage* result, cISC4Occupant* occupant) = 0;  // 25 (SDK: float())
    virtual int32_t GetTotalDispatchCount() = 0;                                            // 26
    virtual int32_t ResetUnjailedCriminals() = 0;                                           // 27 (SDK: bool)
    virtual uint32_t GetJailCapacityOf(cISC4Occupant* jail) = 0;                            // 28 GetJailCapacity(occ)
    virtual uint32_t GetJailCapacity() = 0;                                                 // 29 GetJailCapacity()
    virtual uint32_t GetJailPopulation() = 0;                                               // 30
    virtual uint32_t GetCriminalCount() = 0;                                                // 31
    virtual int32_t GetJails(SC4List<cISC4Occupant*>& list) = 0;                            // 32
    virtual bool IsJail(cISC4Occupant* occupant) = 0;                                       // 33
    virtual uint32_t GetArrestCount() = 0;                                                  // 34
    virtual uint32_t GetJailCount() = 0;                                                    // 35
    virtual SC4Percentage* GetJailedPercentage(SC4Percentage* result) = 0;                  // 36 (SDK: uint32)
    virtual int64_t GetJailCurrentUpkeepCost(cISC4Occupant* jail) = 0;                      // 37 (SDK: no argument)
    // 38: no meaningful return (§15.2); [choice] a replacement returns true.
    virtual bool ProcessMonthlyCrimeStats() = 0;                                            // 38
    virtual bool IsInJailbreak(cISC4Occupant* jail) = 0;                                    // 39 (SDK: no argument)
    virtual bool BeginJailbreak(cISC4Occupant* jail) = 0;                                   // 40 (SDK: float)
    virtual bool EndJailbreak(cISC4Occupant* jail) = 0;                                     // 41 (AL = 0 always)
    virtual int32_t GetJailbreakBuildings(SC4List<cISC4Occupant*>& list) = 0;               // 42 (SDK: bool)
    virtual uint32_t GetPoliceStationArrestCount(cISC4Occupant* station) = 0;               // 43
    virtual uint32_t GetPoliceStationCrimeCount(cISC4Occupant* station) = 0;                // 44
};

// The dispatch source (IID 0xC9C00435, no SDK header), 12 slots (§15.4). The
// fire and police simulators implement it on a sub-object; the dispatch
// manager calls it. Slots 3 and 6 put 0 / 1 in the whole of EAX.
class cFC4ISC4DispatchSource : public cIGZUnknown {
public:
    virtual uint32_t CanHandleDispatchType(uint32_t type) = 0;                              // 3  EAX 0/1
    virtual bool CanDispatch(float x, float z, bool* allBusy) = 0;                          // 4
    virtual int32_t GetMaxPossibleDispatchCount() = 0;                                      // 5
    virtual uint32_t GetStations(SC4List<cISC4Occupant*>& list) = 0;                        // 6  EAX 0/1
    virtual bool GetDispatchesAtStation(cISC4Occupant* station, uint32_t* flags, uint32_t* base,
                                        uint32_t* count) = 0;                               // 7
    virtual cISC4Occupant* DispatchFromNearestStation(float x, float z, uint32_t* id) = 0;  // 8  borrowed
    virtual bool DispatchFromStation(cISC4Occupant* station, uint32_t* id) = 0;             // 9
    virtual bool RevokeDispatchFromStation(cISC4Occupant* station, uint32_t id) = 0;        // 10
    virtual bool GetDispatchAutomataProperties(uint32_t a, uint32_t* b, float* c, uint32_t* d,
                                               uint32_t e) = 0;                             // 11
};

// cISC4DispatchManager's dispatch record (docs/abi/layouts.md, 0x2C bytes):
// GetDispatchInfo copies it without taking references. protection.md §5
// (PR1): +0x08 is the unit's call ID and +0x10 "active".
struct FC4DispatchInfo {
    int32_t type;                    // +0x00
    uint32_t id;                     // +0x04
    uint32_t callId;                 // +0x08  the unit's call ID, base + i (the station's vehicle slot)
    uint32_t viewHandle;             // +0x0C
    bool active;                     // +0x10  the dispatch state is 5 (posts active / inactive on a change)
    uint8_t pad[3];
    float x, y, z;                   // +0x14  the target position
    cISC4Occupant* station;          // +0x20
    cISC4Automaton* automaton;       // +0x24
    cISC4PylonOccupant* pylon;       // +0x28
};
static_assert(sizeof(void*) != 4 || sizeof(FC4DispatchInfo) == 0x2C, "DispatchInfo layout (layouts.md)");

// cISC4DispatchManager (cSC4DispatchManager vtable 0x00A88108, docs/abi
// vtables.tsv; 23 interface slots, slot 23 is the destructor). SDK error fixed:
// RevokeAllDispatches takes the type (pops 4; SDK: no argument). Slot 12's
// count is the capacity and is never written (allocators.md, Dispatch ID array).
class cFC4ISC4DispatchManager : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                                // 3
    virtual bool Shutdown() = 0;                                                            // 4
    virtual bool CanDispatch(int32_t type, float x, float z) = 0;                           // 5
    virtual uint32_t Dispatch(int32_t type, float x, float z) = 0;                          // 6
    virtual bool RevokeDispatchByID(uint32_t id) = 0;                                       // 7
    virtual bool RevokeDispatchByPylon(cISC4PylonOccupant* pylon) = 0;                      // 8
    virtual bool RevokeAllDispatches(int32_t type) = 0;                                     // 9
    virtual bool GetDispatchInfo(uint32_t id, FC4DispatchInfo& info) = 0;                   // 10
    virtual uint32_t GetDispatchCount(int32_t type) = 0;                                    // 11
    virtual uint32_t GetDispatchIDArray(int32_t type, uint32_t* ids, uint32_t& capacity) = 0;  // 12
    virtual bool RegisterDispatchSource(cISC4DispatchSource* source) = 0;                   // 13
    virtual bool UnregisterDispatchSource(cISC4DispatchSource* source) = 0;                 // 14
    virtual cISC4DispatchSource* GetDispatchSource(int32_t type) = 0;                       // 15
    virtual bool BeginDispatchTool(int32_t type) = 0;                                       // 16
    virtual bool EndDispatchTool(int32_t type) = 0;                                         // 17
    virtual cISC4PylonOccupant* GetActivePylon() = 0;                                       // 18
    virtual void SetActivePylon(cISC4PylonOccupant* pylon) = 0;                             // 19
    virtual cISC4Automaton* GetAutomatonFromPylon(cISC4PylonOccupant* pylon) = 0;           // 20
    virtual uint32_t GetAvailablePylonCount(int32_t type) = 0;                              // 21
    virtual void* GetDispatchVehicleView() = 0;                                             // 22
};

// ---- demolition utility (docs/specs/demolition.md §8) --------------------------------------------------

class cISC4Demolition;        // the SDK's travel type (cISC4City slot 31 returns it)
class cISC4OccupantFilter;
template <typename T> class SC4CellRegion;
template <typename T> class SC4List;

// cISC4Demolition (IID 0x26DDE2EA, cSC4Demolition vtable 0x00A88004, 13 slots).
// docs/specs/demolition.md §8. SDK errors fixed: slot 9 returns the count of
// differing vertices in EAX (SDK: void) and its `cost` out pointer is never
// written (DM2); slots 10 and 11 are swapped against the decomp's names (slot
// 10 is the occupant effect, slot 11 the group effect). Slot 5 does nothing.
// Container arguments are B-side STLport containers (allocators.md): the `set`
// arguments are std::set<cRZAutoRefCount<cISC4Occupant>> and DemolishLots'
// list is std::list<cRZAutoRefCount<cISC4Lot>>.
class cFC4ISC4Demolition : public cIGZUnknown {
public:
    // [abi] Slots 3 and 4 are Init and Shutdown (docs/abi/interfaces.tsv,
    // demolition.md §4.1); without them every later slot sits two places
    // early in the vtable. Declared here so the implementing class's table
    // matches the game's.
    virtual bool Init() = 0;                                               // 3  [abi-ret] eax-or-void
    virtual bool Shutdown() = 0;                                           // 4  [abi-ret] eax-or-void
    virtual void SetDefaultOccupantFilter(cISC4OccupantFilter* filter) = 0;  // 5  does nothing
    // `demolished` is the caller's std::set<cRZAutoRefCount<cISC4Occupant>>*
    // (demolition.md §8 "Container arguments"); the SDK types it intptr_t.
    virtual bool DemolishRegion(bool doIt, SC4CellRegion<int32_t> const& region, int32_t privilege, uint32_t flags,
                                bool dezone, cISC4OccupantFilter* filter, int64_t* cost, intptr_t demolished,
                                cISC4Occupant* clicked, long x, long y) = 0;                          // 6
    // `lots` is std::list<cRZAutoRefCount<cISC4Lot>> const&.
    virtual bool DemolishLots(bool doIt, SC4List<cISC4Lot*> const& lots, int32_t privilege, uint32_t flags,
                              bool dezone, cISC4OccupantFilter* filter, int64_t* cost,
                              intptr_t demolished) = 0;                                               // 7
    // `orphans` is the allocator-service list<cISC4Occupant*>*; the sixth
    // argument is the effects flag of demolition.md §8 (SDK: `excludeNetworks`).
    virtual bool DemolishOccupant(bool doIt, cISC4Occupant* occ, int32_t privilege, uint32_t flags, int64_t* cost,
                                  bool effects, SC4List<cISC4Occupant*>* orphans) = 0;                // 8
    // Returns the number of differing vertices (EAX); `cost` is never written
    // (demolition.md §4.7, DM2).
    virtual int32_t ModifyTerrainHeight(bool doIt, bool perVertex, int32_t x0, int32_t z0, int32_t x1, int32_t z1,
                                        float h, cISC4Lot* exempt, uint32_t reason, int64_t* cost) = 0;  // 9
    virtual void DoDemolishOccupantEffect(cISC4Occupant* occ, uint32_t flags, cISC4Lot* lot, bool clicked,
                                          long x, long y) = 0;                                        // 10
    virtual void DoDemolishGroupEffect(uint32_t cause, int32_t x, int32_t z,
                                       SC4Rect<int32_t> const& r) = 0;                                 // 11
};

// ---- city situation manager (docs/specs/city-situation.md §8) ----------------------------------------

class cISC4CitySituationManager;  // the SDK's travel type (cISC4City slot 49 returns it)
class cISC4CitySituationAdvice;
class cISC4Automaton;
class cIGZString;

// cISC4CitySituationManager (IID 0x6BB1437A, cSC4CitySituationManager vtable
// 0x00A97E58, 40 slots; slot 34 is the deleting destructor). docs/specs/
// city-situation.md §8. Slots 13-15 return `float` in ST0 (the §8 note: a
// single `fld` of a member or a difference of two); declaring them void or
// int leaves a value on the x87 stack (docs/abi/README.md). Slot 5 returns
// only AX. Slot 24's result is whatever the call leaves (EAX).
class cFC4ISC4CitySituationManager : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                                       // 3
    virtual bool Shutdown() = 0;                                                                   // 4
    virtual uint16_t GetMissionSuccessCount(uint32_t guid) = 0;                                    // 5  AX
    virtual cISC4CitySituationAdvice* GetActiveSituation() = 0;                                    // 6
    virtual uint32_t GetCurrentStatus() = 0;                                                       // 7
    virtual cISC4Automaton* GetActiveAuto() = 0;                                                   // 8
    virtual cISC4Occupant* GetCurrentTarget() = 0;                                                 // 9
    virtual uint32_t GetCurrentTargetIndex() = 0;                                                  // 10
    virtual uint32_t GetNumTargets() = 0;                                                          // 11
    virtual uint32_t GetNumTargetsRemaining() = 0;                                                 // 12
    virtual float GetCurrentTimeLimitSeconds() = 0;                                                // 13 ST0
    virtual float GetCurrentSituationTime() = 0;                                                   // 14 ST0
    virtual float GetCurrentTimeRemainingSeconds() = 0;                                            // 15 ST0
    virtual cIGZString& GetCurrentTargetName(cIGZString& out) = 0;                                 // 16 EAX = &out
    virtual bool GetSituationAura(float& radius, float& magnitude) = 0;                            // 17
    virtual bool GetSituationMoney(int64_t& money) = 0;                                            // 18
    virtual bool IsSituationAvailable(cISC4Automaton* automaton) = 0;                              // 19
    virtual bool IsSituationAvailable(cISC4Occupant* occupant) = 0;                                // 20 overload
    virtual bool InitiateSituationByGuid(uint32_t guid, bool showBriefing, uint32_t group) = 0;     // 21
    virtual bool InitiateSituationForOccupant(cISC4Occupant* occ, cISC4CitySituationAdvice* a) = 0;  // 22
    virtual bool InitiateSituationByAutomataGroup(uint32_t group, bool showBriefing, bool search) = 0;  // 23
    virtual void StartJoyride(cISC4Automaton* automaton, bool b, cIGZMessageTarget2* target) = 0;   // 24
    virtual void TreatAutomatonAsGroup(cISC4Automaton* automaton, uint32_t group) = 0;              // 25
    virtual void SetMaxCSI(uint8_t n) = 0;                                                         // 26
    virtual uint8_t GetMaxCSI() = 0;                                                               // 27
    virtual int32_t GetTrackedAutomataCount() = 0;                                                 // 28
    virtual void SetCSIVisible(bool visible) = 0;                                                  // 29
    virtual bool GetCSIVisible() = 0;                                                              // 30
    virtual bool HandleTuningParametersChanged() = 0;                                              // 31
    virtual uint32_t CountdownIsInProgress() = 0;  // 32 EAX 0 or 1 (city-situation.md §8)
    virtual void ResetTutorials() = 0;                                                             // 33
    // 34: the deleting destructor (not in the SDK)
    virtual void InitiatePlayerDriveMode(bool on) = 0;                                             // 35
    virtual bool InitiateSituation(cISC4CitySituationAdvice* advice, cISC4Automaton* automaton,
                                  bool showBriefing) = 0;                                          // 36
    virtual void ActivateSituation() = 0;                                                          // 37
    virtual void DeactivateSituation() = 0;                                                        // 38
    virtual void EndSituation() = 0;                                                               // 39
};

// ---- landfill (docs/specs/landfill.md §7.1, §8) -------------------------------------------------------

class cISC4LandfillDeveloper;

// cISC4LandfillDeveloper (IID 0x8A09D502, vtable 0x00AB157C, 13 slots; §7.1).
// SDK errors fixed: slot 5 returns the SC4Percentage through a hidden result
// pointer (EAX = that pointer, pops 4; §7.4), slot 9 returns an int64 in
// EDX:EAX (SDK: int32; the zone manager reads EDX:EAX, §9).
class cFC4ISC4LandfillDeveloper : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                  // 3
    virtual bool Shutdown() = 0;                                              // 4
    virtual SC4Percentage* GetEfficiency(SC4Percentage* result) = 0;          // 5
    virtual uint32_t GetSimulatorType() = 0;                                  // 6
    virtual int64_t GetMaintenanceCosts() = 0;                                // 7
    virtual int64_t GetCurrentBudget() = 0;                                   // 8
    virtual int64_t GetZoningCost(SC4CellRegion<int32_t> const* region) = 0;  // 9
    virtual void GetLandfillUsage(uint32_t& usage, uint32_t& capacity) = 0;   // 10
    virtual int32_t GetLandfillTileCount() const = 0;                         // 11
    // 12: a cSC4LandfillZone* (§9: callers read its memory) or null.
    virtual intptr_t GetLandfillZoneFromCell(int32_t x, int32_t z) = 0;       // 12
};

// cISC4TrafficNetworkMap (cSC4TrafficNetworkMap, vtable 0x00A8FED8, 52 slots;
// not in the SDK). The city's slot 40 (GetTrafficNetwork, SDK intptr_t)
// returns it (landfill §8). Declared up to slot 8, the one the landfill
// developer calls (§4.4.5); slots 5-7 by their argument size, never called.
class cFC4ISC4TrafficNetworkMap : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                       // 3
    virtual bool Shutdown() = 0;                                                   // 4
    virtual void HandleTuningParametersChanged() = 0;                              // 5
    virtual intptr_t ChooseWeightedEdge(intptr_t a, intptr_t b) = 0;               // 6 (pops 8)
    virtual intptr_t NetworkMapSlot7(intptr_t a, intptr_t b, intptr_t c) = 0;      // 7 (pops 12)
    // 8: the network tile at the cell whose flags match `flagMask` / `flag`,
    // or null (a pointer into the map; no reference).
    virtual void* FindNetworkTile(int32_t x, int32_t z, uint32_t flagMask, bool flag) = 0;  // 8
    virtual intptr_t NetworkMapSlot9(intptr_t a) = 0;                               // 9
    virtual intptr_t NetworkMapSlot10(intptr_t a, intptr_t b, intptr_t c, intptr_t d) = 0; // 10
    // regional-city-records §4.6 RR3: nonzero for any traffic-map entry at x,z.
    virtual uint32_t AnyNetworkAt(int32_t x, int32_t z) = 0;                       // 11
};

// ---- zone manager (docs/specs/zones.md §11.1) ----------------------------------------------------

// cISC4ZoneManager as compiled (IID 0x67652433, vtable 0x00A92E00, 21 slots).
// SDK errors fixed: slots 7 and 8 (the overloads compile in reverse order),
// slot 9's parameters (zone, result), slot 11's argument 8 (the int64 cost)
// and 10 (a std::set<cISC4Lot*>*, allocators.md "map and set"), slot 12's
// return (the int8 zone type sign-extended in EAX), slot 13 (AL only) and
// slot 18 (void).
class cFC4ISC4ZoneManager : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                              // 3
    virtual bool Shutdown() = 0;                                                          // 4
    virtual cISC4SimGrid<int8_t>* GetZoneGrid() = 0;                                      // 5 (no AddRef)
    virtual uint32_t GetZoneCount(int32_t type) = 0;                                      // 6
    virtual uint32_t GetUndevelopedTileCountOfType(int32_t type) = 0;                     // 7
    virtual uint32_t GetUndevelopedTileCount() = 0;                                       // 8
    virtual uint32_t GetDevelopmentFailureCount(int32_t zone, int32_t result) = 0;        // 9
    virtual uint32_t GetAbandonedTileCount() = 0;                                         // 10
    virtual bool PlaceZone(SC4CellRegion<long>& region, int32_t type, bool commit, bool skipFunds, bool checkSlope,
                           bool skipSizeCheck, bool checkLotInside, int64_t* outCost, int32_t* outError,
                           void* outLots) = 0;                                            // 11
    virtual int32_t GetZoneType(int32_t x, int32_t z) = 0;                                // 12
    virtual bool IsRCIZone(int32_t type) = 0;                                             // 13
    virtual bool GetTextureForZone(int32_t type, cGZPersistResourceKey& key) = 0;         // 14
    virtual uint32_t GetZoneDragColor(int32_t type) = 0;                                  // 15
    virtual uint8_t GetZoneDisplayAlpha() = 0;                                            // 16
    virtual uint8_t GetDefaultZoneDisplayAlpha() = 0;                                     // 17
    virtual void SetZoneDisplayAlpha(uint8_t alpha) = 0;                                  // 18
    virtual int32_t GetMinZoneSize(int32_t type) = 0;                                     // 19
    virtual int32_t GetMaxZoneSize(int32_t type) = 0;                                     // 20
};

// ---- traffic (docs/specs/traffic-simulator.md §7, traffic-network.md §7.1, pathfinder.md §7) -------------

class cISC4Occupant;

// GetTrafficEdgeDensity's record (traffic-simulator.md §4.8, 0x42 bytes, read
// as 33 u16: usage at +0x20).
struct FC4TrafficEdgeDensity {
    uint16_t last[16];
    uint16_t usage;
    uint16_t current[16];
};
static_assert(sizeof(FC4TrafficEdgeDensity) == 0x42, "TrafficEdgeDensity is 0x42 bytes");

// GetTransitSwitchQueryData's out (traffic-simulator.md §4.16, five 32-bit fields).
struct FC4TransitSwitchQueryData {
    uint32_t available;
    uint32_t maxCapacity;
    float loadFraction;
    uint32_t usage;
    uint32_t cost;
};

// cISC4PathFinder (IID 0xAA0DB766, vtable 0x00AB2150, 34 slots; pathfinder.md
// §7). No SDK header exists. Slot 33 is the class's scalar deleting
// destructor (not part of the interface): an implementing class's virtual
// destructor lands there. Slots 25 and 26 return in ST0.
class cFC4ISC4PathFinder : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                     // 3
    virtual bool Shutdown() = 0;                                                 // 4
    virtual void SetFlag(uint32_t flags) = 0;                                    // 5
    virtual void ClearFlag(uint32_t flags) = 0;                                  // 6
    virtual void ClearAllFlags() = 0;                                            // 7
    // 8: a std::deque<RouteData>* of the caller (3-byte elements), or null.
    virtual void SetRouteInfo(void* routeDeque) = 0;                             // 8
    virtual void SetTripDestinationFilter(uint32_t filter) = 0;                  // 9
    virtual void SetDestinationFlag(uint8_t bits) = 0;                           // 10
    virtual void ClearDestinationFlag(uint8_t bits) = 0;                         // 11
    virtual void ClearAllDestinationFlags() = 0;                                 // 12
    virtual void SetTravelTypeFlag(uint16_t mask, bool alsoStarting) = 0;        // 13
    virtual void ClearTravelTypeFlag(uint16_t mask) = 0;                         // 14
    virtual void ClearAllTravelTypeFlags() = 0;                                  // 15
    virtual void SetStartingCostArray(float* costs9) = 0;                        // 16
    virtual void SetMaxTripLength(float length) = 0;                             // 17
    virtual void SetTrafficDensity(uint32_t trips) = 0;                          // 18
    virtual void SetStartRect(SC4Rect<int32_t> rect) = 0;                        // 19 (by value)
    virtual void SetEndRect(SC4Rect<int32_t> rect) = 0;                          // 20 (by value)
    virtual void SetMaxWorkTime(uint32_t ms) = 0;                                // 21
    virtual void Reset() = 0;                                                    // 22
    virtual uint32_t FindPath() = 0;                                             // 23
    virtual uint32_t GetTrafficArrived() = 0;                                    // 24
    virtual float GetTripLength() = 0;                                           // 25
    virtual float GetTripLengthScaledForDisplay() = 0;                           // 26
    virtual void GetEndCell(int32_t& x, int32_t& z) const = 0;                   // 27
    virtual uint8_t GetLastTravelType() const = 0;                               // 28
    virtual void ApplyTraffic(uint32_t style) = 0;                               // 29
    virtual bool WasTripReversible() = 0;                                        // 30
    virtual void ReleasePathNodes() = 0;                                         // 31
    // 32: the caller's std::vector<uint8_t> (B) and mask (allocators.md).
    virtual void GetCompressedRoute(fc4::GameVector<uint8_t>& out, uint32_t& mask) = 0;  // 32
};

// cISC4TrafficSimulator as compiled (IID 0x68027F37, vtable 0x00AB33D0, 44
// slots; traffic-simulator.md §7, traffic-network.md §7.1). SDK errors fixed:
// slots 11, 23 return in ST0; 19 and 26 return nothing (26 takes a pointer to
// an STL vector of floats, B); 22, 29 and 34 return pointers; 23 and 25 take a
// transit network 0-5; 28-31 and 39-42 fill STL containers (allocators.md),
// and a replacement returns true where the original returns garbage
// (traffic-network.md §7.1 [choice]). Slot 43 is the class's scalar deleting
// destructor (not part of the interface).
class cFC4ISC4TrafficSimulator : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                     // 3
    virtual bool Shutdown() = 0;                                                 // 4
    virtual uint32_t GetSimulatorType() = 0;                                     // 5
    virtual bool CreatePathFinder(cFC4ISC4PathFinder** out) = 0;                 // 6
    virtual bool SetupPathFinderForLot(cFC4ISC4PathFinder* pf, cISC4Lot* lot) = 0;  // 7
    virtual cISC4SimGrid<uint8_t>* GetAirPollutingTrafficMap() = 0;              // 8
    virtual cISC4SimGrid<uint8_t>* GetCommercialTrafficMap() = 0;                // 9
    virtual cISC4SimGrid<uint8_t>* GetCongestionMap() = 0;                       // 10
    virtual float GetBackgroundTraffic(int32_t x, int32_t z) = 0;                // 11
    virtual FC4TrafficEdgeDensity* GetTrafficEdgeDensity(uint8_t x, uint8_t z, uint32_t travelType,
                                                         bool evening) = 0;      // 12
    virtual cISC4SimGrid<uint8_t>* GetTripLengthMap() = 0;                       // 13
    virtual float GetTripScale() = 0;                                            // 14
    virtual float GetTripScaleForDisplay() = 0;                                  // 15
    virtual bool IsRoadDamaged(int32_t x, int32_t z) = 0;                        // 16
    virtual bool CheckRailAccident(int32_t x, int32_t z) = 0;                    // 17
    virtual float GetLastMonthlyIncome() = 0;                                    // 18
    virtual void SetMaxTripCapacity(cISCPropertyHolder* holder, uint32_t k, uint32_t value) = 0;  // 19
    virtual uint32_t GetMaxTripCapacity(cISCPropertyHolder* holder, uint32_t k) = 0;  // 20
    virtual uint32_t GetTrafficArrived(cISCPropertyHolder* holder, uint32_t k) = 0;   // 21
    // 22: the lot manager's safe lot iterator during the lot sweep, else null.
    virtual void* GetDesiredLotInsertionPoint() = 0;                             // 22
    virtual float GetCapacity(uint32_t transitNetwork, int32_t x, int32_t z) = 0;  // 23
    virtual float GetTravelTimeRatio(int32_t x, int32_t z, uint32_t travelType) = 0;  // 24
    virtual uint32_t GetConnectionCount(uint32_t transitNetwork, int32_t x, int32_t z) = 0;  // 25
    virtual void GetTravelStrategyPercentages(uint32_t wealth, fc4::GameVector<float>* out) = 0;  // 26
    virtual float GetFreightScalingFactor() = 0;                                 // 27
    // 28: ilist<cISC4Occupant*> (allocator A): SC4List's layout and allocator.
    virtual bool GetTransitSwitches(SC4List<cISC4Occupant*>& list) = 0;          // 28
    // 29: a pointer to a std::vector<uint8_t> the simulator owns, or null.
    virtual fc4::GameVector<uint8_t>* GetFerryRouteBetweenTiles(int32_t x1, int32_t z1, int32_t x2,
                                                                int32_t z2) = 0;  // 29
    // 30, 31: a std::list of vector<uint8_t> / of FerryRouteInfo (B nodes).
    virtual bool GetAllFerryRoutes(void* list) = 0;                              // 30
    virtual bool GetFerryRoutesInUse(void* list) = 0;                            // 31
    virtual uint32_t GetFerryTerminalCount(uint32_t type) = 0;                   // 32
    virtual bool GetWaterRoute(int32_t x1, int32_t z1, int32_t x2, int32_t z2,
                               fc4::GameVector<uint8_t>& out) = 0;               // 33
    // 34: the statistics block (FC4_TRAFFIC_STATS_BLOCK bytes, fc4.h).
    virtual const uint8_t* GetTrafficStats() = 0;                                // 34
    virtual bool GetTransitSwitchQueryData(uint32_t x, uint32_t z, FC4TransitSwitchQueryData& out) = 0;  // 35
    virtual bool AreLotsConnected(cISC4Lot* a, cISC4Lot* b) = 0;                 // 36
    virtual uint32_t GetConnectedOccupantCount(cISC4Lot* lot, uint32_t developerType) = 0;  // 37
    virtual uint32_t GetConnectedDestinationCount(cISC4Lot* lot, int32_t k) = 0;  // 38
    virtual bool GetSubnetworksForLot(cISC4Lot* lot, fc4::GameVector<uint32_t>& out) = 0;  // 39
    virtual bool GetSubnetworksInRectangle(SC4Rect<int32_t> const& rect, fc4::GameVector<uint32_t>& out) = 0;  // 40
    virtual bool GetSubnetworksInRegion(SC4CellRegion<long> const& region, fc4::GameVector<uint32_t>& out) = 0;  // 41
    virtual bool GetOccupantCountForAllSubnetworks(uint32_t developerType, fc4::GameVector<uint32_t>& out) = 0;  // 42
};

// ---- flora (docs/specs/flora.md §11.1) ------------------------------------------------------------

// cISC4FloraTypeFilter: caller-supplied (the SDK only forward-declares it).
// The flora simulator's slot 13 asks its slot 3 +0x0C about every type and
// calls no other slot (flora §11.1, FQ9; docs/abi: pops 4, AL).
class cISC4FloraTypeFilter;
class cFC4ISC4FloraTypeFilter : public cIGZUnknown {
public:
    virtual bool IsFloraTypeIncluded(uint32_t floraType) = 0;  // 3
};

// ---- history warehouse (docs/specs/history.md §8) ---------------------------------------------------

class cISC4HistoryWarehouse;

// cISC4HistoryWarehouse (IID 0x69F4A803, vtable 0x00A8CCB4, 14 slots; §8).
// SDK errors fixed (foundations §16): the two Dispose overloads are in the
// reverse of the SDK's order (slot 10 takes the key alone), and Coalesce
// (slot 9) takes two dates and the granularity. Slot 12 returns false (HW12:
// the original returns the final Release's EAX, 0; no caller reads it). Every
// slot returns in AL / EAX and pops its arguments (docs/abi).
class cFC4ISC4HistoryWarehouse : public cIGZUnknown {
public:
    virtual bool Init() = 0;      // 3
    virtual bool Shutdown() = 0;  // 4
    virtual bool GetCount(uint32_t key, cFC4IGZDate const& from, cFC4IGZDate const& to, float& n) = 0;  // 5
    virtual bool GetTrend(uint32_t key, cFC4IGZDate const& from, cFC4IGZDate const& to, float& slope,
                          float& intercept) = 0;  // 6
    virtual bool GetMinMaxAverage(uint32_t key, cFC4IGZDate const& from, cFC4IGZDate const& to, float& min,
                                  float& max, float& avg) = 0;  // 7
    virtual bool GetPoint(uint32_t key, cFC4IGZDate const& date, float& v) = 0;  // 8
    virtual bool Coalesce(uint32_t key, cFC4IGZDate const& from, cFC4IGZDate const& to,
                          int32_t granularity) = 0;  // 9
    virtual bool DisposeAll(uint32_t key) = 0;      // 10  Dispose(key)
    virtual bool DisposeRange(uint32_t key, cFC4IGZDate const& from, cFC4IGZDate const& to) = 0;  // 11  Dispose(key, from, to)
    virtual bool RecordHistoryData(uint32_t key, int32_t value) = 0;           // 12
    virtual bool SetHistoryDataQueueLength(uint32_t key, uint32_t n) = 0;      // 13
};

// ---- 24-hour clock (docs/specs/clock24.md §8) ------------------------------------------------------

class cISC424HourClock;

// cISC424HourClock (IID 0x681BD53A, vtable 0x00A86588, 30 slots; §8). SDK
// errors fixed (the game's form is binding): slots 7 and 9 take a float* out
// and return nothing (SDK: float(void)); slot 19 takes a dword index (SDK:
// bool); slot 20 returns the float's bits in EAX. Every slot returns in AL /
// EAX and pops its arguments (docs/abi); none returns in ST0.
class cFC4ISC424HourClock : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                      // 3
    virtual bool Shutdown() = 0;                                                  // 4
    virtual uint32_t SetVisualTimeMode(int32_t mode) = 0;                         // 5  (no meaningful value)
    virtual int32_t GetVisualTimeMode() = 0;                                      // 6
    virtual void GetTimePercentage(float* out) = 0;                               // 7
    virtual uint32_t SetTimePercentage(float& p) = 0;                             // 8  (no meaningful value)
    virtual void GetVisualTimePercentage(float* out) = 0;                         // 9
    virtual uint32_t GetHour() = 0;                                               // 10
    virtual uint32_t SetHour(uint32_t hour) = 0;                                  // 11 (no meaningful value)
    virtual uint32_t GetVisualHour() = 0;                                         // 12
    virtual uint32_t GetMinute() = 0;                                             // 13
    virtual uint32_t SetMinute(uint32_t minute) = 0;                              // 14 the new seconds
    virtual uint32_t GetVisualMinute() = 0;                                       // 15
    virtual uint32_t GetSeconds() = 0;                                            // 16
    virtual uint32_t SetSeconds(uint32_t seconds) = 0;                            // 17 GetMinute()
    virtual uint32_t GetVisualSeconds() = 0;                                      // 18
    virtual uint32_t GetEventHour(uint32_t index) = 0;                            // 19
    virtual uint32_t GetTimeMagnification(float& out) = 0;                        // 20 the bits
    virtual bool SetTimeMagnification(float m) = 0;                               // 21
    virtual uint32_t GetTimeMagnificationMinMax(float& min, float& max) = 0;      // 22 the bits of max
    virtual bool SetTimeMagnificationMinMax(float min, float max) = 0;            // 23
    virtual bool Pause(bool on) = 0;                                              // 24
    virtual bool IsPaused() = 0;                                                  // 25
    virtual bool HiddenPause(bool on) = 0;                                        // 26
    virtual bool IsHiddenPaused() = 0;                                            // 27
    virtual bool IsNight() = 0;                                                   // 28
    virtual void EnableDayNightTransitionNotifications(bool on) = 0;              // 29
};

// ---- surface water (docs/specs/terrain.md §10) ------------------------------------------------------

class cISTESurfaceWater;
class cISC4Lot;

// cISTESurfaceWater (vtable 0x00AB3B00, 14 interface slots; terrain §10). The
// SDK declares the two IsNavigable overloads position-first; MSVC groups the
// overloads in reverse, so the SDK's compiled slot 7 is the cell form (16
// bytes) and slot 8 the position form: the opposite of the game (slot 7: the
// position form, 5 argument dwords; slot 8: the cell form, §10.4). Slot 11
// returns a pointer to the vertex's velocity in EAX (SDK: float). Call it
// only through this declaration (the terrain shadow, glue16).
class cFC4ISTESurfaceWater : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                       // 3
    virtual bool Shutdown() = 0;                                                   // 4
    virtual float GetWaterDepthAtVertex(int32_t index) const = 0;                  // 5
    virtual float GetWaterDepthForCell(int32_t x, int32_t z) const = 0;            // 6
    // 7: the position form; converts the position to a cell and calls slot 8.
    // Not called by the plugin.
    virtual bool IsNavigableAtPosition(uint32_t a0, uint32_t a1, uint32_t a2, uint32_t a3,
                                       uint32_t a4) const = 0;                     // 7
    virtual bool IsNavigable(int32_t cellX, int32_t cellZ, int32_t radius, cISC4Lot* lot) const = 0;  // 8
    virtual void* GetBodyOfWaterMap() const = 0;                                   // 9
    virtual float AltitudeAtWhichFloodingWillOccur(int32_t x, int32_t z) = 0;      // 10
    virtual void* GetWaterVelocity(int32_t x, int32_t z) = 0;                      // 11 a pointer
    virtual int32_t GetWaterType(int32_t x, int32_t z) = 0;                        // 12
    virtual bool HasDraftClearance(int32_t x, int32_t z) = 0;                      // 13
};

// ---- tutorial system (docs/specs/tutorial.md §4.4, §6) ----------------------------------------------

#include "cISC4TutorialSystem.h"

// cISC4TutorialSystem (vtable 0x00A861B8, 21 slots; §6). Slots 0-17 are the
// SDK's (the flammability glue calls slot 8 IsTutorial through them); the
// game's vtable has three more interface methods that the SDK header lacks
// (§4.4; the names are the spec's). Call slots 18-20 only through this
// declaration. Spec-silent argument types (the literal reading of §6, one
// dword each): `e` and `n` are uint32_t; slots 19 and 20 have no defined
// return value [TU3].
class cFC4ISC4TutorialSystem : public cISC4TutorialSystem {
public:
    virtual bool OnTutorialEvent(uint32_t e) = 0;        // 18 AL = 1
    virtual void SetTutorialTaskNumber(uint32_t n) = 0;  // 19
    virtual void IncrementTutorialTaskNumber() = 0;      // 20
};

namespace fc4 {
// The tutorial system travels as the SDK's pointer (cISC4City slot 82).
inline cFC4ISC4TutorialSystem* AsGame(cISC4TutorialSystem* p) { return static_cast<cFC4ISC4TutorialSystem*>(p); }
}  // namespace fc4

// ---- command parameter set (docs/abi/interfaces.tsv, re-answers12 CB16) -----------------------------

#include "cIGZCommandParameterSet.h"

// cIGZCommandParameterSet as compiled (cGZCommandParameterSet, vtable
// 0x00AD16C8), declared up to slot 10, the last slot docs/abi lists (rows 3
// and 5-8, 10). The SDK declares slot 10 as SetStatusParameterValue(int32_t);
// the game's takes the value as a variant and stores it under the status ID
// (cIGZCommandParameterSet::kStatusParameterID). Slots 4 and 9 have no rows:
// the SDK's signatures, never called. The building development and flora glue
// call slots 3 and 5 through the SDK's pointer (its declaration agrees there);
// call slot 10 only through this one.
class cFC4IGZCommandParameterSet : public cIGZUnknown {
public:
    virtual uint32_t GetParameterCount() const = 0;                        // 3
    virtual void SetParameterCount(uint32_t count) = 0;                    // 4 (no row)
    virtual cIGZVariant* operator[](uint32_t id) = 0;                      // 5 never null
    virtual cIGZVariant* GetParameter(uint32_t id) = 0;                    // 6 null when absent
    virtual void SetParameter(uint32_t id, cIGZVariant const& value) = 0;  // 7 find-or-insert
    virtual void AppendParameter(cIGZVariant const& value) = 0;            // 8 ID = last + 1
    virtual void RemoveParameter(uint32_t id) = 0;                         // 9 (no row)
    virtual void SetStatusParameterValue(cIGZVariant const& value) = 0;    // 10
};

// ---- occupant manager (docs/specs/occupant-manager.md §11) ----------------------------------------

class cISC4Occupant;
class cISC4OccupantFilter;

// cISC4OccupantManager (cSC4OccupantManager vtable 0x00A8F7F0; 50 slots, slot
// 49 is the deleting destructor). docs/specs/occupant-manager.md §11.
//
// Every overload pair is given a unique name here, in the game's COMPILED
// order: the filter overload first and the typed one second for slots 28/29 up
// to 44/45, but **the other way round for 46/47 only** (§9.4: the SDK declares
// the typed overload first for every pair, and MSVC's reversal rule then puts
// the filter overload at 46 and the typed one at 47 - the opposite of the
// game). The SDK's `RemoveOccupants(int32_t, ...)` is slot 23 (the typed one),
// not an overload of slot 22.
//
// SDK errors fixed: GetBoundingCells' cells is `int32_t (*)[2]` (the SDK
// declares `int* [2]`, an array of two pointers) and it returns n in EAX.
// Slots 6-19 do not set AL in the original (§3.2 quirk): a replacement
// returns true (§3.2 [choice]), which no shipped caller tests.
class cFC4ISC4OccupantManager : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                                                 // 3
    virtual bool Shutdown() = 0;                                                                             // 4
    virtual bool SetCitySize(float fx, float fz) = 0;                                                        // 5
    virtual bool GetOccupantManagerCellSizes(float& x, float& z) = 0;                                        // 6
    virtual bool GetWorldCellCount(int32_t& x, int32_t& z) = 0;                                              // 7
    virtual bool GetOccupantManagerCellCount(int32_t& x, int32_t& z) = 0;                                    // 8
    virtual bool GetOccupantManagerCellBounds(int32_t& x0, int32_t& z0, int32_t& x1, int32_t& z1) = 0;        // 9
    virtual bool WorldCellToOccupantManagerCell(int32_t x, int32_t z, int32_t& ox, int32_t& oz) = 0;         // 10
    virtual bool OccupantManagerCellToWorldCell(int32_t x, int32_t z, int32_t& ox, int32_t& oz) = 0;         // 11
    virtual bool WorldCellToStandardCityCell(int32_t x, int32_t z, int32_t& ox, int32_t& oz) = 0;            // 12 (bug, §3.2)
    virtual bool StandardCityCellToWorldCell(int32_t x, int32_t z, int32_t& ox, int32_t& oz) = 0;            // 13
    virtual bool WorldCellToPosition(int32_t x, int32_t z, float& fx, float& fz) = 0;                        // 14
    virtual bool PositionToWorldCell(float fx, float fz, int32_t& x, int32_t& z) = 0;                        // 15
    virtual bool OccupantManagerCellToPosition(int32_t x, int32_t z, float& fx, float& fz) = 0;              // 16
    virtual bool PositionToOccupantManagerCell(float fx, float fz, int32_t& x, int32_t& z) = 0;              // 17
    virtual bool StandardCityCellToPosition(int32_t x, int32_t z, float& fx, float& fz) = 0;                 // 18
    virtual bool PositionToStandardCityCell(float fx, float fz, int32_t& x, int32_t& z) = 0;                 // 19
    virtual bool InsertOccupant(cISC4Occupant* occ, uint32_t data4) = 0;                                     // 20
    virtual bool RemoveOccupant(cISC4Occupant* occ, bool shutdown, uint32_t data4) = 0;                      // 21
    virtual bool RemoveOccupants(cISC4OccupantFilter* filter, bool shutdown, uint32_t data4) = 0;            // 22
    virtual bool RemoveOccupantsOfType(uint32_t occType, bool shutdown, uint32_t data4) = 0;                 // 23
    virtual bool MoveOccupant(cISC4Occupant* occ, bool post) = 0;                                            // 24
    virtual bool IsCellEmpty(int32_t x, int32_t z) = 0;                                                      // 25
    virtual int32_t GetBoundingCells(int32_t x, int32_t z, int32_t (*cells)[2], int32_t& n) = 0;             // 26
    virtual bool FindOccupant(cISC4Occupant* occ, bool searchAll, int32_t* x, int32_t* z) = 0;               // 27
    virtual bool GetFirstOccupantByPositionFilter(cISC4Occupant*& out, float fx, float fz,
                                                  cISC4OccupantFilter* filter) = 0;                          // 28
    virtual bool GetFirstOccupantByPositionTyped(cISC4Occupant*& out, float fx, float fz, uint32_t occType) = 0;  // 29
    virtual bool GetFirstOccupantFilter(cISC4Occupant*& out, int32_t x, int32_t z,
                                        cISC4OccupantFilter* filter) = 0;                                    // 30
    virtual bool GetFirstOccupantTyped(cISC4Occupant*& out, int32_t x, int32_t z, uint32_t occType) = 0;     // 31
    virtual bool GetFirstOccupantByStandardCityCellFilter(cISC4Occupant*& out, int32_t x, int32_t z,
                                                          cISC4OccupantFilter* filter) = 0;                  // 32
    virtual bool GetFirstOccupantByStandardCityCellTyped(cISC4Occupant*& out, int32_t x, int32_t z,
                                                         uint32_t occType) = 0;                              // 33
    virtual bool GetFirstOccupantByStandardCityCellsFilter(cISC4Occupant*& out, SC4Point<int> const* min,
                                                           SC4Point<int> const* max,
                                                           cISC4OccupantFilter* filter) = 0;                 // 34
    virtual bool GetFirstOccupantByStandardCityCellsTyped(cISC4Occupant*& out, SC4Point<int> const* min,
                                                          SC4Point<int> const* max, uint32_t occType) = 0;   // 35
    virtual bool GetOccupantsByBBoxFilter(SC4List<cISC4Occupant*>& list, float const* min3, float const* max3,
                                          cISC4OccupantFilter* filter, uint32_t maxCount) = 0;               // 36
    virtual bool GetOccupantsByBBoxTyped(SC4List<cISC4Occupant*>& list, float const* min3, float const* max3,
                                         uint32_t occType, uint32_t maxCount) = 0;                          // 37
    virtual bool GetOccupantsByOccupantManagerCellsFilter(SC4List<cISC4Occupant*>& list, int32_t const* min,
                                                          int32_t const* max, cISC4OccupantFilter* filter,
                                                          uint32_t maxCount) = 0;                            // 38
    virtual bool GetOccupantsByOccupantManagerCellsTyped(SC4List<cISC4Occupant*>& list, int32_t const* min,
                                                         int32_t const* max, uint32_t occType,
                                                         uint32_t maxCount) = 0;                             // 39
    virtual bool GetOccupantsByStandardCityCellsFilter(SC4List<cISC4Occupant*>& list, SC4Point<int> const* min,
                                                       SC4Point<int> const* max, cISC4OccupantFilter* filter,
                                                       uint32_t maxCount) = 0;                              // 40
    virtual bool GetOccupantsByStandardCityCellsTyped(SC4List<cISC4Occupant*>& list, SC4Point<int> const* min,
                                                      SC4Point<int> const* max, uint32_t occType,
                                                      uint32_t maxCount) = 0;                               // 41
    virtual bool IterateOccupantsByBBoxFilter(bool (*fn)(cISC4Occupant*, void*), void* data, float const* min3,
                                              float const* max3, cISC4OccupantFilter* filter) = 0;          // 42
    virtual bool IterateOccupantsByBBoxTyped(bool (*fn)(cISC4Occupant*, void*), void* data, float const* min3,
                                             float const* max3, uint32_t occType) = 0;                      // 43
    virtual bool IterateOccupantsFilter(bool (*fn)(cISC4Occupant*, void*), void* data, int32_t const* min,
                                        int32_t const* max, cISC4OccupantFilter* filter) = 0;               // 44
    virtual bool IterateOccupantsTyped(bool (*fn)(cISC4Occupant*, void*), void* data, int32_t const* min,
                                       int32_t const* max, uint32_t occType) = 0;                           // 45
    // 46/47 only: the TYPED overload comes first (§9.4).
    virtual bool IterateOccupantsByStandardCityCellTyped(bool (*fn)(cISC4Occupant*, void*), void* data,
                                                         int32_t const* min, int32_t const* max,
                                                         uint32_t occType) = 0;                             // 46
    virtual bool IterateOccupantsByStandardCityCellFilter(bool (*fn)(cISC4Occupant*, void*), void* data,
                                                          int32_t const* min, int32_t const* max,
                                                          cISC4OccupantFilter* filter) = 0;                 // 47
    virtual bool ReleaseOccupantList(SC4List<cISC4Occupant*>& list) = 0;                                     // 48
};

namespace fc4 {
// A parameter set travels as the SDK's pointer (cIGZCommandServer slot 5).
inline cFC4IGZCommandParameterSet* AsGame(cIGZCommandParameterSet* p) {
    return reinterpret_cast<cFC4IGZCommandParameterSet*>(p);
}
}  // namespace fc4

namespace fc4 {
// A simulator handed out by the game (cISC4City::GetSimulator), seen through
// the verified slot order instead of the SDK's declaration.
inline cFC4ISC4Simulator* AsGame(cISC4Simulator* p) { return reinterpret_cast<cFC4ISC4Simulator*>(p); }
// The same for cISC4City::GetBudgetSimulator (the SDK's declaration has the
// 32-bit monthly totals; the verified one returns 64-bit values).
inline cFC4ISC4BudgetSimulator* AsGame(cISC4BudgetSimulator* p) {
    return reinterpret_cast<cFC4ISC4BudgetSimulator*>(p);
}

// Department budgets travel as the SDK's cISC4DepartmentBudget* (the type the
// SDK's budget signatures and containers use). Both are single-inheritance
// interfaces whose vtable is at offset 0, so the pointer is the same; only
// the view of the slots differs. Never call slot 26 through the SDK type.
inline cFC4ISC4DepartmentBudget* AsGame(cISC4DepartmentBudget* p) {
    return reinterpret_cast<cFC4ISC4DepartmentBudget*>(p);
}
inline cISC4DepartmentBudget* AsSdk(cFC4ISC4DepartmentBudget* p) { return reinterpret_cast<cISC4DepartmentBudget*>(p); }

// The same for the framework interfaces of foundations §17: objects arrive as
// SDK pointers and are called only through the verified declarations.
inline cFC4IGZDate* AsGame(cIGZDate* p) { return reinterpret_cast<cFC4IGZDate*>(p); }
inline cFC4IGZDate const* AsGame(cIGZDate const* p) { return reinterpret_cast<cFC4IGZDate const*>(p); }
inline cIGZDate* AsSdk(cFC4IGZDate* p) { return reinterpret_cast<cIGZDate*>(p); }
inline cFC4ISCPropertyHolder* AsGame(cISCPropertyHolder* p) { return reinterpret_cast<cFC4ISCPropertyHolder*>(p); }
inline cFC4ISCPropertyHolder const* AsGame(cISCPropertyHolder const* p) {
    return reinterpret_cast<cFC4ISCPropertyHolder const*>(p);
}
inline cFC4IGZVariant* AsGame(cIGZVariant* p) { return reinterpret_cast<cFC4IGZVariant*>(p); }
inline cFC4IGZVariant const* AsGame(cIGZVariant const* p) { return reinterpret_cast<cFC4IGZVariant const*>(p); }
inline cIGZVariant const* AsSdk(cFC4IGZVariant const* p) { return reinterpret_cast<cIGZVariant const*>(p); }
inline cFC4ISC4LotManager* AsGame(cISC4LotManager* p) { return reinterpret_cast<cFC4ISC4LotManager*>(p); }
inline cFC4ISC4Lot* AsGame(cISC4Lot* p) { return reinterpret_cast<cFC4ISC4Lot*>(p); }
inline cFC4ISC4WeatherSimulator* AsGame(cISC4WeatherSimulator* p) {
    return reinterpret_cast<cFC4ISC4WeatherSimulator*>(p);
}
inline cFC4ISC4NeighborsSimulator* AsGame(cISC4NeighborsSimulator* p) {
    return reinterpret_cast<cFC4ISC4NeighborsSimulator*>(p);
}
// Ordinances travel as the SDK's cISC4Ordinance* (single inheritance, vtable
// at offset 0); slot 10 is only ever called through cFC4ISC4Ordinance.
inline cFC4ISC4Ordinance* AsGame(cISC4Ordinance* p) { return reinterpret_cast<cFC4ISC4Ordinance*>(p); }
inline cISC4Ordinance* AsSdk(cFC4ISC4Ordinance* p) { return reinterpret_cast<cISC4Ordinance*>(p); }
// The ordinance simulator: slot 24 only ever through cFC4ISC4OrdinanceSimulator.
inline cFC4ISC4OrdinanceSimulator* AsGame(cISC4OrdinanceSimulator* p) {
    return reinterpret_cast<cFC4ISC4OrdinanceSimulator*>(p);
}
// Demand nodes travel as the SDK's cISC4Demand* (single inheritance); slots
// 16, 17 and 30 are only ever called through cFC4ISC4Demand.
inline cFC4ISC4Demand* AsGame(cISC4Demand* p) { return reinterpret_cast<cFC4ISC4Demand*>(p); }
inline cISC4Demand* AsSdk(cFC4ISC4Demand* p) { return reinterpret_cast<cISC4Demand*>(p); }
inline cFC4ISC4DemandSimulator* AsGame(cISC4DemandSimulator* p) {
    return reinterpret_cast<cFC4ISC4DemandSimulator*>(p);
}
inline cISC4DemandSimulator* AsSdk(cFC4ISC4DemandSimulator* p) { return reinterpret_cast<cISC4DemandSimulator*>(p); }
inline cFC4ISC4ResidentialSimulator* AsGame(cISC4ResidentialSimulator* p) {
    return reinterpret_cast<cFC4ISC4ResidentialSimulator*>(p);
}
inline cFC4ISC4RegionalCity* AsGame(cISC4RegionalCity* p) { return reinterpret_cast<cFC4ISC4RegionalCity*>(p); }
// Plumbing travels as the SDK's cISC4PlumbingSimulator* (that is the type the
// city getter and the SDK headers use). Both are single-inheritance interfaces
// whose vtable is at offset 0, so the pointer is the same; only the view of the
// slots differs. Call slot 11 (IsCellWatered) through the verified one.
inline cFC4ISC4PlumbingSimulator* AsGame(cISC4PlumbingSimulator* p) {
    return reinterpret_cast<cFC4ISC4PlumbingSimulator*>(p);
}
inline cISC4PlumbingSimulator* AsSdk(cFC4ISC4PlumbingSimulator* p) {
    return reinterpret_cast<cISC4PlumbingSimulator*>(p);
}
// Power travels as the SDK's cISC4PowerSimulator* (the city getter's type);
// single inheritance, vtable at offset 0. Call it only through the verified
// declaration (the SDK's overloads are out of compiled order).
inline cFC4ISC4PowerSimulator* AsGame(cISC4PowerSimulator* p) {
    return reinterpret_cast<cFC4ISC4PowerSimulator*>(p);
}
// Pollution travels as the SDK's cISC4PollutionSimulator* (the city getter's
// type); single inheritance, vtable at offset 0. Call slots 29, 30, 33-41,
// 51 and 65 only through the verified declaration.
inline cFC4ISC4PollutionSimulator* AsGame(cISC4PollutionSimulator* p) {
    return reinterpret_cast<cFC4ISC4PollutionSimulator*>(p);
}
inline cISC4PollutionSimulator* AsSdk(cFC4ISC4PollutionSimulator* p) {
    return reinterpret_cast<cISC4PollutionSimulator*>(p);
}
// The protection simulators and the dispatch manager travel as the SDK's
// pointers (the city getters' types); single inheritance, vtable at offset 0.
// Call them only through the verified declarations (§15).
inline cFC4ISC4FireProtectionSimulator* AsGame(cISC4FireProtectionSimulator* p) {
    return reinterpret_cast<cFC4ISC4FireProtectionSimulator*>(p);
}
inline cFC4ISC4PoliceSimulator* AsGame(cISC4PoliceSimulator* p) { return reinterpret_cast<cFC4ISC4PoliceSimulator*>(p); }
inline cFC4ISC4DispatchManager* AsGame(cISC4DispatchManager* p) {
    return reinterpret_cast<cFC4ISC4DispatchManager*>(p);
}
inline cISC4DispatchSource* AsSdk(cFC4ISC4DispatchSource* p) { return reinterpret_cast<cISC4DispatchSource*>(p); }
// The landfill developer travels as the SDK's pointer (cISC4City slot 78);
// call it only through the verified declaration (§7.1).
inline cFC4ISC4LandfillDeveloper* AsGame(cISC4LandfillDeveloper* p) {
    return reinterpret_cast<cFC4ISC4LandfillDeveloper*>(p);
}
// The zone manager travels as the SDK's pointer (cISC4City slot 36); call it
// only through the compiled declaration (zones.md §11.1).
inline cFC4ISC4ZoneManager* AsGame(cISC4ZoneManager* p) { return reinterpret_cast<cFC4ISC4ZoneManager*>(p); }

// A plugin's flora type filter (flora §11.1 slot 13), seen through its one
// verified slot.
inline cFC4ISC4FloraTypeFilter* AsGame(cISC4FloraTypeFilter* p) {
    return reinterpret_cast<cFC4ISC4FloraTypeFilter*>(p);
}

// The history warehouse travels as the SDK's pointer (cISC4City slot 32);
// slots 9-11 only ever through the compiled declaration (history.md §8).
inline cFC4ISC4HistoryWarehouse* AsGame(cISC4HistoryWarehouse* p) {
    return reinterpret_cast<cFC4ISC4HistoryWarehouse*>(p);
}

// The 24-hour clock travels as the SDK's pointer (cISC4City slot 86); slots
// 7, 9, 19 and 20 only ever through the compiled declaration (clock24.md §8).
inline cFC4ISC424HourClock* AsGame(cISC424HourClock* p) { return reinterpret_cast<cFC4ISC424HourClock*>(p); }

// The demolition utility (cISC4City slot 31), the city situation manager
// (slot 49) and the dispatch manager (slot 39) travel as the SDK's pointers
// (the city getters' types); single inheritance, vtable at offset 0. Call them
// only through the compiled declarations (demolition.md §8, city-situation.md
// §8, dispatch.md §8).
inline cFC4ISC4Demolition* AsGame(cISC4Demolition* p) {
    return reinterpret_cast<cFC4ISC4Demolition*>(p);
}
inline cFC4ISC4CitySituationManager* AsGame(cISC4CitySituationManager* p) {
    return reinterpret_cast<cFC4ISC4CitySituationManager*>(p);
}
// The occupant manager travels as the SDK's pointer (cISC4City's getter, city
// field +0x12C); single inheritance, vtable at offset 0. Call it only through
// the compiled declaration (occupant-manager.md §11) - in particular slots 46
// and 47, whose overload order the SDK reverses (§9.4).
inline cFC4ISC4OccupantManager* AsGame(cISC4OccupantManager* p) {
    return reinterpret_cast<cFC4ISC4OccupantManager*>(p);
}

// The surface water travels as the SDK's pointer (cISC4City slot 83); call it
// only through the compiled declaration (terrain §10).
inline cFC4ISTESurfaceWater* AsGame(cISTESurfaceWater* p) { return reinterpret_cast<cFC4ISTESurfaceWater*>(p); }
}  // namespace fc4

// ---- effects (docs/specs/effects.md §11, docs/abi/interfaces.tsv rows 2268-2330) -------------------

class cS3DTransform;
class cS3DVector3;
class cISC4EffectMaps;
class cISC4Occupant;
class cISC4VisualEffect;
class cISC4DisasterManagerEffect;

// The effect component interface (§11.2): not an IUnknown — the vtable starts
// at Start. Three implementations share it: a visual effect's own sub-object
// at +4 (vtable 0x00AA1148), the brush effect (0x00A9EDD0) and the game effect
// (0x00AA0FF8). Slots 7 and 8 are AddRef and Release.
class cFC4ISC4EffectComponent {
public:
    virtual void Start(uint32_t mode) = 0;                                                 // 0
    virtual void Stop(uint32_t mode) = 0;                                                  // 1
    virtual bool IsActive() = 0;                                                           // 2 AL
    virtual void SetTransform(cS3DTransform const& local, cS3DTransform const& parent) = 0; // 3
    virtual void Update(float dtSeconds) = 0;                                              // 4
    virtual bool SetParameter(uint32_t index, float const* data, uint32_t count) = 0;      // 5
    virtual bool SetParameter2(uint32_t index, void const* data, uint32_t count) = 0;      // 6
    virtual uint32_t AddRef() = 0;                                                         // 7
    virtual uint32_t Release() = 0;                                                        // 8
};

// cISC4VisualEffect (IID as cIGZUnknown's, vtable 0x00AA1170, 19 slots;
// effects.md §11.1). The SDK header is readable but its two transform
// accessors are named the other way round from docs/abi's SetTransform /
// SetSecondaryTransform rows (slot 8 is the world transform, slot 7 the
// secondary at +0x6C); the slot numbers below are the measured ones.
class cFC4ISC4VisualEffect : public cIGZUnknown {
public:
    virtual void Start(uint32_t mode) = 0;                                                   // 3
    virtual void Stop(uint32_t mode) = 0;                                                    // 4
    virtual bool IsStarted() = 0;                                                            // 5 AL
    virtual bool IsStopping() = 0;                                                           // 6 AL
    virtual void SetSecondaryTransform(cS3DTransform const& t) = 0;                          // 7
    virtual void SetTransform(cS3DTransform const& t) = 0;                                   // 8
    virtual bool AttachToOccupant(cISC4Occupant* occupant) = 0;                              // 9 render
    virtual void SetEndMessage(uint32_t type, uint32_t data1) = 0;                           // 10
    virtual bool SetParameter3(uint32_t index, float const* data, uint32_t count) = 0;       // 11
    virtual bool SetParameter(uint32_t index, float const* data, uint32_t count) = 0;        // 12
    virtual bool ForwardParameter(uint32_t index, void const* data, uint32_t count) = 0;     // 13
    virtual void GetTransform(cS3DTransform& t) = 0;                                         // 14
    virtual void GetSecondaryTransform(cS3DTransform& t) = 0;                                // 15
    virtual bool AddComponent(void* descRec, cFC4ISC4EffectComponent* component) = 0;        // 16 internal
    virtual bool TestFlag(uint32_t bit) = 0;                                                 // 17 AL, bit <= 8
};

// cISC4EffectsManager (IID 0xC97CF5CD, vtable 0x00A9F264, 11 slots;
// effects.md §1, §4). The SDK header declares the two CreateVisualEffect
// overloads name-first; the measured vtable is the other way round — slot 6 is
// the ID-pair form (0x00593AE0, 3 dwords, pop 12) and slot 7 the name form
// (0x005939B0, 2 dwords, pop 8). This declaration is the measured order.
class cFC4ISC4EffectsManager : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                                 // 3
    virtual bool Shutdown() = 0;                                                             // 4
    virtual bool HasVisualEffect(char const* name) = 0;                                      // 5
    // Distinct virtual names prevent MSVC's overload grouping from reversing
    // slots with different callee stack cleanup (12 versus 8 bytes).
    virtual bool CreateVisualEffectById(uint32_t id0, uint32_t id1, cISC4VisualEffect** out) = 0; // 6
    virtual bool CreateVisualEffectByName(char const* name, cISC4VisualEffect** out) = 0;         // 7
    virtual void GetStatsString(cIGZString& stats) = 0;                                      // 8
    virtual cISC4EffectMaps* GetEffectMaps() = 0;                                            // 9
    virtual float GetTallestBuildingAltitude() = 0;                                          // 10 x87

    bool CreateVisualEffect(uint32_t id0, uint32_t id1, cISC4VisualEffect** out) {
        return CreateVisualEffectById(id0, id1, out);
    }
    bool CreateVisualEffect(char const* name, cISC4VisualEffect** out) {
        return CreateVisualEffectByName(name, out);
    }
};


// The effects manager's second sub-object: the 3D view's draw-pass override.
// effects.md §7: `Update(SC4DrawContext*)` (`0x00593120`) has no direct callers and is
// dispatched as a vtable slot at `0x00A9F25C`, next to the cISC4EffectsManager vtable
// `0x00A9F264`. The entry is the fourth of that vtable (an IUnknown prefix), so the
// replacement keeps the same shape: IUnknown at slots 0-2 and Update at slot 3.
class cFC4ISC4EffectsDrawPass : public cIGZUnknown {
public:
    virtual void Update(void* drawContext) = 0;  // 3 (render; effects.md §7)
};

// cISC4DisasterManagerEffect (IID 0xCA7B3F3C, vtable 0x00A9E190, 6 slots;
// effects.md §11.3). The original implements it as a sub-object of the
// effect-disaster manager at this-0x24. Slot 3 returns the new instance in
// EAX (docs/abi note).
class cFC4ISC4DisasterManagerEffect : public cIGZUnknown {
public:
    // Returns the new effect-disaster instance.
    virtual void* StartDisasterEffect(cISC4VisualEffect* effect, cS3DVector3 const* position, uint32_t type) = 0;  // 3
    virtual void AttractDisasterEffect(cISC4VisualEffect* effect) = 0;                                             // 4
    virtual void CancelAttract() = 0;                                                                              // 5
};

namespace fc4 {
// The effects manager travels as the SDK's pointer (cISC4City slot 85,
// city +0x1FC). Call slots 6 and 7 only through the compiled declaration: the
// SDK's two CreateVisualEffect overloads are declared in the opposite order.
inline cFC4ISC4EffectsManager* AsGame(cISC4EffectsManager* p) {
    return reinterpret_cast<cFC4ISC4EffectsManager*>(p);
}
inline cISC4EffectsManager* AsSdk(cFC4ISC4EffectsManager* p) {
    return reinterpret_cast<cISC4EffectsManager*>(p);
}
inline cFC4ISC4VisualEffect* AsGame(cISC4VisualEffect* p) {
    return reinterpret_cast<cFC4ISC4VisualEffect*>(p);
}
inline cISC4VisualEffect* AsSdk(cFC4ISC4VisualEffect* p) {
    return reinterpret_cast<cISC4VisualEffect*>(p);
}
inline cFC4ISC4DisasterManagerEffect* AsGame(cISC4DisasterManagerEffect* p) {
    return reinterpret_cast<cFC4ISC4DisasterManagerEffect*>(p);
}
}  // namespace fc4

// Terrain prefix through slot 6 (docs/abi/interfaces.tsv cISTETerrain and
// vtables.tsv cSTETerrain::vftable). The SDK's CellWidth is uint32_t, but
// the game returns float in ST0. Use this view for the pole's terrain width;
// ReadCellWidth distinguishes this corrected call from the unused SDK method.
class cFC4ISTETerrainCellWidth : public cIGZUnknown {
public:
    virtual bool Init() = 0;                 // 3
    virtual bool Shutdown() = 0;             // 4
    virtual bool IsInitialized() = 0;        // 5
    virtual float ReadCellWidth() = 0;       // 6
};
