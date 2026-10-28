// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once

#include "cIGZUnknown.h"
#include "cISC4Occupant.h"
#include "cS3DVector3.h"
#include "fc4.h"

class cIGZString;

namespace fc4 {

// dispatch.md 4.1.1: the native primary has AsOccupant at slot 3.
// The SDK's cISC4PylonOccupant omits that slot and shifts every later call.
class DispatchPylon : public cIGZUnknown {
public:
    virtual cISC4Occupant* AsOccupant() = 0;
    virtual bool Init() = 0;
    virtual bool Shutdown() = 0;
    virtual void SetUIState(uint32_t) = 0;
    virtual uint32_t GetUIState() = 0;
    virtual void SetBitmapInfo(uint32_t, const cIGZString&) = 0;
    virtual void SetDispatchType(uint32_t) = 0;
    virtual uint32_t GetDispatchType() = 0;
    virtual void SetDispatchState(uint32_t) = 0;
    virtual uint32_t GetDispatchState() = 0;
    virtual void SetCallNumber(uint16_t) = 0;
    virtual uint16_t GetCallNumber() = 0;
    virtual void SetDragState(bool) = 0;
    virtual bool GetDragState() = 0;
    virtual void SetSourceOccupant(cISC4Occupant*) = 0;
    virtual cISC4Occupant* GetSourceOccupant() = 0;
    virtual void SetPylonOwner(cIGZUnknown*) = 0;
    virtual cIGZUnknown* GetPylonOwner() = 0;
};

inline DispatchPylon* Pylon(uint64_t token) {
    return reinterpret_cast<DispatchPylon*>(static_cast<uintptr_t>(token));
}

inline void WireDispatchPylon(fc4_dispatch_host& h) {
    h.pylon_source = [](void*, uint64_t p) -> uint64_t {
        return p ? reinterpret_cast<uintptr_t>(Pylon(p)->GetSourceOccupant()) : 0;
    };
    h.pylon_set_source = [](void*, uint64_t p, uint64_t source) {
        if (p) Pylon(p)->SetSourceOccupant(reinterpret_cast<cISC4Occupant*>(static_cast<uintptr_t>(source)));
    };
    h.pylon_call_number = [](void*, uint64_t p) -> uint32_t {
        return p ? Pylon(p)->GetCallNumber() : 0;
    };
    h.pylon_set_call_number = [](void*, uint64_t p, uint32_t n) {
        if (p) Pylon(p)->SetCallNumber(static_cast<uint16_t>(n));
    };
    h.pylon_dispatch_type = [](void*, uint64_t p) -> uint32_t {
        return p ? Pylon(p)->GetDispatchType() : 0;
    };
    h.pylon_set_dispatch_type = [](void*, uint64_t p, uint32_t type) {
        if (p) Pylon(p)->SetDispatchType(type);
    };
    h.pylon_set_dispatch_state = [](void*, uint64_t p, uint32_t state) {
        if (p) Pylon(p)->SetDispatchState(state);
    };
    h.pylon_set_ui_state = [](void*, uint64_t p, uint32_t state) {
        if (p) Pylon(p)->SetUIState(state);
    };
    h.pylon_move_to = [](void*, uint64_t p, float x, float y, float z) {
        if (p) {
            cISC4Occupant* occupant = Pylon(p)->AsOccupant();  // borrowed
            const cS3DVector3 position(x, y, z);
            if (occupant) occupant->SetPosition(&position);
        }
    };
    h.pylon_shutdown = [](void*, uint64_t p) {
        if (p) Pylon(p)->Shutdown();
    };
}

}  // namespace fc4
