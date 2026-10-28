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

// The dispatch manager's GZCOM class (docs/specs/dispatch.md §1): city
// component 3, the cISC4City slot 39 getter. The record, the list and the
// messages live in the Rust core (fc4_dispatch); this class adapts the
// interfaces to it and owns the game services it uses.
//
// Layout (§1): cISC4DispatchManager and cIGZUnknown at +0, the
// cIGZMessageTarget2 sub-object at +4, cIGZSerializable at +8.
// QueryInterface answers 1 and 0x694028EC at +0, 0x452294AA at +4 and
// 0xE4FDA3D4 at +8 (the core's fc4_dispatch_query_interface gives the
// sub-object offset).
//
// Threading: core state uses a mutex for reads and writes. Getters run from a second thread while
// OnTick runs (HANDOFF §5.1), so nothing here lives in a RefCell.
//
// OFF by default: FreeCity4.ini `enable = dispatch`; `disable` still wins.
#pragma once

#include "FC4Interfaces.h"
#include "cIGZMessageTarget2.h"
#include "cIGZSerializable.h"
#include "fc4.h"

#include <atomic>
#include <cstdint>

class cIGZMessageServer2;
class cISC4City;
class cISC43DRender;

static const uint32_t kFC4_CLSID_cSC4DispatchManager = 0x4990BD93;
static const uint32_t kFC4_IID_cISC4DispatchManager = 0x694028EC;

class cFC4DispatchManager final : public cFC4ISC4DispatchManager,
                                  public cIGZMessageTarget2,
                                  public cIGZSerializable {
public:
    cFC4DispatchManager();
    ~cFC4DispatchManager();

    // cIGZUnknown
    bool QueryInterface(uint32_t riid, void** ppvObj) override;
    uint32_t AddRef() override;
    uint32_t Release() override;

    // cISC4DispatchManager (§8, the compiled order)
    bool Init() override;
    bool Shutdown() override;
    bool CanDispatch(int32_t type, float x, float z) override;
    uint32_t Dispatch(int32_t type, float x, float z) override;
    bool RevokeDispatchByID(uint32_t id) override;
    bool RevokeDispatchByPylon(cISC4PylonOccupant* pylon) override;
    bool RevokeAllDispatches(int32_t type) override;
    bool GetDispatchInfo(uint32_t id, FC4DispatchInfo& info) override;
    uint32_t GetDispatchCount(int32_t type) override;
    uint32_t GetDispatchIDArray(int32_t type, uint32_t* ids, uint32_t& capacity) override;
    bool RegisterDispatchSource(cISC4DispatchSource* source) override;
    bool UnregisterDispatchSource(cISC4DispatchSource* source) override;
    cISC4DispatchSource* GetDispatchSource(int32_t type) override;
    bool BeginDispatchTool(int32_t type) override;
    bool EndDispatchTool(int32_t type) override;
    cISC4PylonOccupant* GetActivePylon() override;
    void SetActivePylon(cISC4PylonOccupant* pylon) override;
    cISC4Automaton* GetAutomatonFromPylon(cISC4PylonOccupant* pylon) override;
    uint32_t GetAvailablePylonCount(int32_t type) override;
    void* GetDispatchVehicleView() override;

    // cIGZMessageTarget2 (§3)
    bool DoMessage(cIGZMessage2* pMessage) override;

    // cIGZSerializable (§7)
    bool Write(cIGZOStream& gzOut) override;
    bool Read(cIGZIStream& gzIn) override;
    uint32_t GetGZCLSID() override;

    // For the host callbacks (cFC4Dispatch.cpp).
    cIGZMessageTarget2* Target() { return static_cast<cIGZMessageTarget2*>(this); }
    cIGZUnknown* Primary() { return static_cast<cIGZUnknown*>(static_cast<cFC4ISC4DispatchManager*>(this)); }

    // The host's view of the game services the core asks for.
    bool msgServerHeld = false;
    cIGZMessageServer2* Server();
    void SetServer(cIGZMessageServer2* ms);
    cISC4City* City();
    void SetCity(cISC4City* c);
    void CountPost();
    cIGZUnknown* NativeVehicleView() { return vehicleView_; }
    cISC43DRender* ViewRenderer() { return viewRenderer_; }
    void SetNativeVehicleView(cIGZUnknown* view, cISC43DRender* renderer) {
        vehicleView_ = view;
        viewRenderer_ = renderer;
    }

private:
    uint32_t refCount_ = 0;
    bool destroying_ = false;
    fc4_dispatch_host host_{};
    fc4_dispatch* core_ = nullptr;
    cISC4City* city_ = nullptr;
    cIGZMessageServer2* msgServer_ = nullptr;
    std::atomic<uint32_t> posts_{0};
    // The core owns the view token; the host holds its renderer until removal.
    cIGZUnknown* vehicleView_ = nullptr;
    cISC43DRender* viewRenderer_ = nullptr;
};

// Factory for the subsystem table (reference count 0).
cIGZUnknown* CreateFC4DispatchManager();
