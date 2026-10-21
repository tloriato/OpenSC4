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

// The lot configuration manager's GZCOM class (docs/specs/prop-lotconfig.md
// Part B): city component 9, the cISC4City slot 37 getter. The catalogue, the
// set queries and the building information live in the Rust core
// (fc4_lot_config_manager); this class adapts the interfaces to it and owns
// the game services it mutates through.
//
// Layout (B1): cISC4LotConfigurationManager and cIGZUnknown at +0,
// cIGZMessageTarget2 at +4 (the message target is NOT offered by
// QueryInterface), cIGZSerializable at +8. QueryInterface answers `1` and
// 0xC8EDBEC0 -> +0, 0xE4FDA3D4 -> +8, otherwise false. The last Release calls
// Shutdown and then the deleting destructor (B7 slot 28), which is the
// `virtual ~` below.
//
// The vtable has no overloads (FC4TractInterfaces.h declares
// `cFC4ISC4LotConfigurationManager` once for the plugin, in the game's
// verified slot order).
//
// Process lifetime (glue15's two traps): the configuration catalogue is
// process-wide and `fc4_lot_config_unload` runs ONCE, at PreAppShutdown. It is
// never run per city or per run - unload empties the map and a rebuild would
// put the configurations in a different bucket order than the original's.
//
// GLUE21-1, answered (lot-configuration.md 1.1): cSC4LotConfiguration is
// not GZCOM-creatable - the class ID 0xA99062B4 exists only for
// serialization refs - and the named route to the allocate-and-construct
// pair is the manager's own helper, which is exactly what this replacement
// replaces. `create_configuration` therefore builds FreeCity4's own
// cISC4LotConfiguration-shaped object (cFC4LotConfiguration.h, the 34-slot
// table of 8 over the layout of 2, loaded from the exemplar by the Load of
// 3); the catalogue is filled by it. Everything else is wired.
//
// OFF by default: FreeCity4.ini `enable = lotcfg`; `disable` still wins.
#pragma once

#include "FC4HistClock.h"
#include "FC4Interfaces.h"
#include "FC4TractInterfaces.h"
#include "cIGZMessageTarget2.h"
#include "cIGZSerializable.h"
#include "fc4.h"

#include <cstdint>

static const uint32_t kFC4_CLSID_cSC4LotConfigManager = 0x299062C0;  // B1
static const uint32_t kFC4_IID_cISC4LotConfigManager = 0xC8EDBEC0;   // B1 (city component 9)
static const uint32_t kFC4_IID_cIGZSerializableLcm = 0xE4FDA3D4;

class cFC4LotConfigManager final : public cFC4ISC4LotConfigurationManager,
                                   public cIGZMessageTarget2,
                                   public cIGZSerializable {
public:
    cFC4LotConfigManager();
    // B7 slot 28: the scalar deleting destructor (a new virtual here).
    virtual ~cFC4LotConfigManager();

    // cIGZUnknown (every sub-object)
    bool QueryInterface(uint32_t riid, void** ppvObj) override;
    uint32_t AddRef() override;
    uint32_t Release() override;

    // cISC4LotConfigurationManager (B7, slots 3-27)
    bool Init() override;
    bool Shutdown() override;
    bool AddLotConfigurationByKey(cGZPersistResourceKey const& key) override;
    bool AddLotConfiguration(cISC4LotConfiguration* config, bool add) override;
    bool RemoveLotConfiguration(uint32_t id) override;
    cISC4LotConfiguration* GetLotConfiguration(uint32_t id) override;
    uint32_t GetLotConfigurationsByFilter(fc4::GameHashSetU32<fc4::TractNodeAllocator>& configs,
                                          cISC4LotConfigurationFilter* filter) override;
    uint32_t GetLotConfigurationsByFamily(void* configs, uint32_t family) override;
    uint32_t GetLotConfigurationsBySize(void* configs, uint32_t x, uint32_t z) override;
    void ClearLotConfigurationSet(void* configs) override;
    uint32_t GetLotConfigurationIDsByFilter(void* ids, cISC4LotConfigurationFilter* filter) override;
    uint32_t GetLotConfigurationIDsByFamily(void* ids, uint32_t family) override;
    uint32_t GetLotConfigurationIDsBySize(void* ids, uint32_t x, uint32_t z) override;
    bool LotConfigurationHasBuildingFamily(cISC4LotConfiguration* config, uint32_t family,
                                           cSC4LotConfigurationObject** object) override;
    bool LotConfigurationHasBuildingType(cISC4LotConfiguration* config, uint32_t type,
                                         cSC4LotConfigurationObject** object) override;
    void RegisterLotConfigurationFilter(cISC4LotConfigurationFilter* filter) override;
    void UnregisterLotConfigurationFilter(cISC4LotConfigurationFilter* filter) override;
    void* GetLotConfigurationMap() override;
    void* GetLotConfigurationFamilyIDSet() override;
    bool CreateGenericLotConfiguration(cISC4LotConfiguration*& config, uint32_t id) override;
    uint32_t FixLotConfiguration(cISC4LotConfiguration* config) override;
    void GetMaxFootprintForBuildingFamilies(void const* families, cS3DRect2D& rect) override;
    void GetMaxFootprintForFloraFamilies(void const* families, cS3DRect2D& rect) override;
    void GetMaxFootprintForPropFamilies(void const* families, cS3DRect2D& rect) override;
    void GetObjectIDArray(cSC4LotConfigurationObject* object, void* ids) override;

    // cIGZMessageTarget2 (B4.3)
    bool DoMessage(cIGZMessage2* pMessage) override;

    // cIGZSerializable (B6)
    bool Write(cIGZOStream& gzOut) override;
    bool Read(cIGZIStream& gzIn) override;
    uint32_t GetGZCLSID() override;

    // For the host callbacks (cFC4LotConfigManager.cpp).
    cIGZMessageTarget2* Target() { return static_cast<cIGZMessageTarget2*>(this); }
    cIGZUnknown* Primary() {
        return static_cast<cIGZUnknown*>(static_cast<cFC4ISC4LotConfigurationManager*>(this));
    }
    fc4_lot_config_manager* Core() { return core_; }

private:
    uint32_t refCount_ = 0;
    bool destroying_ = false;
    bool initialised_ = false;
    fc4_lot_config_host host_{};
    fc4_lot_config_manager* core_ = nullptr;
};

// Factory for the subsystem table (reference count 0).
cIGZUnknown* CreateFC4LotConfigManager();

namespace fc4 {

// The process-wide configuration catalogue (created once, torn down and
// unloaded only at PreAppShutdown; shared by every FreeCity4 manager).
fc4_lot_config_catalogue* LotConfigCatalogueProcess();

// The director's PreAppShutdown: the process unload callback runs over the
// process catalogue (glue15: once per process, never per run), then the
// catalogue is destroyed.
void LotConfigManagerAppShutdown();

}  // namespace fc4
