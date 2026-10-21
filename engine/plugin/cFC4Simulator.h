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

// cFC4Simulator: the GZCOM face of the FreeCity4 simulator
// (docs/specs/simulator.md §1). All behaviour lives in the Rust core behind
// the C ABI in fc4.h; this class only adapts GZCOM interfaces to it.
#pragma once

#include "cFC4Date.h"
#include "cIGZMessageTarget2.h"
#include "cIGZSerializable.h"
#include "cIGZSystemService.h"
#include "FC4Interfaces.h"
#include "fc4.h"

#include <cstdint>
#include <map>
#include <string>
#include <utility>

class cIGZCheatCodeManager;
class cIGZMessageServer2;

// Class ID of the simulator (spec §1).
static const uint32_t kFC4_CLSID_cSC4Simulator = 0x2990C1E5;

class cFC4Simulator final : public cFC4ISC4Simulator,
                            public cIGZSerializable,
                            public cIGZSystemService,
                            public cIGZMessageTarget2 {
public:
    cFC4Simulator();
    ~cFC4Simulator();

    // cIGZUnknown
    bool QueryInterface(uint32_t riid, void** ppvObj) override;
    uint32_t AddRef() override;
    uint32_t Release() override;

    // cISC4Simulator in the game's slot order (FC4Interfaces.h; Init/Shutdown
    // also implement cIGZSystemService's)
    bool Init() override;
    bool Shutdown() override;
    bool GetSimStartDate(cFC4IGZDate& sDate) override;
    void GetSimDateParts(uint32_t* year, uint32_t* month, uint32_t* day, uint32_t* dayOfYear, uint32_t* weekDay) override;
    cFC4IGZDate* GetSimDate() override;
    int32_t GetSimDateNumber() override;
    bool Pause() override;
    bool HiddenPause() override;
    bool EmergencyPause() override;
    bool Resume() override;
    bool HiddenResume() override;
    bool EmergencyResume() override;
    bool IsPaused() override;
    bool IsHiddenPaused() override;
    bool IsEmergencyPaused() override;
    bool IsAnyPaused() override;
    bool AddAgent(cIGZMessageTarget2* pAgent, uint32_t agentType, cIGZString const& szAgentName, uint32_t flags) override;
    bool RemoveAgentAllLevels(cIGZMessageTarget2* pAgent) override;
    bool RemoveAgent(cIGZMessageTarget2* pAgent, uint32_t agentType) override;
    bool RemoveAllAgentsAt(uint32_t agentType) override;
    bool RemoveAllAgents() override;
    bool EnumerateAgentsByName(fc4::GameVector<fc4::GameString>& sAgents) override;
    bool GetAgentEnabled(cIGZString const& szAgentName) override;
    bool SetAgentEnabled(cIGZString const& szAgentName, bool bEnabled) override;
    int32_t GetSimSpeed() override;
    bool SetSimSpeed(int32_t lSpeed) override;
    int32_t GetSimTime() override;
    bool SetSimTime(int32_t lTime) override;
    bool SetMaxMillisecondsPerTick(uint32_t dwTime) override;
    float GetAnimationTimeDilation() override;
    bool SetCityEstablished(bool bEstablished) override;

    // cIGZSerializable
    bool Write(cIGZOStream& gzOut) override;
    bool Read(cIGZIStream& gzIn) override;
    uint32_t GetGZCLSID() override;

    // cIGZSystemService
    uint32_t GetServiceID() override;
    cIGZSystemService* SetServiceID(uint32_t dwServiceId) override;
    int32_t GetServicePriority() override;
    bool IsServiceRunning() override;
    cIGZSystemService* SetServiceRunning(bool bRunning) override;
    bool OnTick(uint32_t unknown1) override;
    bool OnIdle(uint32_t unknown1) override;
    int32_t GetServiceTickPriority() override;

    // cIGZMessageTarget2
    bool DoMessage(cIGZMessage2* pMessage) override;

    // Host callbacks for the core (fc4_host).
    static uint32_t HostNowMs(void* user);
    static bool HostDeliver(void* user, fc4_sim* sim, fc4_agent agent, const fc4_message* msg);
    static void HostAddRef(void* user, fc4_agent agent);
    static void HostRelease(void* user, fc4_agent agent);
    static void HostPost(void* user, const fc4_message* msg);
    // Deterministic pacing (FC4Pacing.h; fc4_pacing_hooks): our agents are
    // paced; the day's settle point sends the deferred posts.
    static bool HostIsPaced(void* user, fc4_agent agent);
    static void HostSettle(void* user, int32_t julian);
    static uint32_t HostIdleSteps(void* user, fc4_agent agent);

    // Limit-timer calibration sources/sink (fc4_timer_source, spec §5.5).
    static uint64_t TimerQpc(void* user);
    static uint64_t TimerTsc(void* user);
    static void TimerPublish(void* user, float cyclesToMs, float msToCycles);

    // Tooling: the turbo clock (FC4Turbo.h; not simulator.md). `untilJulian`
    // 0 = no end date; `why` is a literal for the log line.
    void SetTurbo(bool on, int32_t untilJulian, const char* why);
    // A turbo request: engaged now if this city load has seen ConnectionsReady,
    // else held until it (or sim_turbo_wait_days game days). True if engaged now.
    bool RequestTurbo(int32_t untilJulian, const char* why);
    // Drops a held request; false if none was held.
    bool DropHeldTurbo(const char* why);
    // The observer saw ConnectionsReady: the city is ready; engages a held request.
    void TurboCityReady();

private:
    void EngageHeldTurbo(bool fallback);
    void TurboAfterTick();
    void TurboMonthLine(int32_t julian);
    void TurboResetReference(int32_t julian);
    fc4_tuning ReadTuning(uint32_t (&msPerDay)[3], size_t& msCount, float (&dilation)[5], size_t& dilCount);
    void RegisterCheats();
    void UnregisterCheats();
    void Post(const fc4_message& msg);
    bool QueryInterfaceImpl(uint32_t riid, void** ppvObj);
    bool TracePause(const char* name, bool ok);
    void InstallTimerSource();
    void InstallPacing();  // fc4_sim_set_pacing with the process mode (FC4Pacing.h)
    void LogCalibration(const char* when);

    // Delivery diagnostics (FC4_LOG_LEVEL=2), keyed by (agent, level).
    struct AgentStats {
        std::string name;
        unsigned long long deliveries = 0;
        unsigned long long falses = 0;
        uint32_t streak = 0;  // consecutive "not finished" results
        uint32_t maxStreak = 0;
    };
    AgentStats& Stats(cIGZMessageTarget2* agent, uint32_t level);
    static uint32_t LevelOfType(uint32_t type);
    void RecordDelivery(cIGZMessageTarget2* agent, const fc4_message& msg, bool finished);
    void LogSchedulerStatus();

    uint32_t refCount_ = 0;
    uint32_t serviceId_;
    bool serviceRunning_ = false;
    fc4_sim* sim_ = nullptr;
    cFC4Date date_;
    cIGZMessageServer2* msgServer_ = nullptr;
    cIGZCheatCodeManager* cheatMgr_ = nullptr;
    bool simDateCheatRegistered_ = false;
    bool commandRegistered_ = false;
    bool addedToFramework_ = false;
    bool postCityInitSubscribed_ = false;
    float lastCyclesToMs_ = 0.0f;
    float lastMsToCycles_ = 0.0f;
    bool limitTimersWritten_ = false;
    // Diagnostics counters (FC4_LOG_LEVEL=2).
    uint32_t qiLogged_ = 0;
    uint32_t ticks_ = 0;
    uint32_t messagesLogged_ = 0;
    uint32_t deliveriesLogged_ = 0;
    uint32_t postsLogged_ = 0;
    uint32_t dateCallsLogged_[3] = {0, 0, 0};
    std::map<std::pair<uintptr_t, uint32_t>, AgentStats> agentStats_;
    // The turbo clock (tooling): nothing is polled or logged until it was on once.
    bool turboUsed_ = false;
    uint32_t turboRefMs_ = 0;      // HostNowMs at the last month line (or when switched on)
    uint32_t turboRefTicks_ = 0;   // ticks_ then
    int32_t turboRefJulian_ = 0;   // the date then
    // The hold until the city is ready (FC4Turbo.h): reset at each Init.
    // Deterministic pacing: the fixed-step idle agents (original or ours) paced with a fixed
    // number of SimIdle per day (FC4Pacing.h PacingFixedIdleSteps).
    std::map<uintptr_t, uint32_t> fixedIdle_;
    bool cityReady_ = false;          // this load's first ConnectionsReady was seen
    bool turboHeld_ = false;          // a request waits for it
    int32_t turboHeldUntil_ = 0;      // its end date (0 = none)
    int32_t turboHeldSince_ = 0;      // the date of the request (the fallback counts from it)
    const char* turboHeldWhy_ = "";   // a literal: sim_turbo, script, ...
};
