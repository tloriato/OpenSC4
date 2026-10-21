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

// cFC4LotConfiguration: FreeCity4's own cISC4LotConfiguration-shaped object
// (docs/specs/lot-configuration.md). The configuration class is NOT
// GZCOM-creatable (GLUE21-1, lot-configuration.md 1.1): the original manager
// allocates it directly, so the replacement manager must build its own
// objects of the same interface shape - the game's 34-slot table (8; slot 33
// is the scalar deleting destructor), the field layout of 2 so the manager
// can write the four fields it writes directly at PostCityInit (+0x3E,
// +0x40, +0x80 and +0x8C), and the Load of 3 reading the exemplar.
//
// The object is never saved (a lot saves only the configuration ID, 1), so
// there is no cIGZSerializable and Save (slot 32) answers false (7: the
// simulation never calls it; Load calls it only for a configuration of a
// version other than 2, which shipped data never is).
//
// [abi] Slot 19 takes and ignores one dword argument (5.4, the SDK error:
// the header declares it without one). Declared with the argument here so a
// caller that follows the game's table does not corrupt the stack; no
// caller was found (5.4).
//
// The state lives in plain members in the original's offset order (2), so
// the manager's direct writes are ordinary member stores. The vectors are
// ours (std::vector); the slot getters return pointers into them and the
// callers borrow them for the call (5.1).
#pragma once

#include "FC4Interfaces.h"
#include "SC4Vector.h"
#include "cGZPersistResourceKey.h"
#include "cISC4LotConfiguration.h"
#include "cSC4LotConfigurationObject.h"

#include <cstdint>
#include <vector>

// lot-configuration.md 1: the configuration's interface IID.
static const uint32_t kFC4_IID_cISC4LotConfiguration = 0x68EDBEBC;

// The game's 34-slot table (8). Slot 19 carries the ignored dword (5.4).
class cFC4ISC4LotConfiguration : public cIGZUnknown {
public:
    virtual uint32_t GetID() const = 0;                                                       // 3
    virtual cIGZString& GetName(cIGZString& name) const = 0;                                  // 4
    virtual bool BelongsToFamily(uint32_t family) const = 0;                                  // 5
    virtual SC4Vector<uint32_t>* GetFamilyIDArray() = 0;                                      // 6
    virtual void GetSize(uint32_t& w, uint32_t& d) const = 0;                                 // 7
    virtual float GetMaximumSlope() const = 0;                                                // 8
    virtual float GetMinimumSlope() const = 0;                                                // 9
    virtual float GetMaximumSlopeBeforeFoundation() const = 0;                                // 10
    virtual uint8_t GetGrowthStage() const = 0;                                               // 11
    virtual uint16_t GetMinBuildingCapacity() const = 0;                                      // 12
    virtual uint16_t GetMaxBuildingCapacity() const = 0;                                      // 13
    virtual bool IsCompatibleWithWater() const = 0;                                           // 14
    virtual bool IsCompatibleWithZoneType(uint32_t type) const = 0;                           // 15
    virtual bool IsCompatibleWithWealthType(uint32_t wealth) const = 0;                       // 16
    virtual bool IsCompatibleWithPurposeType(uint32_t purpose) const = 0;                     // 17
    virtual bool IsCompatibleWithStyleType(uint32_t group) const = 0;                         // 18
    virtual bool IsCompatibleWithMonopolyPieceUse(uint32_t ignored) = 0;                      // 19, pop 4 (5.4)
    virtual bool IsDoConstruction() const = 0;                                                // 20
    virtual void GetRequiredRoads(SC4Vector<bool>& roads) const = 0;                          // 21
    virtual uint32_t GetLotConfigurationObjectCount() = 0;                                    // 22
    virtual SC4Vector<cSC4LotConfigurationObject>* GetLotConfigurationObjectArray() = 0;      // 23
    virtual cSC4LotConfigurationObject* GetPrimaryBuildingConfigurationObject() = 0;          // 24
    virtual cSC4LotConfigurationObject* GetPrimaryBaseConfigurationObject() = 0;              // 25
    virtual cSC4LotConfigurationObject* GetPrimaryFoundationConfigurationObject() = 0;        // 26
    virtual bool HasObjectsOfType(uint32_t type) = 0;                                         // 27
    virtual SC4Vector<uint32_t>* GetRetainingWallIDArray() = 0;                               // 28
    virtual SC4Vector<uint32_t>* GetFenceIDArray() = 0;                                       // 29
    virtual SC4Vector<int8_t>* GetFenceSegmentPositionArray() = 0;                            // 30
    virtual bool Load(cGZPersistResourceKey const& key, uint8_t detail) = 0;                  // 31
    virtual bool Save(cGZPersistResourceKey const& key, void* segment) = 0;                   // 32
    // slot 33: the implementing class's `virtual ~` (scalar deleting dtor).
};

class cFC4LotConfiguration final : public cFC4ISC4LotConfiguration {
public:
    cFC4LotConfiguration();
    // 8 slot 33: the scalar deleting destructor (a new virtual here,
    // cIGZUnknown has no virtual destructor).
    virtual ~cFC4LotConfiguration();

    // cIGZUnknown. 1: IIDs 1 and 0x68EDBEBC give the object with a
    // reference; anything else false.
    bool QueryInterface(uint32_t riid, void** ppvObj) override;
    uint32_t AddRef() override;
    uint32_t Release() override;

    // The 34-slot table (8).
    uint32_t GetID() const override;
    cIGZString& GetName(cIGZString& name) const override;
    bool BelongsToFamily(uint32_t family) const override;
    SC4Vector<uint32_t>* GetFamilyIDArray() override;
    void GetSize(uint32_t& w, uint32_t& d) const override;
    float GetMaximumSlope() const override;
    float GetMinimumSlope() const override;
    float GetMaximumSlopeBeforeFoundation() const override;
    uint8_t GetGrowthStage() const override;
    uint16_t GetMinBuildingCapacity() const override;
    uint16_t GetMaxBuildingCapacity() const override;
    bool IsCompatibleWithWater() const override;
    bool IsCompatibleWithZoneType(uint32_t type) const override;
    bool IsCompatibleWithWealthType(uint32_t wealth) const override;
    bool IsCompatibleWithPurposeType(uint32_t purpose) const override;
    bool IsCompatibleWithStyleType(uint32_t group) const override;
    // 5.4: always true; the argument is ignored and popped (the SDK error).
    bool IsCompatibleWithMonopolyPieceUse(uint32_t ignored) override;
    bool IsDoConstruction() const override;
    void GetRequiredRoads(SC4Vector<bool>& roads) const override;
    uint32_t GetLotConfigurationObjectCount() override;
    SC4Vector<cSC4LotConfigurationObject>* GetLotConfigurationObjectArray() override;
    cSC4LotConfigurationObject* GetPrimaryBuildingConfigurationObject() override;
    cSC4LotConfigurationObject* GetPrimaryBaseConfigurationObject() override;
    cSC4LotConfigurationObject* GetPrimaryFoundationConfigurationObject() override;
    bool HasObjectsOfType(uint32_t type) override;
    SC4Vector<uint32_t>* GetRetainingWallIDArray() override;
    SC4Vector<uint32_t>* GetFenceIDArray() override;
    SC4Vector<int8_t>* GetFenceSegmentPositionArray() override;
    bool Load(cGZPersistResourceKey const& key, uint8_t detail) override;
    bool Save(cGZPersistResourceKey const& key, void* segment) override;

    // The four fields the original manager writes directly at PostCityInit
    // (4): the SDK has no slots for them, so the manager calls this.
    void SetBuildingInfo(uint16_t minCapacity, uint16_t maxCapacity, std::vector<uint32_t> styles,
                         bool monopoly) {
        minCapacity_ = minCapacity;
        maxCapacity_ = maxCapacity;
        styles_ = std::move(styles);
        monopoly_ = monopoly ? 1 : 0;
    }

private:
    // 3.1 step 3: every field back to its default.
    void Reset();
    // 3.3: one byte list into a bit set of the given size.
    static void SetBits(std::vector<bool>& set, size_t size, const std::vector<uint8_t>& values);
    // 3.6: the position correction with the 24-bit arithmetic.
    void CorrectPositions();
    // 3.7: the foundation record appended after the correction.
    void AppendFoundation(uint32_t type);

public:
    // 2: the fields, in the original's offset order. The offsets are
    // recorded in the comments; the manager reads and writes them through
    // the members.
    uint32_t refCount_ = 0;                            // +0x04
    uint32_t id_ = 0;                                  // +0x08
    std::vector<char> name_;                           // +0x0C (a cRZString in the original)
    uint8_t width_ = 0;                                // +0x20
    uint8_t depth_ = 0;                                // +0x21
    std::vector<uint32_t> families_;                   // +0x24
    float maxSlope_ = 90.0f;                           // +0x30
    float minSlope_ = 0.0f;                            // +0x34
    float maxSlopeFoundation_ = 90.0f;                 // +0x38
    uint8_t growthStage_ = 1;                          // +0x3C
    uint16_t minCapacity_ = 0xFFFF;                    // +0x3E (manager-written)
    uint16_t maxCapacity_ = 0;                         // +0x40 (manager-written)
    uint8_t water_ = 0;                                // +0x42
    std::vector<bool> zones_;                          // +0x44 (16 bits)
    std::vector<bool> wealths_;                        // +0x58 (4 bits)
    std::vector<bool> purposes_;                       // +0x6C (9 bits)
    std::vector<uint32_t> styles_;                     // +0x80 (manager-written)
    uint8_t monopoly_ = 0;                             // +0x8C (manager-written; nothing reads it)
    uint8_t doConstruction_ = 0;                       // +0x8D
    uint8_t roadSides_ = 8;                            // +0x8E
    std::vector<cSC4LotConfigurationObject> objects_;  // +0x90
    std::vector<uint32_t> walls_;                      // +0x9C
    std::vector<uint32_t> fences_;                     // +0xA8
    std::vector<int8_t> fencePositions_;               // +0xB4
};
