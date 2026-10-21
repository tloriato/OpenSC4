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

// The prop manager's GZCOM class (docs/specs/prop-lotconfig.md Part A): city
// component 5, the cISC4City slot 5 getter. The catalogue, the queries and
// the prop creation live in the Rust core (fc4_prop_manager); this class
// adapts the interfaces to it and owns the game services it mutates through.
//
// Layout (§A1): cISC4PropManager and cIGZUnknown at +0, cIGZMessageTarget2 at
// +4, cIGZSerializable at +8 (here: wherever MSVC puts it). QueryInterface
// answers 0x28326F3E -> +0, 1 and 0x452294AA -> +4, 0xE4FDA3D4 -> +8 and 1
// otherwise false. Reference count at +0x10 in the original; ours is a member.
// The last Release calls Shutdown and then the deleting destructor (§A9 slot
// 40), which is the `virtual ~` below.
//
// The vtable has no overloads (FC4PropLotInterfaces.h): every method is
// uniquely named in the game's verified slot order (§A9).
//
// Process lifetime (glue15's two traps): the prop catalogue is created once
// per process and torn down only at PreAppShutdown. It is never rebuilt per
// city or per run - a rebuild would put the types in a different bucket order
// than the original's, and the catalogue caches what it built.
//
// OFF by default: FreeCity4.ini `enable = propmgr`; `disable` still wins.
#pragma once

#include "FC4HistClock.h"
#include "FC4Interfaces.h"
#include "FC4PropLotInterfaces.h"
#include "cIGZMessageTarget2.h"
#include "cIGZSerializable.h"
#include "fc4.h"

#include <cstdint>

class cIGZMessageServer2;

static const uint32_t kFC4_IID_cIGZMessageTarget2Prop = 0x452294AA;
static const uint32_t kFC4_IID_cIGZSerializableProp = 0xE4FDA3D4;

class cFC4PropManager final : public cFC4ISC4PropManager,
                              public cIGZMessageTarget2,
                              public cIGZSerializable {
public:
    cFC4PropManager();
    // §A9 slot 40: the scalar deleting destructor. It is a new virtual here
    // (cIGZUnknown has no virtual destructor), so it lands after slot 39.
    virtual ~cFC4PropManager();

    // cIGZUnknown (every sub-object)
    bool QueryInterface(uint32_t riid, void** ppvObj) override;
    uint32_t AddRef() override;
    uint32_t Release() override;

    // cISC4PropManager (§A9, slots 3-39)
    bool Init() override;
    bool Shutdown() override;
    void GetAllPropTypes(SC4Vector<uint32_t>& types) override;
    void GetAllPropFamilyTypes(SC4Vector<uint32_t>& families) override;
    bool GetPropKeyFromType(uint32_t propID, cGZPersistResourceKey& key) override;
    bool IsPropTypeValid(uint32_t propID) override;
    uint32_t GetPropTypesByFamily(SC4Vector<uint32_t>& types, uint32_t family) override;
    bool IsPropInFamily(uint32_t propID, uint32_t family) override;
    bool IsPropFamilyTypeValid(uint32_t family) override;
    cIGZString& GetPropName(uint32_t propID) override;
    cISC4PropOccupant::PropProfile& GetPropProfile(uint32_t propID) override;
    const cISC4PropOccupant::PropProfile* GetPropProfilePtr(uint32_t propID) override;
    bool CreateProp(uint32_t propID, cISC4PropOccupant*& prop) override;
    bool CreateGenericProp(cGZPersistResourceKey const& key, cISC4PropOccupant*& prop) override;
    bool GetCityPropsWorld(SC4List<cISC4Occupant*>& list, SC4Rect<float> const& rect) override;
    bool GetCityPropsCells(SC4List<cISC4Occupant*>& list, SC4Rect<int32_t> const& rect) override;
    bool AddCityPropOcc(cISC4Occupant* prop) override;
    cISC4Occupant* AddCityPropNew(uint32_t propID, cS3DVector3 const& position, int32_t orientation) override;
    bool RemoveCityPropsWorld(SC4Rect<float> const& rect) override;
    bool RemoveCityPropsCells(SC4Rect<int32_t> const& rect) override;
    bool GetLotPropsAt(SC4List<cISC4Occupant*>& list, int32_t x, int32_t z) override;
    bool GetLotPropsOfLot(SC4List<cISC4Occupant*>& list, cISC4Lot* lot) override;
    bool AddLotPropToLot(cISC4Lot* lot, cISC4Occupant* prop) override;
    cISC4Occupant* AddLotPropNew(uint32_t propID, cS3DVector3 const& position, int32_t orientation) override;
    bool RemoveLotPropsAt(int32_t x, int32_t z) override;
    bool RemoveLotPropsOfLot(cISC4Lot* lot) override;
    bool RemovePropOcc(cISC4Occupant* prop) override;
    bool GetStreetPropsCells(SC4List<cISC4Occupant*>& list, SC4Rect<int32_t> const& rect) override;
    bool GetStreetPropsAt(SC4List<cISC4Occupant*>& list, int32_t x, int32_t z) override;
    cISC4Occupant* AddStreetPropNew(uint32_t propID, cS3DVector3 const& position, int32_t orientation) override;
    void RemoveStreetPropsFromTile(int32_t x, int32_t z) override;
    bool GetBuildingPropsAt(SC4List<cISC4Occupant*>& list, int32_t x, int32_t z) override;
    bool GetBuildingPropsOfBuilding(SC4List<cISC4Occupant*>& list, cISC4Occupant* building) override;
    bool AddBuildingPropToBuilding(cISC4Occupant* building, cISC4Occupant* prop) override;
    bool RemoveBuildingPropsAt(int32_t x, int32_t z) override;
    bool RemoveBuildingPropsOfBuilding(cISC4Occupant* building) override;
    bool RemoveBuildingPropOne(cISC4Occupant* building, cISC4Occupant* prop) override;

    // cIGZMessageTarget2 (§A4.3)
    bool DoMessage(cIGZMessage2* pMessage) override;

    // cIGZSerializable (§A8)
    bool Write(cIGZOStream& gzOut) override;
    bool Read(cIGZIStream& gzIn) override;
    uint32_t GetGZCLSID() override;

    // For the host callbacks (cFC4PropManager.cpp).
    cIGZMessageTarget2* Target() { return static_cast<cIGZMessageTarget2*>(this); }
    cIGZUnknown* Primary() {
        return static_cast<cIGZUnknown*>(static_cast<cFC4ISC4PropManager*>(this));
    }
    fc4::CoreLock& Lock() { return lock_; }
    fc4_prop_manager* Core() { return core_; }

private:
    uint32_t refCount_ = 0;
    bool destroying_ = false;
    bool initialised_ = false;
    fc4::CoreLock lock_;
    fc4_prop_manager_host host_{};
    fc4_prop_manager* core_ = nullptr;
    // §A5 slot 12: a reference to a string that is always empty (a process
    // GameString, cFC4PropManager.cpp's GetPropName).
};

// Factory for the subsystem table (reference count 0).
cIGZUnknown* CreateFC4PropManager();

namespace fc4 {

// The process-wide prop catalogue (created once, torn down only at
// PreAppShutdown; shared by every FreeCity4 prop manager instance).
fc4_prop_catalogue* PropCatalogueProcess();

// The director's PreAppShutdown: the process catalogue is shut down and freed.
void PropManagerAppShutdown();

// The live FreeCity4 prop manager behind `p`, or nullptr (an original).
cFC4PropManager* OurPropManager(cISC4PropManager* p);

}  // namespace fc4
