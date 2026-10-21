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

// The prop manager (cSC4PropManager) as the game compiles it, in the verified
// slot order of docs/specs/prop-lotconfig.md §A9.
//
// The lot configuration manager is NOT declared here: `cFC4ISC4LotConfigurationManager`
// lives in FC4TractInterfaces.h (the tract lane declares it once for the whole
// plugin; the glue21 lane corrected its slot 5/6 names and its return channels
// against §B7 and docs/abi/interfaces.tsv).
//
// Rule (FC4Interfaces.h): NO overloads here. The SDK's cISC4PropManager
// overloads GetCityProps, RemoveCityProps, GetLotProps, RemoveLotProps,
// GetStreetProps, GetBuildingProps, AddCityProp and AddLotProp; the game's
// vtable lists each pair in reverse declaration order, so a subclass of the
// SDK interface would put a pair on the wrong slots (glue15). Every method
// below is uniquely named and slot N is the N-th virtual function, counting
// QueryInterface / AddRef / Release as slots 0-2. The last slot of each
// game vtable is the class's scalar deleting destructor; it is not declared
// here - the implementing class declares `virtual ~...()` after its methods
// and that lands there (§A9 slot 40, §B7 slot 28; the cFC4Development.h and
// cFC4TrafficSimulator.h convention).
//
// This class is FreeCity4's replacement (`enable = propmgr`;
// cFC4PropManager.h), off by default. docs/abi/interfaces.tsv carries every
// slot's callee pop and return channel; the C++ return types below match its
// `ret` column.
#pragma once

#include "FC4GameList.h"
#include "cIGZUnknown.h"
#include "cISC4LotConfiguration.h"
#include "cISC4PropOccupant.h"
#include "SC4Rect.h"

#include <cstdint>

class cGZPersistResourceKey;
class cIGZString;
class cISC4Lot;
class cISC4Occupant;
class cS3DVector3;
template <typename T> class SC4List;
template <typename T> class SC4Vector;

static const uint32_t kFC4_CLSID_cSC4PropManager = 0xA990BD10;             // §A1
static const uint32_t kFC4_IID_cISC4PropManager = 0x28326F3E;              // §A1 (city component 5)

// cSC4PropManager's primary vtable (0x00A8F928, 41 slots; §A9). Slot 40 is
// the scalar deleting destructor and is not declared here.
class cFC4ISC4PropManager : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                     // 3
    virtual bool Shutdown() = 0;                                                 // 4
    // §A5: "no defined return value" (interfaces.tsv eax-or-void).
    virtual void GetAllPropTypes(SC4Vector<uint32_t>& types) = 0;               // 5, pop 4
    virtual void GetAllPropFamilyTypes(SC4Vector<uint32_t>& families) = 0;      // 6, pop 4
    virtual bool GetPropKeyFromType(uint32_t propID, cGZPersistResourceKey& key) = 0;  // 7, pop 8
    virtual bool IsPropTypeValid(uint32_t propID) = 0;                          // 8, pop 4
    // §Q.1: a single prop type gives the vector [id] and returns 0.
    virtual uint32_t GetPropTypesByFamily(SC4Vector<uint32_t>& types, uint32_t family) = 0;  // 9, pop 8
    virtual bool IsPropInFamily(uint32_t propID, uint32_t family) = 0;          // 10, pop 8
    virtual bool IsPropFamilyTypeValid(uint32_t family) = 0;                    // 11, pop 4
    // §A5: a reference to a string that is always empty (SDK: cIGZString*).
    virtual cIGZString& GetPropName(uint32_t propID) = 0;                       // 12, pop 4
    // §A3.2: a reference to the 0x1C-byte profile; inserts a zero profile for
    // an unknown ID (the map is written).
    virtual cISC4PropOccupant::PropProfile& GetPropProfile(uint32_t propID) = 0;  // 13, pop 4
    virtual const cISC4PropOccupant::PropProfile* GetPropProfilePtr(uint32_t propID) = 0;  // 14, pop 4
    virtual bool CreateProp(uint32_t propID, cISC4PropOccupant*& prop) = 0;     // 15, pop 8
    virtual bool CreateGenericProp(cGZPersistResourceKey const& key, cISC4PropOccupant*& prop) = 0;  // 16, pop 8
    // §A7.2: the SDK's pair, in the game's order (world rectangle first).
    virtual bool GetCityPropsWorld(SC4List<cISC4Occupant*>& list, SC4Rect<float> const& rect) = 0;  // 17, pop 8
    virtual bool GetCityPropsCells(SC4List<cISC4Occupant*>& list, SC4Rect<int32_t> const& rect) = 0;  // 18, pop 8
    // §A7.1: usage 1 (city). The two AddCityProp overloads, game order.
    virtual bool AddCityPropOcc(cISC4Occupant* prop) = 0;                        // 19, pop 4
    virtual cISC4Occupant* AddCityPropNew(uint32_t propID, cS3DVector3 const& position,
                                          int32_t orientation) = 0;             // 20, pop 12
    virtual bool RemoveCityPropsWorld(SC4Rect<float> const& rect) = 0;          // 21, pop 4, AL = 1
    virtual bool RemoveCityPropsCells(SC4Rect<int32_t> const& rect) = 0;        // 22, pop 4, AL = 1
    virtual bool GetLotPropsAt(SC4List<cISC4Occupant*>& list, int32_t x, int32_t z) = 0;  // 23, pop 12
    virtual bool GetLotPropsOfLot(SC4List<cISC4Occupant*>& list, cISC4Lot* lot) = 0;      // 24, pop 8
    virtual bool AddLotPropToLot(cISC4Lot* lot, cISC4Occupant* prop) = 0;       // 25, pop 8
    virtual cISC4Occupant* AddLotPropNew(uint32_t propID, cS3DVector3 const& position,
                                         int32_t orientation) = 0;              // 26, pop 12
    virtual bool RemoveLotPropsAt(int32_t x, int32_t z) = 0;                    // 27, pop 8, AL = 1
    virtual bool RemoveLotPropsOfLot(cISC4Lot* lot) = 0;                        // 28, pop 4, AL = 1
    virtual bool RemovePropOcc(cISC4Occupant* prop) = 0;                        // 29, pop 4, AL = 1 (SDK RemovePropA)
    virtual bool GetStreetPropsCells(SC4List<cISC4Occupant*>& list, SC4Rect<int32_t> const& rect) = 0;  // 30, pop 8
    // §A9: returns AL = 0.
    virtual bool GetStreetPropsAt(SC4List<cISC4Occupant*>& list, int32_t x, int32_t z) = 0;  // 31, pop 12
    virtual cISC4Occupant* AddStreetPropNew(uint32_t propID, cS3DVector3 const& position,
                                            int32_t orientation) = 0;           // 32, pop 12
    // §A9: no defined return value (interfaces.tsv eax-or-void).
    virtual void RemoveStreetPropsFromTile(int32_t x, int32_t z) = 0;           // 33, pop 8
    virtual bool GetBuildingPropsAt(SC4List<cISC4Occupant*>& list, int32_t x, int32_t z) = 0;  // 34, pop 12
    virtual bool GetBuildingPropsOfBuilding(SC4List<cISC4Occupant*>& list, cISC4Occupant* building) = 0;  // 35, pop 8
    virtual bool AddBuildingPropToBuilding(cISC4Occupant* building, cISC4Occupant* prop) = 0;  // 36, pop 8
    virtual bool RemoveBuildingPropsAt(int32_t x, int32_t z) = 0;               // 37, pop 8, AL = 1
    virtual bool RemoveBuildingPropsOfBuilding(cISC4Occupant* building) = 0;    // 38, pop 4, AL = 1
    virtual bool RemoveBuildingPropOne(cISC4Occupant* building, cISC4Occupant* prop) = 0;  // 39, pop 8, AL = 1
    // slot 40: the implementing class's `virtual ~` (scalar deleting dtor).
};

namespace fc4 {

// Single inheritance: a game pointer travels unchanged either way.
inline cFC4ISC4PropManager* AsGame(cISC4PropManager* p) {
    return reinterpret_cast<cFC4ISC4PropManager*>(p);
}
inline cISC4PropManager* AsSdk(cFC4ISC4PropManager* p) { return reinterpret_cast<cISC4PropManager*>(p); }

}  // namespace fc4
