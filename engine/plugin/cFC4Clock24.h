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

// The 24-hour clock's GZCOM class (docs/specs/clock24.md §1): city component
// 50, the cISC4City slot 86 getter. The time of day, the step, the pause
// counters, the cheats and the record live in the Rust core (fc4_clock24);
// this class adapts the interfaces to it and owns the game services it uses.
//
// Layout (§1): cISC424HourClock and cIGZUnknown at +0, cIGZMessageTarget2 at
// +4 (the cheat notifications), cIGZSerializable at +8 and the system service
// (cIGZSystemService, service ID 0xA7E7F92A, priority -2,000,000) at +0xC.
// QueryInterface answers those five IIDs (0x287FB697 is the system-service
// base's answer, §1) and nothing else.
//
// The step (§4.3): Init adds the system service to the framework and to its
// on-idle list (AddSystemService, AddToOnIdle: the same framework calls as
// cFC4Simulator's AddToTick registration), so the framework's idle loop calls
// OnIdle. The clock is real-time driven: the turbo clock (sim_turbo) does not
// speed it up (it has no simulator agents).
//
// Threading: one CoreLock (FC4HistClock.h). The getters are read from other
// threads (audio, rendering); every game call a core call asks for (the
// framework, the cheat manager, the posts) is queued and made once the
// section is left.
//
// OFF by default: FreeCity4.ini `enable = clock24`; `disable` still wins.
#pragma once

#include "FC4HistClock.h"
#include "FC4Interfaces.h"
#include "cIGZMessageTarget2.h"
#include "cIGZSerializable.h"
#include "cIGZSystemService.h"
#include "fc4.h"

#include <atomic>
#include <cstdint>

class cIGZCheatCodeManager;

static const uint32_t kFC4_CLSID_cSC424HourClock = 0xA7E7F929;
static const uint32_t kFC4_IID_cISC424HourClock = 0x681BD53A;
static const uint32_t kFC4_Clock24ServiceID = 0xA7E7F92A;
static const int32_t kFC4_Clock24ServicePriority = -2000000;  // 0xFFE17B80

class cFC4Clock24 final : public cFC4ISC424HourClock,
                          public cIGZMessageTarget2,
                          public cIGZSerializable,
                          public cIGZSystemService {
public:
    cFC4Clock24();
    ~cFC4Clock24();

    // cIGZUnknown (every sub-object)
    bool QueryInterface(uint32_t riid, void** ppvObj) override;
    uint32_t AddRef() override;
    uint32_t Release() override;

    // cISC424HourClock (§8) and the system service's Init / Shutdown thunks
    bool Init() override;
    bool Shutdown() override;
    uint32_t SetVisualTimeMode(int32_t mode) override;
    int32_t GetVisualTimeMode() override;
    void GetTimePercentage(float* out) override;
    uint32_t SetTimePercentage(float& p) override;
    void GetVisualTimePercentage(float* out) override;
    uint32_t GetHour() override;
    uint32_t SetHour(uint32_t hour) override;
    uint32_t GetVisualHour() override;
    uint32_t GetMinute() override;
    uint32_t SetMinute(uint32_t minute) override;
    uint32_t GetVisualMinute() override;
    uint32_t GetSeconds() override;
    uint32_t SetSeconds(uint32_t seconds) override;
    uint32_t GetVisualSeconds() override;
    uint32_t GetEventHour(uint32_t index) override;
    uint32_t GetTimeMagnification(float& out) override;
    bool SetTimeMagnification(float m) override;
    uint32_t GetTimeMagnificationMinMax(float& min, float& max) override;
    bool SetTimeMagnificationMinMax(float min, float max) override;
    bool Pause(bool on) override;
    bool IsPaused() override;
    bool HiddenPause(bool on) override;
    bool IsHiddenPaused() override;
    bool IsNight() override;
    void EnableDayNightTransitionNotifications(bool on) override;

    // cIGZMessageTarget2 (§3.2, §4.8)
    bool DoMessage(cIGZMessage2* pMessage) override;

    // cIGZSerializable (§7)
    bool Write(cIGZOStream& gzOut) override;
    bool Read(cIGZIStream& gzIn) override;
    uint32_t GetGZCLSID() override;

    // cIGZSystemService (§1, §4.3)
    uint32_t GetServiceID() override;
    cIGZSystemService* SetServiceID(uint32_t id) override;
    int32_t GetServicePriority() override;
    bool IsServiceRunning() override;
    cIGZSystemService* SetServiceRunning(bool running) override;
    bool OnTick(uint32_t unknown1) override;
    bool OnIdle(uint32_t unknown1) override;
    int32_t GetServiceTickPriority() override;

    // For the host callbacks (cFC4Clock24.cpp).
    cIGZMessageTarget2* Target() { return static_cast<cIGZMessageTarget2*>(this); }
    cIGZSystemService* Service() { return static_cast<cIGZSystemService*>(this); }
    cIGZUnknown* Primary() { return static_cast<cIGZUnknown*>(static_cast<cFC4ISC424HourClock*>(this)); }
    fc4::CoreLock& Lock() { return lock_; }
    bool HasCheatManager() const { return hasCheatManager_; }
    void Post(uint32_t type, uint32_t data2, uint32_t data3);
    void Framework(uint32_t op);
    void Cheat(uint32_t op, uint32_t id, const char* name);

    // Diagnostics (the shadow and the log): the core's state, under the lock.
    fc4_clock24_state State();
    uint32_t PostCount(uint32_t i) const { return i < 7 ? posts_[i].load() : 0; }

private:
    bool AcquireCheatManager();

    uint32_t refCount_ = 0;
    bool destroying_ = false;
    fc4::CoreLock lock_;
    fc4_clock24_host host_{};
    fc4_clock24* core_ = nullptr;
    cIGZCheatCodeManager* cheatMgr_ = nullptr;
    bool hasCheatManager_ = false;  // read by the core under the lock (set before the section)
    uint32_t serviceId_ = kFC4_Clock24ServiceID;
    bool serviceRunning_ = false;
    std::atomic<bool> initialized_{false};
    std::atomic<uint32_t> idles_{0};
    std::atomic<uint32_t> posts_[7] = {};  // by type 0x44778230 + i
};

// Factory for the subsystem table (reference count 0).
cIGZUnknown* CreateFC4Clock24();

// §6: the "Main Simulator" exemplar's clock properties (or the test hooks');
// false = no exemplar. Main thread (the class's Init and the shadow).
bool FC4Clock24ReadTuning(fc4_clock24_tuning& t);
