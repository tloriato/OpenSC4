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

// Loads the real FreeCity4.dll the way a GZCOM host does (LoadLibrary +
// GZDllGetGZCOMDirector), without a framework, and drives the director and
// the simulator through their vtables. Then checks that the diagnostics log
// recorded the calls.
//
// Usage: fc4_plugin_load_test <path to FreeCity4.dll>

#include "cIGZCOMDirector.h"
#include "cIGZCOM.h"
#include "cISC4SimGrid.h"
#include "cISC4AuraSimulator.h"
#include "SC4Point.h"
#include "cIGZDate.h"
#include "cIGZMessage2Standard.h"
#include "cGZMessage.h"
#include "cIGZMessageTarget.h"
#include "cIGZMessageTarget2.h"
#include "cIGZSerializable.h"
#include "cS3DVector2.h"
#include "cIGZSystemService.h"
#include "cISC4BudgetSimulator.h"
#include "cISC4DepartmentBudget.h"
#include "cISC4LandValueSimulator.h"
#include "cISC4LineItem.h"
#include "cISC4Ordinance.h"
#include "cISC4OrdinanceSimulator.h"
#include "cISCPropertyHolder.h"
#include "cISCResExemplar.h"
#include "cISCResExemplarCohort.h"
#include "cRZBaseVariant.h"
#include "cSCBaseProperty.h"
#include "cFC4Variant.h"
#include "cISC4Simulator.h"
#include "cISC4NeighborConnection.h"
#include "cISC4NeighborDeal.h"
#include "cISC4TrafficSource.h"
#include "SC4List.h"
#include "SC4Percentage.h"
#include "cRZBaseString.h"
#include "FC4AuraShadow.h"
#include "FC4FloraShadow.h"
#include "SC4Rect.h"
#include "cFC4Flora.h"
#include "cISC4FloraSimulator.h"
#include "FC4BudgetShadow.h"
#include "FC4CrimeInterfaces.h"
#include "FC4CrimeShadow.h"
#include "FC4DemandShadow.h"
#include "FC4DevelopmentShadow.h"
#include "FC4Dialogs.h"
#include "FC4GameMap.h"
#include "FC4GrowthProbe.h"
#include "FC4HarnessLogic.h"
#include "cFC4Demand.h"
#include "FC4DevelopmentInterfaces.h"
#include "cGZPersistResourceKey.h"
#include "FC4PollutionShadow.h"
#include "FC4FlammabilityShadow.h"
#include "FC4LandValueShadow.h"
#include "FC4LandfillShadow.h"
#include "FC4TrafficShadow.h"
#include "FC4PlumbingShadow.h"
#include "FC4PowerShadow.h"
#include "FC4ResidentialShadow.h"
#include "FC4Streams.h"
#include "cS3DVector3.h"
#include "cIGZOStream.h"
#include "FC4Interfaces.h"
#include "FC4HistClock.h"
#include "FC4Clock24Shadow.h"
#include "FC4HistoryShadow.h"
#include "FC4TestSeam.h"
#include "cFC4TrafficSimulator.h"
#include "cFC4ZoneManager.h"
#include "cFC4TractDeveloper.h"
#include "cFC4LotDeveloper.h"
#include "cFC4ZoneDeveloper.h"
#include "FC4SafeLotIterator.h"
#include "cGZPersistResourceKey.h"
#include "fc4.h"
#include "game_abi.h"

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>

#include <algorithm>
#include <array>
#include <atomic>
#include <chrono>
#include <cmath>
#include <condition_variable>
#include <cstddef>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <map>
#include <memory>
#include <mutex>
#include <sstream>
#include <string>
#include <thread>
#include <tuple>
#include <utility>
#include <vector>

namespace {

int g_failures = 0;

#define CHECK(cond)                                                                       \
    do {                                                                                  \
        if (!(cond)) {                                                                    \
            std::fprintf(stderr, "%s:%d: CHECK failed: %s\n", __FILE__, __LINE__, #cond); \
            ++g_failures;                                                                 \
        }                                                                                 \
    } while (0)

constexpr uint32_t kCLSID_Simulator = 0x2990C1E5;
constexpr uint32_t kIID_cISC4Simulator = 0x8695664E;
constexpr uint32_t kDirectorID = 0x5C7A43E1;
constexpr uint32_t kCLSID_Budget = 0xE990BE01;
constexpr uint32_t kCLSID_Department = 0xE990BFFC;
constexpr uint32_t kCLSID_LineItem = 0xAA313C9F;
constexpr uint32_t kIID_cISC4BudgetSimulator = 0x6772BD23;
constexpr uint32_t kIID_cISC4DepartmentBudget = 0x223E2B67;
constexpr uint32_t kCLSID_LandValue = 0xE990C18E;
constexpr uint32_t kIID_cISC4LandValueSimulator = 0xE7781D8C;
constexpr uint32_t kCLSID_Weather = 0x2990C1BC;
constexpr uint32_t kIID_cISC4WeatherSimulator = 0xC6C66245;
constexpr uint32_t kCLSID_OrdinanceSim = 0xA990C08E;
constexpr uint32_t kIID_cISC4OrdinanceSimulator = 0x41193C3A;
constexpr uint32_t kCLSID_FreeClinics = 0x40D07236;
constexpr uint32_t kCLSID_DemandSim = 0x2990BFF2;
constexpr uint32_t kIID_cISC4DemandSimulator = 0x47967E16;
constexpr uint32_t kCLSID_Demand = 0x8990C12D;
constexpr uint32_t kIID_cISC4Demand = 0x0797A4E8;
constexpr uint32_t kCLSID_Plumbing = 0x0990C075;
constexpr uint32_t kIID_cISC4PlumbingSimulator = 0x47782CCC;
constexpr uint32_t kCLSID_Pollution = 0x8990C065;
constexpr uint32_t kIID_cISC4PollutionSimulator = 0x877181EC;
constexpr uint32_t kIID_cISC4PropRequester = 0x89C498BE;
constexpr uint32_t kCLSID_Crime = 0x8990C09A;
constexpr uint32_t kIID_cISC4CrimeSimulator = 0x67659555;

void CollectClass(uint32_t clsid, uint32_t, void* ctx) { static_cast<std::vector<uint32_t>*>(ctx)->push_back(clsid); }

// demand capacity seam (op 20): every (map, developer type, amount) reported.
void CollectCapacity(void* ctx, uint32_t map, uint32_t devType, uint16_t amount) {
    static_cast<std::vector<std::array<uint32_t, 3>>*>(ctx)->push_back({map, devType, amount});
}

// A building profile with one capacity map {0x1010: 12, 0x1020: 7, 0x1030: 3}, built as
// docs/abi/allocators.md ("map and set") and layouts.md (the vector at +0x0C) describe it:
// root 0x1020 black, 0x1010 and 0x1030 red; the high half of each value is junk.
struct FakeCapacityProfile {
    fc4::GameMapNode header{}, a{}, r{}, c{};
    fc4::GameMapObject map{};
    unsigned char bytes[0x40]{};
    FakeCapacityProfile() {
        header.color = fc4::kGameMapRed;
        header.parent = &r;
        header.left = &a;
        header.right = &c;
        r = {fc4::kGameMapBlack, {}, &header, &a, &c, 0x1020, 0xDEAD0007u};
        a = {fc4::kGameMapRed, {}, &r, nullptr, nullptr, 0x1010, 0xDEAD000Cu};
        c = {fc4::kGameMapRed, {}, &r, nullptr, nullptr, 0x1030, 0xDEAD0003u};
        map = {&header, 3, 0};
        SetVector(&map, &map + 1, &map + 1);
    }
    void SetVector(const void* begin, const void* end, const void* cap) {
        const void* v[3] = {begin, end, cap};
        std::memcpy(bytes + fc4::kProfileCapacityVectorOffset, v, sizeof(v));
    }
};

// A handled access violation in the HOST's code (not the DLL's): the DLL's
// vectored handler must log it with the call ring.
int ForeignFault() {
    __try {
        *static_cast<volatile int*>(nullptr) = 1;
    } __except (EXCEPTION_EXECUTE_HANDLER) {
        return 1;
    }
    return 0;
}

// Another faulting address, with one value (1.0) on the x87 stack at the
// fault: the crash report's fpu line must show it (TOP 7, one register).
int ForeignFaultX87() {
    int r = 0;
    __asm fld1
    __try {
        *static_cast<volatile int*>(nullptr) = 2;
    } __except (EXCEPTION_EXECUTE_HANDLER) {
        r = 1;
    }
    __asm fstp st(0)
    return r;
}

// A building's property holder with just what the budget reads and writes
// (budget-item arrays), laid out like the game's (foundations §17.2): the
// plugin calls holders only through those slots.
class FakeHolder : public cFC4ISCPropertyHolder {
public:
    virtual ~FakeHolder() { RemoveAllProperties(false); }
    bool QueryInterface(uint32_t riid, void** pp) override {
        if (riid != GZIID_cISCPropertyHolder && riid != GZIID_cIGZUnknown) return false;
        *pp = this;
        AddRef();
        return true;
    }
    uint32_t AddRef() override { return ++refs; }
    uint32_t Release() override { return --refs; }  // stack object: never deleted

    bool HasProperty(uint32_t id) const override { return props.count(id) != 0; }
    bool GetPropertyList(cIGZUnknownList**) const override { return false; }
    bool GetPropertyData(uint32_t, void*, uint32_t&) const override { return false; }
    bool GetPropertyInterface(uint32_t, uint32_t, void**) const override { return false; }
    bool GetPropertyString(uint32_t, cIGZString&) const override { return false; }
    bool GetPropertyU32(uint32_t, uint32_t&) const override { return false; }
    cISCProperty* GetProperty(uint32_t id) const override {
        auto it = props.find(id);
        return it == props.end() ? nullptr : it->second;
    }
    bool AddPropertyData(uint32_t, void*, uint32_t, bool) override { return false; }
    bool AddPropertyObject(uint32_t, cIGZUnknown*, bool) override { return false; }
    bool AddPropertyString(uint32_t, cIGZString const&, bool) override { return false; }
    bool AddPropertyU32(uint32_t, uint32_t, bool) override { return false; }
    bool AddPropertyVariant(uint32_t id, cFC4IGZVariant const* fv, bool) override {
        cIGZVariant const* v = fc4::AsSdk(fv);  // a game holder copies through the SDK slots
        RemoveProperty(id, false);
        cSCBaseProperty* p = new cSCBaseProperty(id, v);  // deep copy
        p->AddRef();
        props[id] = p;
        return true;
    }
    bool AddPropertyRecord(cISCProperty*, bool) override { return false; }
    bool CopyAddProperty(cISCProperty*, bool) override { return false; }
    bool RemoveProperty(uint32_t id, bool) override {
        auto it = props.find(id);
        if (it == props.end()) return false;
        it->second->Release();
        props.erase(it);
        return true;
    }
    bool RemoveAllProperties(bool) override {
        for (auto& kv : props) kv.second->Release();
        props.clear();
        return true;
    }
    bool EnumPropertiesFiltered(FilterFn, EnumFn, void*) const override { return false; }
    bool EnumProperties(EnumFn, void*) const override { return false; }
    bool CompactProperties() override { return true; }

    cISCPropertyHolder* AsSdk() { return reinterpret_cast<cISCPropertyHolder*>(static_cast<cFC4ISCPropertyHolder*>(this)); }

    template <class T>
    void Set(uint32_t id, T value) {
        cRZBaseVariant v;
        SetArray(v, &value);
        AddPropertyVariant(id, fc4::AsGame(static_cast<cIGZVariant const*>(&v)), false);
    }
    float Get32f(uint32_t id) const {
        const cISCProperty* p = GetProperty(id);
        return p != nullptr ? p->GetPropertyValue()->RefFloat32()[0] : -1.0f;
    }

    uint32_t refs = 1;

private:
    static void SetArray(cRZBaseVariant& v, uint32_t* x) { v.RefUint32(x, 1); }
    static void SetArray(cRZBaseVariant& v, int64_t* x) { v.RefSint64(x, 1); }
    static void SetArray(cRZBaseVariant& v, float* x) { v.RefFloat32(x, 1); }
    std::map<uint32_t, cSCBaseProperty*> props;
};

// A day agent that reports "not finished" for its first 150 calls.
class SlowAgent final : public cIGZMessageTarget2 {
public:
    uint32_t refs = 0;
    uint32_t calls = 0;
    uint32_t badMessages = 0;
    int32_t lastJulian = 0;
    bool QueryInterface(uint32_t riid, void** pp) override {
        if (riid == GZIID_cIGZUnknown) {
            *pp = static_cast<cIGZUnknown*>(this);
            AddRef();
            return true;
        }
        return false;
    }
    uint32_t AddRef() override { return ++refs; }
    uint32_t Release() override { return refs > 0 ? --refs : 0; }
    bool DoMessage(cIGZMessage2* msg) override {
        ++calls;
        // Read the message the way game agents do: cast to the standard interface.
        auto* m = static_cast<cIGZMessage2Standard*>(msg);
        if (msg->GetType() != 0x66956814 || !m->GetHasData1() || m->GetData2() != 0 || m->GetData1() < 2451545) {
            ++badMessages;
        }
        lastJulian = static_cast<int32_t>(m->GetData1());
        return calls > 150;
    }
};

// A day agent whose first call blocks for 6.5 s (the watchdog's stall threshold is 5 s).
class BlockingAgent final : public cIGZMessageTarget2 {
public:
    uint32_t refs = 0;
    uint32_t calls = 0;
    bool QueryInterface(uint32_t, void**) override { return false; }
    uint32_t AddRef() override { return ++refs; }
    uint32_t Release() override { return refs > 0 ? --refs : 0; }
    bool DoMessage(cIGZMessage2*) override {
        if (calls++ == 0) Sleep(6500);
        return true;
    }
};

// A DLL plugin's ordinance (ordinance §10.1): the simulator calls it through
// the interface only.
class FakeOrdinance final : public cFC4ISC4Ordinance {
public:
    uint32_t refs = 0;
    bool available = true, on = false, enabled = true;
    cRZBaseString name{"Fake Ordinance"};
    bool QueryInterface(uint32_t, void**) override { return false; }
    uint32_t AddRef() override { return ++refs; }
    uint32_t Release() override { return refs > 0 ? --refs : 0; }
    bool Init() override { return true; }
    bool Shutdown() override { return true; }
    int64_t GetCurrentMonthlyIncome() override { return -7; }
    uint32_t GetID() const override { return 0x12345678; }
    cIGZString* GetName() override { return &name; }
    cIGZString* GetDescription() override { return &name; }
    uint32_t GetYearFirstAvailable() override { return 0; }
    uint32_t GetChanceAvailability() override { return 0; }
    int64_t GetEnactmentIncome() override { return -10; }
    int64_t GetRetracmentIncome() override { return -5; }
    int64_t GetMonthlyConstantIncome() override { return -7; }
    float GetMonthlyIncomeFactor() override { return 0.0f; }
    uint32_t GetAdvisorID() override { return 0; }
    cISCPropertyHolder* GetMiscProperties() override { return nullptr; }
    bool IsAvailable() override { return available; }
    bool IsOn() override { return available && on; }
    bool IsEnabled() override { return enabled; }
    int64_t GetMonthlyAdjustedIncome() override { return -7; }
    bool CheckConditions() override { return enabled; }
    bool IsIncomeOrdinance() override { return false; }
    bool Simulate() override { return true; }
    bool SetAvailable(bool v) override { return available = v, true; }
    bool SetOn(bool v) override { return on = v, true; }
    bool SetEnabled(bool v) override { return enabled = v, true; }
    bool ForceAvailable(bool v) override { return SetAvailable(v); }
    bool ForceOn(bool v) override { return SetOn(v); }
    bool ForceMonthlyAdjustedIncome(int64_t) override { return true; }
};

// A message with only a type (the ordinance simulator reads nothing else, §5).
class TypeMessage final : public cIGZMessage2 {
public:
    explicit TypeMessage(uint32_t t) : type(t) {}
    uint32_t type;
    bool QueryInterface(uint32_t, void**) override { return false; }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    bool Create(uint32_t, void**) const override { return false; }
    uint32_t GetType() const override { return type; }
    void SetType(uint32_t t) override { type = t; }
    bool operator==(cIGZMessage2 const&) const override { return false; }
    bool operator<(cIGZMessage2 const&) const override { return false; }
};

size_t CountOf(const std::string& text, const std::string& what) {
    size_t n = 0;
    for (size_t at = text.find(what); at != std::string::npos; at = text.find(what, at + 1)) ++n;
    return n;
}

std::string ReadFile(const std::string& path) {
    std::ifstream f(path, std::ios::binary);
    std::stringstream ss;
    ss << f.rdbuf();
    return ss.str();
}

void WriteTextFile(const std::string& path, const std::string& text) {
    std::ofstream f(path, std::ios::binary | std::ios::trunc);
    f << text;
}

// ---- budget §10.1: what the posted messages carry (test seam op 1) ----------------------------

struct PostedMessage {
    uint32_t type;
    void* void1;
    void* unknown;
    intptr_t d1, d2, d3;
};

void RecordPost(void* ctx, cIGZMessage2Standard* m) {
    static_cast<std::vector<PostedMessage>*>(ctx)->push_back(
        {m->GetType(), m->GetVoid1(), static_cast<void*>(m->GetIGZUnknown()), m->GetData1(), m->GetData2(),
         m->GetData3()});
}

size_t CountPosts(const std::vector<PostedMessage>& posts, uint32_t type) {
    size_t n = 0;
    for (const PostedMessage& p : posts) n += p.type == type ? 1 : 0;
    return n;
}

// ---- the x87 control word (foundations §18.2) ---------------------------------------------------
// The game runs its main thread at 24-bit x87 precision; nothing in
// FreeCity4.dll may change that. The normal run sets it like the game and
// checks it after every stage.

uint16_t RawCw() {
    uint16_t cw = 0;
    __asm fnstcw cw
    return cw;
}

// The x87 stack top (status word bits 11-13): unchanged across a call that
// leaves the stack balanced.
unsigned X87Top() {
    uint16_t sw = 0;
    __asm fnstsw sw
    return (sw >> 11) & 7u;
}

// A lot whose slot 15 behaves like cSC4Lot::GetCenter (crime.md §13, CQ3):
// thiscall with one stack argument (here __fastcall with an unused EDX), the
// result in the out parameter, EAX = &out, nothing left on the x87 stack.
void* __fastcall FakeLotGetCenter(void*, void*, float* out) {
    out[0] = 256.5f;
    out[1] = 10.0f;
    out[2] = 512.25f;
    return out;
}
// The docs/abi reading (a float in ST0), to exercise the glue's diagnostic.
float __fastcall FakeLotGetCenterSt0(void*, void*, float* out) {
    out[0] = 1.0f;
    out[1] = 2.0f;
    out[2] = 3.0f;
    return 7.0f;
}
struct FakeLotObject {
    void* const* vtable;
};

uint16_t g_cwExpected = 0;
bool g_cwChanged = false;

void SetGamePrecision() {
    unsigned int unused = 0;
    _controlfp_s(&unused, _PC_24, _MCW_PC);
    g_cwExpected = RawCw();
}

void CheckCw(const char* where) {
    const uint16_t cw = RawCw();
    if (cw != g_cwExpected && !g_cwChanged) {
        g_cwChanged = true;
        std::fprintf(stderr, "x87 control word changed by FreeCity4.dll (%s): 0x%04X, expected 0x%04X\n", where, cw,
                     g_cwExpected);
        ++g_failures;
    }
}

// ---- a fake city for the observer (test seam op 2) -------------------------------------------
// Game objects are called through their vtables only, so a fake is a table of
// __fastcall functions (ECX = this, like __thiscall; callee-pop) with traps in
// every slot the observer must not call.

template <size_t N>
void __fastcall Trap(void*, void*) {
    std::fprintf(stderr, "fake game object: unexpected call of slot %u\n", static_cast<unsigned>(N));
    std::abort();
}

template <size_t... I>
std::array<void*, sizeof...(I)> MakeTraps(std::index_sequence<I...>) {
    return {{reinterpret_cast<void*>(&Trap<I>)...}};
}

struct FakeVtable {
    std::array<void*, 160> slots = MakeTraps(std::make_index_sequence<160>());
};

struct FakeObject {
    void** vtbl;
};

struct FakeDemand {
    void** vtbl;
    uint32_t id;
    float supply;
    float demand;
};

struct FakeWorld {
    void* sim = nullptr;
    void* budget = nullptr;
    int demandCalls = 0;
    int demandBadIndex = 0;
    int saveCalls = 0;
    bool noRes = false;  // GetResidentialSimulator returns null (observe month: residential EQ/HQ/LE=-)
    FakeVtable cityVt, resVt, demandSimVt, demandVt, appVt;
    FakeObject city{}, res{}, demandSim{}, app{};
    FakeDemand demands[7] = {
        {nullptr, 0x1010, 1000.0f, 250.5f},  // R$
        {nullptr, 0x3110, 300.0f, -12.0f},   // Cs$
        {nullptr, 0x3320, 200.0f, 40.0f},    // Co$$
        {nullptr, 0x4100, 150.0f, 7.5f},     // I-R
        {nullptr, 0x4400, 50.0f, 0.0f},      // I-HT
        {nullptr, 0x3001, 480.0f, 0.0f},     // C census (P1: the digest reads its supply)
        {nullptr, 0x4001, 160.0f, 0.0f},     // I census
    };
};

FakeWorld* g_world = nullptr;

void* __fastcall CityGetSimulator(void*, void*) { return g_world->sim; }                   // cISC4City 50
void* __fastcall CityGetBudgetSimulator(void*, void*) { return g_world->budget; }          // cISC4City 52
void* __fastcall CityGetDemandSimulator(void*, void*) { return &g_world->demandSim; }      // cISC4City 56
void* __fastcall CityGetResidentialSimulator(void*, void*) {                                // cISC4City 68
    return g_world->noRes ? nullptr : &g_world->res;
}
void* __fastcall CityGetPowerSimulator(void*, void*) { return nullptr; }                  // cISC4City 67 (none)
int32_t __fastcall ResGetPopulation(void*, void*) { return 1234; }                          // residential 16
float __fastcall ResGetGlobalEQ(void*, void*) { return 12.5f; }                              // residential 32 (ST0)
float __fastcall ResGetGlobalHQ(void*, void*) { return 34.25f; }                             // residential 33 (ST0)
// Residential 34 returns in AL only (residential §10.1): the upper bits of EAX
// are not part of the value, so the observer must print 0x4B = 75.
uint32_t __fastcall ResGetGlobalLE(void*, void*) { return 0xABCDEF4Bu; }
void* __fastcall DemandSimGetDemand(void*, void*, uint32_t id, uint32_t index) {           // demand sim 6
    ++g_world->demandCalls;
    if (index != 0) ++g_world->demandBadIndex;
    for (FakeDemand& d : g_world->demands) {
        if (d.id == id) return &d;
    }
    return nullptr;
}
// cISC4App 16 = SaveCity(bool) (pop 4); true only for a full save, so the
// log line "returned 1" shows the argument arrived.
bool __fastcall AppSaveCity(void*, void*, bool fast) {
    ++g_world->saveCalls;
    return !fast;
}
float __fastcall DemandQuerySupply(void* self, void*) { return static_cast<FakeDemand*>(self)->supply; }  // 8
float __fastcall DemandQueryDemand(void* self, void*) { return static_cast<FakeDemand*>(self)->demand; }  // 9

void InitFakeWorld(FakeWorld& w) {
    g_world = &w;
    w.cityVt.slots[50] = reinterpret_cast<void*>(&CityGetSimulator);
    w.cityVt.slots[52] = reinterpret_cast<void*>(&CityGetBudgetSimulator);
    w.cityVt.slots[56] = reinterpret_cast<void*>(&CityGetDemandSimulator);
    w.cityVt.slots[68] = reinterpret_cast<void*>(&CityGetResidentialSimulator);
    w.cityVt.slots[67] = reinterpret_cast<void*>(&CityGetPowerSimulator);  // observe month: power=-
    w.resVt.slots[16] = reinterpret_cast<void*>(&ResGetPopulation);
    w.resVt.slots[32] = reinterpret_cast<void*>(&ResGetGlobalEQ);
    w.resVt.slots[33] = reinterpret_cast<void*>(&ResGetGlobalHQ);
    w.resVt.slots[34] = reinterpret_cast<void*>(&ResGetGlobalLE);
    w.demandSimVt.slots[6] = reinterpret_cast<void*>(&DemandSimGetDemand);
    w.demandVt.slots[8] = reinterpret_cast<void*>(&DemandQuerySupply);
    w.demandVt.slots[9] = reinterpret_cast<void*>(&DemandQueryDemand);
    w.appVt.slots[16] = reinterpret_cast<void*>(&AppSaveCity);
    w.app.vtbl = w.appVt.slots.data();
    w.city.vtbl = w.cityVt.slots.data();
    w.res.vtbl = w.resVt.slots.data();
    w.demandSim.vtbl = w.demandSimVt.slots.data();
    for (FakeDemand& d : w.demands) d.vtbl = w.demandVt.slots.data();
}

// ---- the flammability fire path (run 37) ---------------------------------------------------------
// A fire manager as the game hands it out: the disaster layer returns it
// BORROWED (flammability.md §6.8); a QueryInterface for
// cISC4DisasterManagerFire returns a sub-object that shares its count, with one
// AddRef. `refs` starts at the layer's own reference.
struct FakeFireCity {
    FakeVtable cityVt, layerVt, mgrVt, fireVt;
    FakeObject city{}, layer{}, mgr{}, fire{};
    int refs = 1;
    int qis = 0, ignites = 0, causes = 0;  // qis: for cISC4DisasterManagerFire only
    uint32_t cause = 0;
};

FakeFireCity* g_fire = nullptr;

void* __fastcall FireCityGetDisasterLayer(void*, void*) { return &g_fire->layer; }  // cISC4City 72
void* __fastcall FireLayerGetManager(void*, void*, uint32_t type) {                  // layer 7
    return type == 0x621CDA33 ? &g_fire->mgr : nullptr;
}
bool __fastcall FireMgrQi(void*, void*, uint32_t iid, void** out) {  // 0, both interfaces
    if (iid != 0x49B43F57 || out == nullptr) return false;  // (the reference audit asks other IIDs)
    ++g_fire->qis;
    *out = &g_fire->fire;
    ++g_fire->refs;
    return true;
}
uint32_t __fastcall FireMgrAddRef(void*, void*) { return static_cast<uint32_t>(++g_fire->refs); }   // 1
uint32_t __fastcall FireMgrRelease(void*, void*) { return static_cast<uint32_t>(--g_fire->refs); }  // 2
bool __fastcall FireIgniteCell(void*, void*, uint32_t, uint32_t, bool, bool, uint32_t) {  // fire 5
    ++g_fire->ignites;
    return true;
}
uint32_t __fastcall FireSetCause(void*, void*, uint32_t c) {  // fire 9
    ++g_fire->causes;
    g_fire->cause = c;
    return 0;
}

void InitFakeFireCity(FakeFireCity& f) {
    g_fire = &f;
    f.cityVt.slots[72] = reinterpret_cast<void*>(&FireCityGetDisasterLayer);
    f.layerVt.slots[7] = reinterpret_cast<void*>(&FireLayerGetManager);
    for (FakeVtable* vt : {&f.mgrVt, &f.fireVt}) {
        vt->slots[0] = reinterpret_cast<void*>(&FireMgrQi);
        vt->slots[1] = reinterpret_cast<void*>(&FireMgrAddRef);
        vt->slots[2] = reinterpret_cast<void*>(&FireMgrRelease);
    }
    f.fireVt.slots[5] = reinterpret_cast<void*>(&FireIgniteCell);
    f.fireVt.slots[9] = reinterpret_cast<void*>(&FireSetCause);
    f.city.vtbl = f.cityVt.slots.data();
    f.layer.vtbl = f.layerVt.slots.data();
    f.mgr.vtbl = f.mgrVt.slots.data();
    f.fire.vtbl = f.fireVt.slots.data();
}

// ---- city slot 97 and the pipe burst (test seam ops 28, 29) ------------------------------------
// The game's CellCenterToPosition (city slot 97) returns nothing (docs/abi,
// corrected 2026-10-14): a void __fastcall popping 16 here. A caller that
// declares it float pops an empty x87 register, which moves TOP.
void __fastcall CityCellCenterToPosition(void*, void*, int32_t cx, int32_t cz, float* x, float* z) {
    *x = static_cast<float>(cx) * 16.0f + 8.0f;
    *z = static_cast<float>(cz) * 16.0f + 8.0f;
}

// The pipe-burst manager (plumbing.md §7.1 step 4) behind the disaster layer;
// AttemptDisasterStart (slot 6) hands back the manager's own instance, which
// the caller must not release.
struct FakePipeCity {
    FakeVtable cityVt, layerVt, mgrVt, instVt;
    FakeObject city{}, layer{}, mgr{}, inst{};
    int instRefs = 1;  // the manager's own reference
    int attempts = 0;
    int32_t x = 0, z = 0;
};

FakePipeCity* g_pipe = nullptr;

void* __fastcall PipeCityGetDisasterLayer(void*, void*) { return &g_pipe->layer; }  // cISC4City 72
void* __fastcall PipeLayerGetManager(void*, void*, uint32_t type) {                  // layer 7
    return type == 0x4A1332E8 ? &g_pipe->mgr : nullptr;
}
void* __fastcall PipeMgrAttempt(void*, void*, int32_t x, int32_t z) {  // manager 6 (pop 8)
    ++g_pipe->attempts;
    g_pipe->x = x;
    g_pipe->z = z;
    return &g_pipe->inst;
}
bool __fastcall PipeInstQi(void*, void*, uint32_t, void**) { return false; }                        // 0
uint32_t __fastcall PipeInstAddRef(void*, void*) { return static_cast<uint32_t>(++g_pipe->instRefs); }   // 1
uint32_t __fastcall PipeInstRelease(void*, void*) { return static_cast<uint32_t>(--g_pipe->instRefs); }  // 2

void InitFakePipeCity(FakePipeCity& p) {
    g_pipe = &p;
    p.cityVt.slots[72] = reinterpret_cast<void*>(&PipeCityGetDisasterLayer);
    p.cityVt.slots[97] = reinterpret_cast<void*>(&CityCellCenterToPosition);
    p.layerVt.slots[7] = reinterpret_cast<void*>(&PipeLayerGetManager);
    p.mgrVt.slots[6] = reinterpret_cast<void*>(&PipeMgrAttempt);
    p.instVt.slots[0] = reinterpret_cast<void*>(&PipeInstQi);
    p.instVt.slots[1] = reinterpret_cast<void*>(&PipeInstAddRef);
    p.instVt.slots[2] = reinterpret_cast<void*>(&PipeInstRelease);
    p.city.vtbl = p.cityVt.slots.data();
    p.layer.vtbl = p.layerVt.slots.data();
    p.mgr.vtbl = p.mgrVt.slots.data();
    p.inst.vtbl = p.instVt.slots.data();
}

constexpr uint32_t kMsgPostCityInit = 0x26D31EC1;
constexpr uint32_t kMsgSimNewDay = 0x66956814;
constexpr uint32_t kMsgSimNewMonth = 0x66956816;
constexpr uint32_t kMsgSimIdle = 0x6695681A;

// ---- the budget shadow (test seam op 10) -------------------------------------------------------
// An "original" budget simulator (not ours: its vtable is in this
// executable) with its month agent. Only getters have slots; any other call,
// a write included, hits a trap and aborts. DoMessage(SimNewMonth) "settles":
// it publishes the values in `settle*`.
struct FakeOrigBudget {
    FakeVtable budgetVt, agentVt, cityVt, otherVt;
    FakeObject budget{}, agent{}, city{}, other{};
    int refs = 1;
    int deliveries = 0;
    int64_t funds = 50000, ytdInc = 100, ytdExp = 60, bonds = 0;
    int64_t wa[12] = {}, tax[12] = {}, total = 0;
    int64_t settleWa[12] = {}, settleTax[12] = {}, settleTotal = 0, settleIncome = 0, settleExpense = 0;
};

FakeOrigBudget* g_orig = nullptr;

bool __fastcall OrigQi(void*, void*, uint32_t iid, void** out) {  // 0 (budget and agent)
    if (iid != 0x452294AA || out == nullptr) return false;  // cIGZMessageTarget2
    *out = &g_orig->agent;
    ++g_orig->refs;
    return true;
}
uint32_t __fastcall OrigAddRef(void*, void*) { return static_cast<uint32_t>(++g_orig->refs); }   // 1
uint32_t __fastcall OrigRelease(void*, void*) { return static_cast<uint32_t>(--g_orig->refs); }  // 2
int64_t __fastcall OrigFunds(void*, void*) { return g_orig->funds; }                           // 6
int64_t __fastcall OrigYtdIncome(void*, void*) { return g_orig->ytdInc; }                      // 10
int64_t __fastcall OrigYtdExpense(void*, void*) { return g_orig->ytdExp; }                     // 12
int64_t __fastcall OrigTaxOf(void*, void*, int32_t t) { return t >= 0 && t < 12 ? g_orig->tax[t] : 0; }  // 18
int64_t __fastcall OrigTax(void*, void*) { return g_orig->total; }                             // 19
float __fastcall OrigRate(void*, void*, uint32_t) { return 9.0f; }                             // 36
int64_t __fastcall OrigWa(void*, void*, uint32_t t) { return t < 12 ? g_orig->wa[t] : 0; }     // 38
int64_t __fastcall OrigBonds(void*, void*) { return g_orig->bonds; }                           // 45
bool __fastcall OrigDoMessage(void*, void*, cIGZMessage2* m) {                                 // agent 3
    ++g_orig->deliveries;
    if (m == nullptr || m->GetType() != kMsgSimNewMonth) return true;
    for (int t = 0; t < 12; ++t) {
        g_orig->wa[t] = g_orig->settleWa[t];
        g_orig->tax[t] = g_orig->settleTax[t];
    }
    g_orig->total = g_orig->settleTotal;
    g_orig->ytdInc += g_orig->settleIncome;
    g_orig->ytdExp += g_orig->settleExpense;
    g_orig->funds += g_orig->settleIncome - g_orig->settleExpense;
    return true;
}
int32_t __fastcall OrigCityDifficulty(void*, void*) { return 2; }                    // city 24
void* __fastcall OrigCityBudget(void*, void*) { return &g_orig->budget; }            // city 52
void* __fastcall OrigCityNone(void*, void*) { return nullptr; }                      // city 62, 63

void InitFakeOrigBudget(FakeOrigBudget& o) {
    g_orig = &o;
    for (FakeVtable* vt : {&o.budgetVt, &o.agentVt}) {
        vt->slots[0] = reinterpret_cast<void*>(&OrigQi);
        vt->slots[1] = reinterpret_cast<void*>(&OrigAddRef);
        vt->slots[2] = reinterpret_cast<void*>(&OrigRelease);
    }
    o.budgetVt.slots[6] = reinterpret_cast<void*>(&OrigFunds);
    o.budgetVt.slots[10] = reinterpret_cast<void*>(&OrigYtdIncome);
    o.budgetVt.slots[12] = reinterpret_cast<void*>(&OrigYtdExpense);
    o.budgetVt.slots[18] = reinterpret_cast<void*>(&OrigTaxOf);
    o.budgetVt.slots[19] = reinterpret_cast<void*>(&OrigTax);
    o.budgetVt.slots[36] = reinterpret_cast<void*>(&OrigRate);
    o.budgetVt.slots[38] = reinterpret_cast<void*>(&OrigWa);
    o.budgetVt.slots[45] = reinterpret_cast<void*>(&OrigBonds);
    o.agentVt.slots[3] = reinterpret_cast<void*>(&OrigDoMessage);
    o.otherVt.slots[3] = reinterpret_cast<void*>(&OrigDoMessage);
    o.cityVt.slots[24] = reinterpret_cast<void*>(&OrigCityDifficulty);
    o.cityVt.slots[52] = reinterpret_cast<void*>(&OrigCityBudget);
    o.cityVt.slots[62] = reinterpret_cast<void*>(&OrigCityNone);
    o.cityVt.slots[63] = reinterpret_cast<void*>(&OrigCityNone);
    o.budget.vtbl = o.budgetVt.slots.data();
    o.agent.vtbl = o.agentVt.slots.data();
    o.city.vtbl = o.cityVt.slots.data();
    o.other.vtbl = o.otherVt.slots.data();
}

// One shadowed month: the "original" settles to `wa` (types 0, 3, 6), with
// its per-type tax the GetTaxIncome(t) formula gives at 9 % with the code
// default multiplier (budget §9: 100000): 1000 -> 750000, 2000 -> 1500000,
// 2500 -> 1875000, 3000 -> 2250000 (float32, checked by hand).
intptr_t ShadowMonth(FC4TestSeamFn seam, FakeOrigBudget& o, cIGZMessageTarget2* target, int64_t wa3, int64_t tax3) {
    // Lots (code-default tuning, land value 0: assessed = building value).
    static fc4_lot_info lots[5] = {};
    lots[0] = fc4_lot_info{1, 1, 0, 0, 1, 1, 1, 1000, 1, 1};  // R$
    lots[1] = fc4_lot_info{1, 1, 0, 0, 2, 1, 1, 2000, 2, 1};  // CS$
    lots[2] = fc4_lot_info{1, 0, 0, 0, 2, 1, 1, 500, 1, 1};   // CS$, not livable
    lots[3] = fc4_lot_info{1, 1, 0, 0, 3, 2, 2, 3000, 1, 2};  // CO$$
    lots[4] = fc4_lot_info{};                                // no building
    static FC4ShadowTestLot feed[5] = {};
    for (int i = 0; i < 5; ++i) feed[i] = FC4ShadowTestLot{&lots[i], i, i + 1};
    for (int t = 0; t < 12; ++t) o.settleWa[t] = o.settleTax[t] = 0;
    o.settleWa[0] = 1000;
    o.settleTax[0] = 750000;
    o.settleWa[3] = wa3;
    o.settleTax[3] = tax3;
    o.settleWa[6] = 3000;
    o.settleTax[6] = 2250000;
    o.settleTotal = 4500000;
    o.settleIncome = 4500000;
    o.settleExpense = 0;
    FC4ShadowTestInput in{reinterpret_cast<cISC4City*>(&o.city), target, feed, 5, 2451576};  // 2000-02-01
    return seam(kFC4SeamBudgetShadow, &in, nullptr);
}

// ---- the demand shadow (test seam op 11) -------------------------------------------------------
// An "original" demand simulator (not ours: its vtables are in this
// executable) with its agent and three nodes. Only getters have slots; any
// other call, a write included, hits a trap and aborts. DoMessage "updates":
// it copies `post` over the nodes (the test writes what the original would
// leave). Graph (demand exemplars, group 0xC7BB4816):
//   0x1011 (census R$, region use 1): Drives 0x21010 100 %
//   0x1810: Caps 0x21010
//   0x21010 (total R$): a link target only
struct FakeOrigNode {
    void** vtbl;
    uint32_t key, ru;
    float supply, demand, ns, nd, active, econ, max, min, tax;
    float cap;  // SC4Percentage (GetDemandCap returns a pointer to it)
};

struct FakeOrigDemand {
    FakeVtable simVt, agentVt, nodeVt, cityVt, otherVt;
    FakeObject sim{}, agent{}, city{}, other{};
    FakeOrigNode nodes[3] = {};
    FakeOrigNode post[3] = {};
    int refs = 1;
    int deliveries = 0;
};

FakeOrigDemand* g_od = nullptr;

bool __fastcall OdQi(void*, void*, uint32_t iid, void** out) {  // 0 (simulator and agent)
    if (iid != 0x452294AA || out == nullptr) return false;     // cIGZMessageTarget2
    *out = &g_od->agent;
    ++g_od->refs;
    return true;
}
uint32_t __fastcall OdAddRef(void*, void*) { return static_cast<uint32_t>(++g_od->refs); }   // 1
uint32_t __fastcall OdRelease(void*, void*) { return static_cast<uint32_t>(--g_od->refs); }  // 2
void* __fastcall OdGetDemand(void*, void*, uint32_t id, uint32_t index) {                     // simulator 6
    if (index != 0) return nullptr;
    for (FakeOrigNode& n : g_od->nodes) {
        if (n.key == id) return &n;
    }
    return nullptr;
}
FakeOrigNode* On(void* self) { return static_cast<FakeOrigNode*>(self); }
float __fastcall OdSupply(void* s, void*) { return On(s)->supply; }    // 8
float __fastcall OdDemand(void* s, void*) { return On(s)->demand; }    // 9
float __fastcall OdNewSupply(void* s, void*) { return On(s)->ns; }     // 10
float __fastcall OdNewDemand(void* s, void*) { return On(s)->nd; }     // 11
float __fastcall OdActive(void* s, void*) { return On(s)->active; }    // 12
float __fastcall OdEcon(void* s, void*) { return On(s)->econ; }        // 13
float __fastcall OdMax(void* s, void*) { return On(s)->max; }          // 14
float __fastcall OdMin(void* s, void*) { return On(s)->min; }          // 15
float __fastcall OdTax(void* s, void*) { return On(s)->tax; }          // 24
void* __fastcall OdCap(void* s, void*) { return &On(s)->cap; }         // 26
uint32_t __fastcall OdRegionUse(void* s, void*) { return On(s)->ru; }  // 28
bool __fastcall OdDoMessage(void*, void*, cIGZMessage2*) {             // agent 3
    ++g_od->deliveries;
    for (int i = 0; i < 3; ++i) g_od->nodes[i] = g_od->post[i];
    return true;
}
void* __fastcall OdCityDemand(void*, void*) { return &g_od->sim; }  // city 56

void InitFakeOrigDemand(FakeOrigDemand& o) {
    g_od = &o;
    for (FakeVtable* vt : {&o.simVt, &o.agentVt}) {
        vt->slots[0] = reinterpret_cast<void*>(&OdQi);
        vt->slots[1] = reinterpret_cast<void*>(&OdAddRef);
        vt->slots[2] = reinterpret_cast<void*>(&OdRelease);
    }
    o.simVt.slots[6] = reinterpret_cast<void*>(&OdGetDemand);
    o.nodeVt.slots[8] = reinterpret_cast<void*>(&OdSupply);
    o.nodeVt.slots[9] = reinterpret_cast<void*>(&OdDemand);
    o.nodeVt.slots[10] = reinterpret_cast<void*>(&OdNewSupply);
    o.nodeVt.slots[11] = reinterpret_cast<void*>(&OdNewDemand);
    o.nodeVt.slots[12] = reinterpret_cast<void*>(&OdActive);
    o.nodeVt.slots[13] = reinterpret_cast<void*>(&OdEcon);
    o.nodeVt.slots[14] = reinterpret_cast<void*>(&OdMax);
    o.nodeVt.slots[15] = reinterpret_cast<void*>(&OdMin);
    o.nodeVt.slots[24] = reinterpret_cast<void*>(&OdTax);
    o.nodeVt.slots[26] = reinterpret_cast<void*>(&OdCap);
    o.nodeVt.slots[28] = reinterpret_cast<void*>(&OdRegionUse);
    o.agentVt.slots[3] = reinterpret_cast<void*>(&OdDoMessage);
    o.otherVt.slots[3] = reinterpret_cast<void*>(&OdDoMessage);
    o.cityVt.slots[56] = reinterpret_cast<void*>(&OdCityDemand);
    o.sim.vtbl = o.simVt.slots.data();
    o.agent.vtbl = o.agentVt.slots.data();
    o.city.vtbl = o.cityVt.slots.data();
    o.other.vtbl = o.otherVt.slots.data();
}

FakeOrigNode OdNode(const FakeOrigDemand& o, uint32_t key, uint32_t ru, float supply, float demand, float ns,
                    float active, float cap, float tax = 0.0f, float econ = 0.0f) {
    FakeOrigNode n{};
    n.vtbl = const_cast<void**>(o.nodeVt.slots.data());
    n.key = key;
    n.ru = ru;
    n.supply = supply;
    n.demand = demand;
    n.ns = ns;
    n.active = active;
    n.econ = econ;
    n.max = 6000.0f;
    n.min = -6000.0f;
    n.tax = tax;
    n.cap = cap;
    return n;
}

// The state before every NewDay case: 0x1011 has 10 pending supply.
void OdPre(FakeOrigDemand& o) {
    o.nodes[0] = OdNode(o, 0x1011, 1, 0.0f, 0.0f, 10.0f, 0.0f, 0.0f);
    o.nodes[1] = OdNode(o, 0x1810, 0, 50000.0f, 10.0f, 0.0f, -6000.0f, 0.0f);
    o.nodes[2] = OdNode(o, 0x21010, 0, 0.0f, 20.0f, 0.0f, 20.0f, 0.0f);
}

// What the original leaves after a NewDay whose census added `census` to
// 0x1011's pending supply, with 0x21010's modifiers set to tax / econ and its
// committed demand off by `skew` (the propagation, §4.4, by hand: 0x1011
// drives 10 + census into 0x21010 and folds; 0x1810 caps 0x21010 at
// 10 / 50000; 0x21010 folds; a second pass moves nothing).
void OdPost(FakeOrigDemand& o, float census, float tax, float econ, float skew) {
    const float a = 10.0f + census;
    const float d = 20.0f + a;
    const float m = tax + econ;
    const float bonus = std::fmin(6000.0f * m, d * m);
    o.post[0] = OdNode(o, 0x1011, 1, a, 0.0f, 0.0f, -a, 0.0f);
    o.post[1] = o.nodes[1];
    o.post[2] = OdNode(o, 0x21010, 0, 0.0f, d + skew, 0.0f, d + bonus + skew, 10.0f / 50000.0f, tax, econ);
}

intptr_t DemandShadowOnce(FC4TestSeamFn seam, FakeOrigDemand& o, cIGZMessageTarget2* target, uint32_t type,
                          float ordinance, float nation) {
    static const uint32_t keys[2] = {0x1011, 0x1810};
    static const FC4DemandShadowTestLink links[2] = {{0x1011, 0x47BB3F30, 0x21010, 100},
                                                     {0x1810, 0x47BB3F41, 0x21010, 0}};
    FC4DemandShadowTestInput in{reinterpret_cast<cISC4City*>(&o.city), target, keys, 2, links, 2, type,
                                2451576,  // 2000-02-01
                                -1.0f, ordinance, nation};
    return seam(kFC4SeamDemandShadow, &in, nullptr);
}

// ---- the pollution shadow (test seam op 16) ------------------------------------------------------
// An "original" pollution simulator: QueryInterface answers its agent
// (cIGZMessageTarget2) and cIGZSerializable, whose Write writes the record's
// version alone (a uint16): a record without the grid references, so the
// shadow reports the delivery INCOMPLETE and reads nothing else. Any other
// call hits a trap and aborts. The city answers GetPollutionSimulator
// (compiled slot 66) only; `noCity` returns null there.
struct FakeOrigPollution {
    FakeVtable simVt, agentVt, serVt, cityVt, noCityVt;
    FakeObject sim{}, agent{}, ser{}, city{}, noCity{}, other{};
    int refs = 1;
    int deliveries = 0;
    int writes = 0;
};

FakeOrigPollution* g_op = nullptr;

bool __fastcall OpQi(void*, void*, uint32_t iid, void** out) {  // 0 (simulator, agent, serializable)
    if (out == nullptr) return false;
    if (iid == 0x452294AA) {
        *out = &g_op->agent;  // cIGZMessageTarget2
    } else if (iid == GZIID_cIGZSerializable) {
        *out = &g_op->ser;
    } else {
        return false;  // (the reference audit asks other IIDs)
    }
    ++g_op->refs;
    return true;
}
uint32_t __fastcall OpAddRef(void*, void*) { return static_cast<uint32_t>(++g_op->refs); }   // 1
uint32_t __fastcall OpRelease(void*, void*) { return static_cast<uint32_t>(--g_op->refs); }  // 2
bool __fastcall OpWrite(void*, void*, cIGZOStream* s) {                                       // serializable 3
    ++g_op->writes;
    return s->SetUint16(4);
}
// serializable 5 (the reference audit's class lookup asks it too)
uint32_t __fastcall OpClsid(void*, void*) { return 0x8990C065; }
bool __fastcall OpDoMessage(void*, void*, cIGZMessage2*) {  // agent 3
    ++g_op->deliveries;
    return true;
}
void* __fastcall OpCityPollution(void*, void*) { return &g_op->sim; }  // city 66
void* __fastcall OpCityNoPollution(void*, void*) { return nullptr; }   // city 66

void InitFakeOrigPollution(FakeOrigPollution& o) {
    g_op = &o;
    for (FakeVtable* vt : {&o.simVt, &o.agentVt, &o.serVt}) {
        vt->slots[0] = reinterpret_cast<void*>(&OpQi);
        vt->slots[1] = reinterpret_cast<void*>(&OpAddRef);
        vt->slots[2] = reinterpret_cast<void*>(&OpRelease);
    }
    o.agentVt.slots[3] = reinterpret_cast<void*>(&OpDoMessage);
    o.serVt.slots[3] = reinterpret_cast<void*>(&OpWrite);
    o.serVt.slots[5] = reinterpret_cast<void*>(&OpClsid);
    o.cityVt.slots[66] = reinterpret_cast<void*>(&OpCityPollution);
    o.noCityVt.slots[66] = reinterpret_cast<void*>(&OpCityNoPollution);
    o.sim.vtbl = o.simVt.slots.data();
    o.agent.vtbl = o.agentVt.slots.data();
    o.ser.vtbl = o.serVt.slots.data();
    o.city.vtbl = o.cityVt.slots.data();
    o.noCity.vtbl = o.noCityVt.slots.data();
    o.other.vtbl = o.agentVt.slots.data();  // an agent, but not the simulator's
}

intptr_t PollutionShadowOnce(FC4TestSeamFn seam, FakeObject* city, cIGZMessageTarget2* target, uint32_t type) {
    FC4PollutionShadowTestInput in{reinterpret_cast<cISC4City*>(city), target, type, 2451576};  // 2000-02-01
    return seam(kFC4SeamPollutionShadow, &in, nullptr);
}

// ---- the power shadow (test seam op 21) ------------------------------------------------------------
// An "original" power simulator on a 32 x 32 city without occupants, plants
// or poles. QueryInterface answers its agent (cIGZMessageTarget2) and
// cIGZSerializable, whose Write writes a §9.1 version-6 record: no
// department, the powered grid's reference, a 32 x 32 consumption map, ring,
// annual and per-type counts of zeros, no plants or poles, the four bitmaps
// (32 words of zeros each) and `rec`'s ring index, recompute, agePlants and
// state. The agent's DoMessage leaves `next` (what the original's update
// did) and, when `poke` >= 0, writes `pokeValue` into that cell of the grid.
// Slots 27-29 answer 0. The grid (cISC4SimGrid<uint8>, tract size 1, 32 x 32,
// its own serializable with the grid class ID) holds `cells`. The city
// answers GetPowerSimulator (compiled slot 67), CellWidthX / Z (91, 92) and
// CellCountX / Z (93, 94). Any other call aborts.
struct FakePowerRecord {
    int32_t ringIndex;
    uint8_t recompute;
    uint8_t agePlants;
    uint32_t state;
};
struct FakeOrigPower {
    FakeVtable simVt, agentVt, serVt, gridVt, gridSerVt, cityVt;
    FakeObject sim{}, agent{}, ser{}, grid{}, gridSer{}, city{}, other{};
    FakePowerRecord rec{}, next{};
    uint8_t cells[32 * 32]{};
    int poke = -1;
    uint8_t pokeValue = 0;
    int refs = 1;
    int deliveries = 0;
    int writes = 0;
};

FakeOrigPower* g_ow = nullptr;

bool __fastcall OwQi(void*, void*, uint32_t iid, void** out) {  // 0 (simulator, agent, serializable)
    if (out == nullptr) return false;
    if (iid == 0x452294AA) {
        *out = &g_ow->agent;  // cIGZMessageTarget2
    } else if (iid == GZIID_cIGZSerializable) {
        *out = &g_ow->ser;
    } else {
        return false;  // (the reference audit asks other IIDs)
    }
    ++g_ow->refs;
    return true;
}
bool __fastcall OwGridQi(void*, void*, uint32_t iid, void** out) {  // 0 (grid, grid serializable)
    if (out == nullptr) return false;
    if (iid == 0xC9BD939A) {
        *out = &g_ow->grid;  // cISC4SimGrid
    } else if (iid == GZIID_cIGZSerializable) {
        *out = &g_ow->gridSer;
    } else {
        return false;  // not an occupant, pole or department
    }
    ++g_ow->refs;
    return true;
}
uint32_t __fastcall OwAddRef(void*, void*) { return static_cast<uint32_t>(++g_ow->refs); }   // 1
uint32_t __fastcall OwRelease(void*, void*) { return static_cast<uint32_t>(--g_ow->refs); }  // 2
bool __fastcall OwWrite(void*, void*, cIGZOStream* s) {                                       // serializable 3
    ++g_ow->writes;
    void* p = nullptr;
    if (!s->QueryInterface(kFC4_IID_cISC4DBSegmentOStream, &p) || p == nullptr) return false;
    auto* seg = static_cast<cFC4ISC4DBSegmentOStream*>(p);  // the capture stream: no reference taken
    const FakePowerRecord& r = g_ow->rec;
    s->SetUint16(6);
    seg->WriteGZSerializable(nullptr);  // department
    s->SetSint32(r.ringIndex);
    s->SetUint32(0);     // production
    s->SetUint32(0);     // consumption
    s->SetFloat32(0.0f);  // deficit
    for (int i = 0; i < 4; ++i) s->SetUint32(0);  // consumers, powered, unpowered, requirement
    s->SetSint64(0);                               // costBase
    for (int i = 0; i < 5; ++i) s->SetUint32(0);  // last production .. demand
    seg->WriteGZSerializable(reinterpret_cast<cIGZSerializable*>(&g_ow->gridSer));
    s->SetUint32(32);
    s->SetUint32(32);
    for (int i = 0; i < 32 * 32; ++i) s->SetSint16(0);   // consumption map
    for (int i = 0; i < 12 * 10 + 10; ++i) s->SetSint32(0);  // ring, annual
    s->SetSint32(0);                                     // plant count
    for (int i = 0; i < 10; ++i) s->SetSint32(0);        // per type
    s->SetUint32(0);                                     // pole count
    s->SetUint32(0);                                     // broken count
    s->SetUint32(32);                                    // words per bitmap
    for (int i = 0; i < 4 * 32; ++i) s->SetUint32(0);    // mark, wire, powered current / next
    s->SetUint8(0);                                      // unused
    s->SetUint8(0);                                      // strike
    s->SetSint32(0);                                     // strike start
    s->SetUint8(r.recompute);
    s->SetUint8(r.agePlants);
    return s->SetUint32(r.state);
}
uint32_t __fastcall OwClsid(void*, void*) { return 0x0990C005; }      // serializable 5
uint32_t __fastcall OwGridClsid(void*, void*) { return 0x49B9E602; }  // grid serializable 5
int32_t __fastcall OwZero(void*, void*) { return 0; }                // simulator 27-29
bool __fastcall OwDoMessage(void*, void*, cIGZMessage2*) {            // agent 3
    ++g_ow->deliveries;
    g_ow->rec = g_ow->next;
    if (g_ow->poke >= 0) g_ow->cells[g_ow->poke] = g_ow->pokeValue;
    return true;
}
int32_t __fastcall OwTractOne(void*, void*) { return 1; }               // grid 10
int32_t __fastcall OwTractShift(void*, void*) { return 0; }             // grid 11
int32_t __fastcall OwTractCount(void*, void*) { return 32; }            // grid 12, 13
float __fastcall OwTractWidth(void*, void*) { return 16.0f; }           // grid 14, 15; city 91, 92
float __fastcall OwTractInvWidth(void*, void*) { return 1.0f / 16.0f; }  // grid 16, 17
uint8_t __fastcall OwTractValue(void*, void*, int32_t x, int32_t z) {    // grid 22
    if (x < 0 || x > 31 || z < 0 || z > 31) return 0;
    return g_ow->cells[x * 32 + z];
}
void* __fastcall OwCityPower(void*, void*) { return &g_ow->sim; }     // city 67
uint32_t __fastcall OwCellCount(void*, void*) { return 32; }         // city 93, 94

void InitFakeOrigPower(FakeOrigPower& o) {
    g_ow = &o;
    for (FakeVtable* vt : {&o.simVt, &o.agentVt, &o.serVt}) {
        vt->slots[0] = reinterpret_cast<void*>(&OwQi);
        vt->slots[1] = reinterpret_cast<void*>(&OwAddRef);
        vt->slots[2] = reinterpret_cast<void*>(&OwRelease);
    }
    for (FakeVtable* vt : {&o.gridVt, &o.gridSerVt}) {
        vt->slots[0] = reinterpret_cast<void*>(&OwGridQi);
        vt->slots[1] = reinterpret_cast<void*>(&OwAddRef);
        vt->slots[2] = reinterpret_cast<void*>(&OwRelease);
    }
    o.agentVt.slots[3] = reinterpret_cast<void*>(&OwDoMessage);
    o.serVt.slots[3] = reinterpret_cast<void*>(&OwWrite);
    o.serVt.slots[5] = reinterpret_cast<void*>(&OwClsid);
    o.gridSerVt.slots[5] = reinterpret_cast<void*>(&OwGridClsid);
    for (int s = 27; s <= 29; ++s) o.simVt.slots[s] = reinterpret_cast<void*>(&OwZero);
    o.gridVt.slots[10] = reinterpret_cast<void*>(&OwTractOne);
    o.gridVt.slots[11] = reinterpret_cast<void*>(&OwTractShift);
    o.gridVt.slots[12] = reinterpret_cast<void*>(&OwTractCount);
    o.gridVt.slots[13] = reinterpret_cast<void*>(&OwTractCount);
    o.gridVt.slots[14] = reinterpret_cast<void*>(&OwTractWidth);
    o.gridVt.slots[15] = reinterpret_cast<void*>(&OwTractWidth);
    o.gridVt.slots[16] = reinterpret_cast<void*>(&OwTractInvWidth);
    o.gridVt.slots[17] = reinterpret_cast<void*>(&OwTractInvWidth);
    o.gridVt.slots[22] = reinterpret_cast<void*>(&OwTractValue);
    o.cityVt.slots[67] = reinterpret_cast<void*>(&OwCityPower);
    o.cityVt.slots[91] = reinterpret_cast<void*>(&OwTractWidth);
    o.cityVt.slots[92] = reinterpret_cast<void*>(&OwTractWidth);
    o.cityVt.slots[93] = reinterpret_cast<void*>(&OwCellCount);
    o.cityVt.slots[94] = reinterpret_cast<void*>(&OwCellCount);
    o.sim.vtbl = o.simVt.slots.data();
    o.agent.vtbl = o.agentVt.slots.data();
    o.ser.vtbl = o.serVt.slots.data();
    o.grid.vtbl = o.gridVt.slots.data();
    o.gridSer.vtbl = o.gridSerVt.slots.data();
    o.city.vtbl = o.cityVt.slots.data();
    o.other.vtbl = o.agentVt.slots.data();  // an agent, but not the simulator's
}

// The original before (`state`, ring index 0, nothing requested) and what its
// update leaves (`next`).
void OwSet(FakeOrigPower& o, uint32_t state, FakePowerRecord next) {
    o.rec = FakePowerRecord{0, 0, 0, state};
    o.next = next;
    o.poke = -1;
}

intptr_t PowerShadowOnce(FC4TestSeamFn seam, FakeObject* city, cIGZMessageTarget2* target, uint32_t type,
                         bool fresh, bool midCity = false) {
    FC4PowerShadowTestInput in{reinterpret_cast<cISC4City*>(city), target, type, 2451576, fresh ? 1u : 0u,
                               midCity ? 1u : 0u};  // 2000-02-01
    return seam(kFC4SeamPowerShadow, &in, nullptr);
}

// ---- the crime shadow (test seam op 13) ----------------------------------------------------------
// An "original" crime simulator on a 4x4 tract grid (S = 4, shift 2).
// QueryInterface answers its agent (cIGZMessageTarget2); the agent runs the
// pipeline of crime.md §5 (a new simulator in state 6, SimNewMonth 6 -> 0,
// one step per SimNewDay) and at state 4 switches the grid, hotspot, count,
// average and incident list from `pre` to `post`. The city answers
// GetLotManager (33), GetZoneManager (36) and GetCrimeSimulator (55); two
// lots, at three cells; the zone manager zones cells x < 8. Any other call
// aborts.
//
// The month by hand (decay 0.5; types 0x1001 severity 40, 0x1002 60, 0x1003
// 0): before, (1,1) = 100 (the hotspot, cells (4,4)), (2,3) = 7, (3,0) = 1.
// Decay: 50, 3, 0; the hotspot re-read is (1,1) = 50. Incidents in order:
// 0x1001 on the lot (8,12)-(11,15) stamps (2,3) to 43; 0x1002 on the lot
// (0,0)-(5,1) stamps (0,0) and (1,0) to 60, the hotspot moves to (0,0) = 60;
// 0x1003 (severity 0) is neither stamped nor counted; 0x1001 at (13,13) has
// no lot: counted, not stamped. Count 3; the zoned tracts (x = 0, 1) sum to
// 170 over 8: average 21.
struct FakeCrimeLot {
    void** vtbl;
    int32_t x1, z1, x2, z2;
};
struct FakeCrimeVector {  // the caller's vector: first, last, end of storage
    FC4CrimeEvent* first;
    FC4CrimeEvent* last;
    FC4CrimeEvent* capacityEnd;
};
struct FakeOrigCrime {
    FakeVtable simVt, agentVt, cityVt, gridVt, lmVt, lotVt, zmVt;
    FakeObject sim{}, agent{}, city{}, grid{}, lm{}, zm{}, other{};
    FakeCrimeLot lots[2]{};
    uint8_t pre[16]{}, post[16]{};
    bool committed = false;
    int state = 6;
    int refs = 1;
    int deliveries = 0;
    uint32_t count = 0;
    std::vector<FC4CrimeEvent> incidents;
};

FakeOrigCrime* g_oc = nullptr;

bool __fastcall OcQi(void*, void*, uint32_t iid, void** out) {  // 0 (simulator, agent)
    if (out == nullptr) return false;
    if (iid != 0x452294AA) return false;  // (the reference audit asks other IIDs)
    *out = &g_oc->agent;
    ++g_oc->refs;
    return true;
}
uint32_t __fastcall OcAddRef(void*, void*) { return static_cast<uint32_t>(++g_oc->refs); }   // 1
uint32_t __fastcall OcRelease(void*, void*) { return static_cast<uint32_t>(--g_oc->refs); }  // 2
bool __fastcall OcDoMessage(void*, void*, cIGZMessage2* m) {                                   // agent 3
    ++g_oc->deliveries;
    const uint32_t type = m->GetType();
    if (type == kMsgSimNewMonth) {
        if (g_oc->state == 6) g_oc->state = 0;
    } else if ((type == kMsgSimNewDay || type == kMsgSimIdle) && g_oc->state < 6) {
        if (g_oc->state == 4) g_oc->committed = true;
        ++g_oc->state;
    }
    return true;
}
void* __fastcall OcGrid(void*, void*) { return &g_oc->grid; }                                 // sim 5
uint32_t __fastcall OcAverage(void*, void*) { return g_oc->committed ? 21u : 0u; }           // sim 6
uint32_t __fastcall OcCount(void*, void*) { return g_oc->committed ? g_oc->count : 0u; }     // sim 7
bool __fastcall OcHotspot(void*, void*, uint8_t* v, int32_t* x, int32_t* z) {                // sim 8
    *v = g_oc->committed ? 60 : 100;
    *x = g_oc->committed ? 0 : 4;
    *z = g_oc->committed ? 0 : 4;
    return true;
}
bool __fastcall OcIncidents(void*, void*, FakeCrimeVector* v) {  // sim 11
    if (!g_oc->committed) return false;
    const size_t n = g_oc->incidents.size();
    if (static_cast<size_t>(v->capacityEnd - v->first) < n) {
        std::fprintf(stderr, "fake crime simulator: the shadow's vector has no room for %u incidents\n",
                     static_cast<unsigned>(n));
        std::abort();
    }
    for (size_t i = 0; i < n; ++i) v->first[i] = g_oc->incidents[i];
    v->last = v->first + n;
    return n > 0;
}
int32_t __fastcall OcTractSize(void*, void*) { return 4; }        // grid 10
int32_t __fastcall OcTractShift(void*, void*) { return 2; }       // grid 11
int32_t __fastcall OcTractCount(void*, void*) { return 4; }       // grid 12, 13
float __fastcall OcTractWidth(void*, void*) { return 64.0f; }     // grid 14, 15
float __fastcall OcTractInvWidth(void*, void*) { return 1.0f / 64.0f; }  // grid 16, 17
uint8_t __fastcall OcTractValue(void*, void*, int32_t x, int32_t z) {    // grid 22
    if (x < 0 || x > 3 || z < 0 || z > 3) return 0;
    return (g_oc->committed ? g_oc->post : g_oc->pre)[x * 4 + z];
}
void* __fastcall OcGetLot(void*, void*, int32_t x, int32_t z, bool) {  // lot manager 6
    if ((x == 8 && z == 12) || (x == 12 && z == 0)) return &g_oc->lots[0];
    if (x == 0 && z == 0) return &g_oc->lots[1];
    return nullptr;
}
bool __fastcall OcLotRect(void* self, void*, SC4Rect<int32_t>* r) {  // lot 14
    const FakeCrimeLot* l = static_cast<const FakeCrimeLot*>(self);
    r->topLeftX = l->x1;
    r->topLeftY = l->z1;
    r->bottomRightX = l->x2;
    r->bottomRightY = l->z2;
    return true;
}
bool __fastcall OcLotLocation(void* self, void*, int32_t* x, int32_t* z) {  // lot 21
    *x = static_cast<const FakeCrimeLot*>(self)->x1;
    *z = static_cast<const FakeCrimeLot*>(self)->z1;
    return true;
}
uint32_t __fastcall OcLotCondition(void*, void*) { return 0; }      // lot 35
void* __fastcall OcLotBuilding(void*, void*) { return nullptr; }    // lot 48
int32_t __fastcall OcZoneType(void*, void*, int32_t x, int32_t) { return x < 8 ? 1 : 0; }  // zone manager 12
void* __fastcall OcCityLots(void*, void*) { return &g_oc->lm; }     // city 33
void* __fastcall OcCityZones(void*, void*) { return &g_oc->zm; }    // city 36
void* __fastcall OcCityCrime(void*, void*) { return &g_oc->sim; }   // city 55

void InitFakeOrigCrime(FakeOrigCrime& o) {
    g_oc = &o;
    for (FakeVtable* vt : {&o.simVt, &o.agentVt}) {
        vt->slots[0] = reinterpret_cast<void*>(&OcQi);
        vt->slots[1] = reinterpret_cast<void*>(&OcAddRef);
        vt->slots[2] = reinterpret_cast<void*>(&OcRelease);
    }
    o.agentVt.slots[3] = reinterpret_cast<void*>(&OcDoMessage);
    o.simVt.slots[5] = reinterpret_cast<void*>(&OcGrid);
    o.simVt.slots[6] = reinterpret_cast<void*>(&OcAverage);
    o.simVt.slots[7] = reinterpret_cast<void*>(&OcCount);
    o.simVt.slots[8] = reinterpret_cast<void*>(&OcHotspot);
    o.simVt.slots[11] = reinterpret_cast<void*>(&OcIncidents);
    o.gridVt.slots[10] = reinterpret_cast<void*>(&OcTractSize);
    o.gridVt.slots[11] = reinterpret_cast<void*>(&OcTractShift);
    o.gridVt.slots[12] = reinterpret_cast<void*>(&OcTractCount);
    o.gridVt.slots[13] = reinterpret_cast<void*>(&OcTractCount);
    o.gridVt.slots[14] = reinterpret_cast<void*>(&OcTractWidth);
    o.gridVt.slots[15] = reinterpret_cast<void*>(&OcTractWidth);
    o.gridVt.slots[16] = reinterpret_cast<void*>(&OcTractInvWidth);
    o.gridVt.slots[17] = reinterpret_cast<void*>(&OcTractInvWidth);
    o.gridVt.slots[22] = reinterpret_cast<void*>(&OcTractValue);
    o.lmVt.slots[6] = reinterpret_cast<void*>(&OcGetLot);
    o.lotVt.slots[14] = reinterpret_cast<void*>(&OcLotRect);
    o.lotVt.slots[21] = reinterpret_cast<void*>(&OcLotLocation);
    o.lotVt.slots[35] = reinterpret_cast<void*>(&OcLotCondition);
    o.lotVt.slots[48] = reinterpret_cast<void*>(&OcLotBuilding);
    o.zmVt.slots[12] = reinterpret_cast<void*>(&OcZoneType);
    o.cityVt.slots[33] = reinterpret_cast<void*>(&OcCityLots);
    o.cityVt.slots[36] = reinterpret_cast<void*>(&OcCityZones);
    o.cityVt.slots[55] = reinterpret_cast<void*>(&OcCityCrime);
    o.sim.vtbl = o.simVt.slots.data();
    o.agent.vtbl = o.agentVt.slots.data();
    o.city.vtbl = o.cityVt.slots.data();
    o.grid.vtbl = o.gridVt.slots.data();
    o.lm.vtbl = o.lmVt.slots.data();
    o.zm.vtbl = o.zmVt.slots.data();
    o.other.vtbl = o.agentVt.slots.data();  // an agent, but not the simulator's
    o.lots[0] = {o.lotVt.slots.data(), 8, 12, 11, 15};
    o.lots[1] = {o.lotVt.slots.data(), 0, 0, 5, 1};
}

// A new month for a new "original": state 6, the grid before, and what the
// original leaves after state 4 (`skew` added to (2,3); count 3 + countSkew).
// `unchanged`: state 4 changes nothing at all (no incidents, the grid as
// before), which the shadow cannot tell from a missed state 4.
void OcMonth(FakeOrigCrime& o, uint8_t skew, uint32_t countSkew, bool unchanged = false) {
    o.state = 6;
    o.committed = false;
    std::memset(o.pre, 0, sizeof(o.pre));
    o.pre[1 * 4 + 1] = 100;
    o.pre[2 * 4 + 3] = 7;
    o.pre[3 * 4 + 0] = 1;
    o.incidents.clear();
    if (unchanged) {
        std::memcpy(o.post, o.pre, sizeof(o.post));
        o.count = 0;
        return;
    }
    std::memset(o.post, 0, sizeof(o.post));
    o.post[0 * 4 + 0] = 60;
    o.post[1 * 4 + 0] = 60;
    o.post[1 * 4 + 1] = 50;
    o.post[2 * 4 + 3] = static_cast<uint8_t>(43 + skew);
    o.count = 3 + countSkew;
    const FC4CrimeEvent e[4] = {{8, 12, 0x1001, 0, {}, 0, 0, 0},
                                {0, 0, 0x1002, 0, {}, 0, 0, 0},
                                {12, 0, 0x1003, 0, {}, 0, 0, 0},
                                {13, 13, 0x1001, 0, {}, 0, 0, 0}};
    o.incidents.assign(e, e + 4);
}

intptr_t CrimeShadowOnce(FC4TestSeamFn seam, FakeOrigCrime& o, cIGZMessageTarget2* target) {
    static const uint32_t types[3] = {0x1001, 0x1002, 0x1003};
    static const uint32_t severities[3] = {40, 60, 0};
    FC4CrimeShadowTestInput in{reinterpret_cast<cISC4City*>(&o.city), target, types, severities, 3, 0.5f, 4,
                               2451576};  // first SimNewDay 2000-02-01: state 4 runs on 2000-02-05
    return seam(kFC4SeamCrimeShadow, &in, nullptr);
}

// ---- the plumbing shadow (test seam op 22) -------------------------------------------------------
// An "original" plumbing simulator on a 4x4 city with one pipe flag, at
// (0,0), and nothing else (no producers, pipe occupants, deals or
// connections): FreeCity4's pass publishes the pipe value 8 at (0,0) and 0
// elsewhere, and every total is 0. Its agent runs one pass state per SimIdle
// or SimNewDay (plumbing.md §6, as far as the shadow can see it): 9 -> 0 when
// a recompute is due; 0 counts the pass; 2 resets the working grid; 3 -> 5
// (no network); 5 -> 6 swaps the grids (the iterator fresh); 7 finishes the
// iterator; 8 -> 9. SimNewMonth sets the recompute (no pipes: nothing to age
// or burst). Its record (its own Write, cIGZSerializable) is a version-4
// record (§9) through the segment stream's WriteGZSerializable; the grid
// references are two fake cSC4SimGrid<uint8> (class 0x49B9E602, through a
// serializable stand-in each) answering geometry (slots 10-17) and
// GetTractValue (22). The second grid starts with stale values (a loaded
// city's last pass). The passCounter (record field 7) moves in state 0 only,
// so the shadow sees each pass start. `wrongCell` flips (2,2) of the grid the second pass
// publishes; `watered` is what GetNumWateredBuildings (slot 21) answers. The
// city answers GetPlumbingSimulator (compiled slot 64); `noCity` returns null
// there. Any other call aborts.
struct FakePlumbGrid {
    void** vtbl;
    uint8_t v[16];  // [z * 4 + x]
};
struct FakePlumbGridSer {
    void** vtbl;
    FakePlumbGrid* grid;
};
struct FakeOrigPlumbing {
    FakeVtable simVt, agentVt, serVt, gridVt, gridSerVt, cityVt, noCityVt;
    FakeObject sim{}, agent{}, ser{}, city{}, noCity{}, other{};
    FakePlumbGrid grids[2]{};
    FakePlumbGridSer gridSers[2]{};
    int working = 0;
    int state = 9;
    bool recompute = true;
    int32_t pass = 0;
    uint8_t iter = 2;  // 0 fresh, 1 running, 2 done
    int swaps = 0;
    bool wrongCell = false;
    uint32_t watered = 0;
    int refs = 1;
    int deliveries = 0;
    int writes = 0;
};

FakeOrigPlumbing* g_opb = nullptr;

bool __fastcall OpbQi(void*, void*, uint32_t iid, void** out) {  // 0 (simulator, agent, serializable)
    if (out == nullptr) return false;
    if (iid == 0x452294AA) {
        *out = &g_opb->agent;  // cIGZMessageTarget2
    } else if (iid == GZIID_cIGZSerializable) {
        *out = &g_opb->ser;
    } else {
        return false;  // (the reference audit asks other IIDs)
    }
    ++g_opb->refs;
    return true;
}
uint32_t __fastcall OpbAddRef(void*, void*) { return static_cast<uint32_t>(++g_opb->refs); }   // 1
uint32_t __fastcall OpbRelease(void*, void*) { return static_cast<uint32_t>(--g_opb->refs); }  // 2
int OpbIndex(const void* grid) { return grid == &g_opb->grids[0] ? 0 : 1; }
bool __fastcall OpbGridQi(void* self, void*, uint32_t iid, void** out) {  // grid 0
    if (out == nullptr) return false;
    if (iid == 0xC9BD939A) {
        *out = self;  // cISC4SimGrid
    } else if (iid == GZIID_cIGZSerializable) {
        *out = &g_opb->gridSers[OpbIndex(self)];
    } else {
        return false;
    }
    ++g_opb->refs;
    return true;
}
bool __fastcall OpbGridSerQi(void* self, void*, uint32_t iid, void** out) {  // grid serializable 0
    if (out == nullptr) return false;
    if (iid == 0xC9BD939A) {
        *out = static_cast<FakePlumbGridSer*>(self)->grid;
    } else if (iid == GZIID_cIGZSerializable) {
        *out = self;
    } else {
        return false;
    }
    ++g_opb->refs;
    return true;
}
uint32_t __fastcall OpbGridClsid(void*, void*) { return 0x49B9E602; }  // grid serializable 5
int32_t __fastcall OpbTractSize(void*, void*) { return 1; }             // grid 10
int32_t __fastcall OpbTractShift(void*, void*) { return 0; }            // grid 11
int32_t __fastcall OpbTractCount(void*, void*) { return 4; }            // grid 12, 13
float __fastcall OpbTractWidth(void*, void*) { return 16.0f; }          // grid 14, 15
float __fastcall OpbTractInvWidth(void*, void*) { return 1.0f / 16.0f; }  // grid 16, 17
uint8_t __fastcall OpbTractValue(void* self, void*, int32_t x, int32_t z) {  // grid 22
    if (x < 0 || x > 3 || z < 0 || z > 3) return 0;
    return static_cast<FakePlumbGrid*>(self)->v[z * 4 + x];
}
void* __fastcall OpbWateredGrid(void*, void*) { return &g_opb->grids[1 - g_opb->working]; }  // sim 7
uint32_t __fastcall OpbZero(void*, void*) { return 0; }                    // sim 12-20, 22, 23
uint32_t __fastcall OpbWateredCount(void*, void*) { return g_opb->watered; }  // sim 21
uint32_t __fastcall OpbClsid(void*, void*) { return kCLSID_Plumbing; }     // serializable 5
// Serializable 3: the version-4 record of §9.
bool __fastcall OpbWrite(void*, void*, cIGZOStream* s) {
    FakeOrigPlumbing& o = *g_opb;
    ++o.writes;
    void* p = nullptr;
    if (s == nullptr || !s->QueryInterface(kFC4_IID_cISC4DBSegmentOStream, &p) || p == nullptr) return false;
    cFC4ISC4DBSegmentOStream* w = static_cast<cFC4ISC4DBSegmentOStream*>(p);
    w->SetUint16(4);
    w->WriteGZSerializable(reinterpret_cast<cIGZSerializable*>(&o.gridSers[o.working]));      // working
    w->WriteGZSerializable(reinterpret_cast<cIGZSerializable*>(&o.gridSers[1 - o.working]));  // published
    w->SetUint32(4);
    w->SetUint32(4);
    uint8_t flags[16] = {};
    flags[0] = 0x10;  // the pipe bit at (0,0)
    w->SetVoid(flags, sizeof(flags));
    w->SetSint32(o.pass);
    w->SetUint32(0);  // average water pollution
    w->SetSint64(0);  // total water cost
    w->SetUint32(0);  // producers
    w->SetUint32(0);  // polluted producers
    for (int i = 0; i < 8; ++i) w->SetUint32(0);  // the current producer: none
    w->WriteGZSerializable(nullptr);
    w->SetUint8(o.iter);
    w->SetSint32(0);
    w->SetSint32(0);
    w->SetUint32(1);
    w->SetSint32(0);
    w->SetSint32(0);
    w->SetSint32(3);
    w->SetSint32(3);
    w->SetSint64(0);  // demand
    w->SetUint32(0);  // pipes
    w->SetUint32(0);  // distressed
    w->SetUint32(0);  // burst
    w->WriteGZSerializable(nullptr);  // the department
    for (int i = 0; i < 4; ++i) w->SetSint64(0);  // last produced, consumed, imported, exported
    w->Release();
    return true;
}
bool __fastcall OpbDoMessage(void*, void*, cIGZMessage2* m) {  // agent 3
    FakeOrigPlumbing& o = *g_opb;
    ++o.deliveries;
    const uint32_t type = m->GetType();
    if (type == kMsgSimNewMonth) {
        o.recompute = true;
        return true;
    }
    if (type != kMsgSimNewDay && type != kMsgSimIdle) return true;
    switch (o.state) {
        case 9:
            if (o.recompute) {
                o.recompute = false;
                o.state = 0;
            }
            break;
        case 0:
            ++o.pass;
            o.state = 1;
            break;
        case 1: o.state = 2; break;
        case 2: {
            FakePlumbGrid& g = o.grids[o.working];
            std::memset(g.v, 0, sizeof(g.v));
            g.v[0] = 8;
            o.iter = 2;
            o.state = 3;
            break;
        }
        case 3: o.state = 5; break;
        case 5:
            o.working = 1 - o.working;
            o.iter = 0;
            o.state = 7;
            if (++o.swaps == 2 && o.wrongCell) o.grids[1 - o.working].v[2 * 4 + 2] ^= 1;
            break;
        case 7:
            o.iter = 2;
            o.state = 8;
            break;
        default:
            o.state = 9;
            break;
    }
    return true;
}
void* __fastcall OpbCityPlumbing(void*, void*) { return &g_opb->sim; }  // city 64
void* __fastcall OpbCityNoPlumbing(void*, void*) { return nullptr; }    // city 64

void InitFakeOrigPlumbing(FakeOrigPlumbing& o) {
    g_opb = &o;
    for (FakeVtable* vt : {&o.simVt, &o.agentVt, &o.serVt}) {
        vt->slots[0] = reinterpret_cast<void*>(&OpbQi);
        vt->slots[1] = reinterpret_cast<void*>(&OpbAddRef);
        vt->slots[2] = reinterpret_cast<void*>(&OpbRelease);
    }
    o.gridVt.slots[0] = reinterpret_cast<void*>(&OpbGridQi);
    o.gridSerVt.slots[0] = reinterpret_cast<void*>(&OpbGridSerQi);
    for (FakeVtable* vt : {&o.gridVt, &o.gridSerVt}) {
        vt->slots[1] = reinterpret_cast<void*>(&OpbAddRef);
        vt->slots[2] = reinterpret_cast<void*>(&OpbRelease);
    }
    o.gridSerVt.slots[5] = reinterpret_cast<void*>(&OpbGridClsid);
    o.gridVt.slots[10] = reinterpret_cast<void*>(&OpbTractSize);
    o.gridVt.slots[11] = reinterpret_cast<void*>(&OpbTractShift);
    o.gridVt.slots[12] = reinterpret_cast<void*>(&OpbTractCount);
    o.gridVt.slots[13] = reinterpret_cast<void*>(&OpbTractCount);
    o.gridVt.slots[14] = reinterpret_cast<void*>(&OpbTractWidth);
    o.gridVt.slots[15] = reinterpret_cast<void*>(&OpbTractWidth);
    o.gridVt.slots[16] = reinterpret_cast<void*>(&OpbTractInvWidth);
    o.gridVt.slots[17] = reinterpret_cast<void*>(&OpbTractInvWidth);
    o.gridVt.slots[22] = reinterpret_cast<void*>(&OpbTractValue);
    o.simVt.slots[7] = reinterpret_cast<void*>(&OpbWateredGrid);
    for (int s = 12; s <= 23; ++s) o.simVt.slots[s] = reinterpret_cast<void*>(&OpbZero);
    o.simVt.slots[21] = reinterpret_cast<void*>(&OpbWateredCount);
    o.serVt.slots[3] = reinterpret_cast<void*>(&OpbWrite);
    o.serVt.slots[5] = reinterpret_cast<void*>(&OpbClsid);
    o.agentVt.slots[3] = reinterpret_cast<void*>(&OpbDoMessage);
    o.cityVt.slots[64] = reinterpret_cast<void*>(&OpbCityPlumbing);
    o.noCityVt.slots[64] = reinterpret_cast<void*>(&OpbCityNoPlumbing);
    o.sim.vtbl = o.simVt.slots.data();
    o.agent.vtbl = o.agentVt.slots.data();
    o.ser.vtbl = o.serVt.slots.data();
    o.city.vtbl = o.cityVt.slots.data();
    o.noCity.vtbl = o.noCityVt.slots.data();
    o.other.vtbl = o.agentVt.slots.data();  // an agent, but not the simulator's
    for (int i = 0; i < 2; ++i) {
        o.grids[i].vtbl = o.gridVt.slots.data();
        o.gridSers[i] = FakePlumbGridSer{o.gridSerVt.slots.data(), &o.grids[i]};
    }
}

// A new "original" (state 9, a recompute due, the grids as a loaded city left
// them: the published one with stale values) for each run.
void OpbReset(FakeOrigPlumbing& o, bool wrongCell, uint32_t watered) {
    o.working = 0;
    o.state = 9;
    o.recompute = true;
    o.pass = 0;
    o.iter = 2;
    o.swaps = 0;
    o.wrongCell = wrongCell;
    o.watered = watered;
    std::memset(o.grids[0].v, 0, sizeof(o.grids[0].v));
    std::memset(o.grids[1].v, 0, sizeof(o.grids[1].v));
    o.grids[1].v[1 * 4 + 1] = 3;  // a watered lot cell of the last pass
}

// One run: twelve SimIdle (the first pass), a SimNewMonth (the pipe funding;
// it also sets the recompute) and twelve SimIdle (the second pass), all on
// 2000-02-01. Both passes are seen from their start (the passCounter the
// fake's state 0 moves, plumbing.md §6.1 PS6). The shadow is made as at a
// city start unless `midCity` (then the month's distressed count is not
// exact, PS3).
intptr_t PlumbingShadowRun(FC4TestSeamFn seam, FakeObject* city, cIGZMessageTarget2* target, bool midCity = false) {
    static uint32_t types[25];
    static int32_t julians[25];
    for (int i = 0; i < 25; ++i) {
        types[i] = i == 12 ? kMsgSimNewMonth : kMsgSimIdle;
        julians[i] = 2451576;
    }
    FC4PlumbingShadowTestInput in{reinterpret_cast<cISC4City*>(city), target, types, julians, 25, 0, nullptr,
                                  midCity ? 1u : 0u};
    return seam(kFC4SeamPlumbingShadow, &in, nullptr);
}

// A vector for GetCrimeIncidentArray in this executable (the game's allocator
// is not available here).
struct TestHeap {
    static bool Available() { return true; }
    static void* Allocate(uint32_t size) { return std::malloc(size); }
    static void Free(void* p) { std::free(p); }
};

// ---- the reference audit's created-temporary release (test seam op 12) ----------------------------
// A game object (vtable in this executable) whose Release returns 0: the
// last reference. QueryInterface (the audit's class lookup) answers no.
bool __fastcall TempQi(void*, void*, uint32_t, void** out) {
    if (out != nullptr) *out = nullptr;
    return false;
}
uint32_t __fastcall TempRelease(void*, void*) { return 0; }

// ---- the controller manager's teardown releases (test seam op 69) ----------------------------------
// A game object with a real count: the controller of a loaded city after a
// save, whose only holder is the manager (foundations 2.7 SV1).
struct CountedFake {
    void** vtbl;
    uint32_t refs;
};
uint32_t __fastcall CountedAddRef(void* self, void*) { return ++static_cast<CountedFake*>(self)->refs; }
uint32_t __fastcall CountedRelease(void* self, void*) {
    CountedFake* o = static_cast<CountedFake*>(self);
    return o->refs == 0 ? 0 : --o->refs;
}

// ---- close_dialogs: a fake window manager, legacy message server and dialogs (test seam op 47) ----
// The closer (FC4Dialogs.h) calls them through their vtables: manager slots
// 14, 24, 36, 43, 54, window slots 11, 63, 67, server slots 5, 6; every other
// slot traps. DlgBegin / DlgFrame / DlgLeave play DoModalWin and its loop
// (modal-dialogs.md §2.2-§2.4): the begin notice before the window becomes
// modal, one framework frame (the observer's tick) per pass, and at the end
// the nest count and flag cleared, one more frame, then the end notice.

constexpr uint32_t kDlgBegin = 0x62BA7B42;
constexpr uint32_t kDlgEnd = 0x62BA7B43;

struct FakeDialog {
    FakeObject obj{};  // first: the cIGZWin* of the notices
    uint32_t id = 0;
    FakeDialog* parent = nullptr;
    bool valid = true;        // in the manager's window set
    bool modal = false;       // flag 0x1000
    bool obeysButton = true;  // false: ignores button 0 (edge reconciliation, missing plugin packs, §3.3)
    int endIn = 0;            // > 0: the loop sees the end message at this many passes from now
    int presses = 0;
    int endModals = 0;
    int flagReads = 0;
    int pressSeq = 0;
    int endSeq = 0;
    uint32_t pressType = 0, pressCode = 0, pressButton = 0xFFFFFFFFu, pressToggle = 0xFFFFFFFFu;
    uint32_t endModalResult = 0xFFFFFFFFu;
};

struct FakeDialogGame {
    FC4TestSeamFn seam = nullptr;
    FakeVtable wmVt, msVt, winVt, delayVt;
    FakeObject wm{}, ms{};
    int32_t nest = 0;
    FakeDialog* focus = nullptr;
    std::vector<FakeDialog*> windows;  // the manager's window set (IsWindowValid)
    std::vector<FakeDialog*> running;  // the modal loops, innermost last
    cIGZMessageTarget* target = nullptr;
    uint32_t added[4] = {};
    int adds = 0, removes = 0, seq = 0;
    int validCalls = 0, nestCalls = 0, focusCalls = 0, strayCalls = 0;
};

FakeDialogGame g_dialogGame;

FakeDialog* DlgOf(void* win) {
    for (FakeDialog* d : g_dialogGame.windows) {
        if (&d->obj == win) return d;
    }
    return nullptr;
}

// cIGZWinMgr 24 IsWindowValid (pop 4, AL): looks the pointer value up.
bool __fastcall DlgWmIsWindowValid(void*, void*, void* win) {
    ++g_dialogGame.validCalls;
    const FakeDialog* d = DlgOf(win);
    return d != nullptr && d->valid;
}
// 43 GetModalNestCount (pop 0).
int32_t __fastcall DlgWmGetModalNestCount(void*, void*) {
    ++g_dialogGame.nestCalls;
    return g_dialogGame.nest;
}
// 36 GZGetFocus (pop 0).
void* __fastcall DlgWmGZGetFocus(void*, void*) {
    ++g_dialogGame.focusCalls;
    return g_dialogGame.focus != nullptr ? &g_dialogGame.focus->obj : nullptr;
}
// 14 PostMessageToTarget(win, type, d1, d2, d3) (pop 20): a command for button
// 0 is delivered at the manager's next tick; the handler of a dialog that
// obeys it calls EndModal, and the loop sees that end one pass later.
void __fastcall DlgWmPost5(void*, void*, void* win, uint32_t type, uint32_t d1, uint32_t d2, uint32_t d3) {
    FakeDialog* d = DlgOf(win);
    if (d == nullptr) {
        ++g_dialogGame.strayCalls;
        return;
    }
    ++d->presses;
    d->pressSeq = ++g_dialogGame.seq;
    d->pressType = type;
    d->pressCode = d1;
    d->pressButton = d2;
    d->pressToggle = d3;
    if (d->obeysButton && type == 3 && d1 == 0x287259F6u && d->endIn == 0) d->endIn = 2;
}
// 54 EndModal(win, result) (pop 8, AL): the end message, seen at the next pass.
bool __fastcall DlgWmEndModal(void*, void*, void* win, uint32_t result) {
    FakeDialog* d = DlgOf(win);
    if (d == nullptr) {
        ++g_dialogGame.strayCalls;
        return false;
    }
    ++d->endModals;
    d->endModalResult = result;
    if (d->endIn == 0) d->endIn = 1;
    return true;
}
// cIGZWin 67 GetFlag(mask) (pop 4), 11 GetParentWin, 63 GetID.
bool __fastcall DlgWinGetFlag(void* self, void*, uint32_t mask) {
    auto* d = reinterpret_cast<FakeDialog*>(self);
    ++d->flagReads;
    return mask == 0x1000u && d->modal;
}
void* __fastcall DlgWinGetParentWin(void* self, void*) {
    auto* d = reinterpret_cast<FakeDialog*>(self);
    return d->parent != nullptr ? &d->parent->obj : nullptr;
}
uint32_t __fastcall DlgWinGetID(void* self, void*) { return reinterpret_cast<FakeDialog*>(self)->id; }
// The legacy cIGZMessageServer 5 AddNotification / 6 RemoveNotification (pop 8):
// the server AddRefs the target it keeps and releases it at the removal.
bool __fastcall DlgMsAddNotification(void*, void*, cIGZMessageTarget* target, uint32_t type) {
    if (g_dialogGame.adds < 4) g_dialogGame.added[g_dialogGame.adds] = type;
    ++g_dialogGame.adds;
    target->AddRef();
    g_dialogGame.target = target;
    return true;
}
bool __fastcall DlgMsRemoveNotification(void*, void*, cIGZMessageTarget* target, uint32_t) {
    ++g_dialogGame.removes;
    target->Release();
    if (g_dialogGame.removes % 2 == 0) g_dialogGame.target = nullptr;
    return true;
}

void DlgSend(uint32_t type, FakeDialog* d, int32_t nest) {
    if (g_dialogGame.target == nullptr) return;  // nobody subscribed
    cGZMessage m(type);
    m.dwData1 = d->id;
    m.dwData2 = static_cast<uint32_t>(reinterpret_cast<uintptr_t>(&d->obj));
    m.dwData3 = static_cast<uint32_t>(nest);
    g_dialogGame.target->DoMessage(m);  // MessageSend: synchronous
}

void DlgTick() { g_dialogGame.seam(kFC4SeamObserverTick, nullptr, nullptr); }

// The installed fakes; `server` = false: no legacy message server (the
// framework's, which a test does not have).
void DlgServices(bool server, bool gameDelay = true) {
    FC4DialogsTestInput in{kFC4DialogsTestServices, &g_dialogGame.wm, server ? &g_dialogGame.ms : nullptr,
                           gameDelay ? g_dialogGame.delayVt.slots.data() : nullptr};
    g_dialogGame.seam(kFC4SeamDialogs, &in, nullptr);
}

intptr_t DlgSeam(uint32_t action) {
    FC4DialogsTestInput in{action, nullptr, nullptr, nullptr};
    return g_dialogGame.seam(kFC4SeamDialogs, &in, nullptr);
}

void DlgSetup(FC4TestSeamFn seam) {
    FakeDialogGame& g = g_dialogGame;
    g.seam = seam;
    g.wmVt.slots[14] = reinterpret_cast<void*>(&DlgWmPost5);
    g.wmVt.slots[24] = reinterpret_cast<void*>(&DlgWmIsWindowValid);
    g.wmVt.slots[36] = reinterpret_cast<void*>(&DlgWmGZGetFocus);
    g.wmVt.slots[43] = reinterpret_cast<void*>(&DlgWmGetModalNestCount);
    g.wmVt.slots[54] = reinterpret_cast<void*>(&DlgWmEndModal);
    g.wm.vtbl = g.wmVt.slots.data();
    g.msVt.slots[5] = reinterpret_cast<void*>(&DlgMsAddNotification);
    g.msVt.slots[6] = reinterpret_cast<void*>(&DlgMsRemoveNotification);
    g.ms.vtbl = g.msVt.slots.data();
    g.winVt.slots[11] = reinterpret_cast<void*>(&DlgWinGetParentWin);
    g.winVt.slots[63] = reinterpret_cast<void*>(&DlgWinGetID);
    g.winVt.slots[67] = reinterpret_cast<void*>(&DlgWinGetFlag);
    // delayVt (SC4GameDelay's window): traps only; nothing may be called on it.
}

// A window the manager knows; `gameDelay`: SC4GameDelay's (its vtable).
void DlgWindow(FakeDialog& d, uint32_t id, bool gameDelay = false) {
    d.id = id;
    d.obj.vtbl = gameDelay ? g_dialogGame.delayVt.slots.data() : g_dialogGame.winVt.slots.data();
    g_dialogGame.windows.push_back(&d);
}

// DoModalWin's start (§2.2 steps 2-3): the begin notice with the nest count
// before; then the focus, the modal flag and the nest count + 1.
void DlgBegin(FakeDialog& d) {
    DlgSend(kDlgBegin, &d, g_dialogGame.nest);
    g_dialogGame.focus = &d;
    d.modal = true;
    ++g_dialogGame.nest;
    g_dialogGame.running.push_back(&d);
}

// The loop has ended (§2.2 steps 5-6): nest count - 1, flag cleared, the
// previous focus, one more frame, then the end notice with the nest count after.
void DlgLeave(FakeDialog& d) {
    --g_dialogGame.nest;
    d.modal = false;
    g_dialogGame.running.pop_back();
    g_dialogGame.focus = g_dialogGame.running.empty() ? nullptr : g_dialogGame.running.back();
    DlgTick();
    d.endSeq = ++g_dialogGame.seq;
    DlgSend(kDlgEnd, &d, g_dialogGame.nest);
}

// One pass of the innermost loop (§2.3): an end message seen now stops the
// loop after this pass's frame.
void DlgFrame() {
    FakeDialog* d = g_dialogGame.running.empty() ? nullptr : g_dialogGame.running.back();
    const bool ends = d != nullptr && d->endIn > 0 && --d->endIn == 0;
    DlgTick();
    if (ends) DlgLeave(*d);
}

// "0x<id> (window 0x<ptr>, depth N)", as the closer logs it.
std::string DlgName(const FakeDialog& d, long depth) {
    char text[64];
    std::snprintf(text, sizeof(text), "0x%08lX (window 0x%08lX, depth %ld)", static_cast<unsigned long>(d.id),
                  static_cast<unsigned long>(reinterpret_cast<uintptr_t>(&d.obj)), depth);
    return text;
}

// ---- autotest: child processes that end with TerminateProcess -----------------------------------

// Runs this executable with `args`; returns its exit code (or -1) and the time it took.
int RunChild(const std::string& args, DWORD timeoutMs, DWORD* elapsedMs) {
    char exe[MAX_PATH] = {};
    GetModuleFileNameA(nullptr, exe, MAX_PATH);
    std::string cmd = "\"" + std::string(exe) + "\" " + args;
    STARTUPINFOA si{};
    si.cb = sizeof(si);
    PROCESS_INFORMATION pi{};
    const DWORD start = GetTickCount();
    if (!CreateProcessA(nullptr, &cmd[0], nullptr, nullptr, FALSE, 0, nullptr, nullptr, &si, &pi)) return -1;
    const DWORD w = WaitForSingleObject(pi.hProcess, timeoutMs);
    *elapsedMs = GetTickCount() - start;
    DWORD code = static_cast<DWORD>(-1);
    if (w == WAIT_OBJECT_0) {
        GetExitCodeProcess(pi.hProcess, &code);
    } else {
        TerminateProcess(pi.hProcess, 99);
        std::fprintf(stderr, "child '%s' did not end within %lu ms\n", args.c_str(), static_cast<unsigned long>(timeoutMs));
    }
    CloseHandle(pi.hThread);
    CloseHandle(pi.hProcess);
    return w == WAIT_OBJECT_0 ? static_cast<int>(code) : -1;
}

// The value of `name=` in a verdict line ("" if absent).
std::string VerdictField(const std::string& line, const std::string& name) {
    const size_t at = line.find(" " + name + "=");
    if (at == std::string::npos) return "";
    const size_t from = at + name.size() + 2;
    const size_t end = line.find_first_of(",)", from);
    return line.substr(from, end == std::string::npos ? std::string::npos : end - from);
}

// "digits.digits" with exactly `decimals` decimals.
bool IsDecimal(const std::string& s, size_t decimals) {
    const size_t dot = s.find('.');
    if (dot == std::string::npos || dot == 0 || s.size() - dot - 1 != decimals) return false;
    for (size_t i = 0; i < s.size(); ++i) {
        if (i != dot && (s[i] < '0' || s[i] > '9')) return false;
    }
    return true;
}

// The log's one "AUTOTEST VERDICT:" line with its run-time fields (the
// harness lane: real_seconds always, days_per_second if `rate`) replaced by
// S and R when well formed, so the rest compares exactly.
std::string NormalVerdict(const std::string& text, bool rate) {
    const size_t at = text.find("AUTOTEST VERDICT:");
    if (at == std::string::npos) return "";
    const size_t end = text.find_first_of("\r\n", at);
    std::string line = text.substr(at, end == std::string::npos ? std::string::npos : end - at);
    auto replace = [&line](const std::string& name, size_t decimals, const char* with) {
        const std::string v = VerdictField(line, name);
        if (!IsDecimal(v, decimals)) return;
        const size_t pos = line.find(" " + name + "=" + v) + name.size() + 2;
        line.replace(pos, v.size(), with);
    };
    replace("real_seconds", 3, "S");
    if (rate) replace("days_per_second", 2, "R");
    return line;
}

// days_per_second x real_seconds of a verdict: the days the run counted.
double VerdictDays(const std::string& text) {
    const size_t at = text.find("AUTOTEST VERDICT:");
    if (at == std::string::npos) return -1.0;
    const std::string line = text.substr(at, text.find_first_of("\r\n", at) - at);
    return std::atof(VerdictField(line, "days_per_second").c_str()) * std::atof(VerdictField(line, "real_seconds").c_str());
}

// A child: loads the DLL with an autotest FreeCity4.ini and plays the game's
// part through the test seam. Never returns normally (the DLL terminates it);
// exit code 3 means it did not.
int AutotestChild(const std::string& dll, const std::string& ini, const std::string& scenario) {
    if (scenario == "clean") {
        WriteTextFile(ini, "[autotest]\nautotest_seconds = 2\nautotest_unpause = 1\n");
    } else if (scenario == "corrupt") {
        WriteTextFile(ini, "[autotest]\nautotest_seconds = 60\n");
    } else if (scenario == "save") {
        WriteTextFile(ini, "[autotest]\nautotest_save_after_months = 2\nautotest_save_fast = 0\nobserve = 1\n");
    } else if (scenario == "panic") {
        WriteTextFile(ini, "[autotest]\n");
    } else if (scenario == "enddate") {  // the harness lane: autotest_end_date alone, passthrough
        WriteTextFile(ini, "[subsystems]\ndisable = all\n[autotest]\nautotest_end_date = 2000-03-01\n");
    } else if (scenario == "enddate-timeout") {  // the timeout comes first
        WriteTextFile(ini, "[subsystems]\ndisable = all\n[autotest]\nautotest_seconds = 2\n"
                           "autotest_end_date = 2100-01-01\n");
    } else if (scenario == "enddate-quit") {  // an end date already past at the city load; autotest_end = quit
        WriteTextFile(ini, "[subsystems]\ndisable = all\n[autotest]\nautotest_seconds = 60\nautotest_end = quit\n"
                           "autotest_end_date = 2000-01-15\n");
    } else if (scenario == "enddate-save") {  // a script save waits for its tick on the end date
        const std::string dir = ini.substr(0, ini.find_last_of("\\/") + 1);
        WriteTextFile(dir + "fc4_script_endsave.txt",
                      "2000-02-26 mark the day before\n2000-02-27 save\n2000-02-27 mark after the save\n");
        WriteTextFile(ini, "[subsystems]\ndisable = all\n[autotest]\nautotest_end_date = 2000-02-27\n[harness]\n"
                           "script = fc4_script_endsave.txt\n");
    } else if (scenario == "enddate-dialogs") {  // close_dialogs = 1: the verdict gains dialogs_closed=
        WriteTextFile(ini, "[subsystems]\ndisable = all\n[autotest]\nautotest_end_date = 2000-03-01\n[harness]\n"
                           "close_dialogs = 1\n");
    } else if (scenario == "enddate-bad") {  // not a date: ignored, the timeout ends the run
        WriteTextFile(ini, "[subsystems]\ndisable = all\n[autotest]\nautotest_seconds = 1\n"
                           "autotest_end_date = 2000-02-30\n");
    } else {  // stalled, ended: passthrough (the observer alone)
        WriteTextFile(ini, "[subsystems]\ndisable = all\n[autotest]\nautotest_seconds = 60\n");
    }
    HMODULE h = LoadLibraryA(dll.c_str());
    if (h == nullptr) return 4;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(h, "GZDllGetGZCOMDirector"));
    auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(h, "FC4TestSeam"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    if (director == nullptr || seam == nullptr) return 5;
    std::vector<uint32_t> classes;
    director->EnumClassObjects(&CollectClass, &classes);  // game start: configuration and autotest settings
    if (scenario == "panic") {
        seam(kFC4SeamRustPanic, nullptr, nullptr);  // release builds end here (fail-fast)
        return 3;
    }

    static FakeWorld world;
    InitFakeWorld(world);
    cISC4Simulator* sim = nullptr;
    if (scenario == "clean" || scenario == "save") {
        if (!director->GetClassObject(kCLSID_Simulator, kIID_cISC4Simulator, reinterpret_cast<void**>(&sim)) ||
            sim == nullptr) {
            return 6;
        }
        sim->Init();
        sim->Pause();  // the autotest must resume it
        world.sim = sim;
    }
    if (scenario == "save") {
        // Two month-ends, then the save at the next tick ends the process.
        seam(kFC4SeamObserverApp, &world.app, nullptr);
        seam(kFC4SeamObserverCity, &world.city, nullptr);
        seam(kFC4SeamObserverMessage, reinterpret_cast<void*>(static_cast<uintptr_t>(kMsgPostCityInit)), nullptr);
        seam(kFC4SeamObserverTick, nullptr, nullptr);  // the "after load" snapshot
        for (intptr_t julian : {2451576, 2451605}) {
            seam(kFC4SeamObserverMessage, reinterpret_cast<void*>(static_cast<uintptr_t>(kMsgSimNewMonth)),
                 reinterpret_cast<void*>(julian));
        }
        seam(kFC4SeamObserverTick, nullptr, nullptr);  // saves and terminates
        return 3;
    }
    if (scenario.compare(0, 7, "enddate") == 0) {
        // The city loads, the clock starts at the first tick, then one new day
        // per tick from 2000-02-25 (2451600) until the DLL ends the process.
        if (scenario == "enddate-save") seam(kFC4SeamObserverApp, &world.app, nullptr);
        seam(kFC4SeamObserverCity, &world.city, nullptr);
        seam(kFC4SeamObserverMessage, reinterpret_cast<void*>(static_cast<uintptr_t>(kMsgPostCityInit)), nullptr);
        seam(kFC4SeamObserverTick, nullptr, nullptr);
        if (scenario == "enddate-dialogs") {
            // One dialog, closed by button before the days run (fake window manager, seam op 47).
            DlgSetup(seam);
            DlgServices(true);
            DlgSeam(kFC4DialogsTestStart);
            static FakeDialog dialog;
            DlgWindow(dialog, 0x2A5CFB2Cu);
            DlgBegin(dialog);
            for (int i = 0; i < 10; ++i) DlgFrame();
        }
        const DWORD start = GetTickCount();
        for (intptr_t julian = 2451600; GetTickCount() - start < 20000; ++julian) {
            Sleep(20);
            seam(kFC4SeamObserverMessage, reinterpret_cast<void*>(static_cast<uintptr_t>(kMsgSimNewDay)),
                 reinterpret_cast<void*>(julian));
            seam(kFC4SeamObserverTick, nullptr, nullptr);
        }
        return 3;
    }
    seam(kFC4SeamObserverCity, &world.city, nullptr);
    seam(kFC4SeamObserverMessage, reinterpret_cast<void*>(static_cast<uintptr_t>(kMsgPostCityInit)), nullptr);
    seam(kFC4SeamObserverMessage, reinterpret_cast<void*>(static_cast<uintptr_t>(kMsgSimNewDay)),
         reinterpret_cast<void*>(static_cast<intptr_t>(2451600)));
    const DWORD start = GetTickCount();
    bool faulted = false;
    for (;;) {
        seam(kFC4SeamObserverTick, nullptr, nullptr);
        Sleep(20);
        const DWORD ms = GetTickCount() - start;
        if (scenario == "corrupt" && ms >= 300 && !faulted) {
            faulted = true;
            ForeignFault();  // then keep ticking until the DLL ends the process
        }
        if (scenario == "stalled" && ms >= 500) Sleep(INFINITE);  // the main thread stops ticking
        if (scenario == "ended" && ms >= 500) {
            // The game shuts the city down by itself (a bankrupt city) long
            // before the 60 s deadline: the run ends at PostCityShutdown.
            seam(kFC4SeamObserverMessage, reinterpret_cast<void*>(static_cast<uintptr_t>(0x26D31EC2u)), nullptr);
            seam(kFC4SeamObserverMessage, reinterpret_cast<void*>(static_cast<uintptr_t>(0x26D31EC3u)), nullptr);
            return 3;  // not reached: the DLL terminates the process
        }
        if (ms > 60000) return 3;
    }
}

// ---- popup_dialogs (FC4Popups.h, advisors §6.8.1 option 1): one child process per FreeCity4.ini ----
// A cISC4App with only slots 23/24; every other slot traps, so the switch can
// call nothing else (no QueryInterface, AddRef or Release of the borrowed app).
struct FakePopupApp {
    FakeVtable vt;
    FakeObject app{};
    bool enabled = true;
    bool ignoreSet = false;  // a setter that does nothing ("did not take effect")
    int gets = 0;
    int sets = 0;
};

FakePopupApp* g_popupApp = nullptr;

// NG35 (neighbors.md §15.4): a stand-in for nSC4UI::GenericConfirmationDialog
// (cdecl, four strings, true only for Yes), given to the plugin through
// FC4TestSeam op 66.
struct FakeConfirm {
    int calls = 0;
    bool answer = false;
    bool stringsOk = false;   // title and text non-null, with text
    bool labelsNull = false;  // the default Yes / No
};
FakeConfirm g_fakeConfirm;
bool __cdecl FakeConfirmDialog(cIGZString* title, cIGZString* text, cIGZString* yes, cIGZString* no) {
    ++g_fakeConfirm.calls;
    g_fakeConfirm.stringsOk =
        title != nullptr && text != nullptr && title->ToChar() != nullptr && text->ToChar() != nullptr;
    g_fakeConfirm.labelsNull = yes == nullptr && no == nullptr;
    return g_fakeConfirm.answer;
}

bool __fastcall AppGetPopupDialogsEnabled(void*, void*) {  // cISC4App 23 (+0x5C), pops 0
    ++g_popupApp->gets;
    return g_popupApp->enabled;
}
void __fastcall AppSetPopupDialogsEnabled(void*, void*, bool on) {  // cISC4App 24 (+0x60), one bool, pops 4
    ++g_popupApp->sets;
    if (!g_popupApp->ignoreSet) g_popupApp->enabled = on;
}

// Scenarios: off (popup_dialogs = 0), on (= 1; the parent also sets
// FC4_POPUP_DIALOGS=0, which the ini overrides), env (FC4_POPUP_DIALOGS=0
// only), bad (= maybe), unset, stuck (= 0 with a setter that does nothing).
// Messages, in order: a city load and a ConnectionsReady without an
// application (tests have no framework); then, with the fake, two
// ConnectionsReady, a second city load and its ConnectionsReady. = 0 sets
// false at the first ConnectionsReady with the app, true at the second load
// and false again at its ConnectionsReady (foundations.md 11.6); = 1 makes
// its one call at the second load. Returns 0 if the fake saw what the
// scenario expects; the parent checks the log.
int PopupsChild(const std::string& dll, const std::string& ini, const std::string& scenario) {
    if (scenario == "off" || scenario == "stuck") {
        WriteTextFile(ini, "[unattended]\npopup_dialogs = 0   ; no advisor popups\n");
    } else if (scenario == "on") {
        WriteTextFile(ini, "[unattended]\npopup_dialogs = 1\n");
    } else if (scenario == "bad") {
        WriteTextFile(ini, "[unattended]\npopup_dialogs = maybe\n");
    } else {  // env, unset
        WriteTextFile(ini, "; no popup_dialogs key\n");
    }
    HMODULE h = LoadLibraryA(dll.c_str());
    if (h == nullptr) return 4;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(h, "GZDllGetGZCOMDirector"));
    auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(h, "FC4TestSeam"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    if (director == nullptr || seam == nullptr) return 5;
    std::vector<uint32_t> classes;
    director->EnumClassObjects(&CollectClass, &classes);  // game start: popup_dialogs is read

    static FakePopupApp fake;
    g_popupApp = &fake;
    fake.vt.slots[23] = reinterpret_cast<void*>(&AppGetPopupDialogsEnabled);
    fake.vt.slots[24] = reinterpret_cast<void*>(&AppSetPopupDialogsEnabled);
    fake.app.vtbl = fake.vt.slots.data();
    fake.enabled = scenario != "on";  // "on": the game's flag starts false
    fake.ignoreSet = scenario == "stuck";
    void* const cityInit = reinterpret_cast<void*>(static_cast<uintptr_t>(kMsgPostCityInit));
    void* const connReady = reinterpret_cast<void*>(static_cast<uintptr_t>(0x6AC284F3u));  // ConnectionsReady

    seam(kFC4SeamObserverMessage, cityInit, nullptr);   // no application: = 1 skipped, logged
    seam(kFC4SeamObserverMessage, connReady, nullptr);  // no application: = 0 skipped, logged
    const int setsWithoutApp = fake.sets;
    seam(kFC4SeamObserverApp, &fake.app, nullptr);
    seam(kFC4SeamObserverMessage, connReady, nullptr);  // = 0: false
    const int setsFirst = fake.sets;
    const bool enabledFirst = fake.enabled;
    seam(kFC4SeamObserverMessage, connReady, nullptr);  // = 0: already false, nothing
    const int setsSecond = fake.sets;
    seam(kFC4SeamObserverMessage, cityInit, nullptr);  // = 0: true until ConnectionsReady; = 1: the call
    const bool enabledLoad = fake.enabled;
    seam(kFC4SeamObserverMessage, connReady, nullptr);  // = 0: false again
    seam(kFC4SeamObserverApp, nullptr, nullptr);
    FreeLibrary(h);

    const bool off = scenario == "off" || scenario == "env";
    // Sets: off/env 3 (false, true, false); stuck 1 (the failed call is not
    // retried); on 1 (at the second city load); bad/unset 0.
    const int wantSets = off ? 3 : (scenario == "stuck" || scenario == "on") ? 1 : 0;
    const int wantFirst = off || scenario == "stuck" ? 1 : 0;
    int bad = 0;
    if (setsWithoutApp != 0) bad |= 1;
    if (setsFirst != wantFirst || setsSecond != setsFirst || fake.sets != wantSets) bad |= 2;
    if (fake.gets != 2 * wantSets) bad |= 4;  // before and after each call
    if (fake.enabled != !off) bad |= 8;
    if (off && (enabledFirst || !enabledLoad)) bad |= 16;
    if (bad != 0) std::fprintf(stderr, "popups child %s: fake app check bits 0x%X\n", scenario.c_str(), bad);
    return bad == 0 ? 0 : 10 + bad;
}

int PopupsTest(const std::string& dll, const std::string& ini, const std::string& log) {
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    const std::string dllArg = "\"" + dll + "\" ";
    const auto npos = std::string::npos;
    auto run = [&](const char* scenario, const char* env) {
        SetEnvironmentVariableA("FC4_POPUP_DIALOGS", env);
        DWORD ms = 0;
        const int code = RunChild(dllArg + "--popups-child-" + scenario, 30000, &ms);
        std::printf("popups child %s: exit code %d in %lu ms\n", scenario, code, static_cast<unsigned long>(ms));
        CHECK(code == 0);
        return ReadFile(log);
    };
    const std::string skipped =
        "popup dialogs: SetPopupDialogsEnabled(false) not called at ConnectionsReady: no cISC4App (no framework); "
        "retried at the next ConnectionsReady";
    const std::string disabled =
        "popup dialogs: disabled (cISC4App::SetPopupDialogsEnabled(false); was enabled=1, now=0) at ConnectionsReady";

    // = 0: read at game start; nothing at a city load before the first false; skipped without an application;
    // false at ConnectionsReady, true again at the next city load, false again at its ConnectionsReady.
    std::string text = run("off", nullptr);
    CHECK(text.find("popup dialogs: popup_dialogs=0 (FreeCity4.ini): cISC4App::SetPopupDialogsEnabled(false) at "
                    "each ConnectionsReady 0x6AC284F3, true again at each later city load") != npos);
    CHECK(CountOf(text, skipped) == 1);
    CHECK(CountOf(text, disabled) == 2);
    CHECK(CountOf(text, "popup dialogs: enabled (cISC4App::SetPopupDialogsEnabled(true); was enabled=0, now=1) at "
                        "city load (PostCityInit), until this load's ConnectionsReady") == 1);
    CHECK(text.find("SetPopupDialogsEnabled(false) not called at city load") == npos);
    CHECK(text.find("config: WARNING") == npos);  // popup_dialogs is a known key
    CHECK(text.find("WARNING: released last reference") == npos && text.find("WARNING: over-release") == npos);

    // = 1, with FC4_POPUP_DIALOGS=0: the ini wins.
    text = run("on", "0");
    CHECK(text.find("popup dialogs: popup_dialogs=1 (FreeCity4.ini): cISC4App::SetPopupDialogsEnabled(true) at "
                    "PostAppInit") != npos);
    CHECK(CountOf(text, "popup dialogs: enabled (cISC4App::SetPopupDialogsEnabled(true); was enabled=0, now=1) at "
                        "city load (PostCityInit)") == 1);
    CHECK(CountOf(text, "popup dialogs: SetPopupDialogsEnabled(true) not called at city load (PostCityInit): no "
                        "cISC4App (no framework); retried at the next city load") == 1);
    CHECK(text.find("at ConnectionsReady") == npos);  // = 1 ignores it

    // The environment alone.
    text = run("env", "0");
    CHECK(text.find("popup dialogs: popup_dialogs=0 (FC4_POPUP_DIALOGS): cISC4App::SetPopupDialogsEnabled(false)") !=
          npos);
    CHECK(CountOf(text, disabled) == 2);

    // Not 0/1: a config warning, and nothing is called.
    text = run("bad", nullptr);
    CHECK(text.find("config: WARNING popup_dialogs='maybe' (FreeCity4.ini) is not 0/1; the game's popup dialogs are "
                    "left alone") != npos);
    CHECK(text.find("SetPopupDialogsEnabled(") == npos);

    // Not set: the game is left alone.
    text = run("unset", nullptr);
    CHECK(text.find("popup dialogs: popup_dialogs not set; the game's popup dialogs are left alone") != npos);
    CHECK(text.find("SetPopupDialogsEnabled(") == npos);

    // A setter that does not take effect: one warning, not retried.
    text = run("stuck", nullptr);
    CHECK(CountOf(text, "popup dialogs: WARNING SetPopupDialogsEnabled(false) did not take effect (was enabled=1, "
                        "now=1) at ConnectionsReady") == 1);
    CHECK(CountOf(text, "SetPopupDialogsEnabled(true)") == 0);
    CHECK(text.find("popup dialogs: disabled") == npos);

    SetEnvironmentVariableA("FC4_POPUP_DIALOGS", nullptr);
    DeleteFileA(ini.c_str());
    if (g_failures == 0) std::printf("plugin popup_dialogs test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}

// ---- protection (protection.md): the replacement and the grid shadow ---------------------------------

constexpr uint32_t kCLSID_FireProtection = 0x2990C05A;
constexpr uint32_t kCLSID_Police = 0x6990C04F;
constexpr uint32_t kIID_cISC4FireProtectionSimulator = 0x88F54C7B;
constexpr uint32_t kIID_cISC4PoliceSimulator = 0x67659895;
constexpr uint32_t kIID_cISC4DispatchSource = 0xC9C00435;
constexpr uint32_t kIID_cIGZMessageTarget2Prot = 0x452294AA;

// An empty caller list for the list getters. An SC4List is its root
// pointer; a local root node needs no allocator (the getters free nodes only
// when there are some, and append nothing here).
struct EmptyOccupantList {
    SC4ListNode<cISC4Occupant*> head{};
    SC4ListNode<cISC4Occupant*>* root = nullptr;
    EmptyOccupantList() {
        head.next = &head;
        head.prev = &head;
        head.value = nullptr;
        root = &head;
    }
    EmptyOccupantList(const EmptyOccupantList&) = delete;
    EmptyOccupantList& operator=(const EmptyOccupantList&) = delete;
    SC4List<cISC4Occupant*>& Get() { return *reinterpret_cast<SC4List<cISC4Occupant*>*>(&root); }
    bool Empty() const { return root == &head && head.next == &head && head.prev == &head; }
};

// What both classes share (§1, §15.4, §5): QueryInterface, the dispatch
// source without stations, and the day / month messages before and after
// Init. `primary` holds one reference, kept.
void CheckProtectionShared(cIGZUnknown* primary, uint32_t kind, uint32_t primaryIid, uint32_t clsid) {
    void* p = nullptr;
    CHECK(primary->QueryInterface(primaryIid, &p) && p == primary);
    if (p) primary->Release();
    void* u = nullptr;
    CHECK(primary->QueryInterface(GZIID_cIGZUnknown, &u) && u == primary);  // the primary interface (§1)
    if (u) primary->Release();
    cIGZSerializable* ser = nullptr;
    CHECK(primary->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) && ser != nullptr);
    if (ser) {
        CHECK(ser->GetGZCLSID() == clsid);
        ser->Release();
    }
    void* untouched = reinterpret_cast<void*>(0xDEADBEEF);
    CHECK(!primary->QueryInterface(0xE98B2F57u, &untouched));  // cIGZMessageTarget: not answered
    CHECK(!primary->QueryInterface(kind == 0 ? kIID_cISC4PoliceSimulator : kIID_cISC4FireProtectionSimulator,
                                   &untouched));
    CHECK(untouched == reinterpret_cast<void*>(0xDEADBEEF));

    cFC4ISC4DispatchSource* src = nullptr;
    CHECK(primary->QueryInterface(kIID_cISC4DispatchSource, reinterpret_cast<void**>(&src)) && src != nullptr);
    if (src != nullptr) {
        // §15.4: slot 3 is a constant per class (EAX 0/1).
        CHECK(src->CanHandleDispatchType(kind == 0 ? 1u : 0u) == 1u);
        CHECK(src->CanHandleDispatchType(kind == 0 ? 0u : 1u) == 0u);
        CHECK(src->CanHandleDispatchType(5u) == 0u);
        bool allBusy = true;
        CHECK(!src->CanDispatch(10.0f, 20.0f, &allBusy));  // no units (§8.2)
        CHECK(src->GetMaxPossibleDispatchCount() == 0);
        EmptyOccupantList l;
        CHECK(src->GetStations(l.Get()) == 0u && l.Empty());
        uint32_t flags = 7, base = 7, count = 7;
        CHECK(!src->GetDispatchesAtStation(nullptr, &flags, &base, &count));
        uint32_t id = 5;
        CHECK(src->DispatchFromNearestStation(1.0f, 2.0f, &id) == nullptr && id == 0);  // *id := 0 first
        CHECK(!src->RevokeDispatchFromStation(nullptr, 3));
        uint32_t b = 0, d = 0;
        float c[3] = {1.0f, 1.0f, 1.0f};
        CHECK(src->GetDispatchAutomataProperties(0, &b, c, &d, 0));
        // §8.2: fire 0x4202; police 0x4228 without a disaster in progress (no city here).
        CHECK(b == (kind == 0 ? 0x4202u : 0x4228u));
        CHECK(d == 0x11882u && c[0] == 0.0f && c[1] == 0.0f && c[2] == 0.0f);
        src->Release();
    }

    cIGZMessageTarget2* mt = nullptr;
    CHECK(primary->QueryInterface(kIID_cIGZMessageTarget2Prot, reinterpret_cast<void**>(&mt)) && mt != nullptr);
    if (mt != nullptr) {
        TypeMessage day(kMsgSimNewDay);
        TypeMessage month(kMsgSimNewMonth);
        CHECK(mt->DoMessage(&month));  // before Init: the status line from the first month
        CHECK(mt->DoMessage(&day));
        mt->Release();
    }
}

void CheckFireNeutral(cFC4ISC4FireProtectionSimulator* f) {
    CHECK(f->GetFireProtectionGrid() == nullptr);
    CHECK(f->GetDepartmentBudget() == nullptr);
    CHECK(!f->IsFireStation(nullptr));
    int32_t cost = 7;
    SC4Percentage pct{0.5f};
    CHECK(!f->GetStationFunding(nullptr, &cost, &pct));
    CHECK(cost == 7 && pct.percentage == 0.5f);  // untouched (§6.6)
    CHECK(!f->SetStationFunding(nullptr, pct));
    SC4Percentage r{9.0f};
    CHECK(f->GetStationEfficiency(&r, nullptr) == &r);  // the hidden result pointer in EAX (§15)
    CHECK(!f->IsOnStrike());
    r.percentage = 9.0f;
    CHECK(f->GetChanceOfStrike(&r) == &r && r.percentage == 0.0f);  // 0 with no stations (§6.4)
    CHECK(f->GetStrikeStartDate() == 0);
    CHECK(f->GetStrikeLength() == 0xFFFFFFFFu);  // -1 without a date (§15.1 slot 16)
    EmptyOccupantList l;
    CHECK(f->GetStrikingStations(l.Get()) == 0 && l.Empty());
    CHECK(!f->IsStationOnStrike(nullptr));
    CHECK(f->GetAverageCoverage() == 0.0f);
    const SC4Percentage* pc = f->GetPercentCoverage();
    CHECK(pc != nullptr && pc->percentage == 0.0f);
    CHECK(f->GetPercentCoverage() == pc);  // a stable member
    float cover = 5.0f, station = 5.0f;
    CHECK(!f->GetOccupantCoverage(nullptr, pct, cover, station));
    CHECK(cover == 0.0f && station == 0.0f);  // fire zeroes both outputs first (§7.3)
    CHECK(f->GetFireStations(l.Get()) == 0 && l.Empty());
    CHECK(f->GetFireStationCount() == 0);
    CHECK(f->GetStationDispatchRadius(nullptr) == 0.0f);
    CHECK(f->GetTotalDispatchCount() == 0);
}

void CheckPoliceNeutral(cFC4ISC4PoliceSimulator* p) {
    CHECK(p->GetPolicePowerGrid() == nullptr);
    CHECK(p->GetDepartmentBudget() == nullptr);
    CHECK(p->GetJailDepartmentBudget() == nullptr);
    CHECK(!p->IsPoliceStation(nullptr));
    int32_t cost = 7;
    SC4Percentage pct{0.5f};
    CHECK(!p->GetStationFunding(nullptr, &cost, &pct));
    CHECK(cost == 7 && pct.percentage == 0.5f);
    CHECK(!p->SetStationFunding(nullptr, pct));
    SC4Percentage r{9.0f};
    CHECK(p->GetStationEfficiency(&r, nullptr) == &r);
    CHECK(!p->IsOnStrike());
    r.percentage = 9.0f;
    CHECK(p->GetChanceOfStrike(&r) == &r && r.percentage == 0.0f);
    CHECK(p->GetStrikeStartDate() == 0);
    CHECK(p->GetStrikeLength() == 0xFFFFFFFFu);
    EmptyOccupantList l;
    CHECK(p->GetStrikingStations(l.Get()) == 0 && l.Empty());
    CHECK(!p->IsStationOnStrike(nullptr));
    CHECK(p->GetAverageCoverage() == 0.0f);
    const SC4Percentage* pc = p->GetPercentCoverage();
    CHECK(pc != nullptr && pc->percentage == 0.0f);
    float cover = 5.0f, station = 5.0f;
    CHECK(!p->GetOccupantCoverage(nullptr, pct, cover, station));
    CHECK(cover == 5.0f && station == 5.0f);  // police leaves them when it fails (§7.3)
    CHECK(p->GetPoliceStations(l.Get()) == 0 && l.Empty());
    CHECK(p->GetPoliceStationCount() == 0);
    CHECK(p->GetStationDispatchRadius(nullptr) == 0.0f);
    CHECK(p->GetTotalDispatchCount() == 0);
    // Jails (§9).
    CHECK(p->GetJailCapacityOf(nullptr) == 0);
    CHECK(p->GetJailCapacity() == 0);
    CHECK(p->GetJailPopulation() == 0);
    CHECK(p->GetCriminalCount() == 0);
    CHECK(p->GetJails(l.Get()) == 0 && l.Empty());
    CHECK(!p->IsJail(nullptr));
    CHECK(p->GetArrestCount() == 0);
    CHECK(p->GetJailCount() == 0);
    r.percentage = 9.0f;
    CHECK(p->GetJailedPercentage(&r) == &r);
    CHECK(p->GetJailCurrentUpkeepCost(nullptr) == 0);
    CHECK(!p->IsInJailbreak(nullptr));
    CHECK(!p->EndJailbreak(nullptr));  // AL = 0 always (§15.2 slot 41)
    CHECK(p->GetJailbreakBuildings(l.Get()) == 0 && l.Empty());  // 0, list untouched (slot 42)
    CHECK(p->GetPoliceStationArrestCount(nullptr) == 0);
    CHECK(p->GetPoliceStationCrimeCount(nullptr) == 0);
    CHECK(p->ProcessMonthlyCrimeStats());  // [choice] true (§15.2 slot 38)
}

// Threads (cFC4Protection.h): the game calls getters from a second thread
// while the main thread is inside a call into the simulator. This message's
// QueryInterface runs inside DoMessage, with the object's lock held by the
// main thread: getters from another thread must not wait for it (20 ms, then
// the month-end snapshot), and are counted as fallbacks.
class LockHeldMessage final : public cIGZMessage2 {
public:
    explicit LockHeldMessage(cFC4ISC4FireProtectionSimulator* f) : fire(f) {}
    cFC4ISC4FireProtectionSimulator* fire;
    bool ran = false;
    uint32_t count = 99;
    bool strike = true;
    const SC4Percentage* pct = nullptr;
    DWORD ms = 0;
    bool QueryInterface(uint32_t, void**) override {
        ran = true;
        const DWORD start = GetTickCount();
        std::thread t([this] {
            count = fire->GetFireStationCount();
            strike = fire->IsOnStrike();
            pct = fire->GetPercentCoverage();
        });
        t.join();
        ms = GetTickCount() - start;
        return false;
    }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    bool Create(uint32_t, void**) const override { return false; }
    uint32_t GetType() const override { return kMsgSimNewDay; }
    void SetType(uint32_t) override {}
    bool operator==(cIGZMessage2 const&) const override { return false; }
    bool operator<(cIGZMessage2 const&) const override { return false; }
};

void CheckFireThreads(cFC4ISC4FireProtectionSimulator* fire) {
    cIGZMessageTarget2* mt = nullptr;
    CHECK(fire->QueryInterface(kIID_cIGZMessageTarget2Prot, reinterpret_cast<void**>(&mt)) && mt != nullptr);
    if (mt == nullptr) return;
    LockHeldMessage msg(fire);
    CHECK(mt->DoMessage(&msg));
    CHECK(msg.ran);
    CHECK(msg.count == 0 && !msg.strike && msg.pct == fire->GetPercentCoverage());  // the snapshot's values
    CHECK(msg.ms >= 15 && msg.ms < 5000);  // the getters waited (about 20 ms each), none blocked
    // Without contention another thread gets the lock at once (no fallback).
    uint32_t n = 99;
    std::thread t([&] { n = fire->GetFireStationCount(); });
    t.join();
    CHECK(n == 0);
    mt->Release();
}

int ProtectionEnableTest(const std::string& dll, const std::string& ini, const std::string& log) {
    // Off by default; `enable = protection` registers both simulators
    // (§13.2: replaced together). protection_shadow is then inactive.
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    SetEnvironmentVariableA("FC4_PROTECTION_SHADOW", nullptr);
    WriteTextFile(ini, "[subsystems]\nenable = protection\n[tools]\nprotection_shadow = 1\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr);
    if (director != nullptr) {
        std::vector<uint32_t> classes;
        director->EnumClassObjects(&CollectClass, &classes);
        // simulator (1) + budget (3) + protection (2); never the game's grid
        // classes (simgrid.md §6.3) nor the air dispatch sources (§8.4).
        CHECK(classes.size() == 6);
        for (uint32_t c : classes) CHECK(c < 0x49B9E600 || c > 0x49B9E60B);
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_FireProtection) == 1);
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_Police) == 1);
        CHECK(std::count(classes.begin(), classes.end(), 0x4C0AE114u) == 0);
        CHECK(std::count(classes.begin(), classes.end(), 0x0C0B06E8u) == 0);

        void* obj = nullptr;
        CHECK(director->GetClassObject(kCLSID_FireProtection, kIID_cISC4FireProtectionSimulator, &obj));
        auto* fire = static_cast<cFC4ISC4FireProtectionSimulator*>(obj);
        if (fire != nullptr) {
            CheckFireNeutral(fire);
            CheckProtectionShared(fire, 0, kIID_cISC4FireProtectionSimulator, kCLSID_FireProtection);
            // Init without a city or GZCOM: no grids, no department, no air
            // source; a clean lifecycle (§4), and still neutral.
            CHECK(fire->Init());
            CHECK(fire->Init());
            CheckFireNeutral(fire);
            CheckFireThreads(fire);
            CHECK(fire->Shutdown());
            CHECK(fire->Release() == 0);
        }
        obj = nullptr;
        CHECK(director->GetClassObject(kCLSID_Police, kIID_cISC4PoliceSimulator, &obj));
        auto* police = static_cast<cFC4ISC4PoliceSimulator*>(obj);
        if (police != nullptr) {
            CheckPoliceNeutral(police);
            CheckProtectionShared(police, 1, kIID_cISC4PoliceSimulator, kCLSID_Police);
            CHECK(police->Init());
            CheckPoliceNeutral(police);
            CHECK(police->Shutdown());
            CHECK(police->Shutdown());
            CHECK(police->Release() == 0);
        }
    }
    FreeLibrary(hd);
    DeleteFileA(ini.c_str());
    const std::string text = ReadFile(log);
    CHECK(text.find("subsystem 'protection': enabled (2 classes replaced) by FreeCity4.ini (enable=)") !=
          std::string::npos);
    CHECK(text.find("subsystem 'flammability': off by default") != std::string::npos);
    CHECK(text.find("cFC4FireProtectionSimulator(") != std::string::npos);
    CHECK(text.find("cFC4PoliceSimulator(") != std::string::npos);
    // The month status line (the first month, before Init) and Init's.
    CHECK(text.find("protection month (fire): init=0") != std::string::npos);
    CHECK(text.find("protection month (police): init=0") != std::string::npos);
    CHECK(text.find("protection month (police jails): jails=0") != std::string::npos);
    // Without a grid there is no 0 x 0 grid to warn about (the WARNING needs
    // a non-null coverage grid with no tracts).
    CHECK(text.find("WARNING the coverage grid") == std::string::npos);
    CHECK(text.find("protection Init (fire): init=1") != std::string::npos);
    CHECK(text.find("protection Init (police): init=1") != std::string::npos);
    CHECK(text.find("fallbacks=0") != std::string::npos);
    CHECK(text.find("protection Shutdown (fire)") != std::string::npos);
    CHECK(text.find("protection Shutdown (police)") != std::string::npos);
    CHECK(text.find("cFC4FireProtectionSimulator destroyed (3 fallbacks)") != std::string::npos);
    CHECK(text.find("cFC4PoliceSimulator destroyed (0 fallbacks)") != std::string::npos);
    CHECK(text.find("protection shadow: requested (protection_shadow from FreeCity4.ini) but INACTIVE") !=
          std::string::npos);
    CHECK(text.find("protection shadow: ON") == std::string::npos);
    CHECK(text.find("config: WARNING unknown key") == std::string::npos);
    CHECK(text.find("WARNING: released last reference") == std::string::npos);
    CHECK(text.find("WARNING: over-release") == std::string::npos);
    if (g_failures == 0) std::printf("plugin protection enable test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}

// The grid shadow (FC4ProtectionShadow.h) against fake ORIGINAL simulators:
// fire with a 4 x 3 tract grid and no stations, police without a grid. Every
// slot the shadow must not call traps. Both simulators keep an unadjusted
// int16 grid pointer at primary +0x84 (protection.md §2, PR5): fire's is a
// 4 x 3 grid, police's null.
struct FakeProtSim {
    void** vtbl;
    uint8_t members[0x80];
    void* unadjusted;  // +0x84
};
static_assert(offsetof(FakeProtSim, unadjusted) == 0x84, "the unadjusted grid pointer is at primary +0x84");

struct FakeProtCity {
    FakeVtable cityVt, fireVt, policeVt, gridVt, grid16Vt;
    FakeObject city{}, grid{}, grid16{};
    FakeProtSim fire{}, police{};
    uint8_t values[4][3] = {};    // [x][z]
    int16_t values16[4][3] = {};  // [x][z], the unadjusted grid
    SC4Percentage pct{0.25f};
    int stationCalls = 0;
    int tractReads = 0;
    int tractReads16 = 0;
};

FakeProtCity* g_prot = nullptr;

void* __fastcall ProtCityFire(void*, void*) { return &g_prot->fire; }        // cISC4City 57
void* __fastcall ProtCityPolice(void*, void*) { return &g_prot->police; }    // cISC4City 65
void* __fastcall ProtNull(void*, void*) { return nullptr; }                  // city 39; fire/police 6; police 5
void* __fastcall ProtGrid(void*, void*) { return &g_prot->grid; }            // fire 5
bool __fastcall ProtFalse(void*, void*) { return false; }                    // fire 11 / police 10
SC4Percentage* __fastcall ProtChance(void*, void*, SC4Percentage* r) {     // fire 12 / police 11 (hidden ptr)
    r->percentage = 0.0f;
    return r;
}
float __fastcall ProtAverage(void*, void*) { return 12.5f; }                // fire 19 / police 18
SC4Percentage const* __fastcall ProtPercent(void*, void*) { return &g_prot->pct; }  // fire 20 / police 19
int32_t __fastcall ProtZero(void*, void*) { return 0; }                     // fire 25 / police 26
int32_t __fastcall ProtStations(void*, void*, void*) {                      // fire 22 / police 21
    ++g_prot->stationCalls;
    return 0;  // no stations: the caller's list is left as it is
}
int32_t __fastcall GridTractSize(void*, void*) { return 2; }      // grid 10
int32_t __fastcall GridShift(void*, void*) { return 1; }          // 11
int32_t __fastcall GridCountX(void*, void*) { return 4; }         // 12
int32_t __fastcall GridCountZ(void*, void*) { return 3; }         // 13
float __fastcall GridWidth(void*, void*) { return 32.0f; }        // 14, 15
float __fastcall GridInvWidth(void*, void*) { return 0.03125f; }  // 16, 17
uint8_t __fastcall GridTractValue(void*, void*, int32_t x, int32_t z) {  // 22
    ++g_prot->tractReads;
    return (x >= 0 && x < 4 && z >= 0 && z < 3) ? g_prot->values[x][z] : 0;
}
int16_t __fastcall Grid16TractValue(void*, void*, int32_t x, int32_t z) {  // int16 grid 22
    ++g_prot->tractReads16;
    return (x >= 0 && x < 4 && z >= 0 && z < 3) ? g_prot->values16[x][z] : 0;
}

void InitFakeProtCity(FakeProtCity& c) {
    g_prot = &c;
    c.cityVt.slots[39] = reinterpret_cast<void*>(&ProtNull);
    c.cityVt.slots[57] = reinterpret_cast<void*>(&ProtCityFire);
    c.cityVt.slots[65] = reinterpret_cast<void*>(&ProtCityPolice);
    c.fireVt.slots[5] = reinterpret_cast<void*>(&ProtGrid);
    c.fireVt.slots[6] = reinterpret_cast<void*>(&ProtNull);
    c.fireVt.slots[11] = reinterpret_cast<void*>(&ProtFalse);
    c.fireVt.slots[12] = reinterpret_cast<void*>(&ProtChance);
    c.fireVt.slots[19] = reinterpret_cast<void*>(&ProtAverage);
    c.fireVt.slots[20] = reinterpret_cast<void*>(&ProtPercent);
    c.fireVt.slots[22] = reinterpret_cast<void*>(&ProtStations);
    c.fireVt.slots[25] = reinterpret_cast<void*>(&ProtZero);
    c.policeVt.slots[5] = reinterpret_cast<void*>(&ProtNull);
    c.policeVt.slots[6] = reinterpret_cast<void*>(&ProtNull);
    c.policeVt.slots[10] = reinterpret_cast<void*>(&ProtFalse);
    c.policeVt.slots[11] = reinterpret_cast<void*>(&ProtChance);
    c.policeVt.slots[18] = reinterpret_cast<void*>(&ProtAverage);
    c.policeVt.slots[19] = reinterpret_cast<void*>(&ProtPercent);
    c.policeVt.slots[21] = reinterpret_cast<void*>(&ProtStations);
    c.policeVt.slots[26] = reinterpret_cast<void*>(&ProtZero);
    c.gridVt.slots[10] = reinterpret_cast<void*>(&GridTractSize);
    c.gridVt.slots[11] = reinterpret_cast<void*>(&GridShift);
    c.gridVt.slots[12] = reinterpret_cast<void*>(&GridCountX);
    c.gridVt.slots[13] = reinterpret_cast<void*>(&GridCountZ);
    c.gridVt.slots[14] = reinterpret_cast<void*>(&GridWidth);
    c.gridVt.slots[15] = reinterpret_cast<void*>(&GridWidth);
    c.gridVt.slots[16] = reinterpret_cast<void*>(&GridInvWidth);
    c.gridVt.slots[17] = reinterpret_cast<void*>(&GridInvWidth);
    c.gridVt.slots[22] = reinterpret_cast<void*>(&GridTractValue);
    for (int s = 10; s <= 17; ++s) c.grid16Vt.slots[s] = c.gridVt.slots[s];  // the same 4 x 3 geometry
    c.grid16Vt.slots[22] = reinterpret_cast<void*>(&Grid16TractValue);
    c.city.vtbl = c.cityVt.slots.data();
    c.fire.vtbl = c.fireVt.slots.data();
    c.police.vtbl = c.policeVt.slots.data();
    c.grid.vtbl = c.gridVt.slots.data();
    c.grid16.vtbl = c.grid16Vt.slots.data();
    c.fire.unadjusted = &c.grid16;
    c.values[1][2] = 5;     // the original's grid has one covered tract; ours (no stations) has none
    c.values16[2][1] = -7;  // likewise its unadjusted grid (a negative strike / dispatch sum)
}

int ProtectionShadowTest(const std::string& dll, const std::string& ini, const std::string& log) {
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    SetEnvironmentVariableA("FC4_PROTECTION_SHADOW", nullptr);
    // The observer's own month lines read other city slots: off here.
    WriteTextFile(ini, "[tools]\nprotection_shadow = 1\ndigest = 0\nobserve = 0\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr && seam != nullptr);
    static FakeProtCity city;
    InitFakeProtCity(city);
    if (director != nullptr && seam != nullptr) {
        std::vector<uint32_t> classes;
        director->EnumClassObjects(&CollectClass, &classes);  // game start: the shadow's setting
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_FireProtection) == 0);  // the originals stay
        seam(kFC4SeamObserverCity, &city.city, nullptr);
        // Three month ends (2000-02-01, 03-01, 04-01): our grid is the same
        // each time, so the third one expects an exact match.
        // Not the game's executable: the original classes cannot be
        // recognised, so the unadjusted grid at +0x84 is never read.
        for (intptr_t julian : {2451576, 2451605, 2451636}) {
            seam(kFC4SeamObserverMessage, reinterpret_cast<void*>(static_cast<uintptr_t>(kMsgSimNewMonth)),
                 reinterpret_cast<void*>(julian));
        }
        CHECK(city.tractReads16 == 0);
        // PR5: with the fakes' vtables as the original classes' (seam op 32),
        // a fourth month end (2000-05-01) compares the unadjusted grids too.
        CHECK(seam(kFC4SeamProtectionShadowVtables, city.fire.vtbl, city.police.vtbl) == 1);
        seam(kFC4SeamObserverMessage, reinterpret_cast<void*>(static_cast<uintptr_t>(kMsgSimNewMonth)),
             reinterpret_cast<void*>(static_cast<intptr_t>(2451666)));
        CHECK(seam(kFC4SeamProtectionShadowVtables, nullptr, nullptr) == 1);
        seam(kFC4SeamObserverCity, nullptr, nullptr);
    }
    FreeLibrary(hd);
    DeleteFileA(ini.c_str());
    CHECK(city.stationCalls == 8);        // both simulators, four months
    CHECK(city.tractReads == 4 * 12);     // fire only: 4 x 3 tracts a month
    CHECK(city.tractReads16 == 12);       // fire only, the fourth month only
    const std::string text = ReadFile(log);
    CHECK(text.find("protection shadow: ON (protection_shadow from FreeCity4.ini)") != std::string::npos);
    CHECK(text.find("and the unadjusted int16 grid with the original's (primary +0x84") != std::string::npos);
    CHECK(text.find("protection shadow: NOT readable read-only (so not compared): the original's cycle "
                    "accumulators") != std::string::npos);
    CHECK(text.find("protection shadow (fire) 2000-02-01: unadjusted grid not compared (the original class cannot "
                    "be recognised: executable not identified)") != std::string::npos);
    CHECK(CountOf(text, "unadjusted grid not compared") == 1);  // once per simulator (police: no grid at all)
    CHECK(CountOf(text, "unadjusted cells=") == 1);
    CHECK(text.find("protection shadow (fire) 2000-05-01: unadjusted cells=12 mismatches=1 max|diff|=7 "
                    "first=(2,1):-7/0") != std::string::npos);
    CHECK(text.find("protection shadow (fire) 2000-02-01: cells=12 mismatches=1 max|diff|=5 stations=0 dispatches=0 "
                    "stable=0 expectExact=0 first=(1,2):5/0; efficiency mismatches=0/0; radius mismatches=0/0; "
                    "strike=0; chance orig/ours=0/0; totalDispatch orig/ours=0/0; avg=12.5 pct=0.25 (original)") !=
          std::string::npos);
    CHECK(text.find("protection shadow (fire) 2000-03-01: cells=12 mismatches=1 max|diff|=5 stations=0 dispatches=0 "
                    "stable=1 expectExact=0") != std::string::npos);
    CHECK(text.find("protection shadow (fire) 2000-04-01: cells=12 mismatches=1 max|diff|=5 stations=0 dispatches=0 "
                    "stable=2 expectExact=1") != std::string::npos);
    CHECK(text.find("protection shadow (fire) 2000-04-01: WARNING 1 cells differ although the grid should be exact") !=
          std::string::npos);
    CHECK(CountOf(text, "protection shadow (police) ") == 4);
    CHECK(text.find("protection shadow (police) 2000-02-01: skipped (the original has no coverage grid yet)") !=
          std::string::npos);
    CHECK(text.find("structured exception") == std::string::npos);
    CHECK(text.find("config: WARNING unknown key") == std::string::npos);
    CHECK(text.find("WARNING: released last reference") == std::string::npos);
    CHECK(text.find("WARNING: over-release") == std::string::npos);
    if (g_failures == 0) std::printf("plugin protection shadow test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}

// ---- aura (aura.md): the replacement without a city, and the shadow (test seam ops 14 and 35) --------
constexpr uint32_t kCLSID_Aura = 0x0990BDD8;
constexpr uint32_t kIID_cISC4AuraSimulator = 0xC773E880;
constexpr uint32_t kIID_cIGZMessageTarget2Aura = 0x452294AA;
constexpr uint32_t kIID_cISC4SimGridAura = 0xC9BD939A;
constexpr uint32_t kIID_cISC4OccupantAura = 0x26BB767E;
constexpr uint32_t kCLSID_SimGridSint8 = 0x49B9E603;
constexpr uint32_t kMsgOccupantInsertedAura = 0x99EF1142;

// A game int8 grid (cISC4SimGrid<int8_t> slots 0-2, 10-17 and 22) and its
// cIGZSerializable face for the record (slots 0-2 and 5).
struct OaGrid {
    void** vtbl;
    int32_t tractSize, count;
    int8_t v[64 * 64];
};
struct OaGridSer {
    void** vtbl;
    OaGrid* grid;
};

// An "original" aura simulator laid out as aura.md §14.3 gives it: the
// cISC4AuraSimulator vtable at +0, its cIGZMessageTarget2 at +4 and
// cIGZSerializable at +8; grids at +0x40 (aura), +0x44 (work), +0x48 (base)
// and +0x4C (transient); the pipeline state at +0x70.
struct OaObject {
    void** vtbl;
    void** mtVtbl;
    void** serVtbl;
    uint8_t pad0[0x40 - 0x0C];
    OaGrid* aura;
    OaGrid* work;
    OaGrid* base;
    OaGrid* transient;
    uint8_t pad1[0x70 - 0x50];
    int32_t state;
    uint8_t pad2[0x98 - 0x74];
};
static_assert(offsetof(OaObject, mtVtbl) == 0x04 && offsetof(OaObject, serVtbl) == 0x08 &&
                  offsetof(OaObject, aura) == 0x40 && offsetof(OaObject, work) == 0x44 &&
                  offsetof(OaObject, transient) == 0x4C && offsetof(OaObject, state) == 0x70,
              "the aura.md §14.3 layout");

// The original's agent runs an empty city's cycle (§6.4): SimNewMonth 0 -> 1,
// each SimIdle / SimNewDay 1 -> 2 -> 3 -> 4 -> 0. Its record (Write) is
// version 9 with its aura, base and transient grids, the other references
// null and everything else 0. The city answers GetAuraSimulator (compiled
// slot 51), CellWidthX / Z (91, 92), CellCountX / Z (93, 94, 64 cells) and
// PositionToCell (95, cell (0, 0)); GetLotManager (33), GetSimulator (50) and
// GetBudgetSimulator (52) are null. `noAura` returns null at 51, `ours` our
// own simulator. `occ` is an occupant without a property holder; `notOcc`
// answers no interface. Any other call aborts.
struct FakeOrigAura {
    FakeVtable simVt, mtVt, serVt, cityVt, noAuraVt, oursVt, gridVt, gridSerVt, occVt, notOccVt;
    OaObject sim{};
    FakeObject city{}, noAura{}, ours{}, occ{}, notOcc{};
    OaGrid aura{}, work{}, base{}, transient{};
    OaGridSer auraSer{}, baseSer{}, transientSer{};
    void* ourSim = nullptr;
    int refs = 1, gridRefs = 1, occRefs = 1;
    int writes = 0, deliveries = 0;
};

FakeOrigAura* g_oa = nullptr;

bool __fastcall OaQi(void*, void*, uint32_t iid, void** out) {  // 0 (simulator, agent, serializable)
    if (out == nullptr) return false;
    if (iid == GZIID_cIGZSerializable) {
        *out = &g_oa->sim.serVtbl;
    } else if (iid == kIID_cIGZMessageTarget2Aura) {
        *out = &g_oa->sim.mtVtbl;
    } else {
        return false;  // (the reference audit asks other IIDs)
    }
    ++g_oa->refs;
    return true;
}
uint32_t __fastcall OaAddRef(void*, void*) { return static_cast<uint32_t>(++g_oa->refs); }   // 1
uint32_t __fastcall OaRelease(void*, void*) { return static_cast<uint32_t>(--g_oa->refs); }  // 2
void* __fastcall OaAuraGrid(void*, void*) { return g_oa->sim.aura; }                         // simulator 12
void* __fastcall OaTransientGrid(void*, void*) { return g_oa->sim.transient; }               // simulator 13
bool __fastcall OaDoMessage(void*, void*, cIGZMessage2* m) {                                 // agent 3
    ++g_oa->deliveries;
    int32_t& s = g_oa->sim.state;
    if (m->GetType() == kMsgSimNewMonth) {
        if (s == 0) s = 1;
    } else if (s >= 1 && s <= 3) {
        ++s;
    } else if (s == 4) {
        s = 0;  // no lots: the cycle ends (§6.4)
    }
    return true;
}
bool __fastcall OaWrite(void*, void*, cIGZOStream* s) {  // serializable 3 (§11.2)
    ++g_oa->writes;
    s->SetUint16(9);
    void* p = nullptr;
    if (s->QueryInterface(kFC4_IID_cISC4DBSegmentOStream, &p) && p != nullptr) {
        auto* db = static_cast<cFC4ISC4DBSegmentOStream*>(p);
        // aura, park map, raw park, base, transient, building effect, landmark map
        db->WriteGZSerializable(reinterpret_cast<cIGZSerializable*>(&g_oa->auraSer));
        db->WriteGZSerializable(nullptr);
        db->WriteGZSerializable(nullptr);
        db->WriteGZSerializable(reinterpret_cast<cIGZSerializable*>(&g_oa->baseSer));
        db->WriteGZSerializable(reinterpret_cast<cIGZSerializable*>(&g_oa->transientSer));
        db->WriteGZSerializable(nullptr);
        db->WriteGZSerializable(nullptr);
        db->Release();
    }
    s->SetUint32(0);  // departments
    s->SetSint32(0);  // unused rating
    s->SetSint32(0);  // mayor rating
    for (int i = 0; i < 2; ++i) {  // max, min: x, z, rating
        s->SetSint32(0);
        s->SetSint32(0);
        s->SetSint16(0);
    }
    return s->SetUint32(0);  // hotspots
}
uint32_t __fastcall OaClsid(void*, void*) { return kCLSID_Aura; }  // serializable 5
void* __fastcall OaCityAura(void*, void*) { return &g_oa->sim; }   // city 51
void* __fastcall OaCityNull(void*, void*) { return nullptr; }      // city 33, 50, 52; noAura 51
void* __fastcall OaCityOurs(void*, void*) { return g_oa->ourSim; }  // city 51
float __fastcall OaCellWidth(void*, void*) { return 16.0f; }       // city 91, 92
uint32_t __fastcall OaCellCount(void*, void*) { return 64; }       // city 93, 94
bool __fastcall OaPositionToCell(void*, void*, float, float, int* x, int* z) {  // city 95
    *x = 0;
    *z = 0;
    return true;
}
bool __fastcall OgQi(void*, void*, uint32_t, void**) { return false; }                     // grid 0
uint32_t __fastcall OgAddRef(void*, void*) { return static_cast<uint32_t>(++g_oa->gridRefs); }   // 1
uint32_t __fastcall OgRelease(void*, void*) { return static_cast<uint32_t>(--g_oa->gridRefs); }  // 2
int32_t __fastcall OgTractSize(OaGrid* g, void*) { return g->tractSize; }                      // 10
int32_t __fastcall OgShift(OaGrid* g, void*) { return g->tractSize == 4 ? 2 : 0; }             // 11
int32_t __fastcall OgCount(OaGrid* g, void*) { return g->count; }                              // 12, 13
float __fastcall OgWidth(OaGrid* g, void*) { return 16.0f * static_cast<float>(g->tractSize); }  // 14, 15
float __fastcall OgInvWidth(OaGrid* g, void*) { return 1.0f / (16.0f * static_cast<float>(g->tractSize)); }  // 16, 17
int8_t __fastcall OgGet(OaGrid* g, void*, int32_t x, int32_t z) {  // 22
    return (x >= 0 && x < g->count && z >= 0 && z < g->count) ? g->v[x * g->count + z] : 0;
}
bool __fastcall OgsQi(OaGridSer* self, void*, uint32_t iid, void** out) {  // grid serializable 0
    if (out == nullptr || iid != kIID_cISC4SimGridAura) return false;
    *out = self->grid;
    ++g_oa->gridRefs;
    return true;
}
uint32_t __fastcall OgsClsid(void*, void*) { return kCLSID_SimGridSint8; }  // grid serializable 5
bool __fastcall OccQi(void* self, void*, uint32_t iid, void** out) {     // occupant 0
    if (out == nullptr || iid != kIID_cISC4OccupantAura) return false;
    *out = self;
    ++g_oa->occRefs;
    return true;
}
uint32_t __fastcall OccAddRef(void*, void*) { return static_cast<uint32_t>(++g_oa->occRefs); }   // 1
uint32_t __fastcall OccRelease(void*, void*) { return static_cast<uint32_t>(--g_oa->occRefs); }  // 2
void* __fastcall OccHolder(void*, void*) { return nullptr; }                                     // 6
int32_t __fastcall OccType(void*, void*) { return 0; }                                           // 7
bool __fastcall OccPosition(void*, void*, float* v) {                                            // 8
    v[0] = v[1] = v[2] = 0.0f;
    return true;
}
bool __fastcall NoQi(void*, void*, uint32_t, void**) { return false; }  // notOcc 0

void InitOaGrid(OaGrid& g, FakeVtable& vt, int32_t tractSize, int8_t fill) {
    g.vtbl = vt.slots.data();
    g.tractSize = tractSize;
    g.count = 64 / tractSize;
    std::fill(std::begin(g.v), std::end(g.v), fill);
}

void InitFakeOrigAura(FakeOrigAura& o) {
    g_oa = &o;
    for (FakeVtable* vt : {&o.simVt, &o.mtVt, &o.serVt}) {
        vt->slots[0] = reinterpret_cast<void*>(&OaQi);
        vt->slots[1] = reinterpret_cast<void*>(&OaAddRef);
        vt->slots[2] = reinterpret_cast<void*>(&OaRelease);
    }
    o.simVt.slots[12] = reinterpret_cast<void*>(&OaAuraGrid);
    o.simVt.slots[13] = reinterpret_cast<void*>(&OaTransientGrid);
    o.mtVt.slots[3] = reinterpret_cast<void*>(&OaDoMessage);
    o.serVt.slots[3] = reinterpret_cast<void*>(&OaWrite);
    o.serVt.slots[5] = reinterpret_cast<void*>(&OaClsid);
    o.cityVt.slots[51] = reinterpret_cast<void*>(&OaCityAura);
    for (int s : {33, 50, 52}) o.cityVt.slots[s] = reinterpret_cast<void*>(&OaCityNull);
    o.cityVt.slots[91] = reinterpret_cast<void*>(&OaCellWidth);
    o.cityVt.slots[92] = reinterpret_cast<void*>(&OaCellWidth);
    o.cityVt.slots[93] = reinterpret_cast<void*>(&OaCellCount);
    o.cityVt.slots[94] = reinterpret_cast<void*>(&OaCellCount);
    o.cityVt.slots[95] = reinterpret_cast<void*>(&OaPositionToCell);
    o.noAuraVt.slots[51] = reinterpret_cast<void*>(&OaCityNull);
    o.oursVt.slots[51] = reinterpret_cast<void*>(&OaCityOurs);
    o.gridVt.slots[0] = reinterpret_cast<void*>(&OgQi);
    o.gridVt.slots[1] = reinterpret_cast<void*>(&OgAddRef);
    o.gridVt.slots[2] = reinterpret_cast<void*>(&OgRelease);
    o.gridVt.slots[10] = reinterpret_cast<void*>(&OgTractSize);
    o.gridVt.slots[11] = reinterpret_cast<void*>(&OgShift);
    o.gridVt.slots[12] = reinterpret_cast<void*>(&OgCount);
    o.gridVt.slots[13] = reinterpret_cast<void*>(&OgCount);
    o.gridVt.slots[14] = reinterpret_cast<void*>(&OgWidth);
    o.gridVt.slots[15] = reinterpret_cast<void*>(&OgWidth);
    o.gridVt.slots[16] = reinterpret_cast<void*>(&OgInvWidth);
    o.gridVt.slots[17] = reinterpret_cast<void*>(&OgInvWidth);
    o.gridVt.slots[22] = reinterpret_cast<void*>(&OgGet);
    o.gridSerVt.slots[0] = reinterpret_cast<void*>(&OgsQi);
    o.gridSerVt.slots[1] = reinterpret_cast<void*>(&OgAddRef);
    o.gridSerVt.slots[2] = reinterpret_cast<void*>(&OgRelease);
    o.gridSerVt.slots[5] = reinterpret_cast<void*>(&OgsClsid);
    o.occVt.slots[0] = reinterpret_cast<void*>(&OccQi);
    o.occVt.slots[1] = reinterpret_cast<void*>(&OccAddRef);
    o.occVt.slots[2] = reinterpret_cast<void*>(&OccRelease);
    o.occVt.slots[6] = reinterpret_cast<void*>(&OccHolder);
    o.occVt.slots[7] = reinterpret_cast<void*>(&OccType);
    o.occVt.slots[8] = reinterpret_cast<void*>(&OccPosition);
    o.notOccVt.slots[0] = reinterpret_cast<void*>(&NoQi);
    InitOaGrid(o.aura, o.gridVt, 4, 0);
    InitOaGrid(o.work, o.gridVt, 1, -128);
    InitOaGrid(o.base, o.gridVt, 4, -128);
    InitOaGrid(o.transient, o.gridVt, 4, 0);
    o.base.v[2 * 16 + 3] = 7;  // one tract with data
    o.aura.v[2 * 16 + 3] = 7;
    o.auraSer = {o.gridSerVt.slots.data(), &o.aura};
    o.baseSer = {o.gridSerVt.slots.data(), &o.base};
    o.transientSer = {o.gridSerVt.slots.data(), &o.transient};
    o.sim.vtbl = o.simVt.slots.data();
    o.sim.mtVtbl = o.mtVt.slots.data();
    o.sim.serVtbl = o.serVt.slots.data();
    o.sim.aura = &o.aura;
    o.sim.work = &o.work;
    o.sim.base = &o.base;
    o.sim.transient = &o.transient;
    o.sim.state = 1;  // after Init (§4.1)
    o.city.vtbl = o.cityVt.slots.data();
    o.noAura.vtbl = o.noAuraVt.slots.data();
    o.ours.vtbl = o.oursVt.slots.data();
    o.occ.vtbl = o.occVt.slots.data();
    o.notOcc.vtbl = o.notOccVt.slots.data();
}

FC4AuraShadowDeliverInput AuraShadowInput(FakeObject* city, const void* vtable, uint32_t type = 0, int32_t julian = 0,
                                          FakeObject* void1 = nullptr) {
    FC4AuraShadowDeliverInput in{};
    in.city = reinterpret_cast<cISC4City*>(city);
    in.target = g_oa != nullptr ? reinterpret_cast<cIGZMessageTarget2*>(&g_oa->sim.mtVtbl) : nullptr;
    in.vtable = vtable;
    in.type = type;
    in.julian = julian;
    in.void1 = reinterpret_cast<cIGZUnknown*>(void1);
    return in;
}

intptr_t AuraShadowLoad(FC4TestSeamFn seam, FakeObject* city, const void* vtable) {
    const FC4AuraShadowDeliverInput in = AuraShadowInput(city, vtable);
    return seam(kFC4SeamAuraShadow, const_cast<FC4AuraShadowDeliverInput*>(&in), nullptr);
}

intptr_t AuraShadowSend(FC4TestSeamFn seam, uint32_t type, int32_t julian, FakeObject* void1 = nullptr) {
    const FC4AuraShadowDeliverInput in = AuraShadowInput(&g_oa->city, g_oa->sim.vtbl, type, julian, void1);
    return seam(kFC4SeamAuraShadowDeliver, const_cast<FC4AuraShadowDeliverInput*>(&in), nullptr);
}

// The neutral answers without a city (§8: no grids, ratings 0).
void CheckAuraNeutral(cISC4AuraSimulator* a) {
    CHECK(a->GetMayorRating() == 0);
    CHECK(a->GetAuraValue(3, 4) == 0);
    SC4Point<long> p{7, 7};
    CHECK(a->GetMaxMayorRating(p) == 0);
    CHECK(a->GetMinMayorRating(p) == 0);
    CHECK(a->GetAuraGrid() == nullptr);
    CHECK(a->GetTransientAuraGrid() == nullptr);
    CHECK(a->GetParkMap() == nullptr);
    CHECK(a->GetLandmarkMap() == nullptr);
}

int AuraEnableTest(const std::string& dll, const std::string& ini, const std::string& log) {
    // Off by default; `enable = aura` registers the aura simulator. The aura
    // shadow (it compares against the original) is then inactive.
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    SetEnvironmentVariableA("FC4_AURA_SHADOW", nullptr);
    WriteTextFile(ini, "[subsystems]\nenable = aura\n[tools]\naura_shadow = 1\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr && seam != nullptr);
    static FakeOrigAura orig;
    InitFakeOrigAura(orig);
    if (director != nullptr && seam != nullptr) {
        std::vector<uint32_t> classes;
        director->EnumClassObjects(&CollectClass, &classes);
        // simulator (1) + budget (3) + aura (1); never the game's grid classes.
        CHECK(classes.size() == 5);
        for (uint32_t c : classes) CHECK(c < 0x49B9E600 || c > 0x49B9E60B);
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_Aura) == 1);

        void* obj = nullptr;
        CHECK(director->GetClassObject(kCLSID_Aura, kIID_cISC4AuraSimulator, &obj));
        auto* aura = static_cast<cISC4AuraSimulator*>(obj);
        if (aura != nullptr) {
            // §1: the primary for its IID and cIGZUnknown; the message target
            // and serializable; nothing else (the out pointer untouched).
            void* p = nullptr;
            CHECK(aura->QueryInterface(GZIID_cIGZUnknown, &p) && p == static_cast<void*>(aura));
            static_cast<cIGZUnknown*>(p)->Release();
            cIGZMessageTarget2* mt = nullptr;
            CHECK(aura->QueryInterface(kIID_cIGZMessageTarget2Aura, reinterpret_cast<void**>(&mt)) && mt != nullptr);
            cIGZSerializable* ser = nullptr;
            CHECK(aura->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) && ser != nullptr);
            if (ser != nullptr) {
                CHECK(ser->GetGZCLSID() == kCLSID_Aura);
                ser->Release();
            }
            p = reinterpret_cast<void*>(0x1234);
            CHECK(!aura->QueryInterface(0x12345678, &p) && p == reinterpret_cast<void*>(0x1234));
            CheckAuraNeutral(aura);
            // Without a city: the month line before Init, a clean lifecycle,
            // and the transient effects and strike busting do nothing harmful.
            if (mt != nullptr) {
                TypeMessage month(kMsgSimNewMonth);
                CHECK(mt->DoMessage(&month));
            }
            CHECK(aura->Init());
            CHECK(aura->Init());
            CheckAuraNeutral(aura);
            aura->AddTransientEffect(10.0f, 20.0f, 30.0f, 5.0f);
            aura->AddTransientEffect(10.0f, 20.0f, 0x12345678u);
            aura->BustStrike(cISC4AuraSimulator::StrikeBusterAll);
            // Five days: states 1 -> 2 -> 3 -> 4, where a city without lots
            // ends the cycle silently (§6.4 [quirk]): no cycle line, no post.
            if (mt != nullptr) {
                for (int i = 0; i < 5; ++i) {
                    TypeMessage day(kMsgSimNewDay);
                    CHECK(mt->DoMessage(&day));
                }
            }
            // Getters from a second thread (they read the snapshot only).
            int8_t rating = 99;
            cISC4SimGrid<int8_t>* grid = reinterpret_cast<cISC4SimGrid<int8_t>*>(0x1);
            std::thread t([&] {
                rating = aura->GetMayorRating();
                grid = aura->GetAuraGrid();
            });
            t.join();
            CHECK(rating == 0 && grid == nullptr);
            CheckAuraNeutral(aura);
            // The shadow (test seam op 14) refuses our own simulator.
            orig.ourSim = aura;
            CHECK(AuraShadowLoad(seam, &orig.ours, orig.sim.vtbl) == -1);
            if (mt != nullptr) mt->Release();
            CHECK(aura->Shutdown());
            CHECK(aura->Release() == 0);
        }
        // The shadow on fake originals: no city, a city without an aura
        // simulator, an original whose vtable is not the expected one (not
        // followed), then one laid out as aura.md §14.3 gives it: ours starts
        // from its record at city load and follows two empty-city cycles
        // (§6.4) step by step, with occupant messages between them.
        CHECK(AuraShadowLoad(seam, nullptr, orig.sim.vtbl) == -1);
        CHECK(AuraShadowLoad(seam, &orig.noAura, orig.sim.vtbl) == -1);
        static void* otherVtable[4] = {};
        CHECK(AuraShadowLoad(seam, &orig.city, otherVtable) == -1);
        CHECK(orig.writes == 0);
        CHECK(AuraShadowLoad(seam, &orig.city, orig.sim.vtbl) == 1);
        for (int32_t s : {2, 3, 4, 0}) CHECK(AuraShadowSend(seam, kMsgSimIdle, 2451576) == s);  // 2000-02-01
        CHECK(AuraShadowSend(seam, kMsgSimIdle, 2451576) == 0);  // idle in state 0: no move
        CHECK(AuraShadowSend(seam, kMsgOccupantInsertedAura, 0, &orig.notOcc) == 0);
        CHECK(AuraShadowSend(seam, kMsgOccupantInsertedAura, 0, &orig.occ) == 1);
        CHECK(AuraShadowSend(seam, kMsgSimNewMonth, 2451605) == 1);  // 2000-03-01
        for (int32_t s : {2, 3, 4, 0}) CHECK(AuraShadowSend(seam, kMsgSimNewDay, 2451605) == s);
        CHECK(orig.deliveries == 10);
        CHECK(orig.writes == 3);  // the city load and the two cycle ends
        CHECK(orig.refs == 1);    // every QueryInterface released
        CHECK(orig.gridRefs == 1);
        CHECK(orig.occRefs == 1);
    }
    FreeLibrary(hd);
    DeleteFileA(ini.c_str());
    const std::string text = ReadFile(log);
    CHECK(text.find("subsystem 'aura': enabled (1 class replaced) by FreeCity4.ini (enable=)") != std::string::npos);
    CHECK(text.find("cFC4AuraSimulator(") != std::string::npos);
    CHECK(text.find("aura month: init=0 state=1") != std::string::npos);
    CHECK(text.find("aura Init: init=1") != std::string::npos);
    CHECK(text.find("lot walks=0 lots=0; inputs none missing so far") != std::string::npos);
    CHECK(text.find("aura cycle ") == std::string::npos);
    CHECK(text.find("aura Shutdown (5 days, 1 months, 0 cycles)") != std::string::npos);
    CHECK(text.find("cFC4AuraSimulator destroyed") != std::string::npos);
    CHECK(text.find("aura shadow: requested (aura_shadow from FreeCity4.ini) but INACTIVE") != std::string::npos);
    CHECK(text.find("aura shadow: ON") == std::string::npos);
    CHECK(text.find("aura shadow: inactive: the city's aura simulator is FreeCity4's own") != std::string::npos);
    CHECK(text.find("aura shadow: the city has no aura simulator; nothing compared") != std::string::npos);
    CHECK(text.find("is NOT followed: its vtable is not the original's (compat table); nothing compared") !=
          std::string::npos);
    CHECK(text.find("aura shadow: following the original aura simulator ") != std::string::npos);
    CHECK(text.find("): state 1 at +0x70; work grid diagnostic on (+0x44)") != std::string::npos);
    CHECK(text.find("aura shadow: comparing with the original aura simulator") != std::string::npos);
    CHECK(text.find("(city ") != std::string::npos && text.find("64 x 64 cells); differing tracts listed per grid: 4") !=
                                                          std::string::npos);
    CHECK(text.find("aura shadow: city load: ours starts from the original's record (Read, then Init: state 1): base "
                    "1 of 256 tracts with data, transient 0 nonzero, 0 departments, 0 hotspots") != std::string::npos);
    const std::string clean =
        "tracts=256; base mismatches=0 max|d|=0; aura mismatches=0 max|d|=0; park map missing; landmark map missing; "
        "transient mismatches=0 max|d|=0 (copied before states 15 and 17: 0 times); work after states 5-13: "
        "-,-,-,-,-,-,-,-,-; mayor rating orig/ours=0/0; max orig/ours=0@(0,0)/0@(0,0); min orig/ours=0@(0,0)/0@(0,0); "
        "hotspots orig/ours=0/0 (same); tax baseline: not taken (no tax-change curve); ended in state 4 (no lots: no "
        "rating, no end message, ";
    CHECK(text.find("aura shadow 2000-02-01 cycle 1 (followed states 1-4 over 0 days: 4 moves, 4 calls, 0 occupant "
                    "messages): " + clean) != std::string::npos);
    CHECK(text.find("aura shadow 2000-03-01 cycle 2 (followed states 1-4 over 0 days: 5 moves, 4 calls, 1 occupant "
                    "messages): " + clean) != std::string::npos);
    CHECK(CountOf(text, "; riot roll not compared (wall-clock RNG); inputs not available (neutral): tuning") == 2);
    CHECK(text.find("structured exception") == std::string::npos);
    CHECK(text.find("config: WARNING unknown key") == std::string::npos);
    CHECK(text.find("WARNING: released last reference") == std::string::npos);
    CHECK(text.find("WARNING: over-release") == std::string::npos);
    if (g_failures == 0) std::printf("plugin aura enable test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}

// ---- flora (flora.md): the replacement without a city, and the census shadow (test seam op 17) -------
constexpr uint32_t kCLSID_Flora = 0xC990C034;
constexpr uint32_t kIID_cISC4FloraSimulatorT = 0x06C66177;
constexpr uint32_t kIID_cIGZMessageTarget2Flora = 0x452294AA;

// An "original" flora simulator whose record (its own Write,
// cIGZSerializable) is the version alone (a uint16 2): the month is not
// compared (INCOMPLETE). Its registry: no types (slot 9), and under key 0
// (slot 7, slot 11) the block `key0` (the test hands it our own key-0 block,
// so the registry check finds no difference). The agent (its
// cIGZMessageTarget2) counts the deliveries. The city answers
// GetFloraSimulator (compiled slot 59) only; `noFlora` returns null there,
// `ours` our own simulator. Any other call aborts.
struct FakeOrigFlora {
    FakeVtable simVt, serVt, agentVt, cityVt, noFloraVt, oursVt;
    FakeObject sim{}, ser{}, agent{}, city{}, noFlora{}, ours{};
    void* ourSim = nullptr;
    void* key0 = nullptr;
    int refs = 1;
    int writes = 0;
    int delivered = 0;
};

FakeOrigFlora* g_ofl = nullptr;

bool __fastcall OflQi(void*, void*, uint32_t iid, void** out) {  // 0 (simulator, serializable, agent)
    if (out == nullptr) return false;
    if (iid == GZIID_cIGZSerializable) {
        *out = &g_ofl->ser;
    } else if (iid == kIID_cIGZMessageTarget2Flora) {
        *out = &g_ofl->agent;
    } else {
        return false;  // (the reference audit asks other IIDs)
    }
    ++g_ofl->refs;
    return true;
}
uint32_t __fastcall OflAddRef(void*, void*) { return static_cast<uint32_t>(++g_ofl->refs); }   // 1
uint32_t __fastcall OflRelease(void*, void*) { return static_cast<uint32_t>(--g_ofl->refs); }  // 2
void* __fastcall OflRecord(void*, void*, uint32_t type) { return type == 0 ? g_ofl->key0 : nullptr; }  // sim 7
uint32_t __fastcall OflTypes(void*, void*, uint32_t*, uint32_t* count) {  // sim 9
    if (count != nullptr) *count = 0;
    return 0;
}
bool __fastcall OflValid(void*, void*, uint32_t type) { return type == 0 && g_ofl->key0 != nullptr; }  // sim 11
bool __fastcall OflWrite(void*, void*, cIGZOStream* s) {  // serializable 3
    ++g_ofl->writes;
    return s->SetUint16(2);
}
uint32_t __fastcall OflClsid(void*, void*) { return kCLSID_Flora; }  // serializable 5
bool __fastcall OflDoMessage(void*, void*, cIGZMessage2*) {        // agent 3
    ++g_ofl->delivered;
    return true;
}
void* __fastcall OflCityFlora(void*, void*) { return &g_ofl->sim; }  // city 59
void* __fastcall OflCityNoFlora(void*, void*) { return nullptr; }   // city 59
void* __fastcall OflCityOurs(void*, void*) { return g_ofl->ourSim; }  // city 59

void InitFakeOrigFlora(FakeOrigFlora& o) {
    g_ofl = &o;
    for (FakeVtable* vt : {&o.simVt, &o.serVt, &o.agentVt}) {
        vt->slots[0] = reinterpret_cast<void*>(&OflQi);
        vt->slots[1] = reinterpret_cast<void*>(&OflAddRef);
        vt->slots[2] = reinterpret_cast<void*>(&OflRelease);
    }
    o.agentVt.slots[0] = o.simVt.slots[0];
    o.simVt.slots[7] = reinterpret_cast<void*>(&OflRecord);
    o.simVt.slots[9] = reinterpret_cast<void*>(&OflTypes);
    o.simVt.slots[11] = reinterpret_cast<void*>(&OflValid);
    o.serVt.slots[3] = reinterpret_cast<void*>(&OflWrite);
    o.serVt.slots[5] = reinterpret_cast<void*>(&OflClsid);
    o.agentVt.slots[3] = reinterpret_cast<void*>(&OflDoMessage);
    o.cityVt.slots[59] = reinterpret_cast<void*>(&OflCityFlora);
    o.noFloraVt.slots[59] = reinterpret_cast<void*>(&OflCityNoFlora);
    o.oursVt.slots[59] = reinterpret_cast<void*>(&OflCityOurs);
    o.sim.vtbl = o.simVt.slots.data();
    o.ser.vtbl = o.serVt.slots.data();
    o.agent.vtbl = o.agentVt.slots.data();
    o.city.vtbl = o.cityVt.slots.data();
    o.noFlora.vtbl = o.noFloraVt.slots.data();
    o.ours.vtbl = o.oursVt.slots.data();
}

intptr_t FloraShadowOnce(FC4TestSeamFn seam, FakeObject* city, FakeObject* target, uint32_t type, int32_t julian,
                         intptr_t* registry) {
    FC4FloraShadowTestInput in{reinterpret_cast<cISC4City*>(city), reinterpret_cast<cIGZMessageTarget2*>(target),
                               type, julian};
    return seam(kFC4SeamFloraShadow, &in, registry);
}

// An obliteration segment (flora §10.3, FQ3): the segment answers
// cISC4DBSegment (0x499899DA) only; that interface opens one write stream
// (slot 8) and closes it (slot 9); the stream keeps what its PutUint16 /
// PutUint32 (slots 7 / 9) get, little-endian, and reports no error (slot 18).
// `refs` starts with the caller's own reference. Any other call aborts.
// `falseOpen`: slot 8 still writes the stream (with its reference) but
// returns false (AQ10).
struct FakeSegment {
    FakeVtable segVt, dbVt, outVt;
    FakeObject seg{}, db{}, out{};
    int refs = 1;
    int opens = 0;
    int closes = 0;
    bool falseOpen = false;
    uint32_t key[3] = {};  // type, group, instance
    std::vector<uint8_t> bytes;
};

FakeSegment* g_fseg = nullptr;

bool __fastcall FsegQi(void* self, void*, uint32_t iid, void** out) {  // 0 (segment)
    if (out == nullptr || self != &g_fseg->seg || iid != 0x499899DAu) return false;
    *out = &g_fseg->db;
    ++g_fseg->refs;
    return true;
}
bool __fastcall FsegNoQi(void*, void*, uint32_t, void**) { return false; }                    // 0 (db, stream)
uint32_t __fastcall FsegAddRef(void*, void*) { return static_cast<uint32_t>(++g_fseg->refs); }   // 1
uint32_t __fastcall FsegRelease(void*, void*) { return static_cast<uint32_t>(--g_fseg->refs); }  // 2
bool __fastcall FsegOpenO(void*, void*, const uint32_t* key, void** out, bool) {                // db 8
    ++g_fseg->opens;
    std::memcpy(g_fseg->key, key, sizeof(g_fseg->key));
    *out = &g_fseg->out;
    ++g_fseg->refs;
    return !g_fseg->falseOpen;
}
void __fastcall FsegCloseO(void*, void*, void*) { ++g_fseg->closes; }  // db 9
bool __fastcall FsegPut16(void*, void*, uint16_t v) {                 // stream 7
    for (int i = 0; i < 2; ++i) g_fseg->bytes.push_back(static_cast<uint8_t>(v >> (8 * i)));
    return true;
}
bool __fastcall FsegPut32(void*, void*, uint32_t v) {  // stream 9
    for (int i = 0; i < 4; ++i) g_fseg->bytes.push_back(static_cast<uint8_t>(v >> (8 * i)));
    return true;
}
int32_t __fastcall FsegError(void*, void*) { return 0; }  // stream 18

void InitFakeSegment(FakeSegment& s) {
    g_fseg = &s;
    for (FakeVtable* vt : {&s.segVt, &s.dbVt, &s.outVt}) {
        vt->slots[0] = reinterpret_cast<void*>(&FsegNoQi);
        vt->slots[1] = reinterpret_cast<void*>(&FsegAddRef);
        vt->slots[2] = reinterpret_cast<void*>(&FsegRelease);
    }
    s.segVt.slots[0] = reinterpret_cast<void*>(&FsegQi);
    s.dbVt.slots[8] = reinterpret_cast<void*>(&FsegOpenO);
    s.dbVt.slots[9] = reinterpret_cast<void*>(&FsegCloseO);
    s.outVt.slots[7] = reinterpret_cast<void*>(&FsegPut16);
    s.outVt.slots[9] = reinterpret_cast<void*>(&FsegPut32);
    s.outVt.slots[18] = reinterpret_cast<void*>(&FsegError);
    s.seg.vtbl = s.segVt.slots.data();
    s.db.vtbl = s.dbVt.slots.data();
    s.out.vtbl = s.outVt.slots.data();
}

// A plugin's flora type filter (flora §11.1 slot 13, FQ9): slot 3 accepts one
// type and records every type it is asked about. Any other call aborts (the
// simulator takes no reference on it).
struct FakeFloraFilter {
    FakeVtable vt;
    FakeObject obj{};
    uint32_t accept = 0;
    std::vector<uint32_t> asked;
};

FakeFloraFilter* g_ffilt = nullptr;

bool __fastcall FfiltIncluded(void*, void*, uint32_t type) {  // 3
    g_ffilt->asked.push_back(type);
    return type == g_ffilt->accept;
}

// The calls that need a city do nothing without one (no occupant manager, no
// terrain): nothing found, nothing planted, nothing removed.
void CheckFloraNoCity(cISC4FloraSimulator* f) {
    float pos[3] = {10.0f, 0.0f, 10.0f};
    CHECK(f->AddNewFloraOccupant(0x12345678u, pos, false) == nullptr);  // not a registered type
    CHECK(!f->AddNewFloraOccupant(static_cast<cISC4Occupant*>(nullptr)));
    CHECK(!f->KillFloraOccupant(nullptr));
    CHECK(!f->DemolishFloraOccupant(nullptr, 0));
    SC4Rect<int32_t> rect{};
    rect.topLeftX = 0;
    rect.topLeftY = 0;
    rect.bottomRightX = 4;
    rect.bottomRightY = 4;
    CHECK(f->DemolishFloraInCellArea(rect, 0));  // no flora there: true
    const float mn[3] = {0.0f, -100.0f, 0.0f};
    const float mx[3] = {64.0f, 100.0f, 64.0f};
    CHECK(f->DemolishFloraInBoundingBox(mn, mx, 0));
    CHECK(f->GetCostOfOccupantDemolition(nullptr, 0) == 0);
    CHECK(f->SaveObliterated(nullptr));  // no segment: nothing opens, true (§10.3)
}

int FloraEnableTest(const std::string& dll, const std::string& ini, const std::string& log) {
    // Off by default; `enable = flora` registers the flora simulator. The
    // flora shadow (it compares against the original) is then inactive.
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    SetEnvironmentVariableA("FC4_FLORA_SHADOW", nullptr);
    WriteTextFile(ini, "[subsystems]\nenable = flora\n[tools]\nflora_shadow = 1\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr && seam != nullptr);
    static FakeOrigFlora orig;
    InitFakeOrigFlora(orig);
    // A record in the game layout (docs/abi/layouts.md) for slot 6; static:
    // the registry keeps it (and its reference) for the life of the process.
    static FC4FloraBlock added{};
    added.type = 0x7E570001;
    added.next = 0x7E570002;  // not registered: no record's seed type changes (§6.11), key 0's stays as built
    added.flags = 0x01;
    added.refCount = 0;
    if (director != nullptr && seam != nullptr) {
        std::vector<uint32_t> classes;
        director->EnumClassObjects(&CollectClass, &classes);
        // simulator (1) + budget (3) + flora (1); never the flora occupant class.
        CHECK(classes.size() == 5);
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_Flora) == 1);
        CHECK(std::count(classes.begin(), classes.end(), 0xA9C05C85u) == 0);

        void* obj = nullptr;
        CHECK(director->GetClassObject(kCLSID_Flora, kIID_cISC4FloraSimulatorT, &obj));
        auto* flora = static_cast<cISC4FloraSimulator*>(obj);
        if (flora != nullptr) {
            // §1: the primary for its IID and cIGZUnknown; the message target
            // and serializable; nothing else (the out pointer untouched).
            void* p = nullptr;
            CHECK(flora->QueryInterface(GZIID_cIGZUnknown, &p) && p == static_cast<void*>(flora));
            static_cast<cIGZUnknown*>(p)->Release();
            cIGZMessageTarget2* mt = nullptr;
            CHECK(flora->QueryInterface(kIID_cIGZMessageTarget2Flora, reinterpret_cast<void**>(&mt)) && mt != nullptr);
            cIGZSerializable* ser = nullptr;
            CHECK(flora->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) && ser != nullptr);
            if (ser != nullptr) {
                CHECK(ser->GetGZCLSID() == kCLSID_Flora);
                ser->Release();
            }
            p = reinterpret_cast<void*>(0x1234);
            CHECK(!flora->QueryInterface(0x12345678, &p) && p == reinterpret_cast<void*>(0x1234));
            // Before the first Init of the process the registry is empty.
            CHECK(flora->GetStaticFloraData(0) == nullptr);
            CHECK(!flora->IsFloraTypeValid(0));
            uint32_t n = 99;
            flora->GetAllFloraTypes(nullptr, n);
            CHECK(n == 0);
            // The month line before Init; then a clean lifecycle without a city.
            if (mt != nullptr) {
                TypeMessage month(kMsgSimNewMonth);
                CHECK(mt->DoMessage(&month));
            }
            CHECK(flora->Init());
            CHECK(flora->Init());
            // §3.2: the key-0 default record, a game-layout block (flags 0x01)
            // that keeps our reference for the life of the process.
            auto* key0 = reinterpret_cast<FC4FloraBlock*>(flora->GetStaticFloraData(0));
            CHECK(key0 != nullptr);
            if (key0 != nullptr) {
                CHECK(key0->type == 0);
                CHECK(key0->flags == 0x01);
                CHECK(key0->refCount == 1);
                CHECK(key0->exemplar == nullptr);
                CHECK(key0->familyBegin == nullptr);
            }
            CHECK(flora->IsFloraTypeValid(0));
            CHECK(flora->IsFloraFamilyTypeValid(0));  // §11.1 slot 12: a family ID or a registered type
            n = 99;
            flora->GetAllFloraTypes(nullptr, n);
            CHECK(n == 0);  // key 0 is in the map only
            // Slot 5 without a key or an exemplar; slot 6 with a record of
            // ours: registered once, its reference taken (+0x170).
            CHECK(!flora->AddStaticFloraData(static_cast<cGZPersistResourceKey*>(nullptr), nullptr));
            CHECK(flora->AddStaticFloraData(reinterpret_cast<cSC4StaticFloraData*>(&added)));
            CHECK(added.refCount == 1);
            CHECK(!flora->AddStaticFloraData(reinterpret_cast<cSC4StaticFloraData*>(&added)));
            CHECK(added.refCount == 1);
            CHECK(flora->GetStaticFloraData(added.type) == reinterpret_cast<cSC4StaticFloraData*>(&added));
            CHECK(flora->IsFloraTypeValid(added.type));
            n = 0;
            flora->GetAllFloraTypes(nullptr, n);
            CHECK(n == 1);
            uint32_t types[4] = {};
            uint32_t cap = 4;
            flora->GetAllFloraTypes(types, cap);
            CHECK(cap == 1 && types[0] == added.type);
            CheckFloraNoCity(flora);
            // §10.3 (FQ3): without a city there are no occupants, so
            // SaveObliterated writes the count record alone, u16 0 then u32 0,
            // through the segment's cISC4DBSegment, closes it and releases
            // both references.
            static FakeSegment fseg;
            InitFakeSegment(fseg);
            CHECK(flora->SaveObliterated(reinterpret_cast<cIGZPersistDBSegment*>(&fseg.seg)));
            CHECK(fseg.opens == 1 && fseg.closes == 1);
            CHECK(fseg.key[0] == 0xC9BB3FC6u && fseg.key[1] == 0xC9BB3FC7u && fseg.key[2] == 1u);
            CHECK((fseg.bytes == std::vector<uint8_t>{0, 0, 0, 0, 0, 0}));
            CHECK(fseg.refs == 1);
            // AQ10 (§10.3): an open that returns false but writes a stream:
            // not open (nothing written, no close), and that stream's
            // reference is released once.
            static FakeSegment fsegFalse;
            InitFakeSegment(fsegFalse);
            fsegFalse.falseOpen = true;
            CHECK(flora->SaveObliterated(reinterpret_cast<cIGZPersistDBSegment*>(&fsegFalse.seg)));
            CHECK(fsegFalse.opens == 1 && fsegFalse.closes == 0);
            CHECK(fsegFalse.bytes.empty());
            CHECK(fsegFalse.refs == 1);
            // §11.1 slot 13 (FQ9): the map {0, added} in ascending order, each
            // key asked of the filter's slot 3; the accepted count returned.
            // (No game allocator here: the inserts fail, the count does not
            // depend on them.)
            static FakeFloraFilter filt;
            g_ffilt = &filt;
            filt.vt.slots[3] = reinterpret_cast<void*>(&FfiltIncluded);
            filt.obj.vtbl = filt.vt.slots.data();
            filt.accept = added.type;
            intptr_t set[5] = {};  // an empty hash_set<uint32_t> (0x14 bytes)
            CHECK(flora->GetFloraTypesByFilter(set[0], reinterpret_cast<cISC4FloraTypeFilter*>(&filt.obj)) == 1);
            CHECK((filt.asked == std::vector<uint32_t>{0u, added.type}));
            filt.accept = 0x12345678u;
            CHECK(flora->GetFloraTypesByFilter(set[0], reinterpret_cast<cISC4FloraTypeFilter*>(&filt.obj)) == 0);
            CHECK(flora->GetFloraTypesByFilter(set[0], nullptr) == 2);  // null: every key
            float score = -1.0f;
            CHECK(flora->GetBestFloraTypeForLocation(10.0f, 10.0f, score) == 0);
            // Days and a month without a city.
            if (mt != nullptr) {
                for (int i = 0; i < 3; ++i) {
                    TypeMessage day(kMsgSimNewDay);
                    CHECK(mt->DoMessage(&day));
                }
                TypeMessage month(kMsgSimNewMonth);
                CHECK(mt->DoMessage(&month));
            }
            // The registry getters from a second thread (the snapshot, no lock).
            void* fromThread = nullptr;
            bool validThread = false;
            std::thread t([&] {
                fromThread = flora->GetStaticFloraData(0);
                validThread = flora->IsFloraTypeValid(added.type);
            });
            t.join();
            CHECK(fromThread == static_cast<void*>(key0) && validThread);
            // The shadow (test seam op 17) refuses our own simulator.
            orig.ourSim = flora;
            intptr_t reg = 0;
            CHECK(FloraShadowOnce(seam, &orig.ours, &orig.agent, kMsgSimNewMonth, 0, &reg) == -1);
            CHECK(reg == -1);
            orig.key0 = key0;  // the fake original's key-0 record: our own block
            if (mt != nullptr) mt->Release();
            CHECK(flora->Shutdown());
            CHECK(flora->Release() == 0);
            // The registry outlives the simulator (§3.2).
            CHECK(key0 == nullptr || key0->refCount == 1);
        }
        // The shadow on fake originals: no city, a city without a flora
        // simulator, a day (not taken), then the original: the registry
        // matches ours (no exemplars, key 0 the same), and its record does
        // not parse, so the month compares nothing (-2).
        intptr_t reg = 0;
        CHECK(FloraShadowOnce(seam, nullptr, &orig.agent, kMsgSimNewMonth, 0, &reg) == -1);
        CHECK(FloraShadowOnce(seam, &orig.noFlora, &orig.agent, kMsgSimNewMonth, 0, &reg) == -1);
        CHECK(FloraShadowOnce(seam, &orig.city, &orig.agent, kMsgSimNewDay, 2451576, &reg) == -1);
        CHECK(orig.delivered == 0);
        CHECK(FloraShadowOnce(seam, &orig.city, &orig.agent, kMsgSimNewMonth, 2451576, &reg) == -2);  // 2000-02-01
        CHECK(reg == 0);
        CHECK(orig.delivered == 1);
        CHECK(orig.writes == 2);
        CHECK(orig.refs == 1);  // every QueryInterface released
    }
    FreeLibrary(hd);
    DeleteFileA(ini.c_str());
    const std::string text = ReadFile(log);
    CHECK(text.find("subsystem 'flora': enabled (1 class replaced) by FreeCity4.ini (enable=)") != std::string::npos);
    CHECK(text.find("flora: the process-wide flora registry") != std::string::npos);
    CHECK(text.find("cFC4FloraSimulator(") != std::string::npos);
    CHECK(text.find("flora month -: flora=- (no census: no occupant manager); planted=0(+0)") != std::string::npos);
    CHECK(text.find("flora Init -: flora=- (no census: no occupant manager)") != std::string::npos);
    CHECK(text.find("is not registered [choice]: the records are kept") != std::string::npos);
    CHECK(text.find("flora: no resource manager; no flora exemplars") != std::string::npos);
    CHECK(text.find("flora: SaveObliterated(00000000) -> 1") != std::string::npos);
    CHECK(text.find("flora Shutdown (3 days, 2 months, ") != std::string::npos);
    CHECK(text.find("cFC4FloraSimulator destroyed") != std::string::npos);
    CHECK(text.find("flora shadow: requested (flora_shadow from FreeCity4.ini) but INACTIVE") != std::string::npos);
    CHECK(text.find("flora shadow: ON") == std::string::npos);
    CHECK(text.find("flora shadow: inactive: the city's flora simulator is FreeCity4's own") != std::string::npos);
    CHECK(text.find("flora shadow: comparing with the original flora simulator") != std::string::npos);
    CHECK(text.find("flora shadow registry: exemplars 0, types ours 0 original 0; records compared 1; key 0 compared "
                    "(flags and plop cost not); differences 0") != std::string::npos);
    CHECK(text.find("flora shadow month 2000-02-01: INCOMPLETE (before: the record is short)") != std::string::npos);
    CHECK(text.find("flora shadow census 2000-02-01: before 0 (young 0, maturing 0, mature 0, burnt 0, dying 0, "
                    "other 0; types 0) after 0") != std::string::npos);
    CHECK(text.find("structured exception") == std::string::npos);
    CHECK(text.find("config: WARNING unknown key") == std::string::npos);
    CHECK(text.find("WARNING: released last reference") == std::string::npos);
    CHECK(text.find("WARNING: over-release") == std::string::npos);
    if (g_failures == 0) std::printf("plugin flora enable test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}

// ---- landfill (landfill.md): the replacement without a city, and the shadow (test seam op 18) -------
constexpr uint32_t kCLSID_Landfill = 0xEA09D4AE;
constexpr uint32_t kIID_cISC4LandfillDeveloper = 0x8A09D502;
constexpr uint32_t kIID_cIGZMessageTarget2Landfill = 0x452294AA;

// In-memory streams for the record (§6.1): primitives little-endian; no
// segment stream, so no references (a record without zones has none).
class LandfillOStream final : public cIGZOStream {
public:
    std::vector<uint8_t> bytes;
    bool QueryInterface(uint32_t, void**) override { return false; }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    void Flush() override {}
    bool SetSint8(int8_t v) override { return Put(&v, 1); }
    bool SetUint8(uint8_t v) override { return Put(&v, 1); }
    bool SetSint16(int16_t v) override { return Put(&v, 2); }
    bool SetUint16(uint16_t v) override { return Put(&v, 2); }
    bool SetSint32(int32_t v) override { return Put(&v, 4); }
    bool SetUint32(uint32_t v) override { return Put(&v, 4); }
    bool SetSint64(int64_t v) override { return Put(&v, 8); }
    bool SetUint64(uint64_t v) override { return Put(&v, 8); }
    bool SetFloat32(float v) override { return Put(&v, 4); }
    bool SetFloat64(double v) override { return Put(&v, 8); }
    bool SetRZCharStr(char const*) override { return false; }
    bool SetGZStr(cIGZString const&) override { return false; }
    bool SetGZSerializable(cIGZSerializable const&) override { return false; }
    bool SetVoid(void const* p, uint32_t n) override { return Put(p, n); }
    int32_t GetError() override { return 0; }
    int32_t SetUserData(cIGZVariant*) override { return 0; }
    int32_t GetUserData() override { return 0; }

private:
    bool Put(const void* p, size_t n) {
        const uint8_t* b = static_cast<const uint8_t*>(p);
        bytes.insert(bytes.end(), b, b + n);
        return true;
    }
};

class LandfillIStream final : public cIGZIStream {
public:
    explicit LandfillIStream(std::vector<uint8_t> b) : bytes(std::move(b)) {}
    std::vector<uint8_t> bytes;
    size_t at = 0;
    int32_t error = 0;
    bool QueryInterface(uint32_t, void**) override { return false; }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    bool Skip(uint32_t n) override { return Get(nullptr, n); }
    bool GetSint8(int8_t& v) override { return Get(&v, 1); }
    bool GetUint8(uint8_t& v) override { return Get(&v, 1); }
    bool GetSint16(int16_t& v) override { return Get(&v, 2); }
    bool GetUint16(uint16_t& v) override { return Get(&v, 2); }
    bool GetSint32(int32_t& v) override { return Get(&v, 4); }
    bool GetUint32(uint32_t& v) override { return Get(&v, 4); }
    bool GetSint64(int64_t& v) override { return Get(&v, 8); }
    bool GetUint64(uint64_t& v) override { return Get(&v, 8); }
    bool GetFloat32(float& v) override { return Get(&v, 4); }
    bool GetFloat64(double& v) override { return Get(&v, 8); }
    bool GetRZCharStr(char*, uint32_t) override { return Fail(); }
    bool GetGZStr(cIGZString&) override { return Fail(); }
    bool GetGZSerializable(cIGZSerializable&) override { return Fail(); }
    bool GetVoid(void* p, uint32_t n) override { return Get(p, n); }
    int32_t GetError() override { return error; }
    int32_t SetUserData(cIGZVariant*) override { return 0; }
    int32_t GetUserData() override { return 0; }

private:
    bool Fail() {
        error = 1;
        return false;
    }
    bool Get(void* p, size_t n) {
        if (error != 0 || bytes.size() - at < n) return Fail();
        if (p != nullptr && n != 0) std::memcpy(p, bytes.data() + at, n);
        at += n;
        return true;
    }
};

// The game's SC4CellRegion<long> (zones.md, 0x24 bytes): the inclusive
// rectangle and a cRZCellMap<bool> (vtable, width, height, words per row,
// rows; bit dz % 32 of rows[dx][dz / 32]).
struct LandfillGameRegion {
    int32_t x1, z1, x2, z2;
    const void* vtbl;
    uint32_t width, height, words;
    uint32_t** rows;
};
static_assert(sizeof(LandfillGameRegion) == 0x24, "SC4CellRegion<long> layout");

// The neutral answers without a city (§7.1; efficiency 1.0 without maintenance).
void CheckLandfillNeutral(cFC4ISC4LandfillDeveloper* d) {
    SC4Percentage e{0.0f};
    CHECK(d->GetEfficiency(&e) == &e && e.percentage == 1.0f);  // the hidden result pointer in EAX
    CHECK(d->GetSimulatorType() == 0x2A398ED6u);
    CHECK(d->GetMaintenanceCosts() == 0);
    CHECK(d->GetCurrentBudget() == 0);
    uint32_t usage = 9, capacity = 9;
    d->GetLandfillUsage(usage, capacity);
    CHECK(usage == 0 && capacity == 0);
    CHECK(d->GetLandfillTileCount() == 0);
    CHECK(d->GetLandfillZoneFromCell(3, 4) == 0);
    CHECK(d->GetZoningCost(nullptr) == 0);
}

// GetZoningCost on game regions: a flat 2 x 3 region (no city: altitude 0
// everywhere) costs nothing; a region whose cell map does not match its
// rectangle is refused (logged once) and costs nothing.
void CheckLandfillRegions(cFC4ISC4LandfillDeveloper* d) {
    uint32_t col0[1] = {0x7u}, col1[1] = {0x5u};
    uint32_t* rows[2] = {col0, col1};
    LandfillGameRegion good{10, 20, 11, 22, nullptr, 2, 3, 1, rows};
    CHECK(d->GetZoningCost(reinterpret_cast<SC4CellRegion<int32_t> const*>(&good)) == 0);
    LandfillGameRegion bad = good;
    bad.width = 5;  // the rectangle is 2 wide
    CHECK(d->GetZoningCost(reinterpret_cast<SC4CellRegion<int32_t> const*>(&bad)) == 0);
    CHECK(d->GetZoningCost(reinterpret_cast<SC4CellRegion<int32_t> const*>(&bad)) == 0);
}

// Threads (cFC4Landfill.h): getters from another thread while the main
// thread holds the lock inside DoMessage wait about 20 ms each, then answer
// from the snapshot (one fallback each).
class LandfillLockHeldMessage final : public cIGZMessage2 {
public:
    explicit LandfillLockHeldMessage(cFC4ISC4LandfillDeveloper* d) : dev(d) {}
    cFC4ISC4LandfillDeveloper* dev;
    bool ran = false;
    int64_t maintenance = 99;
    int32_t tiles = 99;
    uint32_t usage = 99, capacity = 99;
    float efficiency = 0.0f;
    DWORD ms = 0;
    bool QueryInterface(uint32_t, void**) override {
        ran = true;
        const DWORD start = GetTickCount();
        std::thread t([this] {
            maintenance = dev->GetMaintenanceCosts();
            tiles = dev->GetLandfillTileCount();
            dev->GetLandfillUsage(usage, capacity);
            SC4Percentage e{0.0f};
            efficiency = dev->GetEfficiency(&e)->percentage;
        });
        t.join();
        ms = GetTickCount() - start;
        return false;
    }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    bool Create(uint32_t, void**) const override { return false; }
    uint32_t GetType() const override { return kMsgSimNewDay; }
    void SetType(uint32_t) override {}
    bool operator==(cIGZMessage2 const&) const override { return false; }
    bool operator<(cIGZMessage2 const&) const override { return false; }
};

// LG20: a DoMessage from another thread waits for the lock instead of being
// dropped. The probe records when the developer asks it for its standard
// interface (after taking the lock); the holder keeps the lock for 60 ms
// while another thread delivers the probe.
class LandfillProbeMessage final : public cIGZMessage2 {
public:
    std::atomic<bool> handled{false};
    std::atomic<DWORD> at{0};
    bool QueryInterface(uint32_t, void**) override {
        at = GetTickCount();
        handled = true;
        return false;
    }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    bool Create(uint32_t, void**) const override { return false; }
    uint32_t GetType() const override { return kMsgSimNewDay; }
    void SetType(uint32_t) override {}
    bool operator==(cIGZMessage2 const&) const override { return false; }
    bool operator<(cIGZMessage2 const&) const override { return false; }
};

class LandfillHoldMessage final : public cIGZMessage2 {
public:
    LandfillHoldMessage(cIGZMessageTarget2* t, LandfillProbeMessage* p) : mt(t), probe(p) {}
    cIGZMessageTarget2* mt;
    LandfillProbeMessage* probe;
    std::thread worker;
    bool probeBeforeRelease = true;
    DWORD released = 0;
    bool QueryInterface(uint32_t, void**) override {
        worker = std::thread([this] { mt->DoMessage(probe); });
        Sleep(60);
        probeBeforeRelease = probe->handled.load();
        released = GetTickCount();
        return false;
    }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    bool Create(uint32_t, void**) const override { return false; }
    uint32_t GetType() const override { return kMsgSimNewDay; }
    void SetType(uint32_t) override {}
    bool operator==(cIGZMessage2 const&) const override { return false; }
    bool operator<(cIGZMessage2 const&) const override { return false; }
};

void CheckLandfillThreads(cFC4ISC4LandfillDeveloper* d, cIGZMessageTarget2* mt) {
    LandfillLockHeldMessage msg(d);
    CHECK(mt->DoMessage(&msg));
    CHECK(msg.ran);
    CHECK(msg.maintenance == 0 && msg.tiles == 0 && msg.usage == 0 && msg.capacity == 0);  // the snapshot
    CHECK(msg.efficiency == 1.0f);
    CHECK(msg.ms >= 60 && msg.ms < 5000);  // four getters waited (about 20 ms each), none blocked
    // Without contention another thread gets the lock at once (no fallback).
    int64_t m = 99;
    std::thread t([&] { m = d->GetMaintenanceCosts(); });
    t.join();
    CHECK(m == 0);
    // LG20: the other thread's message waited 60 ms (well past the getters'
    // 20) and was then handled, after the holder let go.
    LandfillProbeMessage probe;
    LandfillHoldMessage hold(mt, &probe);
    CHECK(mt->DoMessage(&hold));
    if (hold.worker.joinable()) hold.worker.join();
    CHECK(!hold.probeBeforeRelease);
    CHECK(probe.handled.load());
    CHECK(probe.at.load() >= hold.released);
}

// An "original" landfill developer: QueryInterface answers its agent
// (cIGZMessageTarget2) and cIGZSerializable, whose Write writes the record
// version `version` and, for 3, a zone count of 0 (§6.1). Slot 7
// (GetMaintenanceCosts, int64) returns `maintenance`; the month sets it to
// `maintenanceAfter`. The city answers GetLandfillDeveloper (compiled slot
// 78) only; `noDev` returns null there, `ours` our own developer. Any other
// call hits a trap and aborts.
struct FakeOrigLandfill {
    FakeVtable devVt, agentVt, serVt, cityVt, noDevVt, oursVt;
    FakeObject dev{}, agent{}, ser{}, city{}, noDev{}, ours{}, other{};
    void* ourDev = nullptr;
    int refs = 1;
    int deliveries = 0;
    int writes = 0;
    uint16_t version = 3;
    int64_t maintenance = 0;
    int64_t maintenanceAfter = 0;
};

FakeOrigLandfill* g_ol = nullptr;

bool __fastcall OlQi(void*, void*, uint32_t iid, void** out) {  // 0 (developer, agent, serializable)
    if (out == nullptr) return false;
    if (iid == kIID_cIGZMessageTarget2Landfill) {
        *out = &g_ol->agent;
    } else if (iid == GZIID_cIGZSerializable) {
        *out = &g_ol->ser;
    } else {
        return false;  // (the reference audit asks other IIDs)
    }
    ++g_ol->refs;
    return true;
}
uint32_t __fastcall OlAddRef(void*, void*) { return static_cast<uint32_t>(++g_ol->refs); }   // 1
uint32_t __fastcall OlRelease(void*, void*) { return static_cast<uint32_t>(--g_ol->refs); }  // 2
bool __fastcall OlWrite(void*, void*, cIGZOStream* s) {                                       // serializable 3
    ++g_ol->writes;
    if (!s->SetUint16(g_ol->version)) return false;
    return g_ol->version != 3 || s->SetSint32(0);
}
uint32_t __fastcall OlClsid(void*, void*) { return kCLSID_Landfill; }  // serializable 5
bool __fastcall OlDoMessage(void*, void*, cIGZMessage2*) {             // agent 3
    ++g_ol->deliveries;
    g_ol->maintenance = g_ol->maintenanceAfter;
    return true;
}
int64_t __fastcall OlMaintenance(void*, void*) { return g_ol->maintenance; }  // developer 7 (EDX:EAX)
void* __fastcall OlCityDev(void*, void*) { return &g_ol->dev; }               // city 78
void* __fastcall OlCityNoDev(void*, void*) { return nullptr; }                // city 78
void* __fastcall OlCityOurs(void*, void*) { return g_ol->ourDev; }            // city 78

void InitFakeOrigLandfill(FakeOrigLandfill& o) {
    g_ol = &o;
    for (FakeVtable* vt : {&o.devVt, &o.agentVt, &o.serVt}) {
        vt->slots[0] = reinterpret_cast<void*>(&OlQi);
        vt->slots[1] = reinterpret_cast<void*>(&OlAddRef);
        vt->slots[2] = reinterpret_cast<void*>(&OlRelease);
    }
    o.devVt.slots[7] = reinterpret_cast<void*>(&OlMaintenance);
    o.agentVt.slots[3] = reinterpret_cast<void*>(&OlDoMessage);
    o.serVt.slots[3] = reinterpret_cast<void*>(&OlWrite);
    o.serVt.slots[5] = reinterpret_cast<void*>(&OlClsid);
    o.cityVt.slots[78] = reinterpret_cast<void*>(&OlCityDev);
    o.noDevVt.slots[78] = reinterpret_cast<void*>(&OlCityNoDev);
    o.oursVt.slots[78] = reinterpret_cast<void*>(&OlCityOurs);
    o.dev.vtbl = o.devVt.slots.data();
    o.agent.vtbl = o.agentVt.slots.data();
    o.ser.vtbl = o.serVt.slots.data();
    o.city.vtbl = o.cityVt.slots.data();
    o.noDev.vtbl = o.noDevVt.slots.data();
    o.ours.vtbl = o.oursVt.slots.data();
    o.other.vtbl = o.agentVt.slots.data();  // an agent, but not the developer's
}

intptr_t LandfillShadowOnce(FC4TestSeamFn seam, FakeObject* city, cIGZMessageTarget2* target,
                            uint32_t type = kMsgSimNewMonth) {
    FC4LandfillShadowTestInput in{reinterpret_cast<cISC4City*>(city), target, type, 2451576};  // 2000-02-01
    return seam(kFC4SeamLandfillShadow, &in, nullptr);
}

int LandfillEnableTest(const std::string& dll, const std::string& ini, const std::string& log) {
    // Off by default; `enable = landfill` registers the landfill developer.
    // The landfill shadow (it compares against the original) is then inactive.
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    SetEnvironmentVariableA("FC4_LANDFILL_SHADOW", nullptr);
    SetEnvironmentVariableA("FC4_LANDFILL_SHADOW_DIFFS", nullptr);
    WriteTextFile(ini, "[subsystems]\nenable = landfill\n[tools]\nlandfill_shadow = 1\nlandfill_shadow_diffs = 4\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr && seam != nullptr);
    static FakeOrigLandfill orig;
    InitFakeOrigLandfill(orig);
    SetGamePrecision();  // as the game's PreFrameWorkInit does: the shadow must leave it
    std::vector<uint8_t> record;
    if (director != nullptr && seam != nullptr) {
        std::vector<uint32_t> classes;
        director->EnumClassObjects(&CollectClass, &classes);
        // simulator (1) + budget (3) + landfill (1); never the game's grid classes.
        CHECK(classes.size() == 5);
        for (uint32_t c : classes) CHECK(c < 0x49B9E600 || c > 0x49B9E60B);
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_Landfill) == 1);

        void* obj = nullptr;
        CHECK(director->GetClassObject(kCLSID_Landfill, kIID_cISC4LandfillDeveloper, &obj));
        auto* dev = static_cast<cFC4ISC4LandfillDeveloper*>(obj);
        if (dev != nullptr) {
            // §1: the primary for its IID and cIGZUnknown; the message target
            // and serializable; nothing else (the out pointer untouched).
            void* p = nullptr;
            CHECK(dev->QueryInterface(GZIID_cIGZUnknown, &p) && p == static_cast<void*>(dev));
            if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
            p = nullptr;
            CHECK(dev->QueryInterface(kIID_cISC4LandfillDeveloper, &p) && p == static_cast<void*>(dev));
            if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
            cIGZMessageTarget2* mt = nullptr;
            CHECK(dev->QueryInterface(kIID_cIGZMessageTarget2Landfill, reinterpret_cast<void**>(&mt)) &&
                  mt != nullptr);
            cIGZSerializable* ser = nullptr;
            CHECK(dev->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) && ser != nullptr);
            if (ser != nullptr) CHECK(ser->GetGZCLSID() == kCLSID_Landfill);
            p = reinterpret_cast<void*>(0x1234);
            CHECK(!dev->QueryInterface(0x12345678, &p) && p == reinterpret_cast<void*>(0x1234));
            CHECK(!dev->QueryInterface(0x2A398ED6, &p) && p == reinterpret_cast<void*>(0x1234));
            CheckLandfillNeutral(dev);
            CheckLandfillRegions(dev);
            // Without a city: the month line before Init, the record (§6.1:
            // version 3, no zones), a clean lifecycle, still neutral.
            if (mt != nullptr) {
                TypeMessage month(kMsgSimNewMonth);
                CHECK(mt->DoMessage(&month));
            }
            if (ser != nullptr) {
                LandfillOStream out;
                CHECK(ser->Write(out));
                record = out.bytes;
                const std::vector<uint8_t> empty = {3, 0, 0, 0, 0, 0};
                CHECK(record == empty);
            }
            CHECK(dev->Init());
            CheckLandfillNeutral(dev);
            if (mt != nullptr) {
                TypeMessage month(kMsgSimNewMonth);
                CHECK(mt->DoMessage(&month));
                TypeMessage day(kMsgSimNewDay);
                CHECK(mt->DoMessage(&day));
                CheckLandfillThreads(dev, mt);
            }
            CheckLandfillNeutral(dev);
            // The shadow (test seam op 18) refuses our own developer.
            orig.ourDev = dev;
            CHECK(LandfillShadowOnce(seam, &orig.ours, reinterpret_cast<cIGZMessageTarget2*>(&orig.agent)) == -1);
            if (ser != nullptr) ser->Release();
            if (mt != nullptr) mt->Release();
            CHECK(dev->Shutdown());
            CHECK(dev->Release() == 0);
        }
        // Read runs before Init (§6.1): the record written above loads into a
        // new developer; a record of another version does not.
        for (uint16_t version : {uint16_t{3}, uint16_t{4}}) {
            obj = nullptr;
            CHECK(director->GetClassObject(kCLSID_Landfill, kIID_cISC4LandfillDeveloper, &obj));
            auto* fresh = static_cast<cFC4ISC4LandfillDeveloper*>(obj);
            if (fresh == nullptr) continue;
            cIGZSerializable* s = nullptr;
            CHECK(fresh->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&s)) && s != nullptr);
            if (s != nullptr) {
                std::vector<uint8_t> bytes = record.empty() ? std::vector<uint8_t>{3, 0, 0, 0, 0, 0} : record;
                bytes[0] = static_cast<uint8_t>(version);
                LandfillIStream in(bytes);
                CHECK(s->Read(in) == (version == 3));
                s->Release();
            }
            CHECK(fresh->Release() == 0);  // the final Release shuts a never-initialised developer down
        }
        // The shadow on fake originals: no city, a city without a landfill
        // developer and an agent that is not the original's are left to the
        // caller (-1, nothing delivered); a SimNewDay is not taken. The
        // original's agent gets its month exactly once, its record captured
        // before and after (its own Write): version 4 is INCOMPLETE (-2,
        // nothing compared); an empty version-3 record matches ours (0); a
        // maintenance that changed in the original's month and not in ours is
        // one difference.
        cIGZMessageTarget2* agent = reinterpret_cast<cIGZMessageTarget2*>(&orig.agent);
        CHECK(LandfillShadowOnce(seam, nullptr, agent) == -1);
        CHECK(LandfillShadowOnce(seam, &orig.noDev, agent) == -1);
        CHECK(orig.deliveries == 0 && orig.writes == 0);
        CHECK(LandfillShadowOnce(seam, &orig.city, agent, kMsgSimNewDay) == -1);
        CHECK(orig.deliveries == 0 && orig.writes == 0);
        orig.version = 4;
        CHECK(LandfillShadowOnce(seam, &orig.city, agent) == -2);
        CHECK(orig.deliveries == 1 && orig.writes == 2);
        orig.version = 3;
        CHECK(LandfillShadowOnce(seam, &orig.city, agent) == 0);
        CHECK(orig.deliveries == 2 && orig.writes == 4);
        orig.maintenanceAfter = 77;
        CHECK(LandfillShadowOnce(seam, &orig.city, agent) == 1);
        CHECK(orig.deliveries == 3 && orig.writes == 6 && orig.maintenance == 77);
        orig.maintenance = orig.maintenanceAfter = 0;
        CHECK(LandfillShadowOnce(seam, &orig.city, reinterpret_cast<cIGZMessageTarget2*>(&orig.other)) == -1);
        CHECK(orig.deliveries == 3 && orig.writes == 6);
        CHECK(orig.refs == 1);  // every QueryInterface released
        CheckCw("landfill shadow");
    }
    FreeLibrary(hd);
    DeleteFileA(ini.c_str());
    const std::string text = ReadFile(log);
    CHECK(text.find("subsystem 'landfill': enabled (1 class replaced) by FreeCity4.ini (enable=)") !=
          std::string::npos);
    CHECK(text.find("cFC4LandfillDeveloper(") != std::string::npos);
    // The month status line from the first month (before Init), Init's, Read's.
    CHECK(text.find("landfill month: init=0 loaded=0 zones=0") != std::string::npos);
    CHECK(text.find("landfill Init: init=1 loaded=0 zones=0") != std::string::npos);
    CHECK(text.find("landfill month: init=1 loaded=0 zones=0") != std::string::npos);
    CHECK(text.find("stored=0 capacity=0 used=0 free=") != std::string::npos);
    CHECK(text.find("maintenance=0 activeCells=0") != std::string::npos);
    CHECK(text.find("landfill: no city; services not acquired") != std::string::npos);
    CHECK(text.find("landfill Write -> 1") != std::string::npos);
    CHECK(text.find("landfill Read -> 1") != std::string::npos);
    CHECK(text.find("landfill Read: init=0 loaded=1 zones=0") != std::string::npos);
    CHECK(text.find("landfill Read -> 0") != std::string::npos);
    CHECK(CountOf(text, "landfill: WARNING a cell region does not have the documented layout (rect 10,20..11,22, "
                        "map 5x3, 1 words per row); it is ignored") == 1);
    CHECK(text.find("landfill: GetLandfillZoneFromCell returns null") != std::string::npos);
    // LG20: the one message from another thread was logged and waited (no fallback).
    CHECK(CountOf(text, "(not the main thread); it waits for the lock") == 1);
    CHECK(text.find("landfill Shutdown: 2 months, 4 fallbacks") != std::string::npos);
    CHECK(text.find("cFC4LandfillDeveloper destroyed (4 fallbacks)") != std::string::npos);
    CHECK(CountOf(text, "cFC4LandfillDeveloper destroyed (0 fallbacks)") == 2);
    CHECK(text.find("landfill shadow: requested (landfill_shadow from FreeCity4.ini) but INACTIVE") !=
          std::string::npos);
    CHECK(text.find("landfill shadow: ON") == std::string::npos);
    CHECK(text.find("landfill shadow: inactive: the city's landfill developer is FreeCity4's own") !=
          std::string::npos);
    CHECK(text.find("landfill shadow: comparing with the original landfill developer") != std::string::npos);
    CHECK(text.find("differences listed per month: 4") != std::string::npos);
    CHECK(text.find("landfill shadow month to 2000-02-01: INCOMPLETE (before: record version not 3); not compared") !=
          std::string::npos);
    CHECK(text.find("landfill shadow month to 2000-02-01: clean; zones orig/ours 0/0; heaps compared=0 mismatched=0") !=
          std::string::npos);
    CHECK(text.find("landfill shadow month to 2000-02-01: mismatches=1; zones orig/ours 0/0") != std::string::npos);
    CHECK(text.find("maintenance orig/ours 77/0 (before 0, recomputed 0)") != std::string::npos);
    CHECK(text.find("first differences: maintenance 77/0") != std::string::npos);
    CHECK(text.find("structured exception") == std::string::npos);
    CHECK(text.find("config: WARNING unknown key") == std::string::npos);
    CHECK(text.find("WARNING: released last reference") == std::string::npos);
    CHECK(text.find("WARNING: over-release") == std::string::npos);
    if (g_failures == 0) std::printf("plugin landfill enable test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}

// ---- residential (residential.md, REH): the replacement without a city, and the shadow (op 25) --------
constexpr uint32_t kCLSID_Residential = 0x4990C013;
constexpr uint32_t kIID_cISC4ResidentialSimulator = 0x077AC1EE;
constexpr uint32_t kIID_cIGZMessageTarget2Residential = 0x452294AA;
constexpr int32_t kJulian20000201 = 2451576;

// Segment streams for the record (§9): primitives little-endian, references
// as a uint32 0. A record written without Init holds null references only;
// any other reference fails the stream.
class ResidentialSegOStream final : public cFC4ISC4DBSegmentOStream {
public:
    std::vector<uint8_t> bytes;
    int nullRefs = 0;
    bool QueryInterface(uint32_t riid, void** p) override {
        if (p == nullptr || (riid != kFC4_IID_cISC4DBSegmentOStream && riid != GZIID_cIGZUnknown)) return false;
        *p = static_cast<cFC4ISC4DBSegmentOStream*>(this);
        return true;
    }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    void Flush() override {}
    bool SetSint8(int8_t v) override { return Put(&v, 1); }
    bool SetUint8(uint8_t v) override { return Put(&v, 1); }
    bool SetSint16(int16_t v) override { return Put(&v, 2); }
    bool SetUint16(uint16_t v) override { return Put(&v, 2); }
    bool SetSint32(int32_t v) override { return Put(&v, 4); }
    bool SetUint32(uint32_t v) override { return Put(&v, 4); }
    bool SetSint64(int64_t v) override { return Put(&v, 8); }
    bool SetUint64(uint64_t v) override { return Put(&v, 8); }
    bool SetFloat32(float v) override { return Put(&v, 4); }
    bool SetFloat64(double v) override { return Put(&v, 8); }
    bool SetRZCharStr(char const*) override { return false; }
    bool SetGZStr(cIGZString const&) override { return false; }
    bool SetGZSerializable(cIGZSerializable const&) override { return false; }
    bool SetVoid(void const* p, uint32_t n) override { return Put(p, n); }
    int32_t GetError() override { return 0; }
    int32_t SetUserData(cIGZVariant*) override { return 0; }
    int32_t GetUserData() override { return 0; }
    bool Open(cISC4DBSegment*, cGZPersistResourceKey const&, bool) override { return false; }
    bool Close() override { return true; }
    bool IsOpen() override { return true; }
    int32_t GetRecord() override { return 0; }
    int32_t GetSegment() override { return 0; }
    bool WriteGZSerializable(cIGZSerializable const* p) override {
        if (p != nullptr) return false;
        ++nullRefs;
        return SetUint32(0);
    }
    bool WriteResKey(cGZPersistResourceKey const&) override { return false; }
    bool WriteVariant(cIGZVariant const&) override { return false; }

private:
    bool Put(const void* p, size_t n) {
        const uint8_t* b = static_cast<const uint8_t*>(p);
        bytes.insert(bytes.end(), b, b + n);
        return true;
    }
};

class ResidentialSegIStream final : public cFC4ISC4DBSegmentIStream {
public:
    explicit ResidentialSegIStream(std::vector<uint8_t> b) : bytes(std::move(b)) {}
    std::vector<uint8_t> bytes;
    size_t at = 0;
    int32_t error = 0;
    bool QueryInterface(uint32_t riid, void** p) override {
        if (p == nullptr || (riid != kFC4_IID_cISC4DBSegmentIStream && riid != GZIID_cIGZUnknown)) return false;
        *p = static_cast<cFC4ISC4DBSegmentIStream*>(this);
        return true;
    }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    bool Skip(uint32_t n) override { return Get(nullptr, n); }
    bool GetSint8(int8_t& v) override { return Get(&v, 1); }
    bool GetUint8(uint8_t& v) override { return Get(&v, 1); }
    bool GetSint16(int16_t& v) override { return Get(&v, 2); }
    bool GetUint16(uint16_t& v) override { return Get(&v, 2); }
    bool GetSint32(int32_t& v) override { return Get(&v, 4); }
    bool GetUint32(uint32_t& v) override { return Get(&v, 4); }
    bool GetSint64(int64_t& v) override { return Get(&v, 8); }
    bool GetUint64(uint64_t& v) override { return Get(&v, 8); }
    bool GetFloat32(float& v) override { return Get(&v, 4); }
    bool GetFloat64(double& v) override { return Get(&v, 8); }
    bool GetRZCharStr(char*, uint32_t) override { return Fail(); }
    bool GetGZStr(cIGZString&) override { return Fail(); }
    bool GetGZSerializable(cIGZSerializable&) override { return Fail(); }
    bool GetVoid(void* p, uint32_t n) override { return Get(p, n); }
    int32_t GetError() override { return error; }
    int32_t SetUserData(cIGZVariant*) override { return 0; }
    int32_t GetUserData() override { return 0; }
    bool Open(cISC4DBSegment*, cGZPersistResourceKey const&, bool) override { return false; }
    bool Close() override { return true; }
    bool IsOpen() override { return true; }
    int32_t GetRecord() override { return 0; }
    int32_t GetSegment() override { return 0; }
    bool ReadGZSerializable(cIGZSerializable** out, bool) override {
        uint32_t id = 1;
        if (out == nullptr || !GetUint32(id) || id != 0) return Fail();
        *out = nullptr;
        return true;
    }
    bool ReadResKey(cGZPersistResourceKey&) override { return Fail(); }
    bool ReadVariant(cIGZVariant&) override { return Fail(); }

private:
    bool Fail() {
        error = 1;
        return false;
    }
    bool Get(void* p, size_t n) {
        if (error != 0 || bytes.size() - at < n) return Fail();
        if (p != nullptr && n != 0) std::memcpy(p, bytes.data() + at, n);
        at += n;
        return true;
    }
};

// A standard message with data1 (SimNewDay's julian day: the status lines' date).
class ResidentialDayMessage final : public cIGZMessage2Standard {
public:
    ResidentialDayMessage(uint32_t t, intptr_t d1) : type(t), data1(d1) {}
    uint32_t type;
    intptr_t data1;
    bool QueryInterface(uint32_t riid, void** p) override {
        if (p == nullptr || riid != GZIID_cIGZMessage2Standard) return false;
        *p = static_cast<cIGZMessage2Standard*>(this);
        return true;
    }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    bool Create(uint32_t, void**) const override { return false; }
    uint32_t GetType() const override { return type; }
    void SetType(uint32_t t) override { type = t; }
    bool operator==(cIGZMessage2 const&) const override { return false; }
    bool operator<(cIGZMessage2 const&) const override { return false; }
    bool GetHasData1() const override { return true; }
    intptr_t GetData1() const override { return data1; }
    void* GetVoid1() const override { return nullptr; }
    void SetData1(intptr_t v) override { data1 = v; }
    void SetVoid1(void*) override {}
    bool GetHasData2() const override { return false; }
    intptr_t GetData2() const override { return 0; }
    void* GetVoid2() const override { return nullptr; }
    void SetData2(intptr_t) override {}
    void SetVoid2(void*) override {}
    bool GetHasData3() const override { return false; }
    intptr_t GetData3() const override { return 0; }
    void* GetVoid3() const override { return nullptr; }
    void SetData3(intptr_t) override {}
    void SetVoid3(void*) override {}
    bool GetHasData4() const override { return false; }
    intptr_t GetData4() const override { return 0; }
    void* GetVoid4() const override { return nullptr; }
    void SetData4(intptr_t) override {}
    void SetVoid4(void*) override {}
    bool GetHasString() const override { return false; }
    bool GetString(uint32_t, void**) const override { return false; }
    void SetString(cIGZString*) override {}
    bool GetHasIGZUnknown() const override { return false; }
    cIGZUnknown* GetIGZUnknown() const override { return nullptr; }
    void SetIGZUnknown(cIGZUnknown*) override {}
    bool GetHasExtra() const override { return false; }
    uint32_t GetExtra(uint32_t, void**) const override { return 0; }
    void SetExtra(cIGZSerializable*) override {}
};

// The answers without a city (§10.1), through the compiled slots. The AL-only
// slots (29, 34, 42) are also called raw: the whole of EAX must equal the
// uint8 (zero-extended). The by-value SC4Percentage slots (10-13, 30, 31)
// return the hidden result pointer in EAX.
void CheckResidentialNeutral(cFC4ISC4ResidentialSimulator* r) {
    using W = cFC4ISC4ResidentialSimulator::WealthType;
    void** vt = *reinterpret_cast<void***>(r);
    CHECK(r->GetTotalCityEducationUpkeepCost() == 0 && r->GetTotalCityHealthUpkeepCost() == 0);  // 17, 18
    CHECK(r->GetMaxEQ() == 200 && r->GetMaxHQ() == 200);                                          // 50, 51
    for (uint32_t w = 0; w <= 4; ++w) CHECK(r->GetProximityMap(static_cast<W>(w)) == nullptr);   // 5 (no grids)
    CHECK(!r->SchoolIsOnStrike() && !r->HealthIsOnStrike());
    CHECK(r->GetPopulation() == 0);
    CHECK(r->GetSchoolBuildingCount() == 0 && r->GetHospitalBuildingCount() == 0);
    CHECK(r->GetSchoolAverageGradeMap() == nullptr && r->GetHospitalAverageGradeMap() == nullptr);
    CHECK(r->GetAverageAgeMap() == nullptr);
    using PctFn = SC4Percentage*(__fastcall*)(void*, void*, SC4Percentage*);
    using PctXZFn = SC4Percentage*(__fastcall*)(void*, void*, SC4Percentage*, uint32_t, uint32_t);
    for (int slot : {10, 11, 12, 13, 31}) {
        SC4Percentage p{-1.0f};
        CHECK(reinterpret_cast<PctFn>(vt[slot])(r, nullptr, &p) == &p);
    }
    SC4Percentage cell{-1.0f};
    CHECK(reinterpret_cast<PctXZFn>(vt[30])(r, nullptr, &cell, 3, 4) == &cell);
    using U32Fn = uint32_t(__fastcall*)(void*, void*);
    using U32WFn = uint32_t(__fastcall*)(void*, void*, uint32_t);
    using U32XZFn = uint32_t(__fastcall*)(void*, void*, uint32_t, uint32_t);
    CHECK(reinterpret_cast<U32XZFn>(vt[29])(r, nullptr, 3, 4) == r->GetCellLifeExpectancy(3, 4));
    CHECK(reinterpret_cast<U32Fn>(vt[34])(r, nullptr) == r->GetGlobalLE());
    for (uint32_t w = 0; w <= 4; ++w) {
        CHECK(reinterpret_cast<U32WFn>(vt[42])(r, nullptr, w) == r->GetAverageNewAgeByWealth(static_cast<W>(w)));
    }
    // 14 / 15: a null group vector counts nothing [choice]; an empty one too.
    FC4ResidentialQueryData q{};
    CHECK(r->GetSchoolSystemTotals(&q, nullptr) == 0);
    // An empty caller vector (mpStart, mpEnd, mpCapacity), built in place:
    // an SC4Vector of our own would free through the game allocator.
    uint32_t* emptyVector[3] = {nullptr, nullptr, nullptr};
    const auto& none = *reinterpret_cast<const SC4Vector<uint32_t>*>(emptyVector);
    CHECK(r->GetHospitalSystemTotals(&q, &none) == 0);
    // 21-25: true; no grids yet (21 / 22: the range 255 / 0 without a grid).
    cISC4SimGrid<float>* f[3] = {};
    cISC4SimGrid<uint16_t>* p16[3] = {};
    for (auto*& g : f) g = reinterpret_cast<cISC4SimGrid<float>*>(1);
    CHECK(r->GetEQGrids(f[0], f[1], f[2]) && !f[0] && !f[1] && !f[2]);
    for (auto*& g : f) g = reinterpret_cast<cISC4SimGrid<float>*>(1);
    CHECK(r->GetHQGrids(f[0], f[1], f[2]) && !f[0] && !f[1] && !f[2]);
    for (auto*& g : p16) g = reinterpret_cast<cISC4SimGrid<uint16_t>*>(1);
    CHECK(r->GetPopulationGrids(p16[0], p16[1], p16[2]) && !p16[0] && !p16[1] && !p16[2]);
    float mn = 0.0f, mx = 1.0f;
    cISC4SimGrid<float>* avg = reinterpret_cast<cISC4SimGrid<float>*>(1);
    CHECK(r->GetAverageEQGrid(avg, &mn, &mx) && avg == nullptr && mn == 255.0f && mx == 0.0f);
    avg = reinterpret_cast<cISC4SimGrid<float>*>(1);
    CHECK(r->GetAverageHQGrid(avg, &mn, &mx) && avg == nullptr);
    // 43 / 44: false, every output written (0) when the population is 0.
    uint32_t a = 9, b = 9, c = 9, d = 9;
    float lo = 9.0f, hi = 9.0f;
    CHECK(!r->GetEQMinAndMaxCellCoords(a, b, c, d, lo, hi) && a == 0 && b == 0 && c == 0 && d == 0 && lo == 0.0f &&
          hi == 0.0f);
    a = b = c = d = 9;
    CHECK(!r->GetHQMinAndMaxCellCoords(a, b, c, d, lo, hi) && a == 0 && d == 0);
    // 45: false, outputs untouched (not a school or hospital).
    float dist = 7.0f, radius = 8.0f;
    const SC4Percentage level{0.5f};
    CHECK(!r->GetOccupantCoverage(nullptr, level, dist, radius) && dist == 7.0f && radius == 8.0f);
    // 19, 20, 28: true.
    SC4Percentage staff{0.5f}, vehicle{0.5f};
    CHECK(r->SetOccupantFundingPercentages(nullptr, staff, vehicle, true));
    CHECK(r->GetOccupantFundingPercentages(nullptr, staff, vehicle, false));
    uint32_t capacity = 0, staffCount = 0;
    CHECK(r->EstimateCurrentOccupantCapacity(nullptr, capacity, staffCount));
    CHECK(!r->GetAutoBudget(nullptr));
    // 47 / 49 (true) and 61: no buildings.
    EmptyOccupantList list;
    CHECK(r->GetSchoolBuildings(list.Get(), nullptr) && list.Empty());
    CHECK(r->GetHospitalBuildings(list.Get(), &none) && list.Empty());
    using ListFn = void(__fastcall*)(void*, void*, SC4List<cISC4Occupant*>*);
    reinterpret_cast<ListFn>(vt[61])(r, nullptr, &list.Get());
    CHECK(list.Empty());
    // 59, 62, 63: empty.
    r->ToggleTractTracking(1, 2);
    reinterpret_cast<void(__fastcall*)(void*, void*)>(vt[62])(r, nullptr);
    reinterpret_cast<void(__fastcall*)(void*, void*, int32_t, int32_t)>(vt[63])(r, nullptr, 1, 2);
}

// Getters and a message from another thread: the published snapshot, the
// message queued for the next main-thread call (fc4.h).
void CheckResidentialThreads(cFC4ISC4ResidentialSimulator* r, cIGZMessageTarget2* mt) {
    int32_t pop = 9;
    bool empty = false, delivered = false, strike = true;
    std::thread t([&] {
        pop = r->GetPopulation();
        strike = r->SchoolIsOnStrike();
        EmptyOccupantList list;
        r->GetHospitalBuildings(list.Get(), nullptr);
        empty = list.Empty();
        (void)r->GetGlobalLE();
        TypeMessage day(kMsgSimNewDay);
        delivered = mt->DoMessage(&day);
    });
    t.join();
    CHECK(pop == 0 && empty && delivered && !strike);
}

// An "original" residential simulator: QueryInterface answers its agent
// (cIGZMessageTarget2) and cIGZSerializable, whose Write writes the record
// version 7 only (a short record: the shadow cannot read it). Slots 16, 32,
// 33 and 34 answer the scalars (34 in AL only: the upper bits of EAX are
// garbage). The city answers GetResidentialSimulator (compiled slot 68)
// only; `noRes` returns null there, `ours` our own simulator. Any other call
// hits a trap and aborts.
struct FakeOrigResidential {
    FakeVtable simVt, agentVt, serVt, cityVt, noResVt, oursVt;
    FakeObject sim{}, agent{}, ser{}, city{}, noRes{}, ours{}, other{};
    void* ourSim = nullptr;
    int refs = 1;
    int deliveries = 0;
    int writes = 0;
    int scalars = 0;
};

FakeOrigResidential* g_or = nullptr;

bool __fastcall OrQi(void*, void*, uint32_t iid, void** out) {  // 0 (simulator, agent, serializable)
    if (out == nullptr) return false;
    if (iid == kIID_cIGZMessageTarget2Residential) {
        *out = &g_or->agent;
    } else if (iid == GZIID_cIGZSerializable) {
        *out = &g_or->ser;
    } else {
        return false;  // (the reference audit asks other IIDs)
    }
    ++g_or->refs;
    return true;
}
uint32_t __fastcall OrAddRef(void*, void*) { return static_cast<uint32_t>(++g_or->refs); }   // 1
uint32_t __fastcall OrRelease(void*, void*) { return static_cast<uint32_t>(--g_or->refs); }  // 2
bool __fastcall OrWrite(void*, void*, cIGZOStream* s) {                                       // serializable 3
    ++g_or->writes;
    return s->SetUint16(7);
}
uint32_t __fastcall OrClsid(void*, void*) { return kCLSID_Residential; }  // serializable 5
bool __fastcall OrDoMessage(void*, void*, cIGZMessage2*) {                // agent 3
    ++g_or->deliveries;
    return true;
}
int32_t __fastcall OrPopulation(void*, void*) {  // simulator 16
    ++g_or->scalars;
    return 1234;
}
float __fastcall OrGlobalQuotient(void*, void*) {  // simulator 32 / 33 (ST0)
    ++g_or->scalars;
    return 100.0f;
}
uint32_t __fastcall OrGlobalLe(void*, void*) {  // simulator 34 (AL)
    ++g_or->scalars;
    return 0xDEAD0047u;
}
void* __fastcall OrCityRes(void*, void*) { return &g_or->sim; }     // city 68
void* __fastcall OrCityNoRes(void*, void*) { return nullptr; }      // city 68
void* __fastcall OrCityOurs(void*, void*) { return g_or->ourSim; }  // city 68

void InitFakeOrigResidential(FakeOrigResidential& o) {
    g_or = &o;
    for (FakeVtable* vt : {&o.simVt, &o.agentVt, &o.serVt}) {
        vt->slots[0] = reinterpret_cast<void*>(&OrQi);
        vt->slots[1] = reinterpret_cast<void*>(&OrAddRef);
        vt->slots[2] = reinterpret_cast<void*>(&OrRelease);
    }
    o.simVt.slots[16] = reinterpret_cast<void*>(&OrPopulation);
    o.simVt.slots[32] = reinterpret_cast<void*>(&OrGlobalQuotient);
    o.simVt.slots[33] = reinterpret_cast<void*>(&OrGlobalQuotient);
    o.simVt.slots[34] = reinterpret_cast<void*>(&OrGlobalLe);
    o.agentVt.slots[3] = reinterpret_cast<void*>(&OrDoMessage);
    o.serVt.slots[3] = reinterpret_cast<void*>(&OrWrite);
    o.serVt.slots[5] = reinterpret_cast<void*>(&OrClsid);
    o.cityVt.slots[68] = reinterpret_cast<void*>(&OrCityRes);
    o.noResVt.slots[68] = reinterpret_cast<void*>(&OrCityNoRes);
    o.oursVt.slots[68] = reinterpret_cast<void*>(&OrCityOurs);
    o.sim.vtbl = o.simVt.slots.data();
    o.agent.vtbl = o.agentVt.slots.data();
    o.ser.vtbl = o.serVt.slots.data();
    o.city.vtbl = o.cityVt.slots.data();
    o.noRes.vtbl = o.noResVt.slots.data();
    o.ours.vtbl = o.oursVt.slots.data();
    o.other.vtbl = o.agentVt.slots.data();  // an agent, but not the simulator's
}

intptr_t ResidentialShadowOnce(FC4TestSeamFn seam, FakeObject* city, cIGZMessageTarget2* target,
                               uint32_t type = kMsgSimNewDay) {
    FC4ResidentialShadowTestInput in{reinterpret_cast<cISC4City*>(city), target, type, kJulian20000201};
    return seam(kFC4SeamResidentialShadow, &in, nullptr);
}

int ResidentialEnableTest(const std::string& dll, const std::string& ini, const std::string& log) {
    // Off by default; `enable = residential` registers the residential
    // simulator. The residential shadow (it compares against the original) is
    // then inactive.
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    SetEnvironmentVariableA("FC4_RESIDENTIAL_SHADOW", nullptr);
    SetEnvironmentVariableA("FC4_RESIDENTIAL_SHADOW_CELLS", nullptr);
    SetEnvironmentVariableA("FC4_RESIDENTIAL_SEED", nullptr);
    WriteTextFile(ini, "[subsystems]\nenable = residential\nresidential_seed = 42\n[tools]\nresidential_shadow = 1\n"
                       "residential_shadow_cells = 3\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr && seam != nullptr);
    static FakeOrigResidential orig;
    InitFakeOrigResidential(orig);
    SetGamePrecision();  // as the game's PreFrameWorkInit does: the simulator and the shadow must leave it
    std::vector<uint8_t> record;
    if (director != nullptr && seam != nullptr) {
        std::vector<uint32_t> classes;
        director->EnumClassObjects(&CollectClass, &classes);
        // simulator (1) + budget (3) + residential (1); never the game's grid classes.
        CHECK(classes.size() == 5);
        for (uint32_t c : classes) CHECK(c < 0x49B9E600 || c > 0x49B9E60B);
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_Residential) == 1);

        void* obj = nullptr;
        CHECK(director->GetClassObject(kCLSID_Residential, kIID_cISC4ResidentialSimulator, &obj));
        auto* res = static_cast<cFC4ISC4ResidentialSimulator*>(obj);
        if (res != nullptr) {
            // §1: the primary for its IID and cIGZUnknown, the message target
            // and the serializable; nothing else (the out pointer untouched).
            void* p = nullptr;
            CHECK(res->QueryInterface(GZIID_cIGZUnknown, &p) && p == static_cast<void*>(res));
            if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
            p = nullptr;
            CHECK(res->QueryInterface(kIID_cISC4ResidentialSimulator, &p) && p == static_cast<void*>(res));
            if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
            cIGZMessageTarget2* mt = nullptr;
            CHECK(res->QueryInterface(kIID_cIGZMessageTarget2Residential, reinterpret_cast<void**>(&mt)) &&
                  mt != nullptr);
            cIGZSerializable* ser = nullptr;
            CHECK(res->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) && ser != nullptr);
            if (ser != nullptr) CHECK(ser->GetGZCLSID() == kCLSID_Residential);
            p = reinterpret_cast<void*>(0x1234);
            CHECK(!res->QueryInterface(0x12345678, &p) && p == reinterpret_cast<void*>(0x1234));
            CHECK(!res->QueryInterface(0x87DDA3A9, &p) && p == reinterpret_cast<void*>(0x1234));
            CheckResidentialNeutral(res);
            // The record before Init (§9.1: version 7, every reference null).
            if (ser != nullptr) {
                ResidentialSegOStream out;
                CHECK(ser->Write(out));
                record = out.bytes;
                CHECK(record.size() > 2 && record[0] == 7 && record[1] == 0);
                CHECK(out.nullRefs > 0);
            }
            // Without a city Init fails (§4.1 step 1: no demand node); the
            // status lines still come, from the first day.
            CHECK(!res->Init());
            if (mt != nullptr) {
                ResidentialDayMessage day(kMsgSimNewDay, kJulian20000201);
                CHECK(mt->DoMessage(&day));
                ResidentialDayMessage month(kMsgSimNewMonth, kJulian20000201);
                CHECK(mt->DoMessage(&month));
                CheckResidentialThreads(res, mt);
            }
            CheckResidentialNeutral(res);
            // The shadow (test seam op 25) refuses our own simulator.
            orig.ourSim = res;
            CHECK(ResidentialShadowOnce(seam, &orig.ours, reinterpret_cast<cIGZMessageTarget2*>(&orig.agent)) == -1);
            if (ser != nullptr) ser->Release();
            if (mt != nullptr) mt->Release();
            CHECK(res->Shutdown());
            CHECK(res->Release() == 0);
        }
        // Read runs before Init (§9.2): the record written above loads into a
        // new simulator; a truncated one does not.
        for (bool whole : {true, false}) {
            obj = nullptr;
            CHECK(director->GetClassObject(kCLSID_Residential, kIID_cISC4ResidentialSimulator, &obj));
            auto* fresh = static_cast<cFC4ISC4ResidentialSimulator*>(obj);
            if (fresh == nullptr) continue;
            cIGZSerializable* s = nullptr;
            CHECK(fresh->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&s)) && s != nullptr);
            if (s != nullptr && !record.empty()) {
                std::vector<uint8_t> bytes = record;
                if (!whole) bytes.resize(bytes.size() / 2);
                ResidentialSegIStream in(bytes);
                CHECK(s->Read(in) == whole);
                s->Release();
            }
            CHECK(fresh->Release() == 0);  // the final Release shuts a never-initialised simulator down
        }
        // The shadow on fake originals: no city, a city without a residential
        // simulator and an agent that is not the original's are left to the
        // caller (-1, nothing delivered); a SimNewMonth is not taken. The
        // original's agent gets its day exactly once, its record captured
        // before and after (its own Write) and its four scalars read after:
        // a short record is INCOMPLETE (-2, nothing compared).
        cIGZMessageTarget2* agent = reinterpret_cast<cIGZMessageTarget2*>(&orig.agent);
        CHECK(ResidentialShadowOnce(seam, nullptr, agent) == -1);
        CHECK(ResidentialShadowOnce(seam, &orig.noRes, agent) == -1);
        CHECK(orig.deliveries == 0 && orig.writes == 0);
        CHECK(ResidentialShadowOnce(seam, &orig.city, agent, kMsgSimNewMonth) == -1);
        CHECK(orig.deliveries == 0 && orig.writes == 0);
        CHECK(ResidentialShadowOnce(seam, &orig.city, agent) == -2);
        CHECK(orig.deliveries == 1 && orig.writes == 2 && orig.scalars == 4);
        CHECK(ResidentialShadowOnce(seam, &orig.city, reinterpret_cast<cIGZMessageTarget2*>(&orig.other)) == -1);
        CHECK(orig.deliveries == 1 && orig.writes == 2);
        CHECK(orig.refs == 1);  // every QueryInterface released
        CheckCw("residential shadow");
    }
    FreeLibrary(hd);
    DeleteFileA(ini.c_str());
    const std::string text = ReadFile(log);
    CHECK(text.find("subsystem 'residential': enabled (1 class replaced) by FreeCity4.ini (enable=)") !=
          std::string::npos);
    CHECK(text.find("cFC4ResidentialSimulator(") != std::string::npos);
    CHECK(CountOf(text, ", session seed 0x0000002A (residential_seed)") == 3);
    CHECK(text.find("residential Write -> 1") != std::string::npos);
    CHECK(text.find("residential: no city; services not acquired") != std::string::npos);
    CHECK(text.find("residential Init FAILED: init=0") != std::string::npos);
    // The month line from the first day after Init, and at SimNewMonth.
    CHECK(CountOf(text, "residential month 2000-02-01: pop=0 (R$ 0 R$$ 0 R$$$ 0) EQ=") == 2);
    CHECK(text.find("residential Read -> 1") != std::string::npos);
    CHECK(text.find("residential Read: init=0 started=1") != std::string::npos);
    CHECK(text.find("residential Read -> 0") != std::string::npos);
    CHECK(text.find("residential Shutdown: 1 days, 1 months, 0 cycles, 1 off-main messages") != std::string::npos);
    CHECK(text.find("cFC4ResidentialSimulator destroyed (1 days, 1 months, 0 cycles, 1 off-main messages)") !=
          std::string::npos);
    CHECK(CountOf(text, "cFC4ResidentialSimulator destroyed (0 days, 0 months, 0 cycles, 0 off-main messages)") == 2);
    CHECK(text.find("residential shadow: requested (residential_shadow from FreeCity4.ini) but INACTIVE") !=
          std::string::npos);
    CHECK(text.find("residential shadow: ON") == std::string::npos);
    CHECK(text.find("residential shadow: inactive: the city's residential simulator is FreeCity4's own") !=
          std::string::npos);
    CHECK(text.find("residential shadow: comparing with the original residential simulator") != std::string::npos);
    CHECK(text.find("differing cells listed per grid: 3") != std::string::npos);
    CHECK(text.find("residential shadow 2000-02-01: INCOMPLETE (before: ") != std::string::npos);
    CHECK(text.find("structured exception") == std::string::npos);
    CHECK(text.find("config: WARNING unknown key") == std::string::npos);
    CHECK(text.find("WARNING: released last reference") == std::string::npos);
    CHECK(text.find("WARNING: over-release") == std::string::npos);
    CHECK(text.find("streams: WARNING") == std::string::npos);
    if (g_failures == 0) std::printf("plugin residential enable test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}

// ---- development (building-development.md, growth-developer.md): the BDS without a city ----------------
constexpr uint32_t kCLSID_Development = 0x8990BE7A;
constexpr uint32_t kIID_cISC4BuildingDevelopmentSimulator = 0x07813EAE;
constexpr uint32_t kIID_cIGZMessageTargetDev = 0xE98B2F57;
constexpr uint32_t kIID_cIGZMessageTarget2Dev = 0x452294AA;

// DG35 (§4.9.2): a retaining wall (primary interface: slot 3 AsOccupant,
// slot 11 GetRetainingWallType) whose occupant sub-object (+0x04) answers QI
// 0x295D1162 with the wall, AddRef'd; the wall's own QI (slot 0, probed by
// the reference audit) answers nothing. Other slots trap.
struct FakeWall {
    void** vtbl;
    void** occVtbl;
    int refs = 1;  // the caller's
    int qis = 0;
    int typeCalls = 0;
    bool isWall = true;
    uint32_t type = 7;
};
FakeWall* g_wall = nullptr;
FakeVtable g_wallVt, g_wallOccVt;
bool __fastcall WallOccQi(void*, void*, uint32_t iid, void** out) {  // occupant 0
    ++g_wall->qis;
    if (out == nullptr || !g_wall->isWall || iid != 0x295D1162u) return false;
    *out = g_wall;
    ++g_wall->refs;
    return true;
}
bool __fastcall WallQi(void*, void*, uint32_t, void** out) {  // wall 0: the reference audit's class probe
    if (out != nullptr) *out = nullptr;
    return false;
}
uint32_t __fastcall WallAddRef(void*, void*) { return static_cast<uint32_t>(++g_wall->refs); }   // 1
uint32_t __fastcall WallRelease(void*, void*) { return static_cast<uint32_t>(--g_wall->refs); }  // 2
void* __fastcall WallAsOccupant(FakeWall* w, void*) { return &w->occVtbl; }                      // wall 3
uint32_t __fastcall WallType(FakeWall* w, void*) {                                               // wall 11
    ++w->typeCalls;
    return w->type;
}

// The answers without a city or catalogue (§7.1), through the compiled
// slots. The AL-only slots 42 / 43 are also called raw: the whole of EAX
// must be the key (zero-extended).
void CheckDevelopmentNeutral(cFC4ISC4BuildingDevelopmentSimulator* b, uint32_t t) {
    void** vt = *reinterpret_cast<void***>(b);
    uint32_t n = 5;
    CHECK(b->GetAllBuildingTypesArray(nullptr, n) == 0 && n == 0);  // 12
    CHECK(!b->IsBuildingTypeValid(t) && !b->IsBuildingFamilyTypeValid(t));
    CHECK(b->IsBuildingInFamily(7, 7) && !b->IsBuildingInFamily(7, 8));  // 20: b == f
    cGZPersistResourceKey key(1, 2, 3);
    CHECK(!b->GetBuildingKeyFromType(t, key) && !b->GetFoundationKeyFromType(t, key) &&
          !b->GetLotRetainingWallKeyFromType(t, key));
    CHECK(b->GetDeveloperType(1, 2) == 0x1020 && b->GetWealth(0x1020) == 2);  // 30 / 31
    CHECK(b->GetDeveloper(0x1020) == nullptr && b->GetDeveloperOf(1, 2) == nullptr);
    CHECK(b->GetDevelopers() != nullptr);  // 34: the map object (empty)
    CHECK(b->GetTractDeveloper() == nullptr && b->GetLotDeveloper() == nullptr);
    CHECK(b->GetBuildingMap() == nullptr);
    CHECK(b->GetSpecialBuildings() == nullptr);  // no allocator service: no list
    CHECK(b->GetMaxCapacity(1, 1, 1) == 0.0f);
    CHECK(b->GetCostOfFoundationDemolition(nullptr, 0) == 0);
    float pos[3] = {0.0f, 0.0f, 0.0f};
    // 23 (§4.9.2, DG6): no foundation class without the game, so nothing is created: false.
    CHECK(!b->AddNewFoundationOccupantAt(1, 0, pos, 0.0f, 0.0f, 0, 0.0f));
    CHECK(pos[1] == 0.0f);
    // 26 / 29: a null occupant [choice] and a type outside the (empty) key maps: false.
    CHECK(!b->LoadFoundationProperties(nullptr, 1) && !b->LoadLotRetainingWallProperties(nullptr, 1));
    // 29, DG35: type 0 takes the wall's type, reached from its occupant
    // token by QI 0x295D1162 (released after); the type is not in the
    // (empty) key map: false. An occupant that is not a wall gives type 0.
    {
        FakeWall wall;
        g_wall = &wall;
        g_wallVt.slots[1] = g_wallOccVt.slots[1] = reinterpret_cast<void*>(&WallAddRef);
        g_wallVt.slots[2] = g_wallOccVt.slots[2] = reinterpret_cast<void*>(&WallRelease);
        g_wallVt.slots[3] = reinterpret_cast<void*>(&WallAsOccupant);
        g_wallVt.slots[11] = reinterpret_cast<void*>(&WallType);
        g_wallOccVt.slots[0] = reinterpret_cast<void*>(&WallOccQi);
        g_wallVt.slots[0] = reinterpret_cast<void*>(&WallQi);
        wall.vtbl = g_wallVt.slots.data();
        wall.occVtbl = g_wallOccVt.slots.data();
        auto* w = reinterpret_cast<cISC4LotRetainingWallOccupant*>(&wall);
        CHECK(!b->LoadLotRetainingWallProperties(w, 0));
        CHECK(wall.qis == 1 && wall.typeCalls == 1 && wall.refs == 1);
        wall.isWall = false;
        CHECK(!b->LoadLotRetainingWallProperties(w, 0));
        CHECK(wall.qis == 2 && wall.typeCalls == 1 && wall.refs == 1);
        g_wall = nullptr;
    }
    CHECK(!b->CheatIgnorePlumbing() && !b->CheatIgnorePower() && !b->CheatIgnoreCaps() && !b->CheatDebugGrowth() &&
          !b->CheatCoffeeBar());
    // 42 / 43: 4p + w + 1, in EAX.
    using KeyOfFn = uint32_t(__fastcall*)(void*, void*, uint32_t, uint32_t);
    using KeyFn = uint32_t(__fastcall*)(void*, void*, uint32_t);
    CHECK(b->GetBuildingMapKeyOf(1, 2) == 7);
    CHECK(reinterpret_cast<KeyOfFn>(vt[42])(b, nullptr, 1, 2) == 7);
    CHECK(reinterpret_cast<KeyOfFn>(vt[42])(b, nullptr, 3, 3) == 16);
    // 40 / 39 / 43: an unknown type has no profile until 39 inserts a zero one (key 1).
    CHECK(b->GetBuildingProfilePtr(t) == nullptr);
    FC4BuildingProfile& p = b->GetBuildingProfile(t);
    CHECK(&p == b->GetBuildingProfilePtr(t) && p.purpose == 0 && p.wealth == 0 && p.capacity == 0);
    CHECK(reinterpret_cast<KeyFn>(vt[43])(b, nullptr, t) == 1);
    // 37 / 38: "" inserted for an unknown type, the same object each time.
    cIGZString* name = b->GetExemplarName(t + 1);
    CHECK(name != nullptr && name->Strlen() == 0 && name == b->GetExemplarName(t + 1));
    cIGZString* bname = b->GetBuildingName(t + 1);
    CHECK(bname != nullptr && bname != name && bname->Strlen() == 0);
}

// Getters and a message from another thread (fc4.h: one process-wide lock;
// the other thread waits at most 20 ms).
void CheckDevelopmentThreads(cFC4ISC4BuildingDevelopmentSimulator* b, cIGZMessageTarget2* mt) {
    int32_t freeRes = 9;
    bool delivered = false, cheat = true;
    cISC4GrowthDeveloper* dev = reinterpret_cast<cISC4GrowthDeveloper*>(1);
    std::thread t([&] {
        freeRes = b->GetFreeConstructionResources();
        cheat = b->CheatCoffeeBar();
        dev = b->GetDeveloper(0x1010);
        TypeMessage day(kMsgSimNewDay);
        delivered = mt->DoMessage(&day);
    });
    t.join();
    CHECK(freeRes == b->GetFreeConstructionResources() && !cheat && dev == nullptr && delivered);
}

// ---- the growth developer (growth-developer.md) without a city ----------------------------------------
constexpr uint32_t kCLSID_GrowthDeveloper = 0x0990C138;
constexpr uint32_t kIID_cISC4GrowthDeveloperDev = 0xE7CDD662;
constexpr uint32_t kMsgSimIdleDev = 0x6695681A;

// QueryInterface (§1: +4, +8), the §2 values through the compiled slots
// (10 and 28 raw: 0/1 in the whole of EAX), Init refused without a holder
// or a city, the v1 record (§9), messages, another thread, and Read before
// Init.
void CheckGrowthDeveloper(cIGZCOMDirector* director) {
    void* obj = nullptr;
    CHECK(director->GetClassObject(kCLSID_GrowthDeveloper, kIID_cISC4GrowthDeveloperDev, &obj));
    auto* g = static_cast<cFC4ISC4GrowthDeveloper*>(obj);
    if (g == nullptr) return;
    void* p = nullptr;
    CHECK(g->QueryInterface(GZIID_cIGZUnknown, &p) && p == static_cast<void*>(g));
    if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
    cIGZMessageTarget2* mt = nullptr;
    CHECK(g->QueryInterface(kIID_cIGZMessageTarget2Dev, reinterpret_cast<void**>(&mt)) &&
          static_cast<void*>(mt) == static_cast<void*>(reinterpret_cast<char*>(g) + 4));
    cIGZSerializable* ser = nullptr;
    CHECK(g->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) &&
          static_cast<void*>(ser) == static_cast<void*>(reinterpret_cast<char*>(g) + 8));
    if (ser != nullptr) CHECK(ser->GetGZCLSID() == kCLSID_GrowthDeveloper);
    p = reinterpret_cast<void*>(0x1234);
    CHECK(!g->QueryInterface(kIID_cISC4BuildingDevelopmentSimulator, &p) && p == reinterpret_cast<void*>(0x1234));
    // §2: the constructor's values.
    CHECK(g->GetType() == 0 && g->GetSubType() == 0 && g->GetPurpose() == 0 && g->GetWealth() == 0);
    CHECK(g->GetPowerSupplyCap() == 0 && g->GetWaterSupplyCap() == 3 && g->GetFireCoverageCap() == 3 &&
          g->GetParkCap() == 6);
    CHECK(g->GetDesirabilityThreshold() == 80 && g->GetMonopolyUse() == 1 && g->GetTotalCapacity() == 0);
    CHECK(g->GetSourceDemand() == nullptr && g->GetDesirabilityMap() == nullptr && g->GetCapacityMap() == nullptr &&
          g->GetPopulationMap() == nullptr && g->GetMaxCapacityMap() == nullptr);
    CHECK(g->GetGrowthList().size() == 0 && g->GetDeclineList().size() == 0 && g->GetStylePercentages() != nullptr);
    int32_t mn = 0, mx = 0;
    g->GetSubjectiveThresholds(0, mn, mx);  // no vectors: -4 / 20
    CHECK(mn == -4 && mx == 20);
    CHECK(g->GetAbandonmentThreshold(2) == 0.0f);
    FC4ConstructionProfile prof{};
    prof.buildingType = 0x77;
    prof.x = 5;
    g->GetConstructionInfo(prof);  // §6.2: the info fields only
    CHECK(prof.buildingType == 0x77 && prof.x == 5 && prof.zone == 0);
    void** vt = *reinterpret_cast<void***>(g);
    using BoolFn = uint32_t(__fastcall*)(void*, void*);
    CHECK(reinterpret_cast<BoolFn>(vt[10])(g, nullptr) == 1);  // state 0 < 17
    CHECK(reinterpret_cast<BoolFn>(vt[28])(g, nullptr) == 1);  // state 0
    g->StartMapUpdate();  // state 1: not initialised, so it stays there (§3.2)
    CHECK(reinterpret_cast<BoolFn>(vt[28])(g, nullptr) == 0 && g->GrowthAndDeclineListsAreValid());
    CHECK(!g->Init(nullptr));  // [choice] a null holder is refused
    // §9: version 1 first.
    std::vector<uint8_t> record;
    if (ser != nullptr) {
        ResidentialSegOStream out;
        CHECK(ser->Write(out));
        record = out.bytes;
        CHECK(record.size() > 10 && record[0] == 1 && record[1] == 0);
    }
    if (mt != nullptr) {
        ResidentialDayMessage idle(kMsgSimIdleDev, kJulian20000201);
        CHECK(mt->DoMessage(&idle));
        ResidentialDayMessage day(kMsgSimNewDay, kJulian20000201);
        CHECK(mt->DoMessage(&day));  // elapsed 0: never blocks
        TypeMessage other(0x12345678);
        CHECK(mt->DoMessage(&other));
    }
    // Another thread takes the lock (nothing holds it) and reads the same values.
    int32_t threshold = 0;
    uint8_t park = 0;
    std::thread t([&] {
        threshold = g->GetDesirabilityThreshold();
        park = g->GetParkCap();
    });
    t.join();
    CHECK(threshold == 80 && park == 6);
    if (ser != nullptr) ser->Release();
    if (mt != nullptr) mt->Release();
    CHECK(g->Shutdown());
    CHECK(g->Release() == 0);
    // Read runs before Init: the record loads into a new developer; a
    // truncated one does not.
    for (bool whole : {true, false}) {
        obj = nullptr;
        CHECK(director->GetClassObject(kCLSID_GrowthDeveloper, kIID_cISC4GrowthDeveloperDev, &obj));
        auto* fresh = static_cast<cFC4ISC4GrowthDeveloper*>(obj);
        if (fresh == nullptr) continue;
        cIGZSerializable* s = nullptr;
        CHECK(fresh->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&s)) && s != nullptr);
        if (s != nullptr && !record.empty()) {
            std::vector<uint8_t> bytes = record;
            if (!whole) bytes.resize(5);
            ResidentialSegIStream in(bytes);
            CHECK(s->Read(in) == whole);
            s->Release();
        }
        CHECK(fresh->Release() == 0);
    }
}

int DevelopmentEnableTest(const std::string& dll, const std::string& ini, const std::string& log) {
    // Off by default; `enable = development` registers the building
    // development simulator and the growth developers.
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    SetEnvironmentVariableA("FC4_DEVELOPMENT_SEED", nullptr);
    SetEnvironmentVariableA("FC4_DEVELOPMENT_GROWTH_ATTEMPTS_PER_DAY", nullptr);
    SetEnvironmentVariableA("FC4_DEVELOPMENT_SHADOW", nullptr);
    WriteTextFile(ini, "[subsystems]\nenable = development\ndevelopment_seed = 42\n"
                       "development_growth_attempts_per_day = 3\n[tools]\ndevelopment_shadow = 1\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr);
    SetGamePrecision();  // as the game's PreFrameWorkInit does: the simulator must leave it
    std::vector<uint8_t> record;
    if (director != nullptr) {
        std::vector<uint32_t> classes;
        director->EnumClassObjects(&CollectClass, &classes);
        // simulator (1) + budget (3) + development (2).
        CHECK(classes.size() == 6);
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_Development) == 1);
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_GrowthDeveloper) == 1);

        void* obj = nullptr;
        CHECK(director->GetClassObject(kCLSID_Development, kIID_cISC4BuildingDevelopmentSimulator, &obj));
        auto* bds = static_cast<cFC4ISC4BuildingDevelopmentSimulator*>(obj);
        if (bds != nullptr) {
            // §1.1: the primary for its IID and cIGZUnknown, the two message
            // targets (+4, +8) and the serializable (+0xC); nothing else.
            void* p = nullptr;
            CHECK(bds->QueryInterface(GZIID_cIGZUnknown, &p) && p == static_cast<void*>(bds));
            if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
            p = nullptr;
            CHECK(bds->QueryInterface(kIID_cISC4BuildingDevelopmentSimulator, &p) && p == static_cast<void*>(bds));
            if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
            p = nullptr;
            CHECK(bds->QueryInterface(kIID_cIGZMessageTargetDev, &p) &&
                  p == static_cast<void*>(reinterpret_cast<char*>(bds) + 4));
            if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
            cIGZMessageTarget2* mt = nullptr;
            CHECK(bds->QueryInterface(kIID_cIGZMessageTarget2Dev, reinterpret_cast<void**>(&mt)) &&
                  static_cast<void*>(mt) == static_cast<void*>(reinterpret_cast<char*>(bds) + 8));
            cIGZSerializable* ser = nullptr;
            CHECK(bds->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) &&
                  static_cast<void*>(ser) == static_cast<void*>(reinterpret_cast<char*>(bds) + 0xC));
            if (ser != nullptr) CHECK(ser->GetGZCLSID() == kCLSID_Development);
            p = reinterpret_cast<void*>(0x1234);
            CHECK(!bds->QueryInterface(0x12345678, &p) && p == reinterpret_cast<void*>(0x1234));
            CheckDevelopmentNeutral(bds, 0x5150);
            // The record before Init (§6: version 4).
            if (ser != nullptr) {
                ResidentialSegOStream out;
                CHECK(ser->Write(out));
                record = out.bytes;
                CHECK(record.size() > 2 && record[0] == 4 && record[1] == 0);
            }
            // Without a city Init runs (no catalogue, no services); the
            // status lines still come.
            CHECK(bds->Init());
            if (mt != nullptr) {
                ResidentialDayMessage day(kMsgSimNewDay, kJulian20000201);
                CHECK(mt->DoMessage(&day));
                ResidentialDayMessage month(kMsgSimNewMonth, kJulian20000201);
                CHECK(mt->DoMessage(&month));
                CheckDevelopmentThreads(bds, mt);
            }
            CheckDevelopmentNeutral(bds, 0x5160);  // the catalogue mirror is process-wide: new types
            if (ser != nullptr) ser->Release();
            if (mt != nullptr) mt->Release();
            CHECK(bds->Shutdown());
            CHECK(bds->Release() == 0);
        }
        // Read runs before Init (§6): the record written above loads into a
        // new simulator; a truncated one does not.
        for (bool whole : {true, false}) {
            obj = nullptr;
            CHECK(director->GetClassObject(kCLSID_Development, kIID_cISC4BuildingDevelopmentSimulator, &obj));
            auto* fresh = static_cast<cFC4ISC4BuildingDevelopmentSimulator*>(obj);
            if (fresh == nullptr) continue;
            cIGZSerializable* s = nullptr;
            CHECK(fresh->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&s)) && s != nullptr);
            if (s != nullptr && !record.empty()) {
                std::vector<uint8_t> bytes = record;
                if (!whole) bytes.resize(bytes.size() / 2);
                ResidentialSegIStream in(bytes);
                CHECK(s->Read(in) == whole);
                s->Release();
            }
            CHECK(fresh->Release() == 0);  // the final Release shuts a never-initialised simulator down
        }
        CheckGrowthDeveloper(director);
        // The development shadow's record bridge (seam op 34, mode 1): one of
        // our developers' record read into a new read-only core.
        auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
        CHECK(seam != nullptr);
        if (seam != nullptr) {
            void* g = nullptr;
            CHECK(director->GetClassObject(kCLSID_GrowthDeveloper, kIID_cISC4GrowthDeveloperDev, &g) && g != nullptr);
            if (g != nullptr) {
                FC4DevelopmentShadowTestInput in{};
                in.mode = 1;
                in.developer = static_cast<cIGZUnknown*>(g);
                CHECK(seam(kFC4SeamDevelopmentShadow, &in, nullptr) == 1);
                CHECK(static_cast<cIGZUnknown*>(g)->Release() == 0);
            }
            FC4DevelopmentShadowTestInput none{};
            none.mode = 1;
            CHECK(seam(kFC4SeamDevelopmentShadow, &none, nullptr) == -1);
        }
        CheckCw("development");
    }
    FreeLibrary(hd);
    DeleteFileA(ini.c_str());
    const std::string text = ReadFile(log);
    CHECK(text.find("subsystem 'development': enabled (2 classes replaced) by FreeCity4.ini (enable=)") !=
          std::string::npos);
    CHECK(text.find("development: process statics ") != std::string::npos);
    CHECK(text.find(", session seed 0x0000002A (development_seed)") != std::string::npos);
    CHECK(CountOf(text, "cFC4BuildingDevelopmentSimulator(") == 3);
    CHECK(CountOf(text, "daily growth budget 3") == 3);
    CHECK(text.find("development Write -> 1") != std::string::npos);
    CHECK(text.find("development: no city; services not acquired") != std::string::npos);
    CHECK(text.find("development Init: init=1") != std::string::npos);
    CHECK(text.find("development month 2000-02-01: developers=0 built=0") != std::string::npos);
    CHECK(text.find("development Read -> 1") != std::string::npos);
    CHECK(text.find("development Read: init=0 loaded=1") != std::string::npos);
    CHECK(text.find("development Read -> 0") != std::string::npos);
    CHECK(text.find("development Shutdown: init=0") != std::string::npos);
    CHECK(CountOf(text, "cFC4BuildingDevelopmentSimulator destroyed") == 3);
    // Slot 23 reached the core, which asked for a foundation occupant (none without the game); slots 26 / 29
    // refused a null occupant. The month and state lines count the calls (calls/successes).
    CHECK(text.find("development: WARNING could not create a foundation occupant (class 0x49C05C8F)") !=
          std::string::npos);
    CHECK(text.find("lotUpdates=0 foundations=1/0 foundationLoads=1/0 wallLoads=3/0") != std::string::npos);
    // The growth developers: one created and initialised (refused), two read.
    CHECK(text.find("development: growth statics ") != std::string::npos);
    CHECK(CountOf(text, "cFC4GrowthDeveloper(") == 8);  // created and destroyed (one for the shadow's bridge)
    CHECK(text.find("growth 0x00000000 Init FAILED: init=0") != std::string::npos);
    CHECK(text.find("growth 0x00000000 Write -> 1") != std::string::npos);
    CHECK(text.find("growth 0x00000000 Read: init=0") != std::string::npos);
    CHECK(text.find("growth 0x00000000 Read FAILED") != std::string::npos);
    CHECK(CountOf(text, ") destroyed (0 updates, 0 calls answered without the lock)") == 4);
    // The development shadow: inactive with ours; its record bridge.
    CHECK(text.find("development shadow: requested (development_shadow from FreeCity4.ini) but INACTIVE: it needs "
                    "FreeCity4's simulator and the original development unit (do not enable development); here the "
                    "simulator is ours and development is ours") != std::string::npos);
    CHECK(text.find("development shadow: ON") == std::string::npos);
    CHECK(text.find("development shadow test: record v1 (30 bytes) phantom=0 total=0 -> read 1 (copies 0, failed 0; "
                    "core total=0 phantom=0)") != std::string::npos);
    // The catalogue mirror: built once per process, after the first Init loaded the catalogue.
    CHECK(CountOf(text, "development catalogue: 0 building types, 0 families") == 1);
    CHECK(CountOf(text, "development: the static-data cleanup is not registered") == 1);
    CHECK(text.find("structured exception") == std::string::npos);
    CHECK(text.find("config: WARNING unknown key") == std::string::npos);
    CHECK(text.find("WARNING: released last reference") == std::string::npos);
    CHECK(text.find("WARNING: over-release") == std::string::npos);
    CHECK(text.find("streams: WARNING") == std::string::npos);
    if (g_failures == 0) std::printf("plugin development enable test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}

// ---- the development shadow (test seam op 34) ---------------------------------------------------------

// An original growth developer as the shadow meets it (its agent answers
// QueryInterface 0xE7CDD662): slot 5 is the target zone, slot 6 the type.
class FakeGrowthDeveloper final : public cFC4ISC4GrowthDeveloper {
public:
    FakeGrowthDeveloper(int32_t zone, uint32_t type) : zone_(zone), type_(type) {}
    bool QueryInterface(uint32_t riid, void** pp) override {
        if (riid != 0xE7CDD662u && riid != GZIID_cIGZUnknown) return false;
        *pp = static_cast<cFC4ISC4GrowthDeveloper*>(this);
        AddRef();
        return true;
    }
    uint32_t AddRef() override { return ++refs; }
    uint32_t Release() override { return --refs; }  // stack object
    bool Init(cISCPropertyHolder*) override { return true; }
    bool Shutdown() override { return true; }
    int32_t GetType() override { return zone_; }
    uint32_t GetSubType() override { return type_; }
    uint32_t GetPurpose() override { return 1; }
    uint32_t GetWealth() override { return 1; }
    void GetConstructionInfo(FC4ConstructionProfile&) override {}
    bool GrowthAndDeclineListsAreValid() override { return false; }
    fc4::GameVector<FC4GrowthItem>& GetGrowthList() override { return List(); }
    fc4::GameVector<FC4GrowthItem>& GetDeclineList() override { return List(); }
    cISC4SimGrid<int16_t>* GetDesirabilityMap() override { return nullptr; }
    cISC4SimGrid<uint16_t>* GetCapacityMap() override { return nullptr; }
    cISC4SimGrid<int16_t>* GetPopulationMap() override { return nullptr; }
    cISC4SimGrid<uint16_t>* GetMaxCapacityMap() override { return nullptr; }
    int32_t GetTotalCapacity() override { return 0; }
    cISC4Demand* GetSourceDemand() override { return nullptr; }
    uint8_t GetPowerSupplyCap() override { return 0; }
    uint8_t GetWaterSupplyCap() override { return 0; }
    uint8_t GetFireCoverageCap() override { return 0; }
    uint8_t GetParkCap() override { return 0; }
    int32_t GetDesirabilityThreshold() override { return 0; }
    float GetAbandonmentThreshold(int32_t) override { return 0.0f; }
    int32_t GetMonopolyUse() override { return 0; }
    fc4::GameMapObject* GetStylePercentages() override { return nullptr; }
    void StartMapUpdate() override {}
    bool IsMapUpdateFinished() override { return true; }
    int16_t EstimateDesirability(int32_t, int32_t) override { return 0; }
    void GetSubjectiveThresholds(int32_t, int32_t& lo, int32_t& hi) override { lo = hi = 0; }
    uint32_t refs = 1;

private:
    int32_t zone_;
    uint32_t type_;
    // An empty game vector (three null pointers) that is never constructed or
    // destroyed: the shadow does not read it, and its destructor would need
    // the game allocator.
    alignas(void*) unsigned char list_[3 * sizeof(void*)] = {};
    fc4::GameVector<FC4GrowthItem>& List() { return *reinterpret_cast<fc4::GameVector<FC4GrowthItem>*>(list_); }
};

class FakeCohort;

// An exemplar's (or a cohort's) property holder as the game's behaves:
// EnumProperties (slot 20) lists the OWN properties only; GetProperty (slot
// 9) resolves the parent cohorts when `effective` (the exemplar's own
// implementation does); QueryInterface 0xA52160F5 answers the exemplar.
class ChainHolder final : public FakeHolder {
public:
    bool QueryInterface(uint32_t riid, void** pp) override {
        if (riid == GZIID_cISCResExemplar && exemplar != nullptr) {
            *pp = exemplar;
            exemplar->AddRef();
            return true;
        }
        return FakeHolder::QueryInterface(riid, pp);
    }
    cISCProperty* GetProperty(uint32_t id) const override;
    bool EnumProperties(EnumFn fn, void* data) const override {
        for (uint32_t id : own) fn(FakeHolder::GetProperty(id), data);
        return true;
    }
    template <class T>
    void SetOwn(uint32_t id, T value) {
        Set(id, value);
        if (std::find(own.begin(), own.end(), id) == own.end()) own.push_back(id);
    }
    std::vector<uint32_t> own;
    cISCResExemplar* exemplar = nullptr;  // the owner, for an exemplar's holder
    FakeCohort* parent = nullptr;          // the cohort whose properties GetProperty also resolves
    bool effective = true;
};

class FakeCohort final : public cISCResExemplarCohort {
public:
    bool QueryInterface(uint32_t riid, void** pp) override {
        if (riid != GZIID_cISCResExemplarCohort && riid != GZIID_cIGZUnknown) return false;
        *pp = static_cast<cISCResExemplarCohort*>(this);
        AddRef();
        return true;
    }
    uint32_t AddRef() override { return ++refs; }
    uint32_t Release() override { return --refs; }
    bool Write(cIGZOStream&) override { return false; }
    bool Read(cIGZIStream&) override { return false; }
    uint32_t GetGZCLSID() override { return 0; }
    cISCResExemplarCohort* GetParent() override { return parent; }
    bool SetParent(cISCResExemplarCohort*, bool) override { return false; }
    cISCPropertyHolder* AsISCPropertyHolder() override { return holder.AsSdk(); }
    cISCPropertyHolder* AsISCPropertyHolder() const override { return const_cast<ChainHolder&>(holder).AsSdk(); }
    bool IsPropertyLocal(uint32_t id) override { return holder.HasProperty(id); }
    bool GetKey(cGZPersistResourceKey&) override { return false; }
    bool SetKey(cGZPersistResourceKey const&) override { return false; }
    bool CompactProperties() override { return true; }
    ChainHolder holder;
    FakeCohort* parent = nullptr;
    uint32_t refs = 1;
};

cISCProperty* ChainHolder::GetProperty(uint32_t id) const {
    if (cISCProperty* p = FakeHolder::GetProperty(id)) return p;
    if (!effective) return nullptr;
    for (const FakeCohort* c = parent; c != nullptr; c = c->parent) {
        if (cISCProperty* p = c->holder.FakeHolder::GetProperty(id)) return p;
    }
    return nullptr;
}

class FakeExemplar final : public cISCResExemplar {
public:
    FakeExemplar() { holder.exemplar = this; }
    bool QueryInterface(uint32_t riid, void** pp) override {
        if (riid != GZIID_cISCResExemplar && riid != GZIID_cIGZUnknown) return false;
        *pp = static_cast<cISCResExemplar*>(this);
        AddRef();
        return true;
    }
    uint32_t AddRef() override { return ++refs; }
    uint32_t Release() override { return --refs; }
    bool Write(cIGZOStream&) override { return false; }
    bool Read(cIGZIStream&) override { return false; }
    uint32_t GetGZCLSID() override { return 0; }
    cISCPropertyHolder* AsISCPropertyHolder() override { return holder.AsSdk(); }
    cISCPropertyHolder* AsISCPropertyHolder() const override { return const_cast<ChainHolder&>(holder).AsSdk(); }
    bool IsPropertyLocal(uint32_t id) override { return holder.HasProperty(id); }
    cISCResExemplarCohort* GetCohort() override { return cohort; }
    bool SetCohort(cISCResExemplarCohort*, bool) override { return false; }
    bool GetKey(cGZPersistResourceKey&) override { return false; }
    bool SetKey(cGZPersistResourceKey const&) override { return false; }
    bool CompactProperties() override { return true; }
    ChainHolder holder;
    FakeCohort* cohort = nullptr;
    uint32_t refs = 1;
};


int DevelopmentShadowIniTest(const std::string& dll, const std::string& ini, const std::string& log) {
    // `development_shadow = 1` with FreeCity4's simulator and the original
    // development unit (the defaults): ON, and the shadow's list comparison.
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    SetEnvironmentVariableA("FC4_DEVELOPMENT_SHADOW", nullptr);
    WriteTextFile(ini, "[tools]\ndevelopment_shadow = 1\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr && seam != nullptr);
    if (director != nullptr && seam != nullptr) {
        std::vector<uint32_t> classes;
        director->EnumClassObjects(&CollectClass, &classes);
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_GrowthDeveloper) == 0);
        const int32_t two[] = {1, 2, 1, 3, 4, 2};
        const int32_t swapped[] = {3, 4, 2, 1, 2, 1};
        const int32_t three[] = {1, 2, 1, 3, 4, 2, 5, 6, 1};
        FC4DevelopmentShadowTestInput in{};
        in.mode = 0;
        in.original = two;
        in.originalCount = 2;
        in.ours = two;
        in.oursCount = 2;
        CHECK(seam(kFC4SeamDevelopmentShadow, &in, nullptr) == 0);
        in.ours = swapped;
        CHECK(seam(kFC4SeamDevelopmentShadow, &in, nullptr) == 2);  // both indices
        in.asSet = 1;
        CHECK(seam(kFC4SeamDevelopmentShadow, &in, nullptr) == 0);  // the industrial decline order
        in.ours = three;
        in.oursCount = 3;
        CHECK(seam(kFC4SeamDevelopmentShadow, &in, nullptr) == 2);  // the count, one only in ours
        in.asSet = 0;
        CHECK(seam(kFC4SeamDevelopmentShadow, &in, nullptr) == 1);  // the count
        in.ours = nullptr;
        CHECK(seam(kFC4SeamDevelopmentShadow, &in, nullptr) == -1);
        CHECK(seam(kFC4SeamDevelopmentShadow, nullptr, nullptr) == -1);
        FC4DevelopmentShadowTestInput other{};
        other.mode = 9;
        CHECK(seam(kFC4SeamDevelopmentShadow, &other, nullptr) == -1);

        // Mode 2: the shadow names an original developer by its type (slot 6,
        // 0x1010 ... 0x4400), not its target zone (slot 5): run 101 loaded the
        // exemplars 0x00000001..3 and compared nothing.
        FakeGrowthDeveloper r1(1, 0x1010), i8(3, 0x4400);
        FC4DevelopmentShadowTestInput dev{};
        dev.mode = 2;
        dev.developer = &r1;
        CHECK(seam(kFC4SeamDevelopmentShadow, &dev, nullptr) == 0x1010);
        CHECK(r1.refs == 1);  // the shadow's reference is dropped with the entry
        dev.developer = &i8;
        CHECK(seam(kFC4SeamDevelopmentShadow, &dev, nullptr) == 0x4400);
        CHECK(i8.refs == 1);
        dev.developer = nullptr;
        CHECK(seam(kFC4SeamDevelopmentShadow, &dev, nullptr) == -1);

        // Mode 3: an exemplar's effective properties (data-formats §4.4). The
        // game's EnumProperties lists own properties only; a building whose
        // ExemplarType, purpose and families sit in its cohorts must still
        // reach the core (run 102: 91 of 744 building types).
        FakeCohort grand, parent;
        parent.parent = &grand;
        grand.holder.SetOwn<uint32_t>(0x27812870, 0x5000);  // families, only in the grandparent
        grand.holder.SetOwn<uint32_t>(0x27812833, 9);       // purpose, overridden by the parent
        parent.holder.SetOwn<uint32_t>(0x10, 2);            // ExemplarType: a building
        parent.holder.SetOwn<uint32_t>(0x27812833, 1);      // purpose
        parent.holder.parent = &grand;
        FakeExemplar ex;
        ex.cohort = &parent;
        ex.holder.parent = &parent;
        ex.holder.SetOwn<uint32_t>(0x21, 0x12345);           // own
        ex.holder.SetOwn<uint32_t>(0x27812833, 2);           // own purpose wins over both cohorts
        FC4DevelopmentShadowTestInput hp{};
        hp.mode = 3;
        hp.holder = reinterpret_cast<const cISCPropertyHolder*>(ex.holder.AsSdk());
        CHECK(seam(kFC4SeamDevelopmentShadow, &hp, nullptr) == 4);
        ex.holder.effective = false;  // an exemplar whose GetProperty does not resolve: read from the cohort
        CHECK(seam(kFC4SeamDevelopmentShadow, &hp, nullptr) == 4);
        CHECK(ex.refs == 1 && parent.refs == 1 && grand.refs == 1);
        grand.parent = &parent;  // a cycle stops at the repeated cohort
        CHECK(seam(kFC4SeamDevelopmentShadow, &hp, nullptr) == 4);
        CHECK(ex.refs == 1 && parent.refs == 1 && grand.refs == 1);
        grand.parent = nullptr;
        FakeHolder plain;  // not an exemplar: its own properties (none listed by its EnumProperties)
        plain.Set<uint32_t>(0x10, 2);
        hp.holder = reinterpret_cast<const cISCPropertyHolder*>(plain.AsSdk());
        CHECK(seam(kFC4SeamDevelopmentShadow, &hp, nullptr) == 0);
        hp.holder = nullptr;
        CHECK(seam(kFC4SeamDevelopmentShadow, &hp, nullptr) == -1);
    }
    FreeLibrary(hd);
    DeleteFileA(ini.c_str());
    const std::string text = ReadFile(log);
    const auto npos = std::string::npos;
    CHECK(text.find("development shadow: ON (development_shadow from FreeCity4.ini): every ORIGINAL growth "
                    "developer's monthly update") != npos);
    CHECK(text.find("development shadow: not compared or assumed: the industrial decline order") != npos);
    CHECK(text.find("development shadow: requested") == npos);
    CHECK(text.find("development shadow test: lists 2/2 in order; mismatches=0") != npos);
    CHECK(text.find("development shadow test: developer 0x1010 (target zone 1), exemplar "
                    "0x6534284A/0x67CD5FA1/0x00001010") != npos);
    CHECK(text.find("development shadow test: developer 0x4400 (target zone 3), exemplar "
                    "0x6534284A/0x67CD5FA1/0x00004400") != npos);
    CHECK(CountOf(text, "development shadow test: holder properties 4 (own 2, inherited 2): 0x00000021 0x27812833 "
                        "0x00000010@1 0x27812870@2") == 3);
    CHECK(text.find("development shadow test: holder properties 0 (own 0, inherited 0):") != npos);
    CHECK(text.find("development shadow test: lists 2/2 in order; mismatches=2; differences (original/ours): "
                    "list[0] (1,2) s1/(3,4) s2, list[1] (3,4) s2/(1,2) s1") != npos);
    CHECK(text.find("development shadow test: lists 2/2 as a multiset; mismatches=0") != npos);
    CHECK(text.find("development shadow test: lists 2/3 as a multiset; mismatches=2; differences (original/ours): "
                    "list count 2/3, list (5,6) s1 only in ours") != npos);
    CHECK(text.find("development shadow test: lists 2/3 in order; mismatches=1; differences (original/ours): "
                    "list count 2/3") != npos);
    CHECK(text.find("subsystem 'development': enabled") == npos);
    CHECK(text.find("structured exception") == npos);
    CHECK(text.find("config: WARNING unknown key") == npos);
    CHECK(text.find("WARNING: released last reference") == npos);
    CHECK(text.find("WARNING: over-release") == npos);
    if (g_failures == 0) std::printf("plugin development shadow test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}

// ---- zones (zones.md): the replacement without a city; PlaceZone and the dezone through stub hosts (op 38) -------
constexpr uint32_t kCLSID_ZoneManager = 0x298F9B2D;
constexpr uint32_t kIID_cISC4ZoneManager = 0x67652433;
constexpr uint32_t kIID_cIGZMessageTarget2Zones = 0x452294AA;
// §10.1 with empty failure maps: version, grid reference, 3 x 16 counts, 4
// scalars, sizes, costs, textures and colours, alpha, two map counts, five
// service references (references as a uint32 0 here).
constexpr size_t kZonesRecordBase = 2 + 4 + 192 + 16 + 128 + 256 + 128 + 1 + 4 + 4 + 20;
constexpr size_t kZonesAlphaAt = 2 + 4 + 192 + 16 + 128 + 256 + 128;

// The answers without a city or grid (§11.1), some through the raw slots: 13,
// 16 and 17 return in AL, 12 the sign-extended int8 in EAX, 18 is void.
void CheckZonesNeutral(cFC4ISC4ZoneManager* z) {
    void** vt = *reinterpret_cast<void***>(z);
    CHECK(z->GetZoneGrid() == nullptr);
    for (int32_t t = 0; t < 16; ++t) {
        CHECK(z->GetZoneCount(t) == 0);
        CHECK(z->GetUndevelopedTileCountOfType(t) == 0);
    }
    CHECK(z->GetUndevelopedTileCount() == 0 && z->GetAbandonedTileCount() == 0);
    using U32XZFn = uint32_t(__fastcall*)(void*, void*, int32_t, int32_t);
    CHECK(reinterpret_cast<U32XZFn>(vt[12])(z, nullptr, 3, 4) == 0u);
    using U32IFn = uint32_t(__fastcall*)(void*, void*, int32_t);
    for (int32_t t = 0; t < 16; ++t) {
        const bool rci = t >= 1 && t <= 9;
        CHECK(z->IsRCIZone(t) == rci);
        CHECK((reinterpret_cast<U32IFn>(vt[13])(z, nullptr, t) & 0xFFu) == (rci ? 1u : 0u));
    }
    cGZPersistResourceKey key(1, 2, 3);
    CHECK(z->GetTextureForZone(1, key) && key.type == 0x7AB50E44u && key.group == 0x1ABE787Du);
    cGZPersistResourceKey untouched(1, 2, 3);
    CHECK(!z->GetTextureForZone(16, untouched) && untouched.type == 1 && untouched.instance == 3);
    using U32Fn = uint32_t(__fastcall*)(void*, void*);
    CHECK((reinterpret_cast<U32Fn>(vt[16])(z, nullptr) & 0xFFu) == z->GetZoneDisplayAlpha());
    CHECK((reinterpret_cast<U32Fn>(vt[17])(z, nullptr) & 0xFFu) == z->GetDefaultZoneDisplayAlpha());
}

// Slot 9 (zone, result) inserts a zero entry into failure map 1 when both
// are in range (§11.1 quirk); slot 18 (void, pop 4) sets the alpha.
void ZonesQuirks(cFC4ISC4ZoneManager* z) {
    void** vt = *reinterpret_cast<void***>(z);
    using U32IIFn = uint32_t(__fastcall*)(void*, void*, int32_t, int32_t);
    CHECK(reinterpret_cast<U32IIFn>(vt[9])(z, nullptr, 2, 3) == 0u);   // inserts {2, 3, 0}
    CHECK(z->GetDevelopmentFailureCount(2, 3) == 0);                    // present: nothing more
    CHECK(z->GetDevelopmentFailureCount(16, 0) == 0);                   // out of range: nothing
    CHECK(z->GetDevelopmentFailureCount(0, 19) == 0);
    using VoidU8Fn = void(__fastcall*)(void*, void*, uint32_t);
    reinterpret_cast<VoidU8Fn>(vt[18])(z, nullptr, 0x80);
    CHECK(z->GetZoneDisplayAlpha() == 0x80);
}

// Threads (cFC4ZoneManager.h): getters from another thread while the main
// thread holds the lock inside DoMessage wait about 20 ms each, then answer
// from the snapshot (one fallback each).
class ZonesLockHeldMessage final : public cIGZMessage2 {
public:
    explicit ZonesLockHeldMessage(cFC4ISC4ZoneManager* z) : zm(z) {}
    cFC4ISC4ZoneManager* zm;
    bool ran = false;
    uint32_t count = 99, total = 99;
    int32_t type = 99, minSize = 99;
    uint8_t alpha = 0;
    DWORD ms = 0;
    bool QueryInterface(uint32_t, void**) override {
        ran = true;
        const DWORD start = GetTickCount();
        std::thread t([this] {
            count = zm->GetZoneCount(1);
            total = zm->GetUndevelopedTileCount();
            type = zm->GetZoneType(1, 1);
            alpha = zm->GetZoneDisplayAlpha();
            minSize = zm->GetMinZoneSize(1);
        });
        t.join();
        ms = GetTickCount() - start;
        return false;
    }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    bool Create(uint32_t, void**) const override { return false; }
    uint32_t GetType() const override { return kMsgSimNewDay; }
    void SetType(uint32_t) override {}
    bool operator==(cIGZMessage2 const&) const override { return false; }
    bool operator<(cIGZMessage2 const&) const override { return false; }
};

void CheckZonesThreads(cFC4ISC4ZoneManager* z, cIGZMessageTarget2* mt) {
    const int32_t minSize = z->GetMinZoneSize(1);
    ZonesLockHeldMessage msg(z);
    CHECK(mt->DoMessage(&msg));
    CHECK(msg.ran);
    CHECK(msg.count == 0 && msg.total == 0 && msg.type == 0);  // the snapshot (no grid: type 0)
    CHECK(msg.alpha == z->GetZoneDisplayAlpha() && msg.minSize == minSize);
    CHECK(msg.ms >= 80 && msg.ms < 5000);  // five getters waited (about 20 ms each), none blocked
    uint32_t c = 99;
    std::thread t([&] { c = z->GetZoneCount(1); });  // no contention: the lock at once
    t.join();
    CHECK(c == 0);
}

std::vector<uint8_t> ZonesWrite(cFC4ISC4ZoneManager* z, int* nullRefs = nullptr) {
    std::vector<uint8_t> out;
    cIGZSerializable* ser = nullptr;
    if (!z->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) || ser == nullptr) return out;
    ResidentialSegOStream s;
    if (ser->Write(s)) out = s.bytes;
    if (nullRefs != nullptr) *nullRefs = s.nullRefs;
    ser->Release();
    return out;
}

bool ZonesRead(cFC4ISC4ZoneManager* z, const std::vector<uint8_t>& bytes) {
    cIGZSerializable* ser = nullptr;
    if (!z->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) || ser == nullptr) return false;
    ResidentialSegIStream s(bytes);
    const bool ok = ser->Read(s);
    ser->Release();
    return ok;
}

cFC4ISC4ZoneManager* NewZoneManager(cIGZCOMDirector* director) {
    void* obj = nullptr;
    if (!director->GetClassObject(kCLSID_ZoneManager, kIID_cISC4ZoneManager, &obj)) return nullptr;
    return static_cast<cFC4ISC4ZoneManager*>(obj);
}

// ---- zones through stub hosts (test seam op 38) --------------------------------------------------------
//
// A stub "game" answers the zone manager's host calls the test needs (an 8 x
// 8 int8 zone grid, funds, water, a lot to demolish, the demolition, the
// landfill cost, parcellization); the rest fall through to the plugin's own
// callbacks, which answer neutrally without a city. ZoningChange messages go
// to a sink; the outLots set's nodes come from the test's allocator.

class ZonesFakeUnknown final : public cIGZUnknown {
public:
    int refs = 1;
    bool QueryInterface(uint32_t, void**) override { return false; }
    uint32_t AddRef() override { return static_cast<uint32_t>(++refs); }
    uint32_t Release() override { return static_cast<uint32_t>(--refs); }
};

struct ZonesStub {
    int8_t grid[8][8] = {};
    int gridCreates = 0, gridReleases = 0;
    int64_t funds = 1000;
    int64_t withdrawn = 0;
    int withdrawals = 0;
    int64_t pricingCost = 50;
    int pricings = 0, commits = 0;
    uint32_t commitFlags = 0;
    size_t commitCount = 0;
    uint64_t commitLot = 0;
    int parcellizations = 0;
    int32_t parcelType = -1;
    uint32_t parcelCells = 0;
    bool parcelFlag = false;
    std::vector<uint32_t> subscribed, unsubscribed;
    ZonesFakeUnknown lot;                  // covers cell (2, 2); its configuration refuses every zone
    ZonesFakeUnknown network;              // the dezone's occupant
    fc4_cell_rect networkRect{4, 0, 4, 7};  // its bounding cells
    int32_t networkFlags = 0;              // HasAnyNetworkFlag(0x80200090)
    int64_t landfillCost = 7;
    std::vector<cIGZMessage2Standard*> posts;  // held (one reference each)
    std::vector<void*> setBlocks;              // the outLots set's node blocks alive
};

ZonesStub* g_zs = nullptr;
constexpr uint64_t kZonesStubGrid = 0x7A000000u;
constexpr uint64_t kZonesStubCfg = 0x7A000010u;

uint64_t ZsGridCreate(void*, uint32_t kind, uint32_t instance, int32_t tract) {
    ++g_zs->gridCreates;
    CHECK(kind == 0x49B9E603u && instance == 0 && tract == 1);
    return kZonesStubGrid;
}
uint32_t ZsGridClass(void*, uint64_t) { return 0x49B9E603u; }
void ZsGridGeometry(void*, uint64_t, fc4_grid_geometry* out) {
    *out = fc4_grid_geometry{1, 0, 8, 8, 16.0f, 16.0f, 1.0f / 16.0f, 1.0f / 16.0f};
}
double ZsGridGet(void*, uint64_t, uint32_t, int32_t x, int32_t z) { return g_zs->grid[x][z]; }
void ZsGridSet(void*, uint64_t, uint32_t, int32_t x, int32_t z, double v) {
    g_zs->grid[x][z] = static_cast<int8_t>(v);
}
void ZsGridFill(void*, uint64_t, uint32_t, double v) {
    for (auto& col : g_zs->grid) {
        for (int8_t& c : col) c = static_cast<int8_t>(v);
    }
}
void ZsGridShutdown(void*, uint64_t) {}
void ZsGridRelease(void*, uint64_t) { ++g_zs->gridReleases; }
void ZsNoServices(void*) {}
void ZsSubscribe(void*, uint32_t m) { g_zs->subscribed.push_back(m); }
void ZsUnsubscribe(void*, uint32_t m) { g_zs->unsubscribed.push_back(m); }
void ZsCellCounts(void*, int32_t* x, int32_t* z) {
    *x = 8;
    *z = 8;
}
int64_t ZsFunds(void*) { return g_zs->funds; }
bool ZsWithdraw(void*, int64_t amount) {
    ++g_zs->withdrawals;
    g_zs->withdrawn += amount;
    return true;
}
bool ZsWater(void*, int32_t x, int32_t z) { return x == 6 && z == 6; }
uint64_t ZsLot(void*, int32_t x, int32_t z) {
    return x == 2 && z == 2 ? static_cast<uint64_t>(reinterpret_cast<uintptr_t>(&g_zs->lot)) : 0;
}
void ZsLotRect(void*, uint64_t, fc4_cell_rect* out) { *out = fc4_cell_rect{2, 2, 2, 2}; }
int32_t ZsLotZone(void*, uint64_t) { return 1; }
uint64_t ZsLotCfg(void*, uint64_t) { return kZonesStubCfg; }
uint64_t ZsLotBuilding(void*, uint64_t) { return 0; }
bool ZsCfgCompatible(void*, uint64_t, int32_t) { return false; }
int64_t ZsDemolish(void*, bool commit, const uint64_t* lots, size_t count, bool, uint32_t flags) {
    if (!commit) {
        ++g_zs->pricings;
        return g_zs->pricingCost;
    }
    ++g_zs->commits;
    g_zs->commitFlags = flags;
    g_zs->commitCount = count;
    g_zs->commitLot = count != 0 ? lots[0] : 0;
    return 0;
}
int64_t ZsLandfillCost(void*, const fc4_zone_region*) { return g_zs->landfillCost; }
void ZsParcel(void*, const fc4_zone_region* r, int32_t type, bool flag) {
    ++g_zs->parcellizations;
    g_zs->parcelType = type;
    g_zs->parcelFlag = flag;
    uint32_t n = 0;
    if (r->mask == nullptr) {
        n = static_cast<uint32_t>((r->x1 - r->x0 + 1) * (r->z1 - r->z0 + 1));
    } else {
        for (size_t i = 0; i < r->len; ++i) n += r->mask[i] != 0 ? 1u : 0u;
    }
    g_zs->parcelCells = n;
}
int32_t ZsNetworkFlags(void*, uint64_t occ, uint32_t flags) {
    if (occ != static_cast<uint64_t>(reinterpret_cast<uintptr_t>(&g_zs->network))) return -1;
    CHECK(flags == 0x80200090u);
    return g_zs->networkFlags;
}
void ZsOccupantCells(void*, uint64_t, fc4_cell_rect* out) { *out = g_zs->networkRect; }

void ZsPostSink(void*, cIGZMessage2Standard* m) {
    m->AddRef();
    g_zs->posts.push_back(m);
}
void* ZsSetAllocate(uint32_t size) {
    void* p = std::calloc(1, size);
    g_zs->setBlocks.push_back(p);
    return p;
}
void ZsSetFree(void* p) {
    auto it = std::find(g_zs->setBlocks.begin(), g_zs->setBlocks.end(), p);
    CHECK(it != g_zs->setBlocks.end());
    if (it != g_zs->setBlocks.end()) g_zs->setBlocks.erase(it);
    std::free(p);
}

fc4_zones_host ZonesStubHost(ZonesStub& s) {
    fc4_zones_host h{};
    h.struct_size = sizeof(h);
    h.user = &s;
    h.grids.struct_size = sizeof(h.grids);
    h.grids.user = &s;
    h.grids.create = &ZsGridCreate;
    h.grids.class_of = &ZsGridClass;
    h.grids.geometry = &ZsGridGeometry;
    h.grids.get = &ZsGridGet;
    h.grids.set = &ZsGridSet;
    h.grids.fill = &ZsGridFill;
    h.grids.shutdown = &ZsGridShutdown;
    h.grids.release = &ZsGridRelease;
    h.acquire_services = &ZsNoServices;
    h.release_services = &ZsNoServices;
    h.subscribe = &ZsSubscribe;
    h.unsubscribe = &ZsUnsubscribe;
    h.cell_counts = &ZsCellCounts;
    h.total_funds = &ZsFunds;
    h.withdraw_funds = &ZsWithdraw;
    h.cell_has_water = &ZsWater;
    h.get_lot = &ZsLot;
    h.lot_bounding_rect = &ZsLotRect;
    h.lot_zone_type = &ZsLotZone;
    h.lot_configuration = &ZsLotCfg;
    h.lot_building = &ZsLotBuilding;
    h.cfg_compatible_with_zone = &ZsCfgCompatible;
    h.demolish_lots = &ZsDemolish;
    h.landfill_zoning_cost = &ZsLandfillCost;
    h.do_parcellization = &ZsParcel;
    h.network_has_any_flag = &ZsNetworkFlags;
    h.occupant_bounding_cells = &ZsOccupantCells;
    return h;
}

// A game region over [x1..x2] x [z1..z2] (one word per column) with every bit set.
struct ZonesGameRegion {
    LandfillGameRegion r{};
    std::vector<uint32_t> cols;
    std::vector<uint32_t*> rows;
    ZonesGameRegion(int32_t x1, int32_t z1, int32_t x2, int32_t z2) {
        const uint32_t w = static_cast<uint32_t>(x2 - x1 + 1), h = static_cast<uint32_t>(z2 - z1 + 1);
        cols.assign(w, h >= 32 ? 0xFFFFFFFFu : ((1u << h) - 1u));
        for (uint32_t& c : cols) rows.push_back(&c);
        r = LandfillGameRegion{x1, z1, x2, z2, nullptr, w, h, 1, rows.data()};
    }
    bool Bit(int32_t x, int32_t z) const { return ((cols[x - r.x1] >> (z - r.z1)) & 1u) != 0; }
};

// The caller's std::set<cISC4Lot*> (allocators.md): a 12-byte object whose
// header node (0x14 bytes, red, parent null, left = right = header) comes
// from the same allocator as the nodes.
struct ZonesOutLots {
    void* header = nullptr;
    uint32_t count = 0;
    uint32_t comparator = 0;
    ZonesOutLots() {
        auto* h = static_cast<uint32_t*>(ZsSetAllocate(0x14));
        h[1] = 0;                                 // parent
        h[2] = h[3] = reinterpret_cast<uintptr_t>(h);  // left, right
        header = h;
    }
    // The root's key (the only element when count is 1).
    uintptr_t RootKey() const {
        const auto* h = static_cast<const uint32_t*>(header);
        const auto* root = reinterpret_cast<const uint32_t*>(static_cast<uintptr_t>(h[1]));
        return root != nullptr ? root[4] : 0;
    }
    // As the caller does: release each element, free the nodes and the header.
    void Destroy() {
        auto* h = static_cast<uint32_t*>(header);
        if (h == nullptr) return;
        auto* root = reinterpret_cast<uint32_t*>(static_cast<uintptr_t>(h[1]));
        if (root != nullptr) {
            CHECK(count == 1 && root[2] == 0 && root[3] == 0);  // one element in these tests
            reinterpret_cast<cIGZUnknown*>(static_cast<uintptr_t>(root[4]))->Release();
            ZsSetFree(root);
        }
        ZsSetFree(h);
        header = nullptr;
        count = 0;
    }
};
static_assert(sizeof(ZonesOutLots) == 12, "std::set object: 12 bytes");

using ZonesPlaceFn = uint32_t(__fastcall*)(void*, void*, void* region, int32_t type, uint32_t commit, uint32_t skipFunds,
                                           uint32_t checkSlope, uint32_t skipSize, uint32_t lotInside, int64_t* cost,
                                           int32_t* error, void* outLots);

// Slot 11 called raw (pop 40, AL).
bool ZonesPlace(cFC4ISC4ZoneManager* z, ZonesGameRegion& g, int32_t type, bool commit, bool skipFunds,
                bool checkSlope, int64_t* cost, int32_t* error, ZonesOutLots* outLots) {
    void** vt = *reinterpret_cast<void***>(z);
    const uint32_t eax = reinterpret_cast<ZonesPlaceFn>(vt[11])(z, nullptr, &g.r, type, commit, skipFunds, checkSlope,
                                                               1, 0, cost, error, outLots);
    return (eax & 0xFFu) != 0;
}

intptr_t ZonesHooks(FC4TestSeamFn seam, cIGZUnknown* zm, const fc4_zones_host* host, bool sink) {
    FC4ZonesTestInput in{};
    in.op = kFC4ZonesTestHooks;
    in.zoneManager = zm;
    in.host = host;
    in.postSink = sink ? &ZsPostSink : nullptr;
    in.setAllocate = host ? &ZsSetAllocate : nullptr;
    in.setFree = host ? &ZsSetFree : nullptr;
    return seam(kFC4SeamZones, &in, nullptr);
}

intptr_t ZonesLiveRegions(FC4TestSeamFn seam) {
    FC4ZonesTestInput in{};
    in.op = kFC4ZonesTestLiveRegions;
    return seam(kFC4SeamZones, &in, nullptr);
}

// A standard message as the occupant manager sends InsertOccupant (§5.1):
// void1 = the occupant, data3 = its occupant type, data4.
class ZonesStdMessage final : public cIGZMessage2Standard {
public:
    ZonesStdMessage(uint32_t t, void* v1, intptr_t d3, intptr_t d4) : type(t), void1(v1), data3(d3), data4(d4) {}
    uint32_t type;
    void* void1;
    intptr_t data3, data4;
    int refs = 1;
    bool QueryInterface(uint32_t riid, void** p) override {
        if (p == nullptr || riid != GZIID_cIGZMessage2Standard) return false;
        *p = static_cast<cIGZMessage2Standard*>(this);
        ++refs;
        return true;
    }
    uint32_t AddRef() override { return static_cast<uint32_t>(++refs); }
    uint32_t Release() override { return static_cast<uint32_t>(--refs); }
    bool Create(uint32_t, void**) const override { return false; }
    uint32_t GetType() const override { return type; }
    void SetType(uint32_t t) override { type = t; }
    bool operator==(cIGZMessage2 const&) const override { return false; }
    bool operator<(cIGZMessage2 const&) const override { return false; }
    bool GetHasData1() const override { return true; }
    intptr_t GetData1() const override { return reinterpret_cast<intptr_t>(void1); }
    void* GetVoid1() const override { return void1; }
    void SetData1(intptr_t) override {}
    void SetVoid1(void*) override {}
    bool GetHasData2() const override { return false; }
    intptr_t GetData2() const override { return 0; }
    void* GetVoid2() const override { return nullptr; }
    void SetData2(intptr_t) override {}
    void SetVoid2(void*) override {}
    bool GetHasData3() const override { return true; }
    intptr_t GetData3() const override { return data3; }
    void* GetVoid3() const override { return reinterpret_cast<void*>(data3); }
    void SetData3(intptr_t) override {}
    void SetVoid3(void*) override {}
    bool GetHasData4() const override { return true; }
    intptr_t GetData4() const override { return data4; }
    void* GetVoid4() const override { return reinterpret_cast<void*>(data4); }
    void SetData4(intptr_t) override {}
    void SetVoid4(void*) override {}
    bool GetHasString() const override { return false; }
    bool GetString(uint32_t, void**) const override { return false; }
    void SetString(cIGZString*) override {}
    bool GetHasIGZUnknown() const override { return false; }
    cIGZUnknown* GetIGZUnknown() const override { return nullptr; }
    void SetIGZUnknown(cIGZUnknown*) override {}
    bool GetHasExtra() const override { return false; }
    uint32_t GetExtra(uint32_t, void**) const override { return 0; }
    void SetExtra(cIGZSerializable*) override {}
};

constexpr uint32_t kMsgInsertOccupantZones = 0x99EF1142;
constexpr intptr_t kNetworkOccupantType = static_cast<intptr_t>(0xC772BF98u);

// The dezone handler (§5.1): a new network without the flags 0x80200090
// re-zones its bounding cells to 0 (commit, free, no slope, no size check,
// the lot inside) and posts the ZoningChange; anything else does nothing.
void ZonesDezoneChecks(cFC4ISC4ZoneManager* zm) {
    cIGZMessageTarget2* mt = nullptr;
    CHECK(zm->QueryInterface(kIID_cIGZMessageTarget2Zones, reinterpret_cast<void**>(&mt)) && mt != nullptr);
    if (mt == nullptr) return;
    // Column 4 zoned residential first (free).
    {
        ZonesGameRegion g(4, 0, 4, 7);
        int64_t cost = -1;
        int32_t error = -1;
        CHECK(ZonesPlace(zm, g, 1, true, true, false, &cost, &error, nullptr));
        CHECK(error == 0 && cost == 0);
        for (cIGZMessage2Standard* m : g_zs->posts) m->Release();
        g_zs->posts.clear();
    }
    const uint32_t before = zm->GetZoneCount(1);
    const int withdrawals = g_zs->withdrawals;
    const int parcels = g_zs->parcellizations;
    // Not a dezone: another message, another occupant type, flagged networks.
    ZonesStdMessage other(kMsgSimNewDay, &g_zs->network, kNetworkOccupantType, 0);
    CHECK(mt->DoMessage(&other));
    ZonesStdMessage building(kMsgInsertOccupantZones, &g_zs->network, 0x278128A0, 0);
    CHECK(mt->DoMessage(&building));
    g_zs->networkFlags = 1;
    ZonesStdMessage flagged(kMsgInsertOccupantZones, &g_zs->network, kNetworkOccupantType, 0);
    CHECK(mt->DoMessage(&flagged));
    ZonesStdMessage notNetwork(kMsgInsertOccupantZones, &g_zs->lot, kNetworkOccupantType, 0);
    CHECK(mt->DoMessage(&notNetwork));
    CHECK(zm->GetZoneCount(1) == before && g_zs->posts.empty() && g_zs->parcellizations == parcels);
    // The dezone.
    g_zs->networkFlags = 0;
    ZonesStdMessage road(kMsgInsertOccupantZones, &g_zs->network, kNetworkOccupantType, 1);
    CHECK(mt->DoMessage(&road));
    for (int z = 0; z < 8; ++z) CHECK(g_zs->grid[4][z] == 0);
    CHECK(zm->GetZoneCount(1) == before - 8);
    CHECK(g_zs->withdrawals == withdrawals + 1);  // WithdrawFunds(0): skipFunds
    CHECK(g_zs->parcellizations == parcels + 1 && g_zs->parcelType == 0 && !g_zs->parcelFlag);
    CHECK(g_zs->posts.size() == 1);
    if (g_zs->posts.size() == 1) {
        cIGZMessage2Standard* m = g_zs->posts[0];
        CHECK(m->GetType() == 0x07699455u && m->GetData3() == 0 && m->GetData4() == 0);
        const auto* r = static_cast<const LandfillGameRegion*>(m->GetVoid2());
        CHECK(r != nullptr && r->x1 == 4 && r->z1 == 0 && r->x2 == 4 && r->z2 == 7 && r->rows[0][0] == 0xFFu);
    }
    for (cIGZMessage2Standard* m : g_zs->posts) m->Release();
    g_zs->posts.clear();
    for (const ZonesStdMessage* m : {&other, &building, &flagged, &notNetwork, &road}) CHECK(m->refs == 1);
    mt->Release();
}

// ---- the zones shadow (op 38, kFC4ZonesTestShadow) ----------------------------------------------------
// An "original" zone manager on the stub world that answers through the live
// FreeCity4 zone manager of the PlaceZone test, so the shadow core must agree
// with it everywhere: slots 5-8, 10, 11 (dry runs only), 14-17, 19 and 20;
// slots 3, 4, 9 (it inserts), 12, 13 and 18 trap. Its record (its own Write,
// cIGZSerializable) is §10.1 with the live one's values, no failure entries,
// zero costs and null services; the grid reference is a fake int8 grid
// (class 0x49B9E603, through a serializable stand-in) over the stub's cells.
// `countSkew` is added to slot 6's type 2 and `costSkew` to slot 11's cost
// for type 5. The city answers GetZoneManager (slot 36) only.
constexpr uint32_t kIID_cISC4SimGridZones = 0xC9BD939A;
constexpr int32_t kZonesShadowJulian = 2451605;  // 2000-03-01

struct FakeOrigZones {
    FakeVtable zmVt, serVt, gridVt, gridSerVt, cityVt;
    FakeObject zm{}, ser{}, grid{}, gridSer{}, city{};
    cFC4ISC4ZoneManager* live = nullptr;
    void* cityZoneManager = nullptr;  // what city slot 36 answers
    int refs = 1;  // the owner's (the reference audit warns of a last release)
    int writes = 0;
    int places = 0;
    uint32_t countSkew = 0;
    int64_t costSkew = 0;
    bool noSerializable = false;
};
FakeOrigZones* g_oz = nullptr;

bool __fastcall OzQi(void* self, void*, uint32_t iid, void** out) {  // 0
    if (out == nullptr) return false;
    if (self == &g_oz->zm || self == &g_oz->ser) {
        if (iid == GZIID_cIGZSerializable && !g_oz->noSerializable) {
            *out = &g_oz->ser;
        } else if (iid == kIID_cISC4ZoneManager || iid == GZIID_cIGZUnknown) {
            *out = &g_oz->zm;
        } else {
            return false;  // (the reference audit asks other IIDs)
        }
    } else if (iid == GZIID_cIGZSerializable) {
        *out = &g_oz->gridSer;
    } else if (iid == kIID_cISC4SimGridZones) {
        *out = &g_oz->grid;
    } else {
        return false;
    }
    ++g_oz->refs;
    return true;
}
uint32_t __fastcall OzAddRef(void*, void*) { return static_cast<uint32_t>(++g_oz->refs); }   // 1
uint32_t __fastcall OzRelease(void*, void*) { return static_cast<uint32_t>(--g_oz->refs); }  // 2
void* __fastcall OzGrid(void*, void*) { return &g_oz->grid; }                                  // 5
uint32_t __fastcall OzCount(void*, void*, int32_t t) {                                          // 6
    return g_oz->live->GetZoneCount(t) + (t == 2 ? g_oz->countSkew : 0u);
}
uint32_t __fastcall OzUndevelopedOf(void*, void*, int32_t t) {  // 7
    return g_oz->live->GetUndevelopedTileCountOfType(t);
}
uint32_t __fastcall OzUndeveloped(void*, void*) { return g_oz->live->GetUndevelopedTileCount(); }  // 8
uint32_t __fastcall OzAbandoned(void*, void*) { return g_oz->live->GetAbandonedTileCount(); }      // 10
uint32_t __fastcall OzPlace(void*, void*, void* region, int32_t type, uint32_t commit, uint32_t skipFunds,  // 11
                            uint32_t checkSlope, uint32_t skipSize, uint32_t lotInside, int64_t* cost, int32_t* error,
                            void* outLots) {
    ++g_oz->places;
    CHECK((commit & 0xFFu) == 0 && outLots == nullptr);  // dry runs only
    void** vt = *reinterpret_cast<void***>(g_oz->live);
    const uint32_t eax = reinterpret_cast<ZonesPlaceFn>(vt[11])(g_oz->live, nullptr, region, type, commit & 0xFFu,
                                                               skipFunds & 0xFFu, checkSlope & 0xFFu, skipSize & 0xFFu,
                                                               lotInside & 0xFFu, cost, error, nullptr);
    if (type == 5 && cost != nullptr) *cost += g_oz->costSkew;
    return eax;
}
bool __fastcall OzTexture(void*, void*, int32_t t, cGZPersistResourceKey* key) {  // 14
    return g_oz->live->GetTextureForZone(t, *key);
}
uint32_t __fastcall OzDrag(void*, void*, int32_t t) { return g_oz->live->GetZoneDragColor(t); }  // 15
uint8_t __fastcall OzAlpha(void*, void*) { return g_oz->live->GetZoneDisplayAlpha(); }            // 16
uint8_t __fastcall OzDefaultAlpha(void*, void*) { return g_oz->live->GetDefaultZoneDisplayAlpha(); }  // 17
int32_t __fastcall OzMin(void*, void*, int32_t t) { return g_oz->live->GetMinZoneSize(t); }          // 19
int32_t __fastcall OzMax(void*, void*, int32_t t) { return g_oz->live->GetMaxZoneSize(t); }          // 20
bool __fastcall OzWrite(void*, void*, cIGZOStream* s) {  // serializable 3 (§10.1)
    ++g_oz->writes;
    void* p = nullptr;
    if (!s->QueryInterface(kFC4_IID_cISC4DBSegmentOStream, &p) || p == nullptr) return false;
    auto* seg = static_cast<cFC4ISC4DBSegmentOStream*>(p);  // the capture stream: no reference taken
    cFC4ISC4ZoneManager* z = g_oz->live;
    s->SetUint16(1);
    seg->WriteGZSerializable(reinterpret_cast<cIGZSerializable*>(&g_oz->gridSer));
    for (int32_t t = 0; t < 16; ++t) s->SetUint32(z->GetZoneCount(t));
    for (int pass = 0; pass < 2; ++pass) {  // undeveloped, then unnamedPerType (the same here)
        for (int32_t t = 0; t < 16; ++t) s->SetUint32(z->GetUndevelopedTileCountOfType(t));
    }
    s->SetUint32(z->GetUndevelopedTileCount());
    s->SetUint32(0);
    s->SetUint32(z->GetAbandonedTileCount());
    s->SetUint32(0);
    for (int32_t t = 0; t < 16; ++t) s->SetSint32(z->GetMinZoneSize(t));
    for (int32_t t = 0; t < 16; ++t) s->SetSint32(z->GetMaxZoneSize(t));
    for (int i = 0; i < 32; ++i) s->SetSint64(0);  // creation, destruction costs
    for (int32_t t = 0; t < 16; ++t) {
        cGZPersistResourceKey k;
        z->GetTextureForZone(t, k);
        s->SetUint32(k.instance);
    }
    for (int32_t t = 0; t < 16; ++t) s->SetUint32(z->GetZoneDragColor(t));
    s->SetUint8(z->GetZoneDisplayAlpha());
    s->SetUint32(0);  // failure map 1
    s->SetUint32(0);  // failure map 2
    for (int i = 0; i < 5; ++i) seg->WriteGZSerializable(nullptr);  // the services
    return true;
}
uint32_t __fastcall OzClass(void* self, void*) {  // serializable 5
    return self == &g_oz->gridSer ? 0x49B9E603u : kCLSID_ZoneManager;
}
int32_t __fastcall OzTractSize(void*, void*) { return 1; }               // grid 10
int32_t __fastcall OzTractShift(void*, void*) { return 0; }              // grid 11
int32_t __fastcall OzTractCount(void*, void*) { return 8; }              // grid 12, 13
float __fastcall OzTractWidth(void*, void*) { return 16.0f; }            // grid 14, 15
float __fastcall OzTractInvWidth(void*, void*) { return 1.0f / 16.0f; }  // grid 16, 17
int8_t __fastcall OzTractValue(void*, void*, int32_t x, int32_t z) {     // grid 22
    return (x >= 0 && x < 8 && z >= 0 && z < 8) ? g_zs->grid[x][z] : 0;
}
void* __fastcall OzCityZones(void*, void*) { return g_oz->cityZoneManager; }  // city 36

void InitFakeOrigZones(FakeOrigZones& o, cFC4ISC4ZoneManager* live) {
    g_oz = &o;
    o.live = live;
    for (FakeVtable* vt : {&o.zmVt, &o.serVt, &o.gridVt, &o.gridSerVt}) {
        vt->slots[0] = reinterpret_cast<void*>(&OzQi);
        vt->slots[1] = reinterpret_cast<void*>(&OzAddRef);
        vt->slots[2] = reinterpret_cast<void*>(&OzRelease);
    }
    o.zmVt.slots[5] = reinterpret_cast<void*>(&OzGrid);
    o.zmVt.slots[6] = reinterpret_cast<void*>(&OzCount);
    o.zmVt.slots[7] = reinterpret_cast<void*>(&OzUndevelopedOf);
    o.zmVt.slots[8] = reinterpret_cast<void*>(&OzUndeveloped);
    o.zmVt.slots[10] = reinterpret_cast<void*>(&OzAbandoned);
    o.zmVt.slots[11] = reinterpret_cast<void*>(&OzPlace);
    o.zmVt.slots[14] = reinterpret_cast<void*>(&OzTexture);
    o.zmVt.slots[15] = reinterpret_cast<void*>(&OzDrag);
    o.zmVt.slots[16] = reinterpret_cast<void*>(&OzAlpha);
    o.zmVt.slots[17] = reinterpret_cast<void*>(&OzDefaultAlpha);
    o.zmVt.slots[19] = reinterpret_cast<void*>(&OzMin);
    o.zmVt.slots[20] = reinterpret_cast<void*>(&OzMax);
    o.serVt.slots[3] = reinterpret_cast<void*>(&OzWrite);
    o.serVt.slots[5] = reinterpret_cast<void*>(&OzClass);
    o.gridSerVt.slots[5] = reinterpret_cast<void*>(&OzClass);
    o.gridVt.slots[10] = reinterpret_cast<void*>(&OzTractSize);
    o.gridVt.slots[11] = reinterpret_cast<void*>(&OzTractShift);
    o.gridVt.slots[12] = reinterpret_cast<void*>(&OzTractCount);
    o.gridVt.slots[13] = reinterpret_cast<void*>(&OzTractCount);
    o.gridVt.slots[14] = reinterpret_cast<void*>(&OzTractWidth);
    o.gridVt.slots[15] = reinterpret_cast<void*>(&OzTractWidth);
    o.gridVt.slots[16] = reinterpret_cast<void*>(&OzTractInvWidth);
    o.gridVt.slots[17] = reinterpret_cast<void*>(&OzTractInvWidth);
    o.gridVt.slots[22] = reinterpret_cast<void*>(&OzTractValue);
    o.cityVt.slots[36] = reinterpret_cast<void*>(&OzCityZones);
    o.zm.vtbl = o.zmVt.slots.data();
    o.ser.vtbl = o.serVt.slots.data();
    o.grid.vtbl = o.gridVt.slots.data();
    o.gridSer.vtbl = o.gridSerVt.slots.data();
    o.city.vtbl = o.cityVt.slots.data();
    o.cityZoneManager = &o.zm;
}

intptr_t ZonesShadowRun(FC4TestSeamFn seam, FakeOrigZones& o, const fc4_zones_host* host, uint32_t event) {
    FC4ZonesTestInput in{};
    in.op = kFC4ZonesTestShadow;
    in.host = host;
    in.city = reinterpret_cast<cISC4City*>(&o.city);
    in.julian = kZonesShadowJulian;
    in.event = event;
    return seam(kFC4SeamZones, &in, nullptr);
}

// The shadow against an original that agrees (0 mismatches on every line),
// then skewed, without a record, FreeCity4's own and none. Nothing in the
// stub world changes (dry runs only).
void ZonesShadowChecks(FC4TestSeamFn seam, cFC4ISC4ZoneManager* zm, const fc4_zones_host* host) {
    static FakeOrigZones o;
    InitFakeOrigZones(o, zm);
    int8_t gridBefore[8][8];
    std::memcpy(gridBefore, g_zs->grid, sizeof(gridBefore));
    const int commits = g_zs->commits, withdrawals = g_zs->withdrawals, parcels = g_zs->parcellizations;
    const uint32_t zoned = zm->GetZoneCount(1);
    CHECK(ZonesShadowRun(seam, o, host, 1) == 0);
    CHECK(o.writes == 1 && o.places == 64 && o.refs == 1);
    // Skewed: one count, and type 5's cost in each of the four regions.
    o.countSkew = 1;
    o.costSkew = 3;
    CHECK(ZonesShadowRun(seam, o, host, 0) == 5);
    o.countSkew = 0;
    o.costSkew = 0;
    // No record: the original and its grid are still compared.
    o.noSerializable = true;
    CHECK(ZonesShadowRun(seam, o, host, 1) == -3);
    o.noSerializable = false;
    CHECK(o.places == 128);  // no dry run without the core
    // FreeCity4's zone manager is not shadowed; a city without one.
    o.cityZoneManager = zm;
    CHECK(ZonesShadowRun(seam, o, host, 1) == -2);
    o.cityZoneManager = nullptr;
    CHECK(ZonesShadowRun(seam, o, host, 1) == -1);
    CHECK(o.refs == 1);
    CHECK(std::memcmp(gridBefore, g_zs->grid, sizeof(gridBefore)) == 0);
    CHECK(g_zs->commits == commits && g_zs->withdrawals == withdrawals && g_zs->parcellizations == parcels);
    CHECK(g_zs->posts.empty() && zm->GetZoneCount(1) == zoned);
}

// PlaceZone (§6) through the stub: reject, abort, funds, accept with outLots
// and the commit (grid, counts, demolition, parcellization, ZoningChange).
void ZonesPlaceZoneTest(FC4TestSeamFn seam, cIGZCOMDirector* director) {
    static ZonesStub stub;
    g_zs = &stub;
    cFC4ISC4ZoneManager* zm = NewZoneManager(director);
    CHECK(zm != nullptr);
    if (zm == nullptr) return;
    const fc4_zones_host host = ZonesStubHost(stub);
    CHECK(ZonesHooks(seam, reinterpret_cast<cIGZUnknown*>(&stub.lot), &host, true) == -1);  // not a zone manager
    CHECK(ZonesHooks(seam, zm, &host, true) == 1);
    CHECK(zm->Init());
    CHECK(stub.gridCreates == 1);
    CHECK(stub.subscribed.size() == 1 && stub.subscribed[0] == 0x99EF1142u);  // the dezone handler (§1.1)
    CHECK(zm->GetZoneGrid() == reinterpret_cast<cISC4SimGrid<int8_t>*>(static_cast<uintptr_t>(kZonesStubGrid)));
    CHECK(zm->GetZoneCount(0) == 64);
    // Slot 12: the int8 sign-extended in EAX.
    stub.grid[7][7] = -1;
    using U32XZFn = uint32_t(__fastcall*)(void*, void*, int32_t, int32_t);
    void** vt = *reinterpret_cast<void***>(zm);
    CHECK(reinterpret_cast<U32XZFn>(vt[12])(zm, nullptr, 7, 7) == 0xFFFFFFFFu);
    stub.grid[7][7] = 0;

    // Reject: water at (6, 6) refuses type 1; the caller's bit is cleared,
    // the dry run succeeds and changes nothing.
    {
        ZonesGameRegion g(5, 5, 6, 6);
        int64_t cost = -1;
        int32_t error = -1;
        CHECK(ZonesPlace(zm, g, 1, false, false, false, &cost, &error, nullptr));
        CHECK(error == 0 && cost == 0);
        CHECK(g.Bit(5, 5) && g.Bit(5, 6) && g.Bit(6, 5) && !g.Bit(6, 6));
        CHECK(stub.grid[5][5] == 0 && stub.posts.empty() && stub.withdrawals == 0);
    }
    // Abort: landfill (14) on water is error 2; the post-loop block still
    // runs (the landfill cost), then the call fails.
    {
        ZonesGameRegion g(6, 6, 6, 6);
        int64_t cost = -1;
        int32_t error = -1;
        CHECK(!ZonesPlace(zm, g, 14, true, false, false, &cost, &error, nullptr));
        CHECK(error == 2 && cost == 7);
        CHECK(stub.posts.empty() && stub.withdrawals == 0 && stub.grid[6][6] == 0);
    }
    // ZG17: a type >= 16 is replaced by 0 (a dezone), not a failure. A dry
    // run, so nothing is withdrawn, written or posted.
    {
        ZonesGameRegion g(0, 0, 0, 0);
        int64_t cost = -1;
        int32_t error = -1;
        CHECK(ZonesPlace(zm, g, 16, false, false, false, &cost, &error, nullptr));
        CHECK(error == 0 && cost == 0);
        CHECK(stub.withdrawals == 0 && stub.commits == 0 && stub.posts.empty());
    }
    // Funds: the lot at (2, 2) is priced at 50 > 10.
    {
        stub.funds = 10;
        ZonesGameRegion g(1, 1, 3, 3);
        int64_t cost = -1;
        int32_t error = -1;
        CHECK(!ZonesPlace(zm, g, 1, true, false, false, &cost, &error, nullptr));
        CHECK(error == 1 && cost == 50);
        CHECK(stub.withdrawals == 0 && stub.commits == 0 && stub.posts.empty());
        stub.funds = 1000;
    }
    // Dragging: dry runs fill the caller's outLots with the lot to demolish,
    // uniquely (one AddRef however many dry runs).
    ZonesOutLots outLots;
    const int lotRefs = stub.lot.refs;
    for (int i = 0; i < 3; ++i) {
        ZonesGameRegion g(1, 1, 3, 3);
        int64_t cost = -1;
        int32_t error = -1;
        CHECK(ZonesPlace(zm, g, 1, false, false, true, &cost, &error, &outLots));
        CHECK(error == 0 && cost == 50);
    }
    CHECK(outLots.count == 1 && outLots.RootKey() == reinterpret_cast<uintptr_t>(&stub.lot));
    CHECK(stub.lot.refs == lotRefs + 1);
    CHECK(stub.commits == 0 && stub.posts.empty() && stub.grid[2][2] == 0);
    // Release: the commit (checkSlope as a user action).
    {
        ZonesGameRegion g(1, 1, 3, 3);
        int64_t cost = -1;
        int32_t error = -1;
        CHECK(ZonesPlace(zm, g, 1, true, false, true, &cost, &error, &outLots));
        CHECK(error == 0 && cost == 50);
        CHECK(outLots.count == 1 && stub.lot.refs == lotRefs + 1);  // already in the set
        CHECK(stub.withdrawals == 1 && stub.withdrawn == 50);
        for (int x = 1; x <= 3; ++x) {
            for (int z = 1; z <= 3; ++z) CHECK(stub.grid[x][z] == 1);
        }
        CHECK(zm->GetZoneCount(0) == 55 && zm->GetZoneCount(1) == 9);
        CHECK(stub.commits == 1 && stub.commitFlags == 0x2000u && stub.commitCount == 1 &&
              stub.commitLot == static_cast<uint64_t>(reinterpret_cast<uintptr_t>(&stub.lot)));
        CHECK(stub.parcellizations == 1 && stub.parcelType == 1 && stub.parcelCells == 9 && stub.parcelFlag);
    }
    outLots.Destroy();
    CHECK(stub.lot.refs == lotRefs);
    CHECK(stub.setBlocks.empty());
    // The ZoningChange (§5.3): void1 = IGZUnknown = the zone manager, void2 =
    // the changed cells over the request rect (0x24-byte region), data3 = the
    // type, data4 = checkSlope; the region lives as long as the message.
    CHECK(stub.posts.size() == 1);
    if (stub.posts.size() == 1) {
        cIGZMessage2Standard* m = stub.posts[0];
        CHECK(m->GetType() == 0x07699455u);
        CHECK(m->GetVoid1() == static_cast<void*>(zm));
        CHECK(m->GetIGZUnknown() == static_cast<cIGZUnknown*>(zm));
        CHECK(m->GetData3() == 1 && m->GetData4() == 1);
        const auto* r = static_cast<const LandfillGameRegion*>(m->GetVoid2());
        CHECK(r != nullptr);
        if (r != nullptr) {
            CHECK(r->x1 == 1 && r->z1 == 1 && r->x2 == 3 && r->z2 == 3);
            CHECK(r->width == 3 && r->height == 3 && r->words == 1);
            for (int dx = 0; dx < 3; ++dx) CHECK(r->rows[dx][0] == 0x7u);
        }
        CHECK(ZonesLiveRegions(seam) == 1);
        m->Release();
        stub.posts.clear();
        CHECK(ZonesLiveRegions(seam) == 0);  // the message freed its region
    }
    // Re-zoning to the same type costs nothing and still posts (§6 notes).
    {
        ZonesGameRegion g(1, 1, 1, 1);
        int64_t cost = -1;
        int32_t error = -1;
        CHECK(ZonesPlace(zm, g, 1, true, false, false, &cost, &error, nullptr));
        CHECK(error == 0 && cost == 0 && zm->GetZoneCount(1) == 9);
        CHECK(stub.posts.size() == 1);
        for (cIGZMessage2Standard* m : stub.posts) m->Release();
        stub.posts.clear();
    }
    ZonesDezoneChecks(zm);
    ZonesShadowChecks(seam, zm, &host);
    CHECK(zm->Shutdown());
    CHECK(stub.gridReleases == 1);
    CHECK(stub.unsubscribed.size() == 1 && stub.unsubscribed[0] == 0x99EF1142u);
    CHECK(zm->Release() == 0);
    CHECK(ZonesLiveRegions(seam) == 0);
    CHECK(ZonesHooks(seam, nullptr, nullptr, false) == 0);  // the game allocator again
}

int ZonesEnableTest(const std::string& dll, const std::string& ini, const std::string& log) {
    // Off by default; `enable = zones` registers the zone manager.
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    SetEnvironmentVariableA("FC4_ZONES_SHADOW", nullptr);
    WriteTextFile(ini, "[subsystems]\nenable = zones\n[tools]\nzones_shadow = 1\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr && seam != nullptr);
    SetGamePrecision();  // as the game's PreFrameWorkInit does
    std::vector<uint8_t> record;
    if (director != nullptr && seam != nullptr) {
        std::vector<uint32_t> classes;
        director->EnumClassObjects(&CollectClass, &classes);
        // simulator (1) + budget (3) + zones (1); never the game's grid classes.
        CHECK(classes.size() == 5);
        for (uint32_t c : classes) CHECK(c < 0x49B9E600 || c > 0x49B9E60B);
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_ZoneManager) == 1);

        cFC4ISC4ZoneManager* zm = NewZoneManager(director);
        CHECK(zm != nullptr);
        if (zm != nullptr) {
            // §1: the primary for its IID and cIGZUnknown, the message target
            // and the serializable; nothing else (the out pointer untouched).
            void* p = nullptr;
            CHECK(zm->QueryInterface(GZIID_cIGZUnknown, &p) && p == static_cast<void*>(zm));
            if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
            p = nullptr;
            CHECK(zm->QueryInterface(kIID_cISC4ZoneManager, &p) && p == static_cast<void*>(zm));
            if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
            cIGZMessageTarget2* mt = nullptr;
            CHECK(zm->QueryInterface(kIID_cIGZMessageTarget2Zones, reinterpret_cast<void**>(&mt)) && mt != nullptr);
            CHECK(static_cast<void*>(mt) != static_cast<void*>(zm));
            cIGZSerializable* ser = nullptr;
            CHECK(zm->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) && ser != nullptr);
            if (ser != nullptr) {
                CHECK(ser->GetGZCLSID() == kCLSID_ZoneManager);
                void* back = nullptr;  // every sub-object answers the others (one object)
                CHECK(static_cast<cIGZUnknown*>(ser)->QueryInterface(kIID_cISC4ZoneManager, &back) && back == zm);
                if (back != nullptr) static_cast<cIGZUnknown*>(back)->Release();
            }
            p = reinterpret_cast<void*>(0x1234);
            CHECK(!zm->QueryInterface(0x12345678, &p) && p == reinterpret_cast<void*>(0x1234));
            CHECK(!zm->QueryInterface(0x498F9B01, &p) && p == reinterpret_cast<void*>(0x1234));
            CheckZonesNeutral(zm);
            ZonesQuirks(zm);
            // The record before Init (§10.1): version 1, the failure entry
            // {zone 2, result 3, 0}, the alpha, every reference null.
            int nullRefs = 0;
            record = ZonesWrite(zm, &nullRefs);
            CHECK(record.size() == kZonesRecordBase + 6);
            CHECK(nullRefs == 6);
            if (record.size() == kZonesRecordBase + 6) {
                CHECK(record[0] == 1 && record[1] == 0);
                CHECK(record[kZonesAlphaAt] == 0x80);
                const std::vector<uint8_t> map1(record.begin() + kZonesAlphaAt + 1, record.begin() + kZonesAlphaAt + 15);
                const std::vector<uint8_t> want = {1, 0, 0, 0, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0};
                CHECK(map1 == want);  // n1 = 1, {2, 3, 0}, n2 = 0
            }
            // Init without a city: no services, no grid; still neutral.
            CHECK(zm->Init());
            CheckZonesNeutral(zm);
            // The observer's month hook writes the month status line.
            seam(kFC4SeamObserverMessage, reinterpret_cast<void*>(static_cast<uintptr_t>(kMsgSimNewMonth)),
                 reinterpret_cast<void*>(static_cast<intptr_t>(2451576)));  // 2000-02-01
            if (mt != nullptr) {
                TypeMessage month(kMsgSimNewMonth);
                CHECK(mt->DoMessage(&month));
                CheckZonesThreads(zm, mt);
            }
            if (ser != nullptr) ser->Release();
            if (mt != nullptr) mt->Release();
            CHECK(zm->Shutdown());
            CHECK(zm->Release() == 0);
        }
        // Read runs before Init (§10.1): the record loads into a new manager,
        // which writes it back byte for byte; a record of another version
        // does not load. ZG3: the default alpha is not saved, so Read leaves
        // the constructor's 0xFF (Init copies the exemplar's alpha to it).
        cFC4ISC4ZoneManager* fresh = NewZoneManager(director);
        CHECK(fresh != nullptr);
        if (fresh != nullptr && !record.empty()) {
            CHECK(ZonesRead(fresh, record));
            CHECK(fresh->GetZoneDisplayAlpha() == 0x80 && fresh->GetDefaultZoneDisplayAlpha() == 0xFF);
            CHECK(ZonesWrite(fresh) == record);
            CHECK(fresh->Release() == 0);  // the final Release shuts a never-initialised manager down
        }
        fresh = NewZoneManager(director);
        if (fresh != nullptr && !record.empty()) {
            std::vector<uint8_t> v2 = record;
            v2[0] = 2;
            CHECK(!ZonesRead(fresh, v2));
            CHECK(fresh->Release() == 0);
        }
        ZonesPlaceZoneTest(seam, director);
        CheckCw("zones");
    }
    FreeLibrary(hd);
    DeleteFileA(ini.c_str());
    const std::string text = ReadFile(log);
    CHECK(text.find("subsystem 'zones': enabled (1 class replaced) by FreeCity4.ini (enable=)") != std::string::npos);
    CHECK(text.find("cFC4ZoneManager(") != std::string::npos);
    CHECK(text.find("zones Write -> 1") != std::string::npos);
    CHECK(text.find("zones Write: version=1 init=0 loaded=0 grid=00000000 0x0 counts=0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0") !=
          std::string::npos);
    CHECK(text.find("zones Init: version=1 init=1 loaded=0") != std::string::npos);
    CHECK(text.find("zones: no city; services not acquired") != std::string::npos);
    CHECK(text.find("zones month 2000-02-01: version=1 init=1") != std::string::npos);
    CHECK(text.find("failures=1 alpha=128/") != std::string::npos);
    CHECK(text.find("zones Shutdown: version=1") != std::string::npos);
    CHECK(text.find("zones Read -> 1") != std::string::npos);
    CHECK(text.find("zones Read: version=1 init=0 loaded=1") != std::string::npos);
    CHECK(text.find("zones Read -> 0") != std::string::npos);
    CHECK(text.find("cFC4ZoneManager destroyed (5 fallbacks)") != std::string::npos);
    CHECK(CountOf(text, "cFC4ZoneManager destroyed (0 fallbacks)") == 3);
    // PlaceZone through the stub (one line per committed call).
    CHECK(text.find("zones: test hooks installed") != std::string::npos);
    CHECK(text.find("zones PlaceZone: type=14 rect=6,6..6,6 cells=1 ok=0 error=2 cost=7 accepted=0") !=
          std::string::npos);
    // ZG17: the type-16 call is a dry run now (dezone), not logged as a commit.
    CHECK(text.find("zones PlaceZone: type=16") == std::string::npos);
    CHECK(text.find("zones PlaceZone: type=1 rect=1,1..3,3 cells=9 ok=0 error=1 cost=50 accepted=0") !=
          std::string::npos);
    CHECK(text.find("zones PlaceZone: type=1 rect=1,1..3,3 cells=9 ok=1 error=0 cost=50 accepted=9 cleared=0 lots=1") !=
          std::string::npos);
    CHECK(text.find("zones PlaceZone: type=1 rect=1,1..1,1 cells=1 ok=1 error=0 cost=0 accepted=1") !=
          std::string::npos);
    CHECK(CountOf(text, "zones PlaceZone: ") == 5);  // the dry runs (type 16 too, ZG17) are verbose-only
    // The dezone: one line, the dezone's own PlaceZone has none.
    CHECK(CountOf(text, "zones dezone: occupant ") == 1);
    CHECK(text.find(" rect 4,0..4,7 changed=8 lots=0") != std::string::npos);
    // zones_shadow = 1 with `enable = zones`: the shadow stays off (it needs the original).
    CHECK(text.find("zones shadow: requested (zones_shadow from FreeCity4.ini) but INACTIVE") != std::string::npos);
    CHECK(text.find("zones shadow: ON") == std::string::npos);
    // The shadow (op 38): an original that agrees, then a skewed one.
    CHECK(text.find("zones shadow (counts) 2000-03-01 month: grid=8x8 cells=64 other=0 gridRef=same mismatches=0 "
                    "orig/recount/ours: 0:55/55/55 1:9/9/9; first=-") != std::string::npos);
    CHECK(text.find("zones shadow (bookkeeping) 2000-03-01 month: mismatches=0 undeveloped[0]=64/64 total=64/64 "
                    "abandoned=0/0 (orig/ours) record=") != std::string::npos);
    CHECK(text.find("zones shadow (tuning) 2000-03-01 month: values=66 mismatches=0 alpha=255/255 default=255/255") !=
          std::string::npos);
    CHECK(text.find("zones shadow (PlaceZone) 2000-03-01 month: calls=64 mismatches=0 ") != std::string::npos);
    CHECK(text.find("zones shadow (counts) 2000-03-01 load: grid=8x8 cells=64 other=0 gridRef=same mismatches=1 ") !=
          std::string::npos);
    CHECK(text.find("first=2:1/0/0") != std::string::npos);
    CHECK(text.find("zones shadow (PlaceZone) 2000-03-01 load: calls=64 mismatches=4 ") != std::string::npos);
    CHECK(text.find("; ours not compared (the original does not answer cIGZSerializable)") != std::string::npos);
    CHECK(text.find("zones shadow (PlaceZone) 2000-03-01 month: INCOMPLETE (the original does not answer "
                    "cIGZSerializable); not compared") != std::string::npos);
    CHECK(CountOf(text, "the city's zone manager is FreeCity4's (enable = zones)") == 1);
    CHECK(CountOf(text, "the city has no zone manager; nothing compared") == 1);
    CHECK(CountOf(text, "zones shadow (PlaceZone) ") == 3);
    CHECK(text.find("structured exception") == std::string::npos);
    CHECK(text.find("config: WARNING unknown key") == std::string::npos);
    CHECK(text.find("WARNING: released last reference") == std::string::npos);
    CHECK(text.find("WARNING: over-release") == std::string::npos);
    if (g_failures == 0) std::printf("plugin zones enable test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}

// ---- land value and flammability grid shadows (test seam ops 23, 24) ---------------------------------
constexpr uint32_t kCLSID_Flammability = 0xE990C045;
constexpr uint32_t kIID_cISC4FlammabilitySimulator = 0x48F41617;
constexpr uint32_t kCLSID_SimGridUint8 = 0x49B9E602;
constexpr uint32_t kIID_cISC4SimGrid = 0xC9BD939A;
constexpr uint32_t kIID_cISC4Lot = 0xE818FCED;
constexpr uint32_t kMsgInsertOccupantFl = 0x99EF1142;
constexpr uint32_t kMsgConditionChangedFl = 0xE99B3D4D;
constexpr uint32_t kMsgWaterMapChangedFl = 0x6946DC34;  // the plumbing pass end (FL5/FL6)
constexpr uint32_t kMsgSimNewDayFl = 0x66956814;        // the day poll after the original's day (FL7/FL8)

// A fake simulation grid (cISC4SimGrid<T>, simgrid.md): the geometry slots
// (10-17) and GetTractValue (22), values x outer; AddRef / Release (1, 2)
// count into `refs`; QueryInterface (0) answers nothing. Every other slot
// traps: the shadows only read.
struct FakeGrid {
    void** vtbl;
    int32_t tractSize, shift, countX, countZ;
    const uint8_t* u8;
    const float* f32;
    int* refs;
    int reads;
};

int32_t __fastcall FgTractSize(FakeGrid* g, void*) { return g->tractSize; }  // 10
int32_t __fastcall FgShift(FakeGrid* g, void*) { return g->shift; }          // 11
int32_t __fastcall FgCountX(FakeGrid* g, void*) { return g->countX; }        // 12
int32_t __fastcall FgCountZ(FakeGrid* g, void*) { return g->countZ; }        // 13
float __fastcall FgWidth(FakeGrid* g, void*) { return 16.0f * static_cast<float>(g->tractSize); }        // 14, 15
float __fastcall FgInvWidth(FakeGrid* g, void*) { return 1.0f / (16.0f * static_cast<float>(g->tractSize)); }  // 16, 17
uint8_t __fastcall FgU8(FakeGrid* g, void*, int32_t x, int32_t z) {  // 22 (uint8)
    ++g->reads;
    return (x >= 0 && x < g->countX && z >= 0 && z < g->countZ) ? g->u8[x * g->countZ + z] : 0;
}
float __fastcall FgF32(FakeGrid* g, void*, int32_t x, int32_t z) {  // 22 (float)
    ++g->reads;
    return (x >= 0 && x < g->countX && z >= 0 && z < g->countZ) ? g->f32[x * g->countZ + z] : 0.0f;
}
bool __fastcall FgQi(FakeGrid*, void*, uint32_t, void**) { return false; }  // 0 (the reference audit asks)
uint32_t __fastcall FgAddRef(FakeGrid* g, void*) { return static_cast<uint32_t>(++*g->refs); }   // 1
uint32_t __fastcall FgRelease(FakeGrid* g, void*) { return static_cast<uint32_t>(--*g->refs); }  // 2

void InitGridVtable(FakeVtable& vt, bool isFloat) {
    vt.slots[0] = reinterpret_cast<void*>(&FgQi);
    vt.slots[1] = reinterpret_cast<void*>(&FgAddRef);
    vt.slots[2] = reinterpret_cast<void*>(&FgRelease);
    vt.slots[10] = reinterpret_cast<void*>(&FgTractSize);
    vt.slots[11] = reinterpret_cast<void*>(&FgShift);
    vt.slots[12] = reinterpret_cast<void*>(&FgCountX);
    vt.slots[13] = reinterpret_cast<void*>(&FgCountZ);
    vt.slots[14] = reinterpret_cast<void*>(&FgWidth);
    vt.slots[15] = reinterpret_cast<void*>(&FgWidth);
    vt.slots[16] = reinterpret_cast<void*>(&FgInvWidth);
    vt.slots[17] = reinterpret_cast<void*>(&FgInvWidth);
    vt.slots[22] = isFloat ? reinterpret_cast<void*>(&FgF32) : reinterpret_cast<void*>(&FgU8);
}

void SetGrid(FakeGrid& g, FakeVtable& vt, int32_t size, int32_t shift, int32_t cx, int32_t cz, int* refs) {
    g.vtbl = vt.slots.data();
    g.tractSize = size;
    g.shift = shift;
    g.countX = cx;
    g.countZ = cz;
    g.refs = refs;
}

// An "original" land value simulator (landvalue.md §11.1) on an 8 x 6 cell
// city: intrinsic and total 4 x 3 tracts (S = 2), slope 1 x 1 (S = 8). The
// city answers GetLandValueSimulator (compiled slot 61) and null for the zone
// manager (36), the building development simulator (53) and the terrain (84),
// so our pass reads no land: scratch 0, no water bonus, no altitude curve
// (the code defaults: the intrinsic grid is kept), no developer, average 0,
// mode altitude min(16, max(0, 0)) = 0, mean 0, slope 0 (§6, §7). The clean
// original therefore has total = intrinsic, slope 0 and the three scalars 0.
// The record (its own Write) is version 1 with three null references.
struct FakeOrigLv {
    FakeVtable simVt, serVt, cityVt, noLvVt, u8Vt, f32Vt;
    FakeObject sim{}, ser{}, city{}, noLv{};
    FakeGrid intrinsic{}, total{}, slope{};
    uint8_t intrinsicV[12] = {10, 60, 90, 20, 55, 130, 0, 74, 75, 49, 50, 200};  // [x][z]
    uint8_t totalV[12] = {10, 60, 90, 20, 55, 130, 0, 74, 75, 49, 50, 200};
    float slopeV[1] = {0.0f};
    uint16_t version = 1;
    uint8_t flag = 0, average = 0;
    float mode = 0.0f, mean = 0.0f;
    int refs = 1, gridRefs = 1, writes = 0, typeCalls = 0;
};

FakeOrigLv* g_lv = nullptr;

bool __fastcall LvQi(void*, void*, uint32_t iid, void** out) {  // 0 (simulator, serializable)
    if (out == nullptr || iid != GZIID_cIGZSerializable) return false;
    *out = &g_lv->ser;
    ++g_lv->refs;
    return true;
}
uint32_t __fastcall LvAddRef(void*, void*) { return static_cast<uint32_t>(++g_lv->refs); }   // 1
uint32_t __fastcall LvRelease(void*, void*) { return static_cast<uint32_t>(--g_lv->refs); }  // 2
int32_t __fastcall LvType(void*, void*, int32_t x, int32_t z) {                              // 5 (cell coordinates)
    ++g_lv->typeCalls;
    const uint8_t v = g_lv->totalV[(x / 2) * 3 + z / 2];
    return v < 50 ? 1 : (v < 75 ? 2 : 3);  // the code-default boundaries 50, 75 (§8)
}
void* __fastcall LvIntrinsic(void*, void*) { return &g_lv->intrinsic; }  // 7
void* __fastcall LvTotal(void*, void*) { return &g_lv->total; }          // 8
void* __fastcall LvSlope(void*, void*) { return &g_lv->slope; }          // 9
uint8_t __fastcall LvAverage(void*, void*) { return g_lv->average; }     // 10
float __fastcall LvMode(void*, void*) { return g_lv->mode; }             // 11
bool __fastcall LvWrite(void*, void*, cIGZOStream* s) {                  // serializable 3 (§10)
    ++g_lv->writes;
    s->SetUint16(g_lv->version);
    for (int i = 0; i < 3; ++i) s->SetUint32(0);  // null references
    s->SetUint8(g_lv->flag);
    s->SetUint8(g_lv->average);
    return s->SetFloat32(g_lv->mean);
}
uint32_t __fastcall LvClsid(void*, void*) { return kCLSID_LandValue; }  // serializable 5
void* __fastcall LvCitySim(void*, void*) { return &g_lv->sim; }         // city 61
void* __fastcall LvCityNull(void*, void*) { return nullptr; }           // city 36, 53, 84; noLv 61

void InitFakeOrigLv(FakeOrigLv& o) {
    g_lv = &o;
    for (FakeVtable* vt : {&o.simVt, &o.serVt}) {
        vt->slots[0] = reinterpret_cast<void*>(&LvQi);
        vt->slots[1] = reinterpret_cast<void*>(&LvAddRef);
        vt->slots[2] = reinterpret_cast<void*>(&LvRelease);
    }
    o.simVt.slots[5] = reinterpret_cast<void*>(&LvType);
    o.simVt.slots[7] = reinterpret_cast<void*>(&LvIntrinsic);
    o.simVt.slots[8] = reinterpret_cast<void*>(&LvTotal);
    o.simVt.slots[9] = reinterpret_cast<void*>(&LvSlope);
    o.simVt.slots[10] = reinterpret_cast<void*>(&LvAverage);
    o.simVt.slots[11] = reinterpret_cast<void*>(&LvMode);
    o.serVt.slots[3] = reinterpret_cast<void*>(&LvWrite);
    o.serVt.slots[5] = reinterpret_cast<void*>(&LvClsid);
    for (int s : {36, 53, 84}) o.cityVt.slots[s] = reinterpret_cast<void*>(&LvCityNull);
    o.cityVt.slots[61] = reinterpret_cast<void*>(&LvCitySim);
    o.noLvVt.slots[61] = reinterpret_cast<void*>(&LvCityNull);
    InitGridVtable(o.u8Vt, false);
    InitGridVtable(o.f32Vt, true);
    SetGrid(o.intrinsic, o.u8Vt, 2, 1, 4, 3, &o.gridRefs);
    SetGrid(o.total, o.u8Vt, 2, 1, 4, 3, &o.gridRefs);
    SetGrid(o.slope, o.f32Vt, 8, 3, 1, 1, &o.gridRefs);
    o.intrinsic.u8 = o.intrinsicV;
    o.total.u8 = o.totalV;
    o.slope.f32 = o.slopeV;
    o.sim.vtbl = o.simVt.slots.data();
    o.ser.vtbl = o.serVt.slots.data();
    o.city.vtbl = o.cityVt.slots.data();
    o.noLv.vtbl = o.noLvVt.slots.data();
}

intptr_t LvShadowOnce(FC4TestSeamFn seam, void* city, int32_t julian, uint32_t event) {
    FC4LandValueShadowTestInput in{reinterpret_cast<cISC4City*>(city), julian, event};
    return seam(kFC4SeamLandValueShadow, &in, nullptr);
}

int LandValueShadowTest(const std::string& dll, const std::string& ini, const std::string& log) {
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    SetEnvironmentVariableA("FC4_LANDVALUE_SHADOW", nullptr);
    SetEnvironmentVariableA("FC4_LANDVALUE_SHADOW_CELLS", nullptr);
    SetEnvironmentVariableA("FC4_LANDVALUE_SHADOW_INPUTS", nullptr);
    WriteTextFile(ini, "[tools]\nlandvalue_shadow = 1\nlandvalue_shadow_cells = 3\ndigest = 0\nobserve = 0\n"
                       "landvalue_shadow_inputs = 0\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr && seam != nullptr);
    static FakeOrigLv orig;
    InitFakeOrigLv(orig);
    if (director != nullptr && seam != nullptr) {
        std::vector<uint32_t> classes;
        director->EnumClassObjects(&CollectClass, &classes);  // game start: the shadow's setting
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_LandValue) == 0);  // the original stays
        // No city, a city without a land value simulator: not run.
        CHECK(LvShadowOnce(seam, nullptr, 2451576, 0) == -1);
        CHECK(LvShadowOnce(seam, &orig.noLv, 2451576, 0) == -1);
        CHECK(LvShadowOnce(seam, nullptr, 2451576, 1) == -1);
        // The startup observation also runs with input tracing disabled. It
        // seeds developer-map history without reading outputs or counting a pass.
        CHECK(LvShadowOnce(seam, &orig.city, 2451576, 3) == 0);
        CHECK(orig.writes == 0 && orig.typeCalls == 0);
        CHECK(orig.total.reads == 0 && orig.intrinsic.reads == 0 && orig.slope.reads == 0);
        // 1. Clean: our pass equals the original's (2000-02-01).
        CHECK(LvShadowOnce(seam, &orig.city, 2451576, 0) == 0);
        // A day: no developer listed (code defaults), so no map changed.
        CHECK(LvShadowOnce(seam, &orig.city, 2451577, 1) == 0);
        // 2. One wrong tract: the original's total (1, 2) is 131, ours 130.
        orig.totalV[1 * 3 + 2] = 131;
        CHECK(LvShadowOnce(seam, &orig.city, 2451605, 0) == 1);
        orig.totalV[1 * 3 + 2] = 130;
        // 3. One wrong scalar: the original's average is 7 (ours 0); its record still says 0.
        orig.average = 7;
        CHECK(LvShadowOnce(seam, &orig.city, 2451636, 0) == 1);
        orig.average = 0;
        // 4. One wrong record scalar (no getter): the mean altitude; and the
        // record says dirty-or-running (settled=0), which is not a difference.
        orig.mean = 2.5f;
        orig.flag = 1;
        CHECK(LvShadowOnce(seam, &orig.city, 2451666, 0) == 1);
        orig.mean = 0.0f;
        orig.flag = 0;
        // 5. A record that does not parse: compared without it.
        orig.version = 9;
        CHECK(LvShadowOnce(seam, &orig.city, 2451697, 0) == 0);
        orig.version = 1;
        // 6. PostCityShutdown drops the shadow core; the next pass is pass 1 of a new one.
        CHECK(LvShadowOnce(seam, &orig.city, 0, 2) == 0);
        CHECK(LvShadowOnce(seam, &orig.city, 2451727, 0) == 0);
        CHECK(orig.writes == 6);
        CHECK(orig.typeCalls == 6 * 12);
        CHECK(orig.refs == 1);      // every QueryInterface released
        CHECK(orig.gridRefs == 1);  // the grids are borrowed: no AddRef, no Release
        CHECK(orig.total.reads == 6 * 12 && orig.intrinsic.reads == 6 * 12 && orig.slope.reads == 6);
    }
    FreeLibrary(hd);
    DeleteFileA(ini.c_str());
    const std::string text = ReadFile(log);
    CHECK(text.find("landvalue shadow: ON (landvalue_shadow from FreeCity4.ini)") != std::string::npos);
    CHECK(text.find("landvalue shadow: NOT readable read-only (left out of the comparison)") != std::string::npos);
    CHECK(text.find("landvalue shadow: the city has no land value simulator; nothing compared") != std::string::npos);
    CHECK(CountOf(text, "landvalue shadow: comparing with the original land value simulator") == 2);
    // All six comparisons (clean, differing and unreadable-record cases)
    // must disclose sampled inputs; output equality cannot prove LV1 capture.
    CHECK(CountOf(text, "; input_capture=pass-end-sample; native_step_capture=0") == 6);
    CHECK(text.find("tuning the code defaults (no exemplar); differing tracts listed per grid: 3") !=
          std::string::npos);
    CHECK(text.find("landvalue shadow 2000-02-01 pass 1: compared on 4x3 tracts (S=2), slope 1x1 (S=8): intrinsic "
                    "cells=12 mismatched=0 max|d|=0; total cells=12 mismatched=0 max|d|=0; slope cells=1 "
                    "mismatched=0 max|d|=0; types mismatched=0/12; average orig/ours=0/0; modeAlt orig/ours=0/0; "
                    "meanAlt orig/ours=0/0; settled=1; desirability=none (no developer listed); mismatches=0; "
                    "passes=1 clean=1; input_capture=pass-end-sample; native_step_capture=0\r\n") != std::string::npos);
    CHECK(text.find("landvalue shadow 2000-03-01 pass 2: compared on 4x3 tracts (S=2), slope 1x1 (S=8): intrinsic "
                    "cells=12 mismatched=0 max|d|=0; total cells=12 mismatched=1 max|d|=1;") != std::string::npos);
    CHECK(text.find("mismatches=1; passes=2 clean=1; input_capture=pass-end-sample; native_step_capture=0; "
                    "first differing (x,z) orig/ours: total (1,2) 131/130") !=
          std::string::npos);
    CHECK(text.find("landvalue shadow 2000-04-01 pass 3: ") != std::string::npos);
    CHECK(text.find("; average orig/ours=7/0!; modeAlt orig/ours=0/0; meanAlt orig/ours=0/0; settled=1;") !=
          std::string::npos);
    CHECK(text.find("meanAlt orig/ours=2.5/0!; settled=0 (the original's record says dirty or a pass running") !=
          std::string::npos);
    CHECK(text.find("landvalue shadow: the original's record (its own Write) did not parse; the mean altitude and the "
                    "settled flag are not compared") != std::string::npos);
    CHECK(text.find("meanAlt not read (no record); settled=?; desirability=none (no developer listed); mismatches=0; "
                    "passes=5 clean=2") != std::string::npos);
    CHECK(text.find("landvalue shadow 2000-07-01 pass 1: ") != std::string::npos);
    CHECK(text.find("INCOMPLETE") == std::string::npos);
    CHECK(text.find("structured exception") == std::string::npos);
    CHECK(text.find("config: WARNING unknown key") == std::string::npos);
    CHECK(text.find("WARNING: released last reference") == std::string::npos);
    CHECK(text.find("WARNING: over-release") == std::string::npos);
    if (g_failures == 0) std::printf("plugin landvalue shadow test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}

// An "original" flammability simulator (flammability.md §11.1) on an 8 x 8
// city: both grids 8 x 8 tracts (S = 1), raw all 0 (no occupant), so no
// daily refresh writes a tract and every adjusted value must stay as the
// previous cycle left it; no zone manager, plumbing, weather, ordinance,
// occupant or lot manager (city slots 36, 64, 70, 63, 34, 33 return null),
// so the average is 0 and there are no hotspots. Its record (§10) writes
// both grids through WriteGZSerializable (each answers cISC4SimGrid
// 0xC9BD939A). Slot 6 answers from the adjusted grid, slot 7 false, slot 8
// the average.
struct FakeGridSer {
    void** vtbl;
    FakeGrid* grid;
};

struct FakeOrigFl {
    FakeVtable simVt, serVt, cityVt, noFlVt, gridVt, gridSerVt, notLotVt;
    FakeObject sim{}, ser{}, city{}, noFl{}, notLot{};
    FakeGrid adjusted{}, raw{};
    FakeGridSer adjustedSer{}, rawSer{};
    uint8_t adjustedV[64] = {};
    uint8_t rawV[64] = {};
    uint16_t version = 2;
    float g = 1.0f;
    uint8_t average = 0;
    int refs = 1, gridRefs = 1, writes = 0, flammableCalls = 0, hotspotCalls = 0, notLotQi = 0;
};

FakeOrigFl* g_fl = nullptr;

bool __fastcall FlQi(void*, void*, uint32_t iid, void** out) {  // 0 (simulator, serializable)
    if (out == nullptr || iid != GZIID_cIGZSerializable) return false;
    *out = &g_fl->ser;
    ++g_fl->refs;
    return true;
}
uint32_t __fastcall FlAddRef(void*, void*) { return static_cast<uint32_t>(++g_fl->refs); }   // 1
uint32_t __fastcall FlRelease(void*, void*) { return static_cast<uint32_t>(--g_fl->refs); }  // 2
bool __fastcall FlFlammable(void*, void*, int x, int z) {                                    // 6 (cells)
    ++g_fl->flammableCalls;
    return x >= 0 && x < 8 && z >= 0 && z < 8 && g_fl->adjustedV[x * 8 + z] != 0;
}
bool __fastcall FlHotspot(void*, void*, uint8_t*, int*, int*) {  // 7: no hotspot, nothing written
    ++g_fl->hotspotCalls;
    return false;
}
uint8_t __fastcall FlAverage(void*, void*) { return g_fl->average; }  // 8
bool __fastcall FlWrite(void*, void*, cIGZOStream* s) {               // serializable 3 (§10)
    ++g_fl->writes;
    s->SetUint16(g_fl->version);
    void* p = nullptr;
    if (s->QueryInterface(kFC4_IID_cISC4DBSegmentOStream, &p) && p != nullptr) {
        auto* db = static_cast<cFC4ISC4DBSegmentOStream*>(p);
        db->WriteGZSerializable(reinterpret_cast<cIGZSerializable*>(&g_fl->adjustedSer));
        db->WriteGZSerializable(reinterpret_cast<cIGZSerializable*>(&g_fl->rawSer));
        db->Release();
    }
    s->SetFloat32(g_fl->g);
    s->SetUint8(g_fl->average);
    return s->SetUint32(0);  // no hotspot
}
uint32_t __fastcall FlClsid(void*, void*) { return kCLSID_Flammability; }  // serializable 5
bool __fastcall GsQi(FakeGridSer* self, void*, uint32_t iid, void** out) {  // grid serializable 0
    if (out == nullptr || iid != kIID_cISC4SimGrid) return false;
    *out = self->grid;
    ++g_fl->gridRefs;
    return true;
}
uint32_t __fastcall GsAddRef(void*, void*) { return static_cast<uint32_t>(++g_fl->gridRefs); }   // 1
uint32_t __fastcall GsRelease(void*, void*) { return static_cast<uint32_t>(--g_fl->gridRefs); }  // 2
uint32_t __fastcall GsClsid(void*, void*) { return kCLSID_SimGridUint8; }                        // 5
bool __fastcall NotLotQi(void*, void*, uint32_t, void**) {                                       // 0
    ++g_fl->notLotQi;
    return false;
}
void* __fastcall FlCitySim(void*, void*) { return &g_fl->sim; }  // city 58
void* __fastcall FlCityNull(void*, void*) { return nullptr; }    // city 33, 34, 36, 63, 64, 70; noFl 58
uint32_t __fastcall FlCellCount(void*, void*) { return 8; }      // city 93, 94

void InitFakeOrigFl(FakeOrigFl& o) {
    g_fl = &o;
    for (FakeVtable* vt : {&o.simVt, &o.serVt}) {
        vt->slots[0] = reinterpret_cast<void*>(&FlQi);
        vt->slots[1] = reinterpret_cast<void*>(&FlAddRef);
        vt->slots[2] = reinterpret_cast<void*>(&FlRelease);
    }
    o.simVt.slots[6] = reinterpret_cast<void*>(&FlFlammable);
    o.simVt.slots[7] = reinterpret_cast<void*>(&FlHotspot);
    o.simVt.slots[8] = reinterpret_cast<void*>(&FlAverage);
    o.serVt.slots[3] = reinterpret_cast<void*>(&FlWrite);
    o.serVt.slots[5] = reinterpret_cast<void*>(&FlClsid);
    o.gridSerVt.slots[0] = reinterpret_cast<void*>(&GsQi);
    o.gridSerVt.slots[1] = reinterpret_cast<void*>(&GsAddRef);
    o.gridSerVt.slots[2] = reinterpret_cast<void*>(&GsRelease);
    o.gridSerVt.slots[5] = reinterpret_cast<void*>(&GsClsid);
    o.notLotVt.slots[0] = reinterpret_cast<void*>(&NotLotQi);
    for (int s : {33, 34, 36, 63, 64, 70}) o.cityVt.slots[s] = reinterpret_cast<void*>(&FlCityNull);
    o.cityVt.slots[58] = reinterpret_cast<void*>(&FlCitySim);
    o.cityVt.slots[93] = reinterpret_cast<void*>(&FlCellCount);
    o.cityVt.slots[94] = reinterpret_cast<void*>(&FlCellCount);
    o.noFlVt.slots[58] = reinterpret_cast<void*>(&FlCityNull);
    InitGridVtable(o.gridVt, false);
    SetGrid(o.adjusted, o.gridVt, 1, 0, 8, 8, &o.gridRefs);
    SetGrid(o.raw, o.gridVt, 1, 0, 8, 8, &o.gridRefs);
    o.adjusted.u8 = o.adjustedV;
    o.raw.u8 = o.rawV;
    o.adjustedSer = {o.gridSerVt.slots.data(), &o.adjusted};
    o.rawSer = {o.gridSerVt.slots.data(), &o.raw};
    for (int i = 0; i < 64; i += 5) o.adjustedV[i] = static_cast<uint8_t>(40 + i);  // the previous cycles' values
    o.sim.vtbl = o.simVt.slots.data();
    o.ser.vtbl = o.serVt.slots.data();
    o.city.vtbl = o.cityVt.slots.data();
    o.noFl.vtbl = o.noFlVt.slots.data();
    o.notLot.vtbl = o.notLotVt.slots.data();
}

intptr_t FlShadowOnce(FC4TestSeamFn seam, void* city, int32_t julian, uint32_t event, uint32_t msgType = 0,
                      void* void1 = nullptr, void* unknown = nullptr) {
    FC4FlammabilityShadowTestInput in{reinterpret_cast<cISC4City*>(city), julian, event, msgType,
                                      static_cast<cIGZUnknown*>(void1), static_cast<cIGZUnknown*>(unknown)};
    return seam(kFC4SeamFlammabilityShadow, &in, nullptr);
}

int FlammabilityShadowTest(const std::string& dll, const std::string& ini, const std::string& log) {
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    SetEnvironmentVariableA("FC4_FLAMMABILITY_SHADOW", nullptr);
    SetEnvironmentVariableA("FC4_FLAMMABILITY_SHADOW_CELLS", nullptr);
    WriteTextFile(ini, "[tools]\nflammability_shadow = 1\ndigest = 0\nobserve = 0\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr && seam != nullptr);
    static FakeOrigFl orig;
    InitFakeOrigFl(orig);
    if (director != nullptr && seam != nullptr) {
        std::vector<uint32_t> classes;
        director->EnumClassObjects(&CollectClass, &classes);  // game start: the shadow's setting
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_Flammability) == 0);  // the original stays
        // No city, a city without a flammability simulator: not run.
        CHECK(FlShadowOnce(seam, nullptr, 2451546, 1) == -1);
        CHECK(FlShadowOnce(seam, &orig.noFl, 2451546, 1) == -1);
        CHECK(FlShadowOnce(seam, nullptr, 0, 0, kMsgInsertOccupantFl) == -1);
        // A message before the first cycle end: no mirror yet.
        CHECK(FlShadowOnce(seam, &orig.city, 0, 0, kMsgInsertOccupantFl) == 0);
        // The original's first day (FlammabilityShadowDeliver's day poll):
        // counted, not mirrored.
        CHECK(FlShadowOnce(seam, &orig.city, 0, 0, kMsgSimNewDayFl) == 0);
        // The first cycle end (the empty one of the first day) is the baseline.
        CHECK(FlShadowOnce(seam, &orig.city, 2451546, 1) == -2);  // 2000-01-02
        // Mirrored messages: an occupant message without an occupant, and a
        // condition change whose object is not a lot (both no-ops in the core).
        CHECK(FlShadowOnce(seam, &orig.city, 0, 0, kMsgInsertOccupantFl) == 1);
        CHECK(FlShadowOnce(seam, &orig.city, 0, 0, kMsgConditionChangedFl, nullptr, &orig.notLot) == 1);
        CHECK(orig.notLotQi == 1);
        // The plumbing pass end: not mirrored (and, after the first day poll,
        // not polled either).
        CHECK(FlShadowOnce(seam, &orig.city, 0, 0, kMsgWaterMapChangedFl) == 0);
        // Days 2-65 (64 tracts, tractsPerDay 1): the second cycle ends on day
        // 65 (flammability.md §6.5); each day is polled (the fake city has no
        // plumbing simulator: nothing watered, no change).
        for (int day = 2; day <= 65; ++day) CHECK(FlShadowOnce(seam, &orig.city, 0, 0, kMsgSimNewDayFl) == 0);
        // 1. Clean.
        CHECK(FlShadowOnce(seam, &orig.city, 2451586, 1) == 0);  // 2000-02-11
        // 2. One wrong tract: adjusted (3, 4) became 9 although its raw value is 0.
        orig.adjustedV[3 * 8 + 4] = 9;
        CHECK(FlShadowOnce(seam, &orig.city, 2451626, 1) == 1);
        // 3. One wrong scalar: the multiplier changed without a season or ordinance message.
        orig.g = 0.5f;
        CHECK(FlShadowOnce(seam, &orig.city, 2451666, 1) == 1);
        // The shadow took the original's state: the next cycle is clean.
        CHECK(FlShadowOnce(seam, &orig.city, 2451706, 1) == 0);
        // 4. A record that does not parse: INCOMPLETE, then a new baseline.
        orig.version = 3;
        CHECK(FlShadowOnce(seam, &orig.city, 2451746, 1) == -2);
        orig.version = 2;
        CHECK(FlShadowOnce(seam, &orig.city, 2451786, 1) == -2);
        CHECK(FlShadowOnce(seam, &orig.city, 2451826, 1) == 0);
        // 5. PostCityShutdown drops the shadow core; the next cycle is a baseline again.
        CHECK(FlShadowOnce(seam, &orig.city, 0, 2) == 0);
        CHECK(FlShadowOnce(seam, &orig.city, 2451866, 1) == -2);
        CHECK(orig.writes == 9);
        CHECK(orig.flammableCalls == 9 * 64);
        CHECK(orig.hotspotCalls == 9);
        CHECK(orig.refs == 1);      // every QueryInterface released
        CHECK(orig.gridRefs == 1);  // each grid's cISC4SimGrid reference released at once
        CHECK(orig.adjusted.reads == 8 * 64 && orig.raw.reads == 8 * 64);  // not when the record did not parse
    }
    FreeLibrary(hd);
    DeleteFileA(ini.c_str());
    const std::string text = ReadFile(log);
    CHECK(text.find("flammability shadow: ON (flammability_shadow from FreeCity4.ini)") != std::string::npos);
    CHECK(text.find("flammability shadow: NOT readable read-only (left out of the comparison)") != std::string::npos);
    CHECK(text.find("flammability shadow: the city has no flammability simulator; nothing compared") !=
          std::string::npos);
    CHECK(CountOf(text, "flammability shadow: following the original flammability simulator of city ") == 2);
    CHECK(text.find("flammability shadow 2000-01-02 cycle 1: baseline (first cycle seen): our simulator takes the "
                    "original's grids (8x8 tracts, tractsPerDay=") != std::string::npos);
    CHECK(text.find("flammability shadow 2000-02-11 cycle 2: tracts=8x8 (S=1) tractsPerDay=") != std::string::npos);
    CHECK(text.find("raw cells=64 mismatched=0 max|d|=0; adjusted cells=64 mismatched=0 max|d|=0") !=
          std::string::npos);
    CHECK(text.find("average orig/ours=0/0; multiplier orig/ours=1/1; hotspots orig/ours=0/0") != std::string::npos);
    CHECK(text.find("original's grids (8x8 tracts, tractsPerDay=1), multiplier 1, average 0, 0 hotspot(s); compared "
                    "from the next cycle; day 1 since Init, 1 tracts of the new cycle visited") != std::string::npos);
    CHECK(text.find("events=2; watered polls=65 by content (at a day 64, with a change 0; published grid pointer "
                    "changes 0); tracts whose watered state changed: 0 (0 carried in); carried into the next cycle: 0 "
                    "(changed after the wrap day 65 (day 65 since Init; 1 tracts of the new cycle visited));") !=
          std::string::npos);
    // No day polls before cycle 3: the count is off from then on (FL8).
    CHECK(text.find("carried into the next cycle: 0 (the comparison's own poll only (the day count is off: cycle end "
                    "3 delivered after day 65, by when the count gives 2));") != std::string::npos);
    CHECK(text.find("flammable getter cells=64 mismatched=0; mismatches=0; cycles=1 clean=1") != std::string::npos);
    CHECK(text.find("flammability shadow 2000-03-22 cycle 3: ") != std::string::npos);
    CHECK(text.find("adjusted cells=64 mismatched=1 max|d|=9") != std::string::npos);
    CHECK(text.find("mismatches=1; cycles=2 clean=1") != std::string::npos);
    CHECK(text.find("multiplier orig/ours=0.5/1!") != std::string::npos);
    CHECK(text.find("mismatches=1; cycles=3 clean=1") != std::string::npos);
    CHECK(text.find("mismatches=0; cycles=4 clean=2") != std::string::npos);
    CHECK(text.find("flammability shadow: the original's record (its own Write) did not parse") != std::string::npos);
    CHECK(text.find("cycle 6: INCOMPLETE (the original's record (its own Write) could not be read); not compared, "
                    "the next cycle is a new baseline") != std::string::npos);
    CHECK(text.find("cycle 7: baseline (first cycle seen)") != std::string::npos);
    CHECK(text.find("mismatches=0; cycles=5 clean=3") != std::string::npos);
    CHECK(text.find("flammability shadow 2000-11-17 cycle 1: baseline (first cycle seen)") != std::string::npos);
    CHECK(text.find("structured exception") == std::string::npos);
    CHECK(text.find("config: WARNING unknown key") == std::string::npos);
    CHECK(text.find("WARNING: released last reference") == std::string::npos);
    CHECK(text.find("WARNING: over-release") == std::string::npos);
    if (g_failures == 0) std::printf("plugin flammability shadow test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}

// Both shadows requested with our simulators enabled: inactive at game start,
// and each refuses a city whose simulator is ours.
void* g_ourLv = nullptr;
void* g_ourFl = nullptr;
void* __fastcall OursLv(void*, void*) { return g_ourLv; }  // city 61
void* __fastcall OursFl(void*, void*) { return g_ourFl; }  // city 58

int LvFlShadowInactiveTest(const std::string& dll, const std::string& ini, const std::string& log) {
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    SetEnvironmentVariableA("FC4_LANDVALUE_SHADOW", nullptr);
    SetEnvironmentVariableA("FC4_FLAMMABILITY_SHADOW", nullptr);
    WriteTextFile(ini, "[subsystems]\nenable = landvalue, flammability\n[tools]\nlandvalue_shadow = 1\n"
                       "flammability_shadow = 1\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr && seam != nullptr);
    static FakeVtable cityVt;
    static FakeObject city{};
    cityVt.slots[58] = reinterpret_cast<void*>(&OursFl);
    cityVt.slots[61] = reinterpret_cast<void*>(&OursLv);
    city.vtbl = cityVt.slots.data();
    if (director != nullptr && seam != nullptr) {
        std::vector<uint32_t> classes;
        director->EnumClassObjects(&CollectClass, &classes);
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_LandValue) == 1);
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_Flammability) == 1);
        CHECK(director->GetClassObject(kCLSID_LandValue, kIID_cISC4LandValueSimulator, &g_ourLv) && g_ourLv);
        CHECK(director->GetClassObject(kCLSID_Flammability, kIID_cISC4FlammabilitySimulator, &g_ourFl) && g_ourFl);
        if (g_ourLv != nullptr && g_ourFl != nullptr) {
            CHECK(LvShadowOnce(seam, &city, 2451576, 0) == -1);
            CHECK(LvShadowOnce(seam, &city, 2451576, 1) == -1);
            CHECK(FlShadowOnce(seam, &city, 2451576, 1) == -1);
            CHECK(FlShadowOnce(seam, &city, 0, 0, kMsgInsertOccupantFl) == -1);
        }
        if (g_ourLv != nullptr) CHECK(static_cast<cIGZUnknown*>(g_ourLv)->Release() == 0);
        if (g_ourFl != nullptr) CHECK(static_cast<cIGZUnknown*>(g_ourFl)->Release() == 0);
    }
    FreeLibrary(hd);
    DeleteFileA(ini.c_str());
    const std::string text = ReadFile(log);
    CHECK(text.find("landvalue shadow: requested (landvalue_shadow from FreeCity4.ini) but INACTIVE") !=
          std::string::npos);
    CHECK(text.find("flammability shadow: requested (flammability_shadow from FreeCity4.ini) but INACTIVE") !=
          std::string::npos);
    CHECK(text.find("landvalue shadow: ON") == std::string::npos);
    CHECK(text.find("flammability shadow: ON") == std::string::npos);
    CHECK(text.find("landvalue shadow: inactive: the city's land value simulator is FreeCity4's own") !=
          std::string::npos);
    CHECK(text.find("flammability shadow: inactive: the city's flammability simulator is FreeCity4's own") !=
          std::string::npos);
    CHECK(text.find("landvalue shadow: comparing") == std::string::npos);
    CHECK(text.find("flammability shadow: following") == std::string::npos);
    CHECK(text.find("config: WARNING unknown key") == std::string::npos);
    CHECK(text.find("WARNING: released last reference") == std::string::npos);
    CHECK(text.find("WARNING: over-release") == std::string::npos);
    if (g_failures == 0) std::printf("plugin landvalue / flammability shadow inactive test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}

// ---- references read from a save (in-game run 88) -------------------------------------------------
// After a save, the land value grids that came from Read were released with
// FC4_RELEASE and the audit warned three times (run 88). Our reading, not a
// finding: the game's load holds its own reference until the next save drops
// it. The fakes below model that: an object a save loaded starts with the
// game's reference, which the "save" drops before Shutdown.
//
// `refs` counts every reference; `gameRefs` the game's own (1 for a loaded
// object, 0 for one GetClassObject made for us). `taken` counts the references
// the plugin received (handed over, AddRef, QueryInterface), `released` its
// Releases; `freed` is set by the Release that reaches 0.
struct SavedRefCounts {
    uint32_t refs = 0, gameRefs = 0, taken = 0, released = 0, overReleases = 0;
    bool freed = false;
    uint32_t Take() {
        ++taken;
        return ++refs;
    }
    uint32_t Give() {
        if (refs == 0) {
            ++overReleases;
            return 0;
        }
        ++released;
        if (--refs == 0) freed = true;
        return refs;
    }
    void GameDrops() {  // the save lets go of the load's reference
        --gameRefs;
        --refs;
    }
};

constexpr uint32_t kSavedIID_SimGrid = 0xC9BD939A;
constexpr uint32_t kSavedClsU8 = 0x49B9E602;
constexpr uint32_t kSavedClsF32 = 0x49B9E60A;
constexpr uint32_t kSavedOrdinanceClass = 0x5AFE0001;  // a DLL plugin's class (not ours)
constexpr uint32_t kSavedOrdinanceID = 0x5AFE0002;

// A game grid (cSC4SimGrid<T>): 16 x 8 cells; the tract counts follow the
// tract size. It is not one of FreeCity4's core objects (the stream reader's
// kFC4_IID_CoreObject query fails), so the audit follows it.
template <class T>
class SavedGrid final : public cISC4SimGrid<T>, public cIGZSerializable {
public:
    static constexpr int32_t kCellsX = 16, kCellsZ = 8;
    SavedGrid(uint32_t clsid, int32_t tractSize, uint32_t gameRefs) : clsid_(clsid), tractSize_(tractSize) {
        c.refs = c.gameRefs = gameRefs;
    }
    SavedRefCounts c;
    int inits = 0, shutdowns = 0, resizes = 0;
    cIGZSerializable* Ser() { return this; }
    cISC4SimGrid<T>* Grid() { return this; }

    bool QueryInterface(uint32_t riid, void** p) override {
        if (p == nullptr) return false;
        if (riid == kSavedIID_SimGrid || riid == GZIID_cIGZUnknown) {
            *p = static_cast<cISC4SimGrid<T>*>(this);
        } else if (riid == GZIID_cIGZSerializable) {
            *p = static_cast<cIGZSerializable*>(this);
        } else {
            return false;
        }
        c.Take();
        return true;
    }
    uint32_t AddRef() override { return c.Take(); }
    uint32_t Release() override { return c.Give(); }

    bool Init() override {
        ++inits;
        return true;
    }
    bool Shutdown() override {
        ++shutdowns;
        return true;
    }
    uint32_t GetInstanceID() const override { return instance_; }
    bool SetInstanceID(uint32_t id) override {
        instance_ = id;
        return true;
    }
    T GetCellValue(int32_t, int32_t) const override { return T{}; }
    T GetAverageValueInCellRect(int32_t, int32_t, int32_t, int32_t) const override { return T{}; }
    bool SetTractSize(int32_t s) override {
        ++resizes;
        tractSize_ = s;
        return true;
    }
    int32_t GetTractSize() const override { return tractSize_; }
    int32_t GetTractShift() const override {
        int32_t s = 0;
        while (s < 30 && (1 << s) < tractSize_) ++s;
        return s;
    }
    int32_t GetTractCountX() const override { return tractSize_ > 0 ? kCellsX / tractSize_ : 0; }
    int32_t GetTractCountZ() const override { return tractSize_ > 0 ? kCellsZ / tractSize_ : 0; }
    float GetTractWidthX() const override { return 16.0f * static_cast<float>(tractSize_); }
    float GetTractWidthZ() const override { return 16.0f * static_cast<float>(tractSize_); }
    float GetOneOverTractWidthX() const override { return 1.0f / GetTractWidthX(); }
    float GetOneOverTractWidthZ() const override { return 1.0f / GetTractWidthZ(); }
    bool TractIsInBounds(uint32_t x, uint32_t z) const override {
        return x < static_cast<uint32_t>(GetTractCountX()) && z < static_cast<uint32_t>(GetTractCountZ());
    }
    bool PositionToTract(float, float, int32_t&, int32_t&) const override { return false; }
    bool TractCornerToPosition(int32_t, int32_t, float&, float&) const override { return false; }
    bool TractCenterToPosition(int32_t, int32_t, float&, float&) const override { return false; }
    T GetTractValue(int32_t, int32_t) const override { return T{}; }
    T GetAverageValueInTractRect(int32_t, int32_t, int32_t, int32_t) const override { return T{}; }
    intptr_t GetGridData() override { return reinterpret_cast<intptr_t>(data_.data()); }
    intptr_t GetGridData() const override { return reinterpret_cast<intptr_t>(data_.data()); }
    void SetTractValue(int32_t, int32_t, T) override {}
    void SetTractValues(T) override {}

    bool Write(cIGZOStream&) override { return true; }
    bool Read(cIGZIStream&) override { return true; }
    uint32_t GetGZCLSID() override { return clsid_; }

private:
    uint32_t clsid_;
    int32_t tractSize_;
    uint32_t instance_ = 0;
    std::array<T, kCellsX * kCellsZ> data_{};
};

// Every reference the plugin took is given back, exactly once; `freed`: the
// plugin's was the last one.
template <class T>
void CheckSavedGrid(const SavedGrid<T>& g, bool freed) {
    CHECK(g.shutdowns == 1);
    CHECK(g.c.refs == g.c.gameRefs);
    CHECK(g.c.taken == g.c.released);
    CHECK(g.c.overReleases == 0);
    CHECK(g.c.freed == freed);
}

// A DLL plugin's ordinance that a save holds by reference.
class SavedOrdinance final : public cFC4ISC4Ordinance, public cIGZSerializable {
public:
    explicit SavedOrdinance(uint32_t gameRefs) { c.refs = c.gameRefs = gameRefs; }
    SavedRefCounts c;
    cRZBaseString name{"Saved Ordinance"};
    cIGZSerializable* Ser() { return this; }
    cFC4ISC4Ordinance* Ord() { return this; }
    bool QueryInterface(uint32_t riid, void** p) override {
        if (p == nullptr) return false;
        if (riid == GZIID_cISC4Ordinance || riid == GZIID_cIGZUnknown) {
            *p = static_cast<cFC4ISC4Ordinance*>(this);
        } else if (riid == GZIID_cIGZSerializable) {
            *p = static_cast<cIGZSerializable*>(this);
        } else {
            return false;
        }
        c.Take();
        return true;
    }
    uint32_t AddRef() override { return c.Take(); }
    uint32_t Release() override { return c.Give(); }
    bool Init() override { return true; }
    bool Shutdown() override { return true; }
    int64_t GetCurrentMonthlyIncome() override { return 0; }
    uint32_t GetID() const override { return kSavedOrdinanceID; }
    cIGZString* GetName() override { return &name; }
    cIGZString* GetDescription() override { return &name; }
    uint32_t GetYearFirstAvailable() override { return 0; }
    uint32_t GetChanceAvailability() override { return 0; }
    int64_t GetEnactmentIncome() override { return 0; }
    int64_t GetRetracmentIncome() override { return 0; }
    int64_t GetMonthlyConstantIncome() override { return 0; }
    float GetMonthlyIncomeFactor() override { return 0.0f; }
    uint32_t GetAdvisorID() override { return 0; }
    cISCPropertyHolder* GetMiscProperties() override { return nullptr; }
    bool IsAvailable() override { return true; }
    bool IsOn() override { return false; }
    bool IsEnabled() override { return true; }
    int64_t GetMonthlyAdjustedIncome() override { return 0; }
    bool CheckConditions() override { return true; }
    bool IsIncomeOrdinance() override { return false; }
    bool Simulate() override { return true; }
    bool SetAvailable(bool) override { return true; }
    bool SetOn(bool) override { return true; }
    bool SetEnabled(bool) override { return true; }
    bool ForceAvailable(bool) override { return true; }
    bool ForceOn(bool) override { return true; }
    bool ForceMonthlyAdjustedIncome(int64_t) override { return true; }
    bool Write(cIGZOStream&) override { return true; }
    bool Read(cIGZIStream&) override { return true; }
    uint32_t GetGZCLSID() override { return kSavedOrdinanceClass; }
};

// A little-endian record; a reference is a uint32 id (0: null, n: objs[n - 1]).
struct SavedRecord {
    std::vector<uint8_t> b;
    template <class V>
    SavedRecord& Put(V v) {
        const uint8_t* p = reinterpret_cast<const uint8_t*>(&v);
        b.insert(b.end(), p, p + sizeof(v));
        return *this;
    }
};

// The segment reader: a reference comes back with one reference for the
// reader (AddRef), on top of the load's own.
class SavedSegIStream final : public cFC4ISC4DBSegmentIStream {
public:
    explicit SavedSegIStream(std::vector<uint8_t> b) : bytes(std::move(b)) {}
    std::vector<uint8_t> bytes;
    std::vector<cIGZSerializable*> objs;
    size_t at = 0;
    int32_t error = 0;
    bool QueryInterface(uint32_t riid, void** p) override {
        if (p == nullptr || (riid != kFC4_IID_cISC4DBSegmentIStream && riid != GZIID_cIGZUnknown)) return false;
        *p = static_cast<cFC4ISC4DBSegmentIStream*>(this);
        return true;
    }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    bool Skip(uint32_t n) override { return Get(nullptr, n); }
    bool GetSint8(int8_t& v) override { return Get(&v, 1); }
    bool GetUint8(uint8_t& v) override { return Get(&v, 1); }
    bool GetSint16(int16_t& v) override { return Get(&v, 2); }
    bool GetUint16(uint16_t& v) override { return Get(&v, 2); }
    bool GetSint32(int32_t& v) override { return Get(&v, 4); }
    bool GetUint32(uint32_t& v) override { return Get(&v, 4); }
    bool GetSint64(int64_t& v) override { return Get(&v, 8); }
    bool GetUint64(uint64_t& v) override { return Get(&v, 8); }
    bool GetFloat32(float& v) override { return Get(&v, 4); }
    bool GetFloat64(double& v) override { return Get(&v, 8); }
    bool GetRZCharStr(char*, uint32_t) override { return Fail(); }
    bool GetGZStr(cIGZString&) override { return Fail(); }
    bool GetGZSerializable(cIGZSerializable&) override { return Fail(); }
    bool GetVoid(void* p, uint32_t n) override { return Get(p, n); }
    int32_t GetError() override { return error; }
    int32_t SetUserData(cIGZVariant*) override { return 0; }
    int32_t GetUserData() override { return 0; }
    bool Open(cISC4DBSegment*, cGZPersistResourceKey const&, bool) override { return false; }
    bool Close() override { return true; }
    bool IsOpen() override { return true; }
    int32_t GetRecord() override { return 0; }
    int32_t GetSegment() override { return 0; }
    bool ReadGZSerializable(cIGZSerializable** out, bool) override {
        uint32_t id = 0;
        if (out == nullptr || !GetUint32(id) || id > objs.size()) return Fail();
        *out = id == 0 ? nullptr : objs[id - 1];
        if (*out != nullptr) (*out)->AddRef();  // the reader's reference
        return true;
    }
    bool ReadResKey(cGZPersistResourceKey&) override { return Fail(); }
    bool ReadVariant(cIGZVariant&) override { return Fail(); }

private:
    bool Fail() {
        error = 1;
        return false;
    }
    bool Get(void* p, size_t n) {
        if (error != 0 || bytes.size() - at < n) return Fail();
        if (p != nullptr && n != 0) std::memcpy(p, bytes.data() + at, n);
        at += n;
        return true;
    }
};

// The segment writer: records each reference's id (objs[n - 1] -> n; unknown
// -> 0xFFFFFFFF); no reference is taken.
class SavedSegOStream final : public cFC4ISC4DBSegmentOStream {
public:
    std::vector<cIGZSerializable*> objs;
    std::vector<uint32_t> ids;
    bool QueryInterface(uint32_t riid, void** p) override {
        if (p == nullptr || (riid != kFC4_IID_cISC4DBSegmentOStream && riid != GZIID_cIGZUnknown)) return false;
        *p = static_cast<cFC4ISC4DBSegmentOStream*>(this);
        return true;
    }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    void Flush() override {}
    bool SetSint8(int8_t) override { return true; }
    bool SetUint8(uint8_t) override { return true; }
    bool SetSint16(int16_t) override { return true; }
    bool SetUint16(uint16_t) override { return true; }
    bool SetSint32(int32_t) override { return true; }
    bool SetUint32(uint32_t) override { return true; }
    bool SetSint64(int64_t) override { return true; }
    bool SetUint64(uint64_t) override { return true; }
    bool SetFloat32(float) override { return true; }
    bool SetFloat64(double) override { return true; }
    bool SetRZCharStr(char const*) override { return false; }
    bool SetGZStr(cIGZString const&) override { return false; }
    bool SetGZSerializable(cIGZSerializable const&) override { return false; }
    bool SetVoid(void const*, uint32_t) override { return true; }
    int32_t GetError() override { return 0; }
    int32_t SetUserData(cIGZVariant*) override { return 0; }
    int32_t GetUserData() override { return 0; }
    bool Open(cISC4DBSegment*, cGZPersistResourceKey const&, bool) override { return false; }
    bool Close() override { return true; }
    bool IsOpen() override { return true; }
    int32_t GetRecord() override { return 0; }
    int32_t GetSegment() override { return 0; }
    bool WriteGZSerializable(cIGZSerializable const* p) override {
        uint32_t id = p == nullptr ? 0 : 0xFFFFFFFFu;
        for (size_t i = 0; p != nullptr && i < objs.size(); ++i) {
            if (objs[i] == p) id = static_cast<uint32_t>(i + 1);
        }
        ids.push_back(id);
        return true;
    }
    bool WriteResKey(cGZPersistResourceKey const&) override { return false; }
    bool WriteVariant(cIGZVariant const&) override { return false; }
};

// GZCOM for a new city: GetClassObject makes the game grids (one reference,
// the caller's). FrameWork() is null, as in the other modes.
class SavedRefsCOM final : public cIGZCOM {
public:
    std::vector<std::unique_ptr<SavedGrid<uint8_t>>> u8;
    std::vector<std::unique_ptr<SavedGrid<float>>> f32;
    bool QueryInterface(uint32_t, void**) override { return false; }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    bool GetClassObject(uint32_t clsid, uint32_t iid, void** p) override {
        if (p == nullptr || iid != kSavedIID_SimGrid) return false;
        if (clsid == kSavedClsU8) {
            u8.push_back(std::make_unique<SavedGrid<uint8_t>>(clsid, 1, 0));
            u8.back()->c.Take();
            *p = u8.back()->Grid();
            return true;
        }
        if (clsid == kSavedClsF32) {
            f32.push_back(std::make_unique<SavedGrid<float>>(clsid, 1, 0));
            f32.back()->c.Take();
            *p = f32.back()->Grid();
            return true;
        }
        return false;
    }
    cIGZFrameWork* FrameWork() override { return nullptr; }
    bool AddLibrary(const cIGZString&) override { return false; }
    void FreeUnusedLibraries() override {}
    bool CreateGuid(uint32_t*) override { return false; }
    bool CreateGuids(uint32_t[], uint32_t) override { return false; }
    bool RealInit() override { return true; }
    void RealShutdown() override {}
    cIGZSystemService* SetServiceRunning(bool) override { return nullptr; }
    uint32_t EnumerateLibraries(cIGZCOMLibrary*[], uint32_t& n) override {
        n = 0;
        return 0;
    }
    void EnumerateLibraries(GZCOMLibraryIterationFunction, void*) override {}
};

size_t CountLinesWith(const std::string& text, const std::string& a, const std::string& b) {
    size_t n = 0;
    std::istringstream in(text);
    for (std::string line; std::getline(in, line);) {
        if (line.find(a) != std::string::npos && line.find(b) != std::string::npos) ++n;
    }
    return n;
}

// Land value: Read hands over three grids the load also holds; Init; with
// `save`, Write and the game drops its references; then the final Release
// (Shutdown: the grids are shut down and released).
void SavedLandValueCase(cIGZCOMDirector* director, bool save) {
    SavedGrid<uint8_t> intrinsic(kSavedClsU8, 2, 1), total(kSavedClsU8, 2, 1);
    SavedGrid<float> slope(kSavedClsF32, 8, 1);
    SavedRecord rec;  // landvalue.md §10: version, three grids, dirty, average, mean altitude
    rec.Put<uint16_t>(1).Put<uint32_t>(1).Put<uint32_t>(2).Put<uint32_t>(3).Put<uint8_t>(0).Put<uint8_t>(0).Put(0.0f);
    SavedSegIStream in(rec.b);
    in.objs = {intrinsic.Ser(), total.Ser(), slope.Ser()};
    void* p = nullptr;
    CHECK(director->GetClassObject(kCLSID_LandValue, kIID_cISC4LandValueSimulator, &p) && p != nullptr);
    if (p == nullptr) return;
    auto* lv = static_cast<cISC4LandValueSimulator*>(p);
    cIGZSerializable* ser = nullptr;
    CHECK(lv->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) && ser != nullptr);
    if (ser != nullptr) {
        CHECK(ser->Read(in));
        CHECK(lv->GetIntrinsicLandValueMap() == intrinsic.Grid());
        CHECK(lv->GetTotalLandValueMap() == total.Grid());
        CHECK(lv->GetSlopeMap() == slope.Grid());
        CHECK(intrinsic.c.refs == 2 && total.c.refs == 2 && slope.c.refs == 2);  // the load's and the core's
        CHECK(lv->Init());
        CHECK(intrinsic.resizes == 0 && total.resizes == 0 && slope.resizes == 0);  // default sizes 2 and 8
        if (save) {
            SavedSegOStream out;
            out.objs = in.objs;
            CHECK(ser->Write(out));
            CHECK(out.ids == std::vector<uint32_t>({1, 2, 3}));
            intrinsic.c.GameDrops();
            total.c.GameDrops();
            slope.c.GameDrops();
        }
        ser->Release();
    }
    CHECK(lv->Release() == 0);
    CheckSavedGrid(intrinsic, save);
    CheckSavedGrid(total, save);
    CheckSavedGrid(slope, save);
}

// Ordinance: Read hands over a DLL plugin's ordinance the load also holds;
// Init; with `save`, Write and the game drops its reference; then the final
// Release (the simulator's destruction releases the map, ordinance §8.2).
void SavedOrdinanceCase(cIGZCOMDirector* director, bool save) {
    SavedOrdinance o(1);
    SavedRecord rec;  // ordinance.md §9.1: version 4, expense, income, count, references, pending
    rec.Put<uint16_t>(4).Put<int64_t>(0).Put<int64_t>(0).Put<uint32_t>(1).Put<uint32_t>(1).Put<int64_t>(0);
    SavedSegIStream in(rec.b);
    in.objs = {o.Ser()};
    void* p = nullptr;
    CHECK(director->GetClassObject(kCLSID_OrdinanceSim, kIID_cISC4OrdinanceSimulator, &p) && p != nullptr);
    if (p == nullptr) return;
    auto* sim = static_cast<cISC4OrdinanceSimulator*>(p);
    cIGZSerializable* ser = nullptr;
    CHECK(sim->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) && ser != nullptr);
    if (ser != nullptr) {
        CHECK(ser->Read(in));
        CHECK(o.c.refs == 2);  // the load's and the core's
        CHECK(sim->Init());
        CHECK(reinterpret_cast<void*>(sim->GetOrdinanceByID(kSavedOrdinanceID)) == o.Ord());
        if (save) {
            SavedSegOStream out;
            out.objs = in.objs;
            CHECK(ser->Write(out));
            CHECK(out.ids == std::vector<uint32_t>({1}));
            o.c.GameDrops();
        }
        ser->Release();
    }
    CHECK(sim->Release() == 0);
    CHECK(o.c.refs == o.c.gameRefs);
    CHECK(o.c.taken == o.c.released);
    CHECK(o.c.overReleases == 0);
    CHECK(o.c.freed == save);
}

// Land value on a new city: Init creates the grids through GZCOM (ours alone).
void CreatedLandValueCase(cIGZCOMDirector* director) {
    static SavedRefsCOM com;
    CHECK(director->InitializeCOM(&com, cRZBaseString("")));
    void* p = nullptr;
    CHECK(director->GetClassObject(kCLSID_LandValue, kIID_cISC4LandValueSimulator, &p) && p != nullptr);
    if (p == nullptr) return;
    auto* lv = static_cast<cISC4LandValueSimulator*>(p);
    CHECK(lv->Init());
    CHECK(com.u8.size() == 2 && com.f32.size() == 1);
    if (com.u8.size() == 2 && com.f32.size() == 1) {
        CHECK(lv->GetIntrinsicLandValueMap() == com.u8[0]->Grid());
        CHECK(lv->GetTotalLandValueMap() == com.u8[1]->Grid());
        CHECK(lv->GetSlopeMap() == com.f32[0]->Grid());
    }
    CHECK(lv->Release() == 0);
    for (auto& g : com.u8) {
        CHECK(g->inits == 1 && g->GetTractSize() == 2);
        CheckSavedGrid(*g, true);
    }
    for (auto& g : com.f32) {
        CHECK(g->inits == 1 && g->GetTractSize() == 8);
        CheckSavedGrid(*g, true);
    }
}

int SavedRefsTest(const std::string& dll, const std::string& ini, const std::string& log) {
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    SetEnvironmentVariableA("FC4_REFAUDIT", nullptr);
    WriteTextFile(ini, "[subsystems]\nenable = landvalue, ordinance\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr);
    if (director != nullptr) {
        std::vector<uint32_t> classes;
        director->EnumClassObjects(&CollectClass, &classes);  // game start: configuration, the audit
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_LandValue) == 1);
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_OrdinanceSim) == 1);
        SavedLandValueCase(director, false);
        SavedLandValueCase(director, true);
        SavedOrdinanceCase(director, false);
        SavedOrdinanceCase(director, true);
        CreatedLandValueCase(director);  // last: it gives the DLL a GZCOM
    }
    FreeLibrary(hd);
    DeleteFileA(ini.c_str());
    const std::string text = ReadFile(log);
    const auto npos = std::string::npos;
    CHECK(text.find("refaudit: on") != npos);
    // Land value: the always-on line; no save: the load still held each grid.
    CHECK(CountOf(text, "landvalue grids (Shutdown): 3 reference(s) released, 0 of them the last") == 1);
    CHECK(CountOf(text, "landvalue grids (Shutdown): 3 reference(s) released, 3 of them the last") == 2);
    CHECK(CountOf(text, "landvalue grids (destroyed") == 0);
    // Each reference given back once, as ours (the audit's verbose lines):
    // 3 grids x 3 cases, the last one in the save and the new-city cases.
    CHECK(CountLinesWith(text, " Release (created for us) ", "ReleaseGameGrid:") == 9);
    CHECK(CountLinesWith(text, "-> count 0, ours 0, at ", "ReleaseGameGrid:") == 6);
    CHECK(CountLinesWith(text, " Release ", "ReleaseGameGrid:") == 9);
    // The foreign ordinance: the saved reference, once per case.
    CHECK(CountLinesWith(text, " Release (created for us) ", "ReleaseToken:") == 2);
    CHECK(CountLinesWith(text, "-> count 0, ours 0, at ", "ReleaseToken:") == 1);
    CHECK(CountLinesWith(text, " Release ", "ReleaseToken:") == 2);
    CHECK(text.find("WARNING: released last reference") == npos);
    CHECK(text.find("WARNING: over-release") == npos);
    CHECK(text.find("config: WARNING unknown key") == npos);
    if (g_failures == 0) std::printf("plugin saved references test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}

// ---- the growth calibration probe (building-development.md §12; seam op 39) ---------------------
// A fake city with the ORIGINAL classes' shape: a tract developer whose
// vtable (on a read-only page, like the game's) the probe patches, a BDS
// whose slot 34 gives a developer map built as docs/abi/allocators.md
// describes it, growth developers (slot 18 GetSourceDemand), demands (slot
// 12 QueryActiveDemandValue, ST0), a lot manager (slot 16) and a residential
// simulator (slot 16). Every other slot traps.
struct GrowthDev {
    void** vtbl;
    void* demand;  // GetSourceDemand (null: none)
};

struct GrowthDemand {
    void** vtbl;
    float active;
};

struct GrowthWorld {
    FakeVtable cityVt, foreignCityVt, bdsVt, foreignBdsVt, devVt, demandVt, lotsVt, resVt, foreignTractVt;
    FakeObject city{}, foreignCity{}, bds{}, foreignBds{}, lots{}, res{}, tract{}, foreignTract{};
    GrowthDemand demands[4] = {{nullptr, 6.0f}, {nullptr, 5.9f}, {nullptr, 100.0f}, {nullptr, -3.0f}};
    GrowthDev devs[5] = {};
    fc4::GameMapNode header{}, nodes[5]{};
    fc4::GameMapObject map{};
    void** tractVt = nullptr;  // VirtualAlloc'd, PAGE_READONLY
    int growCalls = 0;
    int32_t growResult = 0;
};

GrowthWorld* g_growth = nullptr;

void* __fastcall GwCityLots(void*, void*) { return &g_growth->lots; }              // cISC4City 33
void* __fastcall GwCityBds(void*, void*) { return &g_growth->bds; }                // cISC4City 53
void* __fastcall GwCityRes(void*, void*) { return &g_growth->res; }                // cISC4City 68
void* __fastcall GwCityTract(void*, void*) { return &g_growth->tract; }            // cISC4City 80
void* __fastcall GwForeignBds(void*, void*) { return &g_growth->foreignBds; }      // cISC4City 53
void* __fastcall GwForeignTract(void*, void*) { return &g_growth->foreignTract; }  // cISC4City 80
intptr_t __fastcall GwBdsDevelopers(void*, void*) {                                // BDS 34
    return reinterpret_cast<intptr_t>(&g_growth->map);
}
void* __fastcall GwDevSourceDemand(void* self, void*) { return static_cast<GrowthDev*>(self)->demand; }  // 18
float __fastcall GwDemandActive(void* self, void*) { return static_cast<GrowthDemand*>(self)->active; }  // 12
int32_t __fastcall GwLotCount(void*, void*) { return 42; }      // lot manager 16
int32_t __fastcall GwPopulation(void*, void*) { return 1234; }  // residential 16
int32_t __fastcall GwGrow(void*, void*, const void*, void*, void* flag) {  // tract developer 5
    ++g_growth->growCalls;
    if (flag != nullptr) *static_cast<bool*>(flag) = true;
    return g_growth->growResult;
}

void InitGrowthWorld(GrowthWorld& w) {
    w.cityVt.slots[33] = reinterpret_cast<void*>(&GwCityLots);
    w.cityVt.slots[53] = reinterpret_cast<void*>(&GwCityBds);
    w.cityVt.slots[68] = reinterpret_cast<void*>(&GwCityRes);
    w.cityVt.slots[80] = reinterpret_cast<void*>(&GwCityTract);
    w.city.vtbl = w.cityVt.slots.data();
    w.foreignCityVt.slots[53] = reinterpret_cast<void*>(&GwForeignBds);
    w.foreignCityVt.slots[80] = reinterpret_cast<void*>(&GwForeignTract);
    w.foreignCity.vtbl = w.foreignCityVt.slots.data();
    w.bdsVt.slots[34] = reinterpret_cast<void*>(&GwBdsDevelopers);
    w.bds.vtbl = w.bdsVt.slots.data();
    w.foreignBds.vtbl = w.foreignBdsVt.slots.data();
    w.foreignTract.vtbl = w.foreignTractVt.slots.data();
    w.devVt.slots[18] = reinterpret_cast<void*>(&GwDevSourceDemand);
    w.demandVt.slots[12] = reinterpret_cast<void*>(&GwDemandActive);
    w.lotsVt.slots[16] = reinterpret_cast<void*>(&GwLotCount);
    w.lots.vtbl = w.lotsVt.slots.data();
    w.resVt.slots[16] = reinterpret_cast<void*>(&GwPopulation);
    w.res.vtbl = w.resVt.slots.data();
    for (GrowthDemand& d : w.demands) d.vtbl = w.demandVt.slots.data();
    for (int i = 0; i < 5; ++i) {
        w.devs[i].vtbl = w.devVt.slots.data();
        w.devs[i].demand = i < 4 ? &w.demands[i] : nullptr;  // the fifth has no demand
    }
    // Developer type -> developer: root 0x1030; 0x1020 (left 0x1010), 0x1040 (right 0x3110).
    const uint32_t keys[5] = {0x1010, 0x1020, 0x1030, 0x1040, 0x3110};
    for (int i = 0; i < 5; ++i) {
        w.nodes[i] = {};
        w.nodes[i].key = keys[i];
        w.nodes[i].value = static_cast<uint32_t>(reinterpret_cast<uintptr_t>(&w.devs[i]));
    }
    fc4::GameMapNode* n = w.nodes;
    w.header.color = fc4::kGameMapRed;
    w.header.parent = &n[2];
    w.header.left = &n[0];
    w.header.right = &n[4];
    n[2].color = fc4::kGameMapBlack;
    n[2].parent = &w.header;
    n[2].left = &n[1];
    n[2].right = &n[3];
    n[1].color = fc4::kGameMapBlack;
    n[1].parent = &n[2];
    n[1].left = &n[0];
    n[3].color = fc4::kGameMapBlack;
    n[3].parent = &n[2];
    n[3].right = &n[4];
    n[0].color = fc4::kGameMapRed;
    n[0].parent = &n[1];
    n[4].color = fc4::kGameMapRed;
    n[4].parent = &n[3];
    w.map = {&w.header, 5, 0};
    // The tract developer vtable: Grow in slot 5, on a read-only page.
    w.tractVt = static_cast<void**>(VirtualAlloc(nullptr, 4096, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE));
    if (w.tractVt != nullptr) {
        FakeVtable traps;
        std::memcpy(w.tractVt, traps.slots.data(), 16 * sizeof(void*));
        w.tractVt[5] = reinterpret_cast<void*>(&GwGrow);
        DWORD old = 0;
        VirtualProtect(w.tractVt, 4096, PAGE_READONLY, &old);
    }
    w.tract.vtbl = w.tractVt;
}

DWORD PageProtection(const void* p) {
    MEMORY_BASIC_INFORMATION mbi{};
    if (VirtualQuery(p, &mbi, sizeof(mbi)) != sizeof(mbi)) return 0;
    return mbi.Protect;
}

intptr_t GrowthSeam(FC4TestSeamFn seam, uint32_t action, cISC4City* city = nullptr, uint32_t type = 0,
                    int32_t data1 = 0, uintptr_t expected = 0) {
    FC4GrowthProbeTestInput in{};
    in.action = action;
    in.vtable = g_growth->tractVt;
    in.expected = expected != 0 ? expected : reinterpret_cast<uintptr_t>(&GwGrow);
    in.bdsVtable = g_growth->bdsVt.slots.data();
    in.city = city;
    in.msgType = type;
    in.data1 = data1;
    return seam(kFC4SeamGrowthProbe, &in, nullptr);
}

// One Grow call the way the BDS makes it: through the tract developer's
// vtable slot 5, three dword arguments, callee-pop 12, EAX.
bool GrowOnce(int32_t result) {
    g_growth->growResult = result;
    int profile = 0, outcomes = 0;
    bool flag = false;
    const fc4test::SlotCall r = fc4test::CallSlot(&g_growth->tract, 5,
                                                  {static_cast<uint32_t>(reinterpret_cast<uintptr_t>(&profile)),
                                                   static_cast<uint32_t>(reinterpret_cast<uintptr_t>(&outcomes)),
                                                   static_cast<uint32_t>(reinterpret_cast<uintptr_t>(&flag))});
    return r.espDelta == 0 && r.x87Pushed == 0 && static_cast<int32_t>(r.eax) == result && flag;
}

int GrowthProbeIniTest(const std::string& dll, const std::string& ini, const std::string& log) {
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    SetEnvironmentVariableA("FC4_GROWTH_PROBE", nullptr);
    static GrowthWorld world;
    g_growth = &world;
    InitGrowthWorld(world);
    CHECK(world.tractVt != nullptr);
    if (world.tractVt == nullptr) return 1;
    void* const grow = reinterpret_cast<void*>(&GwGrow);
    constexpr uint32_t kNewDay = 0x66956814, kCityInit = 0x26D31EC1, kCityShutdown = 0x26D31EC2;
    cISC4City* city = reinterpret_cast<cISC4City*>(&world.city);
    cISC4City* foreign = reinterpret_cast<cISC4City*>(&world.foreignCity);

    WriteTextFile(ini, "[tools]\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr && seam != nullptr);
    int viaProbe = 0;
    if (director != nullptr && seam != nullptr) {
        CHECK(seam(kFC4SeamGrowthProbe, nullptr, nullptr) == -1);
        // Off by default, and off for a value other than 0 / 1: nothing patched.
        CHECK(GrowthSeam(seam, kFC4GrowthProbeTestStart) == 0);
        WriteTextFile(ini, "[tools]\ngrowth_probe = 2\n");
        CHECK(GrowthSeam(seam, kFC4GrowthProbeTestStart) == 0);
        CHECK(world.tractVt[5] == grow);
        CHECK(GrowthSeam(seam, kFC4GrowthProbeTestMessage, city, kNewDay, 2451576) == -1);  // not running
        WriteTextFile(ini, "[tools]\ngrowth_probe = 1\n");
        std::vector<uint32_t> classes;
        director->EnumClassObjects(&CollectClass, &classes);  // game start: the configuration
        // Not the expected original: refused, nothing written.
        CHECK(GrowthSeam(seam, kFC4GrowthProbeTestStart, nullptr, 0, 0, 0x0070D130u) == -1);
        CHECK(world.tractVt[5] == grow);
        // Installed: slot 5 holds the forwarder; the page stays read-only.
        CHECK(GrowthSeam(seam, kFC4GrowthProbeTestStart) == 1);
        CHECK(world.tractVt[5] != grow && world.tractVt[5] != nullptr);
        CHECK(world.tractVt[4] != grow && world.tractVt[6] != grow);
        CHECK(PageProtection(world.tractVt) == PAGE_READONLY);
        CHECK(GrowthSeam(seam, kFC4GrowthProbeTestStart) == 1);  // already running
        // City load: both classes are "the originals" here.
        CHECK(GrowthSeam(seam, kFC4GrowthProbeTestMessage, city, kCityInit) == 0);
        // Day 1: three Grow calls; two developers qualify (6.0, 100.0; not 5.9,
        // -3.0, nor the one without a demand).
        CHECK(GrowOnce(0x100));
        CHECK(GrowOnce(-3));
        CHECK(GrowOnce(7));
        CHECK(GrowthSeam(seam, kFC4GrowthProbeTestMessage, city, kNewDay, 2451576) == 3);
        // Day 2: one call for two qualifying developers: cut.
        CHECK(GrowOnce(0));
        CHECK(GrowthSeam(seam, kFC4GrowthProbeTestMessage, city, kNewDay, 2451577) == 1);
        // Day 3: no city to read.
        CHECK(GrowthSeam(seam, kFC4GrowthProbeTestMessage, nullptr, kNewDay, 2451578) == 0);
        CHECK(GrowthSeam(seam, kFC4GrowthProbeTestMessage, city, 0x12345678u) == 0);  // not ours
        CHECK(GrowthSeam(seam, kFC4GrowthProbeTestMessage, city, kCityShutdown) == 0);
        // A city whose classes are not the originals.
        CHECK(GrowthSeam(seam, kFC4GrowthProbeTestMessage, foreign, kCityInit) == 0);
        viaProbe = world.growCalls;
        // Stop: the original back, the page still read-only; no more counting.
        CHECK(GrowthSeam(seam, kFC4GrowthProbeTestStop) == 1);
        CHECK(world.tractVt[5] == grow);
        CHECK(PageProtection(world.tractVt) == PAGE_READONLY);
        CHECK(GrowOnce(5));
        CHECK(GrowthSeam(seam, kFC4GrowthProbeTestMessage, city, kNewDay, 2451579) == -1);
        CHECK(GrowthSeam(seam, kFC4GrowthProbeTestStop) == 0);
        // Started again and never stopped: the DLL's detach puts the original back.
        CHECK(GrowthSeam(seam, kFC4GrowthProbeTestStart) == 1);
        CHECK(world.tractVt[5] != grow);
        CHECK(GrowOnce(11));
    }
    FreeLibrary(hd);
    CHECK(world.tractVt[5] == grow);
    CHECK(PageProtection(world.tractVt) == PAGE_READONLY);
    CHECK(viaProbe == 4 && world.growCalls == 6);
    CHECK(GrowOnce(12));  // the original, directly
    DeleteFileA(ini.c_str());
    const std::string text = ReadFile(log);
    const auto npos = std::string::npos;
    CHECK(text.find("growth probe: WARNING growth_probe='2' (FreeCity4.ini) is not 0/1; the probe stays off") !=
          npos);
    CHECK(CountOf(text, "growth probe: ON (growth_probe from FreeCity4.ini)") == 3);
    CHECK(CountOf(text, "growth probe: NOTE cut=1 (attempts < qualifying) is approximate") == 3);
    CHECK(text.find("growth probe: NOT installed: the entry does not hold the expected original") != npos);
    CHECK(CountOf(text, "growth probe: hook installed at ") == 2);
    CHECK(text.find("page protection 0x2 restored") != npos);  // PAGE_READONLY
    CHECK(text.find("x87 check at the forwarder's exit: on (verbose)") != npos);
    CHECK(text.find("is the original class (vtable ") != npos);
    CHECK(text.find("its Grow calls are counted; BDS ") != npos);
    CHECK(text.find("growth probe 2000-02-01: attempts=3 qualifying=2 cut=0 elapsed_us=") != npos);
    CHECK(text.find("growth probe 2000-02-02: attempts=1 qualifying=2 cut=1 elapsed_us=") != npos);
    CHECK(text.find("growth probe 2000-02-03: attempts=0 qualifying=- cut=- elapsed_us=0 per_attempt_us=- lots=- "
                    "population=- max_us=0 cycles=0") != npos);
    CHECK(CountOf(text, " lots=42 population=1234 max_us=") == 2);
    CHECK(text.find("growth probe summary (city shutdown): days=3; attempts min/median/max=0/1/3 (total 4); cut "
                    "days=1, attempts on cut days min/median/max=1/1/1; qualifying min/median/max=2/2/2 (unknown "
                    "on 1 day(s)); mean_us_per_attempt=") != npos);
    CHECK(text.find("growth probe summary (app shutdown)") == npos);  // nothing left after the city's
    CHECK(text.find("its Grow calls are NOT counted (attempts stay 0)") != npos);
    CHECK(text.find("is NOT the original class (vtable ") != npos);
    CHECK(text.find("growth probe: hook removed: entry 0x") != npos);
    CHECK(text.find("4 Grow call(s) counted in all, 3 day line(s)") != npos);
    CHECK(text.find("growth probe: DLL detach: hook removed, the original restored") != npos);
    CHECK(text.find("'growth probe Grow exit'") == npos);  // no x87 leftovers at the forwarder's exit
    CHECK(text.find("config: WARNING unknown key") == npos);
    CHECK(text.find("WARNING: released last reference") == npos);
    CHECK(text.find("WARNING: over-release") == npos);
    if (g_failures == 0) std::printf("plugin growth probe test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}


// ---- neighbours (neighbors.md): the replacement, its objects, the shadow's start-up -------------------
constexpr uint32_t kCLSID_NeighborsN = 0xC990C0A4;
constexpr uint32_t kIID_cISC4NeighborsSimulatorN = 0x0773FEF0;
constexpr uint32_t kIID_cISC4NeighborDealN = 0x0773FEF1;
constexpr uint32_t kIID_cISC4NeighborConnectionN = 0x0773FEF2;
constexpr uint32_t kCLSID_NeighborConnectionN = 0x0A134F24;
constexpr uint32_t kCLSID_NeighborDealN = 0x2A134F42;
constexpr uint32_t kIID_CoreObjectN = 0x7F4C0B1E;  // FreeCity4's core objects: never answered here (§15.2)
constexpr uint32_t kIID_PropertyHolderN = 0x25216283;
constexpr uint32_t kIID_MessageTarget2N = 0x452294AA;
using NbType = cISC4NeighborsSimulator::ConnectionType;

// An ORIGINAL connection (the game's class 0x0A134F24) as a save hands it
// over: answers its interface and cIGZSerializable; counts its references.
class NbOrigConnection final : public cISC4NeighborConnection, public cIGZSerializable {
public:
    explicit NbOrigConnection(uint32_t gameRefs) { c.refs = c.gameRefs = gameRefs; }
    SavedRefCounts c;
    int writes = 0;
    cIGZSerializable* Ser() { return this; }
    cISC4NeighborConnection* Conn() { return this; }
    bool QueryInterface(uint32_t riid, void** p) override {
        if (p == nullptr) return false;
        if (riid == kIID_cISC4NeighborConnectionN || riid == GZIID_cIGZUnknown) {
            *p = Conn();
        } else if (riid == GZIID_cIGZSerializable) {
            *p = Ser();
        } else {
            return false;
        }
        c.Take();
        return true;
    }
    uint32_t AddRef() override { return c.Take(); }
    uint32_t Release() override { return c.Give(); }
    cISCPropertyHolder* AsISCPropertyHolder() override { return nullptr; }
    cISC4TrafficSource* AsTrafficSource() override { return nullptr; }
    NbType GetType() const override { return NbType::Rail; }
    void GetConnectionCell(int32_t& x, int32_t& z) const override {
        x = 3;
        z = -1;
    }
    bool GetConnectionInnerCell(int32_t& x, int32_t& z) const override {
        x = 3;
        z = 0;
        return true;
    }
    void GetRegionCellOnOtherSideOfConnection(int32_t& x, int32_t& z) const override {
        x = 7;
        z = 8;
    }
    cISC4RegionalCity* GetCityOnOtherSideOfConnection() const override { return nullptr; }
    void Repair(cISC4RegionalCity*) override {}
    bool GetVisitedThisCycle() const override { return false; }
    void SetVisitedThisCycle(bool) override {}
    bool GetIsActive() const override { return true; }
    void SetIsActive(bool) override {}
    bool Write(cIGZOStream&) override { return ++writes, true; }
    bool Read(cIGZIStream&) override { return true; }
    uint32_t GetGZCLSID() override { return kCLSID_NeighborConnectionN; }
};

// A foreign deal (another plugin's, or the original's): read through its
// interface only; the dates through the caller's date (Set, slot 4). A sell
// water deal whose cost needs more than 32 bits (NG11).
constexpr int32_t kNbForeignType = 0x4773FFE3;
constexpr int64_t kNbForeignCost = (int64_t{1} << 32) + 1234;
class NbForeignDeal final : public cISC4NeighborDeal {
public:
    SavedRefCounts c;
    std::vector<std::pair<int32_t, bool>> setFlags;  // SetFlags calls (NG4)
    NbForeignDeal() { c.refs = c.gameRefs = 1; }
    bool QueryInterface(uint32_t riid, void** p) override {
        if (p == nullptr || (riid != kIID_cISC4NeighborDealN && riid != GZIID_cIGZUnknown)) return false;
        *p = static_cast<cISC4NeighborDeal*>(this);
        c.Take();
        return true;
    }
    uint32_t AddRef() override { return c.Take(); }
    uint32_t Release() override { return c.Give(); }
    void SetRegionCellOnCitySideOfConnection(int32_t, int32_t) override {}
    void GetRegionCellOnCitySideOfConnection(int32_t& x, int32_t& z) const override { x = 4, z = 5; }
    void SetRegionCellOnOtherSideOfConnection(int32_t, int32_t) override {}
    void GetRegionCellOnOtherSideOfConnection(int32_t& x, int32_t& z) const override { x = 6, z = 5; }
    cISC4RegionalCity* GetCityOnOtherSideOfConnection() const override { return nullptr; }
    int32_t GetType() const override { return kNbForeignType; }
    void SetType(int32_t) override {}
    int32_t GetAmount() const override { return 50; }
    void SetAmount(int32_t) override {}
    int32_t GetLastAmount() const override { return 40; }
    void SetLastAmount(int32_t) override {}
    int32_t GetLastIncAmount() const override { return 4; }
    void SetLastIncAmount(int32_t) override {}
    int32_t GetIncAmount() const override { return 5; }
    void SetIncAmount(int32_t) override {}
    int32_t GetIncAmount2() const override { return 6; }
    void SetIncAmount2(int32_t) override {}
    int32_t GetAmountUsed() const override { return 30; }
    void SetAmountUsed(int32_t) override {}
    int64_t GetCost() const override { return kNbForeignCost; }
    void UpdateCost() override {}
    void GetStartDate(cIGZDate& d) const override { fc4::AsGame(&d)->SetDayNumber(kJulian20000201); }
    void SetStartDate(cIGZDate const&) override {}
    void GetExpirationDate(cIGZDate& d) const override { fc4::AsGame(&d)->SetDayNumber(kJulian20000201 + 365); }
    void SetExpirationDate(cIGZDate const&) override {}
    void GetCurrentDate(cIGZDate& d) override { fc4::AsGame(&d)->SetDayNumber(kJulian20000201 + 10); }
    void SetCurrentDate(cIGZDate const&) override {}
    void GetSuspensionDate(cIGZDate& d) override { fc4::AsGame(&d)->SetDayNumber(0); }
    void SetSuspensionDate(cIGZDate const&) override {}
    void Copy(cISC4NeighborDeal const&) override {}
    void UpdateState() override {}
    void Cancel() override {}
    bool Activate() override { return false; }
    int32_t GetFlags() const override { return 0x2; }  // Active
    void SetFlags(int32_t f, bool on) override { setFlags.emplace_back(f, on); }
    bool CheckFlagsAnd(int32_t f) const override { return (0x2 & f) == f; }
    bool CheckFlagsOr(int32_t f) const override { return (0x2 & f) != 0; }
    bool Clone(cISC4NeighborDeal**, bool) const override { return false; }
};

// A one-node caller list built in place (FindDealsAll reuses the node: list
// assignment, allocators.md); the node's value is managed by the test.
struct NbOneNodeList {
    SC4ListNode<cISC4NeighborDeal*> head{}, node{};
    SC4ListNode<cISC4NeighborDeal*>* root = nullptr;
    explicit NbOneNodeList(cISC4NeighborDeal* v) {
        head.next = &node;
        head.prev = &node;
        head.value = nullptr;
        node.next = &head;
        node.prev = &head;
        node.value = v;
        root = &head;
    }
    SC4List<cISC4NeighborDeal*>& Get() { return *reinterpret_cast<SC4List<cISC4NeighborDeal*>*>(&root); }
    bool OneNode() const { return root == &head && head.next == &node && node.next == &head; }
};

template <class T>
T* NbQuery(cIGZUnknown* u, uint32_t iid) {
    void* p = nullptr;
    return (u != nullptr && u->QueryInterface(iid, &p)) ? static_cast<T*>(p) : nullptr;
}

// Our deal (one reference held by the caller): its interfaces, its state as
// copied from the foreign deal (test seam op 48), Clone and the §5.6 record.
void CheckNeighborDeal(cISC4NeighborDeal* d) {
    void* p = reinterpret_cast<void*>(0x1234);
    CHECK(!d->QueryInterface(kIID_CoreObjectN, &p) && p == reinterpret_cast<void*>(0x1234));
    CHECK(!d->QueryInterface(kIID_PropertyHolderN, &p) && p == reinterpret_cast<void*>(0x1234));
    auto* same = NbQuery<cISC4NeighborDeal>(d, kIID_cISC4NeighborDealN);
    CHECK(same == d);
    if (same != nullptr) same->Release();
    CHECK(d->GetType() == kNbForeignType && d->GetAmount() == 50 && d->GetCost() == kNbForeignCost &&
          d->GetAmountUsed() == 30);
    CHECK(d->GetIncAmount() == 5 && d->GetLastAmount() == 40);
    int32_t x = 0, z = 0;
    d->GetRegionCellOnOtherSideOfConnection(x, z);
    CHECK(x == 6 && z == 5);
    d->GetRegionCellOnCitySideOfConnection(x, z);
    CHECK(x == 4 && z == 5);
    CHECK(d->CheckFlagsOr(0x2) && !d->CheckFlagsOr(0x80));
    cISC4NeighborDeal* copy = nullptr;
    CHECK(d->Clone(&copy, true) && copy != nullptr && copy != d);
    if (copy != nullptr) {
        CHECK(copy->GetType() == kNbForeignType && copy->GetAmount() == 50 && copy->CheckFlagsOr(0x80));  // Silent (§5.5)
        CHECK(copy->Release() == 0);
    }
    auto* ser = NbQuery<cIGZSerializable>(d, GZIID_cIGZSerializable);
    CHECK(ser != nullptr);
    if (ser != nullptr) {
        CHECK(ser->GetGZCLSID() == kCLSID_NeighborDealN);
        ResidentialSegOStream out;
        CHECK(ser->Write(out));
        // §5.6: version 5, then the type.
        CHECK(out.bytes.size() > 6 && out.bytes[0] == 5 && out.bytes[1] == 0 && out.bytes[2] == 0xE3 &&
              out.bytes[5] == 0x47);
        ser->Release();
    }
}

// Our connection (borrowed from the list): its interfaces, state, holder,
// traffic source and the §4.3 record (read back into the same object).
void CheckNeighborConnection(cISC4NeighborConnection* c) {
    void* p = reinterpret_cast<void*>(0x1234);
    CHECK(!c->QueryInterface(kIID_CoreObjectN, &p) && p == reinterpret_cast<void*>(0x1234));
    CHECK(!c->QueryInterface(kIID_PropertyHolderN, &p) && p == reinterpret_cast<void*>(0x1234));
    auto* same = NbQuery<cISC4NeighborConnection>(c, kIID_cISC4NeighborConnectionN);
    CHECK(same == c);
    if (same != nullptr) same->Release();
    CHECK(c->GetType() == NbType::Road);
    int32_t x = 0, z = 0;
    c->GetConnectionCell(x, z);
    CHECK(x == -1 && z == 5);
    CHECK(c->GetIsActive() && !c->GetVisitedThisCycle());
    c->SetVisitedThisCycle(true);
    CHECK(c->GetVisitedThisCycle());
    c->SetVisitedThisCycle(false);
    cISC4TrafficSource* ts = c->AsTrafficSource();
    CHECK(ts != nullptr);
    if (ts != nullptr) {
        CHECK(ts->GetTripDataList() == nullptr);  // no game node allocator in this process
        CHECK(ts->UpdateTrafficVolume());
        SC4Rect<uint32_t> r;
        CHECK(ts->GetSourceCells(r) && r.topLeftX == 0xFFFFFFFFu && r.topLeftY == 5 && r.bottomRightX == 0xFFFFFFFFu);
    }
    cISCPropertyHolder* h = c->AsISCPropertyHolder();
    CHECK(h != nullptr);
    if (h != nullptr) {
        auto* gh = fc4::AsGame(h);
        auto* hq = NbQuery<cIGZUnknown>(gh, kIID_PropertyHolderN);  // the holder answers itself
        CHECK(hq == static_cast<void*>(gh));
        if (hq != nullptr) hq->Release();
        CHECK(gh->AddPropertyU32(0xAA9EA81D, 7, false));
        uint32_t v = 0;
        CHECK(gh->GetPropertyU32(0xAA9EA81D, v) && v == 7);
    }
    auto* ser = NbQuery<cIGZSerializable>(c, GZIID_cIGZSerializable);
    CHECK(ser != nullptr);
    if (ser != nullptr) {
        CHECK(ser->GetGZCLSID() == kCLSID_NeighborConnectionN);
        ResidentialSegOStream out;
        CHECK(ser->Write(out));
        // §4.3: version 1, type 2, cell (-1, 5).
        CHECK(out.bytes.size() > 19 && out.bytes[0] == 1 && out.bytes[1] == 0 && out.bytes[2] == 2);
        CHECK(out.bytes.size() > 10 && out.bytes[6] == 0xFF && out.bytes[10] == 5);
        if (h != nullptr) fc4::AsGame(h)->RemoveAllProperties(false);
        ResidentialSegIStream in(out.bytes);
        CHECK(ser->Read(in));
        uint32_t v = 0;
        if (h != nullptr) CHECK(fc4::AsGame(h)->GetPropertyU32(0xAA9EA81D, v) && v == 7);  // the holder read back
        ser->Release();
    }
}

// The game's own property (cSCProperty) as the traffic simulator's element
// store makes one: one reference for its creator; its variant in the game's
// slot order (cFC4Variant, compiled into this test: not one of the DLL's
// objects). Counts its references; never deleted.
constexpr uint32_t kNbTripCapacityId = 0x8A0B4100;
class NbGameProperty final : public cISCProperty {
public:
    NbGameProperty() { c.refs = 1; }
    SavedRefCounts c;
    uint32_t id = 0;
    cFC4Variant value;
    bool QueryInterface(uint32_t riid, void** p) override {
        if (p == nullptr || (riid != 0x452160EC && riid != GZIID_cIGZUnknown)) return false;
        *p = static_cast<cISCProperty*>(this);
        c.Take();
        return true;
    }
    uint32_t AddRef() override { return c.Take(); }
    uint32_t Release() override { return c.Give(); }
    uint32_t GetPropertyID() const override { return id; }
    void SetPropertyID(uint32_t v) override { id = v; }
    cIGZVariant* GetPropertyValue() override {
        return reinterpret_cast<cIGZVariant*>(static_cast<cFC4IGZVariant*>(&value));
    }
    const cIGZVariant* GetPropertyValue() const override {
        return reinterpret_cast<const cIGZVariant*>(static_cast<const cFC4IGZVariant*>(&value));
    }
    bool SetPropertyValue(const cIGZVariant&) override { return false; }
    bool Write(cIGZOStream&) const override { return false; }
    bool Read(cIGZIStream&) override { return false; }
};

// foundations §10.2 "Ownership" (run r50e2): the holder keeps the caller's
// object. The traffic simulator's element store on a holder without
// 0x8A0B4100: (a) its own property with a 12-element u16 array, (b)
// SetPropertyID + AddProperty(p, false) (slot 15), (c) p->Release(), (d)
// p->GetPropertyValue() (slot 5), the u16 data getter (slot 99) and a write to
// one element; later readers see it through GetProperty (slot 9) -> value ->
// data. Then the copying setter (slot 102), a same-ID replacement (the old
// object released), the record written with the current values and read back
// (the kept object released).
void CheckHolderKeepsObjects(cISC4NeighborConnection* conn, NbGameProperty& a, NbGameProperty& b) {
    cISCPropertyHolder* sdk = conn->AsISCPropertyHolder();
    CHECK(sdk != nullptr);
    if (sdk == nullptr) return;
    cFC4ISCPropertyHolder* h = fc4::AsGame(sdk);
    h->RemoveAllProperties(false);
    CHECK(h->GetProperty(kNbTripCapacityId) == nullptr);
    // (a)
    uint16_t init[12] = {};
    a.value.SetRefUint16(init, 12);  // slot 98, copying
    // (b), (c)
    a.SetPropertyID(kNbTripCapacityId);
    CHECK(h->AddPropertyRecord(&a, false));
    CHECK(a.c.refs == 2);
    CHECK(a.Release() == 1);  // the holder's reference keeps it
    CHECK(!a.c.freed);
    // (d)
    cFC4IGZVariant* v = fc4::AsGame(a.GetPropertyValue());
    CHECK(v->GetType() == (cIGZVariant::Uint16 | cIGZVariant::TypeArray) && v->GetCount() == 12);
    uint16_t* data = v->RefUint16();  // slot 99
    CHECK(data != nullptr);
    if (data != nullptr) data[5] = 1234;
    // A later reader: the same object, borrowed, and the element written.
    cISCProperty* got = h->GetProperty(kNbTripCapacityId);
    CHECK(got == static_cast<cISCProperty*>(&a));
    CHECK(a.c.refs == 1);  // borrowed: no reference taken
    if (got != nullptr) {
        cFC4IGZVariant* gv = fc4::AsGame(got->GetPropertyValue());
        CHECK(gv == v);
        const uint16_t* gd = gv->RefUint16();
        CHECK(gd != nullptr && gd == data && gd[5] == 1234);
    }
    uint32_t first = 0;
    CHECK(h->HasProperty(kNbTripCapacityId) && h->GetPropertyU32(kNbTripCapacityId, first) && first == 0);
    // Slot 102 replaces the array in place; the holder reads the new values.
    uint32_t replaced[3] = {77, 88, 99};
    v->SetRefUint32(replaced, 3);
    CHECK(h->GetPropertyU32(kNbTripCapacityId, first) && first == 77);
    // NG23: no size query, and a u32 array is not raw bytes (length unchanged).
    uint32_t len = 77;
    uint8_t buf[4] = {};
    CHECK(!h->GetPropertyData(kNbTripCapacityId, nullptr, len) && len == 77);
    CHECK(!h->GetPropertyData(kNbTripCapacityId, buf, len) && len == 77);
    // A same-ID replacement releases the old object (here its last reference).
    uint16_t two[2] = {5, 6};
    b.value.SetRefUint16(two, 2);
    b.SetPropertyID(kNbTripCapacityId);
    CHECK(h->AddPropertyRecord(&b, false));
    CHECK(a.c.refs == 0 && a.c.freed && a.c.overReleases == 0);
    CHECK(b.Release() == 1);
    CHECK(h->GetProperty(kNbTripCapacityId) == static_cast<cISCProperty*>(&b));
    uint16_t* bd = fc4::AsGame(b.GetPropertyValue())->RefUint16();
    if (bd != nullptr) bd[1] = 4321;  // written in place after the add
    // The record (§4.3) holds the current values; reading it back replaces the
    // holder's contents (the kept object released).
    auto* ser = NbQuery<cIGZSerializable>(conn, GZIID_cIGZSerializable);
    CHECK(ser != nullptr);
    if (ser != nullptr) {
        ResidentialSegOStream out;
        CHECK(ser->Write(out));
        ResidentialSegIStream in(out.bytes);
        CHECK(ser->Read(in));
        CHECK(in.at == in.bytes.size() && in.error == 0);
        ser->Release();
    }
    CHECK(b.c.refs == 0 && b.c.freed && b.c.overReleases == 0);
    cISCProperty* back = h->GetProperty(kNbTripCapacityId);
    CHECK(back != nullptr && back != static_cast<cISCProperty*>(&b));
    if (back != nullptr) {
        cFC4IGZVariant* bv = fc4::AsGame(back->GetPropertyValue());
        CHECK(bv->GetType() == (cIGZVariant::Uint16 | cIGZVariant::TypeArray) && bv->GetCount() == 2);
        const uint16_t* d = bv->RefUint16();
        CHECK(d != nullptr && d[0] == 5 && d[1] == 4321);
    }
    h->RemoveAllProperties(false);
}

// NG23 (neighbors.md §15.4): holder slot 5 has no size query and answers raw
// bytes (type 0x8000) only: length := min(length, count), that many bytes
// copied; a null buffer is refused [choice]; other types and a missing
// property give false with the length unchanged. Example: the 5 bytes
// 01..05 read with length 8 (5 copied) and 3 (01 02 03).
void CheckGetPropertyData(cISC4NeighborConnection* conn, NbGameProperty& raw) {
    cISCPropertyHolder* sdk = conn->AsISCPropertyHolder();
    CHECK(sdk != nullptr);
    if (sdk == nullptr) return;
    cFC4ISCPropertyHolder* h = fc4::AsGame(sdk);
    uint8_t bytes[5] = {1, 2, 3, 4, 5};
    raw.value.SetRefVoid(bytes, 5);  // slot 122, copying
    CHECK(raw.value.GetType() == cIGZVariant::VoidArray && raw.value.GetCount() == 5);
    raw.SetPropertyID(0x5AFE0200);
    CHECK(h->AddPropertyRecord(&raw, false));
    CHECK(raw.Release() == 1);
    uint8_t buf[8] = {};
    uint32_t len = 8;
    CHECK(h->GetPropertyData(0x5AFE0200, buf, len) && len == 5);
    CHECK(buf[0] == 1 && buf[4] == 5 && buf[5] == 0);
    uint8_t three[8] = {};
    len = 3;
    CHECK(h->GetPropertyData(0x5AFE0200, three, len) && len == 3);
    CHECK(three[0] == 1 && three[2] == 3 && three[3] == 0);
    len = 0;
    CHECK(h->GetPropertyData(0x5AFE0200, buf, len) && len == 0);
    len = 8;
    CHECK(!h->GetPropertyData(0x5AFE0200, nullptr, len) && len == 8);
    CHECK(!h->GetPropertyData(0x5AFE0201, buf, len) && len == 8);
    CHECK(h->AddPropertyU32(0x5AFE0202, 7, false));
    CHECK(!h->GetPropertyData(0x5AFE0202, buf, len) && len == 8);
    CHECK(h->RemoveProperty(0x5AFE0202, false));
    CHECK(h->RemoveProperty(0x5AFE0200, false));
    CHECK(raw.c.refs == 0 && raw.c.freed && raw.c.overReleases == 0);
    // NG27: GetProperty(id, uint32&) on an array gives its first element; an
    // empty numeric array gives 0 and true.
    cFC4Variant arr;
    uint16_t trips[2] = {30, 0};
    arr.SetRefUint16(trips, 2);
    CHECK(h->AddPropertyVariant(0x5AFE0204, &arr, false));
    uint32_t u = 9;
    CHECK(h->GetPropertyU32(0x5AFE0204, u) && u == 30);
    arr.SetRefUint16(trips, 0);
    CHECK(arr.GetType() == (cIGZVariant::Uint16 | cIGZVariant::TypeArray) && arr.GetCount() == 0);
    CHECK(h->AddPropertyVariant(0x5AFE0205, &arr, false));
    u = 9;
    CHECK(h->GetPropertyU32(0x5AFE0205, u) && u == 0);
    CHECK(h->RemoveProperty(0x5AFE0204, false) && h->RemoveProperty(0x5AFE0205, false));
    // NG44 (data-formats §5.1): an empty character array gives the empty
    // text, which parses as 0: true and 0 (it was false); a string parses as
    // a number; a signed scalar sign-extends.
    char none[1] = {};
    arr.SetRefRZChar(none, 0);
    CHECK(h->AddPropertyVariant(0x5AFE0206, &arr, false));
    u = 9;
    CHECK(h->GetPropertyU32(0x5AFE0206, u) && u == 0);
    char twelve[2] = {'1', '2'};
    arr.SetRefRZChar(twelve, 2);
    CHECK(h->AddPropertyVariant(0x5AFE0207, &arr, false));
    CHECK(h->GetPropertyU32(0x5AFE0207, u) && u == 12);
    arr.SetValSint32(-1);
    CHECK(h->AddPropertyVariant(0x5AFE0208, &arr, false));
    CHECK(h->GetPropertyU32(0x5AFE0208, u) && u == 0xFFFFFFFFu);
    float big[1] = {1e30f};
    arr.SetRefFloat32(big, 1);
    CHECK(h->AddPropertyVariant(0x5AFE0209, &arr, false));
    CHECK(h->GetPropertyU32(0x5AFE0209, u) && u == 0x19058000u);
    CHECK(h->RemoveProperty(0x5AFE0206, false) && h->RemoveProperty(0x5AFE0207, false));
    CHECK(h->RemoveProperty(0x5AFE0208, false) && h->RemoveProperty(0x5AFE0209, false));
    // NG37: AddPropertyData (slot 10) stores raw bytes (type 0x8000, count =
    // length), which GetPropertyData reads back; a null `data` keeps the
    // count; the void array reads as 0 through slot 8.
    uint8_t five[5] = {9, 8, 7, 6, 5};
    CHECK(h->AddPropertyData(0x5AFE020A, five, 5, false));
    cISCProperty* dp = h->GetProperty(0x5AFE020A);
    CHECK(dp != nullptr);
    if (dp != nullptr) {
        const cFC4IGZVariant* dv = fc4::AsGame(dp->GetPropertyValue());
        CHECK(dv->GetType() == cIGZVariant::VoidArray && dv->GetCount() == 5);
        uint8_t back[5] = {};
        uint32_t want = 5;
        CHECK(dv->GetValVoid(back, want) && want == 5 && back[0] == 9 && back[4] == 5);  // slot 57
    }
    uint8_t got[8] = {};
    len = 8;
    CHECK(h->GetPropertyData(0x5AFE020A, got, len) && len == 5 && got[0] == 9 && got[4] == 5);
    u = 9;
    CHECK(h->GetPropertyU32(0x5AFE020A, u) && u == 0);
    CHECK(h->AddPropertyData(0x5AFE020A, nullptr, 3, false));
    len = 8;
    CHECK(h->GetPropertyData(0x5AFE020A, got, len) && len == 3);
    CHECK(h->RemoveProperty(0x5AFE020A, false));
}

int NeighborsEnableTest(const std::string& dll, const std::string& ini, const std::string& log) {
    // Off by default; `enable = neighbors` registers the neighbours simulator
    // (no factory for its connection and deal classes, §15.2). The shadow (it
    // compares against the original) is then inactive.
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    SetEnvironmentVariableA("FC4_NEIGHBOURS_SHADOW", nullptr);
    SetEnvironmentVariableA("FC4_NEIGHBORS_SEED", nullptr);
    SetEnvironmentVariableA("FC4_REFAUDIT", nullptr);
    WriteTextFile(ini, "[subsystems]\nenable = neighbors\nneighbors_seed = 42\n[tools]\nneighbours_shadow = 1\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr);
    SetGamePrecision();
    static NbOrigConnection orig(1);  // the load's own reference
    static NbForeignDeal foreign;
    static NbGameProperty propA, propB, propKept, propRaw;  // the game's properties (foundations §10.2)
    if (director != nullptr) {
        std::vector<uint32_t> classes;
        director->EnumClassObjects(&CollectClass, &classes);
        // simulator (1) + budget (3) + neighbours (1); no connection or deal factory.
        CHECK(classes.size() == 5);
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_NeighborsN) == 1);
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_NeighborConnectionN) == 0);
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_NeighborDealN) == 0);

        void* obj = nullptr;
        CHECK(director->GetClassObject(kCLSID_NeighborsN, kIID_cISC4NeighborsSimulatorN, &obj));
        auto* sim = static_cast<cFC4ISC4NeighborsSimulator*>(obj);
        if (sim != nullptr) {
            // §1: the primary for its IID and cIGZUnknown, the message target
            // and the serializable; nothing else (the out pointer untouched).
            void* p = nullptr;
            CHECK(sim->QueryInterface(GZIID_cIGZUnknown, &p) && p == static_cast<void*>(sim));
            if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
            auto* mt = NbQuery<cIGZMessageTarget2>(sim, kIID_MessageTarget2N);
            auto* ser = NbQuery<cIGZSerializable>(sim, GZIID_cIGZSerializable);
            CHECK(mt != nullptr && ser != nullptr);
            if (ser != nullptr) CHECK(ser->GetGZCLSID() == kCLSID_NeighborsN);
            p = reinterpret_cast<void*>(0x1234);
            CHECK(!sim->QueryInterface(0x12345678, &p) && p == reinterpret_cast<void*>(0x1234));
            CHECK(!sim->QueryInterface(kIID_CoreObjectN, &p) && p == reinterpret_cast<void*>(0x1234));
            // Before Init: nothing; the record (§13) is version 2 with no connection.
            CHECK(sim->GetTotalMonthlyNeighborDealIncome() == 0 && sim->GetTotalMonthlyNeighborDealExpense() == 0);
            CHECK(sim->GetTuningExemplar() == nullptr);
            CHECK(!sim->HasConnection(NbType::Road, -1, 5));
            if (ser != nullptr) {
                ResidentialSegOStream out;
                CHECK(ser->Write(out));
                CHECK(out.bytes == std::vector<uint8_t>({2, 0, 0, 0, 0, 0}));
            }
            CHECK(sim->Init());  // §14.1: true (no city here)
            CHECK(sim->GetTuningExemplar() == nullptr);
            // NG14 (§15.4): with popups disabled the original's confirmation
            // dialog answers no, so the request is refused. Enabled (NG35),
            // the dialog is asked and its answer returned; with no dialog (an
            // unidentified executable, as here) it is confirmed without
            // asking [choice]. The app's flag through its slot 23 (an app
            // whose other slots trap).
            static FakePopupApp popupApp;
            g_popupApp = &popupApp;
            popupApp.vt.slots[23] = reinterpret_cast<void*>(&AppGetPopupDialogsEnabled);
            popupApp.app.vtbl = popupApp.vt.slots.data();
            auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
            CHECK(seam != nullptr);
            if (seam != nullptr) {
                CHECK(seam(kFC4SeamNeighborsApp, &popupApp.app, nullptr) == 1);
                popupApp.enabled = false;
                CHECK(!sim->RequestCreateConnection(NbType::Road, -1, 5, false));
                CHECK(popupApp.gets == 1 && !sim->HasConnection(NbType::Road, -1, 5));
                popupApp.enabled = true;
                CHECK(sim->RequestCreateConnection(NbType::Road, -1, 5, false));
                CHECK(popupApp.gets == 2 && !sim->HasConnection(NbType::Road, -1, 5));
                // NG35: the dialog's answer, with the §6.2 title and text and
                // null labels; No refuses, Yes confirms.
                CHECK(seam(kFC4SeamNeighborsConfirm, reinterpret_cast<void*>(&FakeConfirmDialog), nullptr) == 1);
                g_fakeConfirm.answer = false;
                CHECK(!sim->RequestCreateConnection(NbType::Road, -1, 5, false));
                CHECK(g_fakeConfirm.calls == 1 && g_fakeConfirm.stringsOk && g_fakeConfirm.labelsNull);
                g_fakeConfirm.answer = true;
                CHECK(sim->RequestCreateConnection(NbType::Road, -1, 5, false));
                CHECK(g_fakeConfirm.calls == 2 && popupApp.gets == 4);
                // Popups off: refused before the dialog.
                popupApp.enabled = false;
                CHECK(!sim->RequestCreateConnection(NbType::Road, -1, 5, false));
                CHECK(g_fakeConfirm.calls == 2 && popupApp.gets == 5);
                popupApp.enabled = true;
                // NG36: no app (tests have no framework) counts as popups off.
                CHECK(seam(kFC4SeamNeighborsApp, nullptr, nullptr) == 1);
                CHECK(!sim->RequestCreateConnection(NbType::Road, -1, 5, false));
                CHECK(g_fakeConfirm.calls == 2);
                CHECK(seam(kFC4SeamNeighborsApp, &popupApp.app, nullptr) == 1);
                CHECK(seam(kFC4SeamNeighborsConfirm, nullptr, nullptr) == 1);
            }
            cISC4NeighborConnection* conn = sim->CreateConnection(NbType::Road, -1, 5, false);
            CHECK(conn != nullptr);
            if (seam != nullptr) {
                // It exists: refused before any dialog (§6.2).
                CHECK(!sim->RequestCreateConnection(NbType::Road, -1, 5, false));
                CHECK(popupApp.gets == 5);
                CHECK(seam(kFC4SeamNeighborsApp, nullptr, nullptr) == 1);
            }
            CHECK(sim->CreateConnection(NbType::Road, -1, 5, false) == nullptr);  // §6.3 step 1
            CHECK(sim->HasConnection(NbType::Road, -1, 5) && sim->FindConnection(NbType::Road, -1, 5) == conn);
            if (conn != nullptr) {
                CheckNeighborConnection(conn);
                CheckHolderKeepsObjects(conn, propA, propB);
                CheckGetPropertyData(conn, propRaw);
                // One kept until the connection goes (released with the holder).
                propKept.SetPropertyID(0x5AFE0100);
                CHECK(fc4::AsGame(conn->AsISCPropertyHolder())->AddPropertyRecord(&propKept, false));
                CHECK(propKept.Release() == 1);
            }
            // NG4 (§15.4): a foreign deal is kept as the listed deal: one
            // reference for the core, Silent cleared through its interface.
            // FindDealsAll reuses the caller's node and hands out the caller's
            // object itself (one reference for the caller).
            CHECK(foreign.AddRef() == 2);  // the caller list's reference
            NbOneNodeList list(&foreign);
            CHECK(sim->AddDeal(&foreign));
            CHECK(foreign.c.refs == 3);  // the game's, the caller list's and the core's
            CHECK(foreign.setFlags.size() == 1 && foreign.setFlags[0] == std::make_pair(int32_t{0x80}, false));
            sim->FindDealsAll(list.Get());
            CHECK(list.OneNode() && list.node.value == &foreign);
            CHECK(foreign.c.refs == 3 && foreign.c.overReleases == 0);  // the replaced element released
            CHECK(foreign.Release() == 2);                               // the caller list's
            // A deal of ours with the foreign deal's state (Copy, §5.4).
            cISC4NeighborDeal* ours = nullptr;
            if (seam != nullptr) CHECK(seam(kFC4SeamNeighborsApp, &foreign, &ours) == 1);
            CHECK(ours != nullptr && ours != &foreign);
            if (ours != nullptr && ours != &foreign) {
                CheckNeighborDeal(ours);
                CHECK(ours->Release() == 0);
            }
            cISC4NeighborDeal* proposed = nullptr;
            CHECK(!sim->ProposeDeal(1, nullptr, &proposed) && proposed == nullptr);  // no city (§7.1)
            sim->MarkDealsDirty();
            sim->SetUIDisableFlag(false);
            (void)sim->GetDealCost(1, 10.0, nullptr);
            // Getters from another thread (the lock is free: answered by the core).
            int64_t income = -1;  // NG11: slots 32/33 return an int64 in EDX:EAX
            bool has = false;
            std::thread t([&] {
                income = sim->GetTotalMonthlyNeighborDealIncome();
                has = sim->HasConnection(NbType::Road, -1, 5);
            });
            t.join();
            CHECK(income == kNbForeignCost && has);  // the foreign sell deal, all 64 bits (NG11)
            if (mt != nullptr) {
                ResidentialDayMessage month(kMsgSimNewMonth, kJulian20000201);
                CHECK(mt->DoMessage(&month));
            }
            // The record holds our connection (§13; ours by reference).
            if (ser != nullptr && conn != nullptr) {
                auto* connSer = NbQuery<cIGZSerializable>(conn, GZIID_cIGZSerializable);
                SavedSegOStream out;
                out.objs = {connSer};
                CHECK(ser->Write(out));
                CHECK(out.ids == std::vector<uint32_t>({1}));
                if (connSer != nullptr) connSer->Release();
            }
            sim->DeleteConnection(NbType::Road, -1, 5);
            // The connection object went with it: its holder released the kept property.
            CHECK(propKept.c.refs == 0 && propKept.c.freed && propKept.c.overReleases == 0);
            CHECK(!sim->HasConnection(NbType::Road, -1, 5));
            if (ser != nullptr) ser->Release();
            if (mt != nullptr) mt->Release();
            CHECK(sim->Shutdown());
            // Shutdown let the foreign deal go: the core's reference is back.
            CHECK(foreign.c.refs == foreign.c.gameRefs && foreign.c.overReleases == 0);
            CHECK(sim->Release() == 0);
        }
        // Read runs before Init (§13): an original connection is accepted (a
        // null reference skipped [choice]) and used through its interface;
        // written back by reference; the never-initialised simulator's final
        // Release gives the load's reference back. A version 3 record is refused.
        for (uint16_t version : {2, 3}) {
            obj = nullptr;
            CHECK(director->GetClassObject(kCLSID_NeighborsN, kIID_cISC4NeighborsSimulatorN, &obj));
            auto* fresh = static_cast<cFC4ISC4NeighborsSimulator*>(obj);
            if (fresh == nullptr) continue;
            auto* s = NbQuery<cIGZSerializable>(fresh, GZIID_cIGZSerializable);
            CHECK(s != nullptr);
            if (s != nullptr) {
                SavedRecord rec;
                rec.Put<uint16_t>(version).Put<uint32_t>(2).Put<uint32_t>(1).Put<uint32_t>(0);
                SavedSegIStream in(rec.b);
                in.objs = {orig.Ser()};
                CHECK(s->Read(in) == (version == 2));
                if (version == 2) {
                    CHECK(orig.c.refs == 2);  // the load's and the list's
                    CHECK(fresh->HasConnection(NbType::Rail, 3, -1));
                    CHECK(fresh->FindConnection(NbType::Rail, 3, -1) == orig.Conn());
                    SavedSegOStream out;
                    out.objs = {orig.Ser()};
                    CHECK(s->Write(out));
                    CHECK(out.ids == std::vector<uint32_t>({1}));
                    CHECK(orig.writes == 0);  // a reference, not the object's own record
                }
                s->Release();
            }
            CHECK(fresh->Release() == 0);
            CHECK(orig.c.refs == orig.c.gameRefs && orig.c.overReleases == 0);
        }
        CHECK(foreign.c.refs == foreign.c.gameRefs && foreign.c.overReleases == 0);
        CheckCw("neighbours");
    }
    FreeLibrary(hd);
    DeleteFileA(ini.c_str());
    const std::string text = ReadFile(log);
    const auto npos = std::string::npos;
    CHECK(text.find("subsystem 'neighbors': enabled (1 class replaced) by FreeCity4.ini (enable=)") != npos);
    CHECK(CountOf(text, ", session seed 0x0000002A (neighbors_seed)") == 3);
    CHECK(text.find("neighbours Init: init=1 ready=0 connections=0 (foreign 0) deals=0") != npos);
    CHECK(CountOf(text, "neighbours: RequestCreateConnection refused: popup dialogs are disabled") == 1);
    CHECK(CountOf(text, "neighbours: RequestCreateConnection confirms without the yes/no dialog") == 1);
    CHECK(CountOf(text, "neighbours connection created: road (type 2) at (-1, 5)") == 1);
    CHECK(CountOf(text, "neighbours connection deleted: road (type 2) at (-1, 5)") == 1);
    CHECK(text.find("neighbours month to 2000-02-01: connections=1 (road=1) foreign=0 deals=1 (proposed 0 active 1") !=
          npos);
    CHECK(CountOf(text, "neighbours Write -> 1") == 3);
    CHECK(text.find("neighbours Read -> 1") != npos);
    CHECK(text.find("neighbours Read: init=0 ready=0 connections=1 (foreign 1)") != npos);
    CHECK(text.find("neighbours Read -> 0") != npos);
    CHECK(text.find("neighbours Shutdown: 1 months") != npos);
    CHECK(text.find("neighbours shadow: requested (neighbours_shadow from FreeCity4.ini) but INACTIVE") != npos);
    CHECK(text.find("neighbours shadow: ON") == npos);
    CHECK(text.find("structured exception") == npos);
    CHECK(text.find("config: WARNING unknown key") == npos);
    CHECK(text.find("WARNING: released last reference") == npos);
    CHECK(text.find("WARNING: over-release") == npos);
    CHECK(text.find("streams: WARNING") == npos);
    if (g_failures == 0) std::printf("plugin neighbours enable test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}

// ---- the zone developer / lot manager (enable = zonedev; test seam op 53) -----------------------------
//
// No city. The QI table (zones.md §1), neutral answers before Init, the
// record (§10.2), then a stub "game" behind the seam's host table: an 8 x 8
// zone grid, fake lots (a vtable each: QueryInterface for cISC4Lot and
// cIGZSerializable, a reference count the host table's add_ref / release
// share), the maximum-capacity grid. The B nodes and C blocks come from
// malloc through the seam. STAGE A: the host builds no lot (new_lot left to
// the plugin), so CreateLot and parcellization fail and say so; with the
// stub's new_lot the lists (slots 7-10), the safe lot iterator's raw layout
// (slot 15), DeleteLot and the shadow on a fake original are checked.

constexpr uint32_t kCLSID_ZoneDeveloper = 0x498F9B01;
constexpr uint32_t kIID_ZoneDeveloper = 0xE818ED2B;
constexpr uint32_t kIID_LotManager = 0x681905BD;
constexpr uint32_t kIID_ZdLot = 0xE818FCED;
constexpr uint32_t kZdLotClass = 0x6A3F2B1Cu;  // the fake lots' class ID (any)
constexpr uint64_t kZdGrid = 0x7B000000u;

struct ZdLot {
    void** vtbl = nullptr;
    int refs = 1;  // the test's own (never given back)
    int32_t x = 0, z = 0, w = 1, h = 1, facing = 0, zone = 0;
    uint32_t state = 0;
    int inits = 0, shutdowns = 0;
};

struct ZdStub {
    FakeVtable lotVt;
    std::array<ZdLot, 64> lots{};
    size_t used = 0;
    bool refuse = false;
    int8_t zone[8][8] = {};
    int newLots = 0, defers = 0, services = 0, levellings = 0, streets = 0, demolitions = 0;
    int gridCreates = 0, gridReleases = 0, foreignRefs = 0;
    int dezones = 0, joins = 0, zoneUpdates = 0;  // ABI 55
    std::vector<uint32_t> subscribed, unsubscribed;
};
ZdStub* g_zd = nullptr;
std::atomic<int> g_zdBlocks{0};  // malloc'd through the seam and not freed yet

void* ZdAllocate(uint32_t n) {
    ++g_zdBlocks;
    return std::malloc(n);
}
void ZdFree(void* p) {
    if (p == nullptr) return;
    --g_zdBlocks;
    std::free(p);
}

ZdLot* ZdLotOf(uint64_t t) { return reinterpret_cast<ZdLot*>(static_cast<uintptr_t>(t)); }
uint64_t ZdToken(const ZdLot* l) { return static_cast<uint64_t>(reinterpret_cast<uintptr_t>(l)); }
bool ZdIsLot(uint64_t t) {
    const ZdLot* p = ZdLotOf(t);
    return g_zd != nullptr && p >= g_zd->lots.data() && p < g_zd->lots.data() + g_zd->lots.size();
}

bool __fastcall ZdLotQi(void* self, void*, uint32_t iid, void** out) {  // 0
    if (out == nullptr || (iid != kIID_ZdLot && iid != GZIID_cIGZSerializable && iid != GZIID_cIGZUnknown)) return false;
    *out = self;
    ++static_cast<ZdLot*>(self)->refs;
    return true;
}
uint32_t __fastcall ZdLotAddRef(void* self, void*) { return static_cast<uint32_t>(++static_cast<ZdLot*>(self)->refs); }
uint32_t __fastcall ZdLotRelease(void* self, void*) { return static_cast<uint32_t>(--static_cast<ZdLot*>(self)->refs); }
uint32_t __fastcall ZdLotClass(void*, void*) { return kZdLotClass; }  // as cIGZSerializable, slot 5

// The host table's callbacks (the stub is global).
bool ZdAcquire(void*) {
    ++g_zd->services;
    return true;
}
void ZdReleaseServices(void*) { --g_zd->services; }
void ZdSubscribe(void*, uint32_t m) { g_zd->subscribed.push_back(m); }
void ZdUnsubscribe(void*, uint32_t m) { g_zd->unsubscribed.push_back(m); }
void ZdCellCounts(void*, int32_t* x, int32_t* z) {
    *x = 8;
    *z = 8;
}
void ZdTools(void*) {}
void ZdDefer(void*, bool) { ++g_zd->defers; }
int32_t ZdZoneType(void*, int32_t x, int32_t z) { return (x >= 0 && x < 8 && z >= 0 && z < 8) ? g_zd->zone[x][z] : 0; }
void ZdDemolish(void*, const fc4_zone_region*) { ++g_zd->demolitions; }
bool ZdStreetPlace(void*, uint32_t, int32_t, int32_t, int32_t, int32_t, bool) {
    ++g_zd->streets;
    return true;
}
void ZdResetTool(void*, uint32_t) {}
bool ZdConnects(void*, uint64_t, int32_t, uint32_t) { return false; }
bool ZdToolExists(void*, uint32_t, int32_t, int32_t, uint32_t, uint8_t*) { return false; }
void ZdJoining(void*, uint64_t, uint32_t) {}
bool ZdWater(void*, int32_t, int32_t) { return false; }
float ZdAltitude(void*, float, float) { return 100.0f; }
float ZdVertexAltitude(void*, int32_t, int32_t) { return 100.0f; }
bool ZdInBounds(void*, const float* p) { return p[0] >= 0.0f && p[2] >= 0.0f && p[0] < 128.0f && p[2] < 128.0f; }
bool ZdPositionToCell(void* u, const float* p, int32_t* x, int32_t* z) {
    *x = static_cast<int32_t>(std::floor(p[0] / 16.0f));
    *z = static_cast<int32_t>(std::floor(p[2] / 16.0f));
    return ZdInBounds(u, p);
}
void ZdCellOccupants(void*, int32_t, int32_t, bool, uint32_t, fc4_token_callback, void*) {}
uint32_t ZdOccupantType(void*, uint64_t) { return 0; }
void ZdOccupantPosition(void*, uint64_t, float* p) { p[0] = p[1] = p[2] = 0.0f; }
int32_t ZdNetworkFlag(void*, uint64_t, uint32_t) { return -1; }
void ZdRemoveOccupant(void*, uint64_t) {}
void ZdAddRef(void*, uint64_t t) {
    if (ZdIsLot(t)) {
        ++ZdLotOf(t)->refs;
    } else if (t != 0) {
        ++g_zd->foreignRefs;
    }
}
void ZdRelease(void*, uint64_t t) {
    if (ZdIsLot(t)) {
        --ZdLotOf(t)->refs;
    } else if (t != 0) {
        --g_zd->foreignRefs;
    }
}
uint64_t ZdNewLot(void*, int32_t x, int32_t z, int32_t w, int32_t h, int32_t facing) {
    ++g_zd->newLots;
    if (g_zd->refuse || g_zd->used >= g_zd->lots.size()) return 0;
    ZdLot& l = g_zd->lots[g_zd->used++];
    l.vtbl = g_zd->lotVt.slots.data();
    l.refs = 1;
    l.x = x;
    l.z = z;
    l.w = w;
    l.h = h;
    l.facing = facing;
    l.zone = ZdZoneType(nullptr, x, z);
    return ZdToken(&l);
}
void ZdLotInit(void*, uint64_t t) {
    if (ZdIsLot(t)) ++ZdLotOf(t)->inits;
}
void ZdLotShutdown(void*, uint64_t t) {
    if (ZdIsLot(t)) ++ZdLotOf(t)->shutdowns;
}
void ZdLotNoop(void*, uint64_t) {}
uint64_t ZdLotCfg(void*, uint64_t) { return 0; }
uint32_t ZdLotFlags(void*, uint64_t) { return 0; }
void ZdLotRect(void*, uint64_t t, fc4_cell_rect* o) {
    const ZdLot* l = ZdLotOf(t);
    *o = fc4_cell_rect{l->x, l->z, l->x + l->w - 1, l->z + l->h - 1};
}
bool __fastcall ZdLotRectOutput(void* self, void*, SC4Rect<int32_t>* out) {
    const auto* l = static_cast<const ZdLot*>(self);
    *out = SC4Rect<int32_t>(l->x, l->z, l->x + l->w - 1, l->z + l->h - 1);
    return false;  // slot 14's output is valid independently of its unspecified AL
}
float ZdLotAltitude(void*, uint64_t) { return 50.0f; }
void ZdLotLocation(void*, uint64_t t, int32_t* x, int32_t* z) {
    *x = ZdLotOf(t)->x;
    *z = ZdLotOf(t)->z;
}
int32_t ZdLotFacing(void*, uint64_t t) { return ZdLotOf(t)->facing; }
bool ZdLotBu(void*, uint64_t) { return false; }
int32_t ZdLotZone(void*, uint64_t t) { return ZdLotOf(t)->zone; }
uint32_t ZdLotState(void*, uint64_t t) { return ZdLotOf(t)->state; }
void ZdLotSetState(void*, uint64_t t, uint32_t s, uint32_t, uint32_t) { ZdLotOf(t)->state = s; }
void ZdLotLocked(void*, uint64_t, bool) {}
bool ZdCfgBool(void*, uint64_t) { return false; }
bool ZdCfgBoolI(void*, uint64_t, int32_t) { return false; }
uint32_t ZdCfgId(void*, uint64_t) { return 0; }
void ZdCfgSize(void*, uint64_t, uint32_t* x, uint32_t* z) { *x = *z = 0; }
uint8_t ZdCfgStage(void*, uint64_t) { return 0; }
bool ZdCfgBoolU(void*, uint64_t, uint32_t) { return false; }
bool ZdCfgIds(void*, uint64_t, fc4_u32_callback, void*) { return false; }
void ZdConfigurations(void*, fc4_token_callback, void*) {}
void ZdLevelling(void*, const fc4_cell_rect*) { ++g_zd->levellings; }
uint64_t ZdBaseTexture(void*, uint64_t) { return 0; }
bool ZdInsertion(void*, uint64_t*, bool*) { return false; }
void ZdSetInsertion(void*, uint64_t) {}
uint64_t ZdBdsDeveloper(void*, uint32_t) { return 0; }
void ZdByFamily(void*, uint32_t, fc4_u32_callback, void*) {}
bool ZdProfile(void*, uint32_t, fc4_zonedev_profile*) { return false; }
int16_t ZdDevI16(void*, uint64_t, int32_t, int32_t) { return 0; }
bool ZdDevDemand(void*, uint64_t, float*) { return false; }
bool ZdDevList(void*, uint64_t, fc4_growth_items_sink, void*) { return false; }
uint32_t ZdDevU32(void*, uint64_t) { return 0; }
uint64_t ZdGridCreate(void*, uint32_t, uint32_t, int32_t) {
    ++g_zd->gridCreates;
    return kZdGrid;
}
uint32_t ZdGridClass(void*, uint64_t) { return 0x49B9E604; }
void ZdGridGeometry(void*, uint64_t, fc4_grid_geometry* o) {
    *o = fc4_grid_geometry{4, 2, 2, 2, 64.0f, 64.0f, 1.0f / 64.0f, 1.0f / 64.0f};
}
double ZdGridGet(void*, uint64_t, uint32_t, int32_t, int32_t) { return 0.0; }
void ZdGridShutdown(void*, uint64_t) {}
void ZdGridRelease(void*, uint64_t) { ++g_zd->gridReleases; }

// The master_* mirror callbacks stay NULL: the plugin's raw list runs.
// ABI 55 (re-specs11): flat land, no slope, no configuration objects, no
// networks; the dezone and the joins only counted (the stub's zones stay).
float ZdAreaSlope(void*, const fc4_cell_rect*, bool) { return 0.0f; }
float ZdCfgMaxSlope(void*, uint64_t) { return 100.0f; }
void ZdCfgObjects(void*, uint64_t, fc4_zonedev_cfg_object_callback, void*) {}
uint32_t ZdNetU32(void*, uint64_t) { return 0; }
bool ZdNetAll(void*, uint64_t, uint32_t) { return false; }
uint32_t ZdNetEdges(void*, uint64_t, uint32_t) { return 0; }
int32_t ZdNetIsOfType(void*, uint64_t, uint32_t) { return -1; }
bool ZdNetBool(void*, uint64_t) { return false; }
void ZdOccCells(void*, uint64_t, fc4_cell_rect* out) { *out = fc4_cell_rect{0, 0, -1, -1}; }
bool ZdToolPresent(void*, uint32_t) { return false; }
void ZdJoin(void*, uint32_t, int32_t, int32_t, int32_t, int32_t) { ++g_zd->joins; }
bool ZdLand(void*, int32_t, int32_t) { return true; }
bool ZdCellInBounds(void*, int32_t x, int32_t z) { return x >= 0 && x < 8 && z >= 0 && z < 8; }
bool ZdNavigable(void*, int32_t, int32_t) { return false; }
void ZdLotUpdateZone(void*, uint64_t t) {
    ++g_zd->zoneUpdates;
    if (ZdIsLot(t)) ZdLotOf(t)->zone = ZdZoneType(nullptr, ZdLotOf(t)->x, ZdLotOf(t)->z);
}
bool ZdForceEmpty(void*) { return false; }
void ZdDezone(void*, const fc4_zone_region*) { ++g_zd->dezones; }

fc4_zonedev_host ZdStubHost() {
    fc4_zonedev_host h{};
    h.struct_size = sizeof(h);
    h.user = g_zd;
    h.grids.struct_size = sizeof(h.grids);
    h.grids.user = g_zd;
    h.grids.create = &ZdGridCreate;
    h.grids.class_of = &ZdGridClass;
    h.grids.geometry = &ZdGridGeometry;
    h.grids.get = &ZdGridGet;
    h.grids.shutdown = &ZdGridShutdown;
    h.grids.release = &ZdGridRelease;
    h.acquire_services = &ZdAcquire;
    h.release_services = &ZdReleaseServices;
    h.subscribe = &ZdSubscribe;
    h.unsubscribe = &ZdUnsubscribe;
    h.cell_counts = &ZdCellCounts;
    h.acquire_network_tools = &ZdTools;
    h.release_network_tools = &ZdTools;
    h.post_defer_occupant_operations = &ZdDefer;
    h.zone_type = &ZdZoneType;
    h.demolish_region = &ZdDemolish;
    h.street_place = &ZdStreetPlace;
    h.reset_network_tool = &ZdResetTool;
    h.connects_to_neighbor = &ZdConnects;
    h.network_tool_occupant_exists = &ZdToolExists;
    h.street_joining_helper = &ZdJoining;
    h.cell_has_water = &ZdWater;
    h.terrain_altitude = &ZdAltitude;
    h.terrain_vertex_altitude = &ZdVertexAltitude;
    h.location_in_bounds = &ZdInBounds;
    h.position_to_cell = &ZdPositionToCell;
    h.cell_occupants = &ZdCellOccupants;
    h.occupant_type = &ZdOccupantType;
    h.occupant_position = &ZdOccupantPosition;
    h.network_has_any_flag = &ZdNetworkFlag;
    h.remove_occupant = &ZdRemoveOccupant;
    h.add_ref = &ZdAddRef;
    h.release = &ZdRelease;
    h.new_lot = &ZdNewLot;
    h.lot_init = &ZdLotInit;
    h.lot_shutdown = &ZdLotShutdown;
    h.lot_post_city_init = &ZdLotNoop;
    h.lot_configuration = &ZdLotCfg;
    h.lot_flags = &ZdLotFlags;
    h.lot_bounding_rect = &ZdLotRect;
    h.lot_altitude = &ZdLotAltitude;
    h.lot_location = &ZdLotLocation;
    h.lot_facing = &ZdLotFacing;
    h.lot_has_business_unit = &ZdLotBu;
    h.lot_zone_type = &ZdLotZone;
    h.lot_state = &ZdLotState;
    h.lot_set_state = &ZdLotSetState;
    h.lot_set_vertices_locked = &ZdLotLocked;
    h.cfg_compatible_with_water = &ZdCfgBool;
    h.cfg_compatible_with_zone = &ZdCfgBoolI;
    h.cfg_id = &ZdCfgId;
    h.cfg_size = &ZdCfgSize;
    h.cfg_growth_stage = &ZdCfgStage;
    h.cfg_compatible_with_wealth = &ZdCfgBoolU;
    h.cfg_compatible_with_purpose = &ZdCfgBoolU;
    h.cfg_primary_building_ids = &ZdCfgIds;
    h.lot_configurations = &ZdConfigurations;
    h.attempt_area_levelling = &ZdLevelling;
    h.remove_building_zot_prop = &ZdLotNoop;
    h.lot_base_texture_occupant = &ZdBaseTexture;
    h.remove_lot_from_business_unit = &ZdLotNoop;
    h.set_empty_lot_base_texture = &ZdLotNoop;
    h.desired_lot_insertion_point = &ZdInsertion;
    h.set_lot_insertion_point = &ZdSetInsertion;
    h.bds_developer = &ZdBdsDeveloper;
    h.building_types_by_family = &ZdByFamily;
    h.building_profile = &ZdProfile;
    h.dev_estimate_desirability = &ZdDevI16;
    h.dev_source_demand = &ZdDevDemand;
    h.dev_growth_list = &ZdDevList;
    h.dev_desirability_value = &ZdDevI16;
    h.dev_desirability_shift = &ZdDevU32;
    h.dev_purpose = &ZdDevU32;
    h.dev_wealth = &ZdDevU32;
    h.area_slope = &ZdAreaSlope;
    h.cfg_max_slope = &ZdCfgMaxSlope;
    h.cfg_objects = &ZdCfgObjects;
    h.network_flags = &ZdNetU32;
    h.network_has_all_flags = &ZdNetAll;
    h.network_edge_mask = &ZdNetEdges;
    h.network_is_of_type = &ZdNetIsOfType;
    h.network_is_flag_0x400000 = &ZdNetBool;
    h.occupant_bounding_cells = &ZdOccCells;
    h.network_tool_present = &ZdToolPresent;
    h.network_tool_join = &ZdJoin;
    h.cell_has_land = &ZdLand;
    h.cell_is_in_bounds = &ZdCellInBounds;
    h.is_navigable = &ZdNavigable;
    h.lot_update_zone_type = &ZdLotUpdateZone;
    h.force_empty_lot_base_textures = &ZdForceEmpty;
    h.zone_manager_dezone = &ZdDezone;
    return h;
}

intptr_t ZdHooks(FC4TestSeamFn seam, cIGZUnknown* zd, const fc4_zonedev_host* host, bool allocators) {
    FC4ZoneDevTestInput in{};
    in.op = kFC4ZoneDevTestHooks;
    in.zoneDeveloper = zd;
    in.host = host;
    in.allocate = allocators ? &ZdAllocate : nullptr;
    in.free = allocators ? &ZdFree : nullptr;
    return seam(kFC4SeamZoneDev, &in, nullptr);
}

FC4ZoneDevTestState ZdState(FC4TestSeamFn seam, cIGZUnknown* zd) {
    FC4ZoneDevTestInput in{};
    in.op = kFC4ZoneDevTestState;
    in.zoneDeveloper = zd;
    FC4ZoneDevTestState st{};
    CHECK(seam(kFC4SeamZoneDev, &in, &st) == 1);
    return st;
}

// A caller's std::list<cISC4Lot*> (one pointer to a head node; B nodes).
struct ZdList {
    struct Node {
        Node* next;
        Node* prev;
        cISC4Lot* value;
    };
    Node* root = nullptr;
    ZdList() {
        root = static_cast<Node*>(ZdAllocate(sizeof(Node)));
        root->next = root->prev = root;
        root->value = nullptr;
    }
    ZdList(const ZdList&) = delete;
    ZdList& operator=(const ZdList&) = delete;
    fc4::GameList<cISC4Lot*>& Get() { return reinterpret_cast<fc4::GameList<cISC4Lot*>&>(root); }
    std::vector<ZdLot*> Lots() const {
        std::vector<ZdLot*> v;
        for (Node* n = root->next; n != root; n = n->next) v.push_back(reinterpret_cast<ZdLot*>(n->value));
        return v;
    }
    // As a caller may hand it in: an element with its reference.
    void Push(ZdLot* l) {
        Node* n = static_cast<Node*>(ZdAllocate(sizeof(Node)));
        n->value = reinterpret_cast<cISC4Lot*>(l);
        ++l->refs;
        n->next = root;
        n->prev = root->prev;
        root->prev->next = n;
        root->prev = n;
    }
    // As the caller does: release each element, free the nodes and the head.
    ~ZdList() {
        for (Node* n = root->next; n != root;) {
            Node* next = n->next;
            if (n->value != nullptr) --reinterpret_cast<ZdLot*>(n->value)->refs;
            ZdFree(n);
            n = next;
        }
        ZdFree(root);
    }
};

// The shadow's "original": a zone developer, lot manager and serializable
// answering from a live FreeCity4 zone developer (so they agree), and a city
// answering GetLotManager (33), GetZoneDeveloper (75) and the cell counts
// (93, 94). `countSkew` is added to slot 16.
struct ZdOrig {
    FakeVtable zdVt, lmVt, serVt, cityVt;
    FakeObject zd{}, lm{}, ser{}, city{};
    cFC4ISC4LotManager* liveLm = nullptr;
    cIGZSerializable* liveSer = nullptr;
    void* cityZd = nullptr;
    void* cityLm = nullptr;
    int refs = 1;
    int writes = 0;
    int32_t countSkew = 0;
};
ZdOrig* g_zo = nullptr;

bool __fastcall ZoQi(void* self, void*, uint32_t iid, void** out) {
    if (out == nullptr) return false;
    if (iid == GZIID_cIGZSerializable && self != &g_zo->lm) {
        *out = &g_zo->ser;
    } else if (iid == kIID_ZoneDeveloper || iid == GZIID_cIGZUnknown) {
        *out = &g_zo->zd;
    } else {
        return false;
    }
    ++g_zo->refs;
    return true;
}
uint32_t __fastcall ZoAddRef(void*, void*) { return static_cast<uint32_t>(++g_zo->refs); }
uint32_t __fastcall ZoRelease(void*, void*) { return static_cast<uint32_t>(--g_zo->refs); }
bool __fastcall ZoWrite(void*, void*, cIGZOStream* s) {  // serializable 3
    ++g_zo->writes;
    return g_zo->liveSer->Write(*s);
}
uint32_t __fastcall ZoClass(void*, void*) { return kCLSID_ZoneDeveloper; }  // serializable 5
cISC4Lot* __fastcall ZoGetLot(void*, void*, int32_t x, int32_t z, uint32_t originOnly) {  // 6
    return g_zo->liveLm->GetLot(x, z, (originOnly & 0xFFu) != 0);
}
uint32_t __fastcall ZoInCellRect(void*, void*, fc4::GameList<cISC4Lot*>* l, int32_t x1, int32_t z1, int32_t x2,  // 7
                                 int32_t z2, uint32_t originOnly) {
    return g_zo->liveLm->GetLotsInCellRect(*l, x1, z1, x2, z2, (originOnly & 0xFFu) != 0);
}
uint32_t __fastcall ZoSurroundingRect(void*, void*, const SC4Rect<long>* r, fc4::GameList<cISC4Lot*>* l,  // 10
                                      int32_t flags) {
    return g_zo->liveLm->GetLotsSurroundingRect(*r, *l, flags);
}
cISC4Lot* __fastcall ZoOccupantLot(void*, void*, cISC4Occupant* o) { return g_zo->liveLm->GetOccupantLot(o); }  // 14
int32_t __fastcall ZoCount(void*, void*) { return g_zo->liveLm->GetLotCount() + g_zo->countSkew; }             // 16
int32_t __fastcall ZoCanCreate(void*, void*, int32_t x, int32_t z, int32_t w, int32_t h, uint32_t zoneNotNeeded,  // 20
                               uint32_t waterAllowed, cISC4LotConfiguration* cfg) {
    return g_zo->liveLm->CanCreateLot(x, z, w, h, (zoneNotNeeded & 0xFFu) != 0, (waterAllowed & 0xFFu) != 0, cfg);
}
void* __fastcall ZoCityLm(void*, void*) { return g_zo->cityLm; }  // city 33
void* __fastcall ZoCityZd(void*, void*) { return g_zo->cityZd; }  // city 75
uint32_t __fastcall ZoCells(void*, void*) { return 8; }            // city 93, 94

void InitZdOrig(ZdOrig& o, cISC4ZoneDeveloper* live) {
    g_zo = &o;
    void* p = nullptr;
    if (live->QueryInterface(kIID_LotManager, &p) && p != nullptr) {
        o.liveLm = static_cast<cFC4ISC4LotManager*>(p);
        o.liveLm->Release();
    }
    p = nullptr;
    if (live->QueryInterface(GZIID_cIGZSerializable, &p) && p != nullptr) {
        o.liveSer = static_cast<cIGZSerializable*>(p);
        o.liveSer->Release();
    }
    for (FakeVtable* vt : {&o.zdVt, &o.lmVt, &o.serVt, &o.cityVt}) {
        vt->slots[0] = reinterpret_cast<void*>(&ZoQi);
        vt->slots[1] = reinterpret_cast<void*>(&ZoAddRef);
        vt->slots[2] = reinterpret_cast<void*>(&ZoRelease);
    }
    o.serVt.slots[3] = reinterpret_cast<void*>(&ZoWrite);
    o.serVt.slots[5] = reinterpret_cast<void*>(&ZoClass);
    o.lmVt.slots[6] = reinterpret_cast<void*>(&ZoGetLot);
    o.lmVt.slots[7] = reinterpret_cast<void*>(&ZoInCellRect);
    o.lmVt.slots[10] = reinterpret_cast<void*>(&ZoSurroundingRect);
    o.lmVt.slots[14] = reinterpret_cast<void*>(&ZoOccupantLot);
    o.lmVt.slots[16] = reinterpret_cast<void*>(&ZoCount);
    o.lmVt.slots[20] = reinterpret_cast<void*>(&ZoCanCreate);
    o.cityVt.slots[33] = reinterpret_cast<void*>(&ZoCityLm);
    o.cityVt.slots[75] = reinterpret_cast<void*>(&ZoCityZd);
    o.cityVt.slots[93] = reinterpret_cast<void*>(&ZoCells);
    o.cityVt.slots[94] = reinterpret_cast<void*>(&ZoCells);
    o.zd.vtbl = o.zdVt.slots.data();
    o.lm.vtbl = o.lmVt.slots.data();
    o.ser.vtbl = o.serVt.slots.data();
    o.city.vtbl = o.cityVt.slots.data();
    o.cityZd = &o.zd;
    o.cityLm = &o.lm;
}

intptr_t ZdShadowRun(FC4TestSeamFn seam, ZdOrig& o, const fc4_zonedev_host* host, uint32_t event) {
    FC4ZoneDevTestInput in{};
    in.op = kFC4ZoneDevTestShadow;
    in.host = host;
    in.city = reinterpret_cast<cISC4City*>(&o.city);
    in.julian = kJulian20000201;
    in.event = event;
    return seam(kFC4SeamZoneDev, &in, nullptr);
}

cISC4ZoneDeveloper* NewZoneDeveloper(cIGZCOMDirector* director) {
    void* obj = nullptr;
    if (!director->GetClassObject(kCLSID_ZoneDeveloper, kIID_ZoneDeveloper, &obj)) return nullptr;
    return static_cast<cISC4ZoneDeveloper*>(obj);
}

cFC4ISC4LotManager* ZdLotManager(cISC4ZoneDeveloper* zd) {
    void* p = nullptr;
    if (!zd->QueryInterface(kIID_LotManager, &p) || p == nullptr) return nullptr;
    return static_cast<cFC4ISC4LotManager*>(p);  // with a reference
}

std::vector<uint8_t> ZdWrite(cISC4ZoneDeveloper* zd, int* nullRefs = nullptr) {
    std::vector<uint8_t> out;
    cIGZSerializable* ser = nullptr;
    if (!zd->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) || ser == nullptr) return out;
    ResidentialSegOStream s;
    if (ser->Write(s)) out = s.bytes;
    if (nullRefs != nullptr) *nullRefs = s.nullRefs;
    ser->Release();
    return out;
}

bool ZdRead(cISC4ZoneDeveloper* zd, const std::vector<uint8_t>& bytes) {
    cIGZSerializable* ser = nullptr;
    if (!zd->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) || ser == nullptr) return false;
    ResidentialSegIStream s(bytes);
    const bool ok = ser->Read(s);
    ser->Release();
    return ok;
}

// Before Init: neutral answers (no map, no grid, no lots).
void CheckZoneDevNeutral(cFC4ISC4LotManager* lm) {
    CHECK(lm->GetLotCount() == 0);
    CHECK(lm->GetLot(1, 1, false) == nullptr && lm->GetLot(1, 1, true) == nullptr);
    CHECK(lm->GetMaximumCapacityGrid() == nullptr);
    CHECK(lm->GetRandomLot() == nullptr);
    CHECK(lm->GetOccupantLot(nullptr) == nullptr);
    // Slot 21 (§8.4.1): no map yet, 0 (logged).
    CHECK(lm->CanCreatePloppedLot(SC4Rect<long>(1, 1, 2, 2), 1, nullptr) == 0);
}

void ZdParcellize(cISC4ZoneDeveloper* zd, int32_t x1, int32_t z1, int32_t x2, int32_t z2, int32_t type) {
    ZonesGameRegion g(x1, z1, x2, z2);
    zd->DoParcellization(reinterpret_cast<const SC4CellRegion<long>&>(g.r),
                         static_cast<cISC4ZoneManager::ZoneType>(type), false);
}

// The live object through the stub: Init, refused lots, lots from the
// stub, the lists, the iterator, the shadow, DeleteLot, Shutdown.
void ZoneDevStubTest(FC4TestSeamFn seam, cIGZCOMDirector* director) {
    static ZdStub stub;
    g_zd = &stub;
    stub.lotVt.slots[0] = reinterpret_cast<void*>(&ZdLotQi);
    stub.lotVt.slots[1] = reinterpret_cast<void*>(&ZdLotAddRef);
    stub.lotVt.slots[2] = reinterpret_cast<void*>(&ZdLotRelease);
    stub.lotVt.slots[5] = reinterpret_cast<void*>(&ZdLotClass);
    stub.lotVt.slots[14] = reinterpret_cast<void*>(&ZdLotRectOutput);
    cISC4ZoneDeveloper* zd = NewZoneDeveloper(director);
    CHECK(zd != nullptr);
    if (zd == nullptr) return;
    fc4_zonedev_host host = ZdStubHost();
    stub.lots[0].vtbl = stub.lotVt.slots.data();
    CHECK(ZdHooks(seam, reinterpret_cast<cIGZUnknown*>(&stub.lots[0]), &host, true) == -1);  // not a zone developer
    CHECK(ZdHooks(seam, zd, &host, true) == 1);
    cFC4ISC4LotManager* lm = ZdLotManager(zd);
    CHECK(lm != nullptr);
    if (lm == nullptr) {
        zd->Release();
        return;
    }
    CHECK(zd->Init());
    CHECK(stub.services == 1 && stub.gridCreates == 1);
    CHECK(stub.subscribed.size() == 3);  // InsertOccupant, PostCityInit, the view value (§1.2)
    CHECK(lm->GetMaximumCapacityGrid() == reinterpret_cast<cISC4SimGrid<uint16_t>*>(static_cast<uintptr_t>(kZdGrid)));
    int nullRefs = 0;
    const std::vector<uint8_t> rec = ZdWrite(zd, &nullRefs);
    CHECK(rec.size() == 10 + 64 * 4 && nullRefs == 64);  // version, 8 x 8, a null reference per cell
    zd->SetOptions(false, false, false);
    for (int x = 1; x <= 2; ++x) {
        for (int z = 1; z <= 2; ++z) stub.zone[x][z] = 7;  // agriculture: 1 x 1 lots (§7.5)
    }

    // The plugin's own new_lot outside the game (the lot pool and constructor
    // not available, the executable not identified): CreateLot and a
    // parcellization fail cleanly, logged.
    host.new_lot = nullptr;
    {
        cISC4Lot* lot = reinterpret_cast<cISC4Lot*>(0x1234);
        CHECK(!lm->CreateLot(1, 1, 1, 1, 0, lot) && lot == nullptr);
        CHECK(ZdState(seam, zd).newLotRefusals == 1);
        ZdParcellize(zd, 1, 1, 2, 2, 7);
        const FC4ZoneDevTestState st = ZdState(seam, zd);
        CHECK(st.newLotRefusals == 5 && st.masterSize == 0 && st.heldConfigs == 0 && st.lotsBuilt == 0);
        CHECK(lm->GetLotCount() == 0 && stub.defers == 2);
        // ZG34 (tract-developer.md §4.15.3): while no lot can be built, slot
        // 20 answers 0 for every rectangle, so the original Grow never deletes
        // lots for an aggregation whose CreateLot would fail.
        CHECK(lm->CanCreateLot(1, 1, 1, 1, false, false, nullptr) == 0);
        CHECK(lm->CanCreateLot(1, 1, 1, 1, true, true, nullptr) == 0);
        CHECK(ZdState(seam, zd).canCreateRefusals == 2);
    }

    // With the stub's lots.
    host.new_lot = &ZdNewLot;
    // ZG34: with lots constructible slot 20 is the core's answer again.
    CHECK(lm->CanCreateLot(1, 1, 1, 1, true, true, nullptr) != 0);
    CHECK(ZdState(seam, zd).canCreateRefusals == 2);
    ZdLot* made = nullptr;
    {
        cISC4Lot* lot = nullptr;
        CHECK(lm->CreateLot(5, 5, 2, 1, 2, lot) && lot != nullptr);
        made = reinterpret_cast<ZdLot*>(lot);
        CHECK(made == &stub.lots[0] && made->refs == 3 && made->inits == 1 && made->state == 1);
        --made->refs;  // the caller's reference
    }
    ZdParcellize(zd, 1, 1, 2, 2, 7);
    const uint32_t lots = static_cast<uint32_t>(lm->GetLotCount());
    CHECK(lots == 5 && stub.used == 5);
    for (size_t i = 1; i < stub.used; ++i) CHECK(stub.lots[i].refs == 2 && stub.lots[i].w == 1 && stub.lots[i].h == 1);
    CHECK(ZdState(seam, zd).masterSize == lots);
    CHECK(lm->GetLot(6, 5, false) == reinterpret_cast<cISC4Lot*>(made));
    CHECK(lm->GetLot(6, 5, true) == nullptr && lm->GetLot(5, 5, true) == reinterpret_cast<cISC4Lot*>(made));

    // Slot 7: the caller's list is cleared (its references given back), then
    // the lots are appended with one reference each.
    {
        ZdList list;
        list.Push(&stub.lots[1]);
        list.Push(&stub.lots[1]);
        CHECK(stub.lots[1].refs == 4);
        const uint32_t n = lm->GetLotsInCellRect(list.Get(), 0, 0, 7, 7, false);
        const std::vector<ZdLot*> got = list.Lots();
        CHECK(n == lots && got.size() == lots);
        for (ZdLot* l : got) CHECK(l->refs == 3);
        // Slot 10 appends without clearing; nothing surrounds a free cell.
        CHECK(lm->GetLotsSurroundingRect(SC4Rect<long>(6, 1, 6, 1), list.Get(), 0xF) == 0);
        CHECK(list.Lots().size() == lots);
    }
    for (size_t i = 0; i < stub.used; ++i) CHECK(stub.lots[i].refs == 2);

    // Exercise the production slot-14 host path, rather than the rectangle
    // callback. A lot that crosses into the query is excluded (§8.3), while
    // that same lot is returned for an adjacent query. Neither depends on AL.
    {
        host.lot_bounding_rect = nullptr;
        ZdList list;
        CHECK(lm->GetLotsSurroundingRect(SC4Rect<long>(6, 5, 6, 5), list.Get(), 0xF) == 0);
        CHECK(list.Lots().empty() && made->refs == 2);
        CHECK(lm->GetLotsSurroundingRect(SC4Rect<long>(7, 5, 7, 5), list.Get(), 0xF) == 1);
        CHECK(list.Lots() == std::vector<ZdLot*>{made} && made->refs == 3);
        host.lot_bounding_rect = &ZdLotRect;
    }

    // Slot 15: a C block {first node, list object} over the raw master list
    // (FC4SafeLotIterator.h), registered until its owner releases it.
    {
        auto* b = reinterpret_cast<fc4::safelots::Block*>(lm->GetSafeLotIterator());
        CHECK(b != nullptr);
        if (b != nullptr) {
            CHECK(b->listObject != nullptr && ZdState(seam, zd).iterators == 1);
            std::vector<void*> out(16, nullptr);
            uint32_t n = 0;
            CHECK(fc4::safelots::CopyLots(b, out.data(), 16, &n) == fc4::safelots::Status::Ok && n == lots);
            std::vector<void*> want;
            for (size_t i = 0; i < stub.used; ++i) want.push_back(&stub.lots[i]);
            out.resize(n);
            CHECK(out == want);  // creation order (no traffic insertion point)
            CHECK(fc4::safelots::Release(b, &ZdFree, &ZdFree, 1000) == 1);
            CHECK(ZdState(seam, zd).iterators == 0);
        }
    }

    // The shadow on an original that agrees, skewed, FreeCity4's own, none.
    // The live object's lots and references are untouched.
    {
        static ZdOrig o;
        InitZdOrig(o, zd);
        CHECK(ZdShadowRun(seam, o, &host, 0) == 0);
        CHECK(o.writes == 1);
        o.countSkew = 1;
        CHECK(ZdShadowRun(seam, o, &host, 1) == 1);
        o.countSkew = 0;
        o.cityZd = zd;
        CHECK(ZdShadowRun(seam, o, &host, 1) == -2);
        o.cityZd = nullptr;
        CHECK(ZdShadowRun(seam, o, &host, 1) == -1);
        CHECK(o.refs == 1);
        CHECK(static_cast<uint32_t>(lm->GetLotCount()) == lots && stub.used == 5);
    }
    for (size_t i = 0; i < stub.used; ++i) CHECK(stub.lots[i].refs == 2);

    // DeleteLot (§8.6): the manager's reference goes, the cells are free.
    CHECK(lm->DeleteLot(reinterpret_cast<cISC4Lot*>(made)));
    CHECK(made->refs == 1 && made->shutdowns == 1);
    CHECK(lm->GetLot(5, 5, false) == nullptr && static_cast<uint32_t>(lm->GetLotCount()) == lots - 1);
    CHECK(ZdState(seam, zd).masterSize == lots - 1);

    // An iterator still registered at the end is orphaned (zones.md §14.3 Q3
    // choice): its list object is cleared, its owner frees the block.
    auto* left = reinterpret_cast<fc4::safelots::Block*>(lm->GetSafeLotIterator());
    CHECK(left != nullptr);
    lm->Release();
    CHECK(zd->Shutdown());
    CHECK(stub.services == 0 && stub.gridReleases == 1 && stub.unsubscribed.size() == 3);
    for (size_t i = 0; i < stub.used; ++i) CHECK(stub.lots[i].refs == 1);
    CHECK(stub.foreignRefs == 0);
    CHECK(zd->Release() == 0);
    if (left != nullptr) {
        CHECK(left->listObject == nullptr);
        CHECK(fc4::safelots::Release(left, &ZdFree, &ZdFree, 1000) == 0);
    }
    g_zd = nullptr;
}

int ZoneDevEnableTest(const std::string& dll, const std::string& ini, const std::string& log) {
    // Off by default; `enable = zonedev` registers the zone developer.
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    SetEnvironmentVariableA("FC4_ZONEDEV_SHADOW", nullptr);
    SetEnvironmentVariableA("FC4_ZONEDEV_SEED", nullptr);
    WriteTextFile(ini, "[subsystems]\nenable = zonedev\nzonedev_seed = 0x2A\n[tools]\nzonedev_shadow = 1\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr && seam != nullptr);
    SetGamePrecision();
    if (director != nullptr && seam != nullptr) {
        CHECK(ZdHooks(seam, nullptr, nullptr, true) == 0);  // the allocators only
        CHECK(seam(kFC4SeamZoneDev, nullptr, nullptr) == -1);
        std::vector<uint32_t> classes;
        director->EnumClassObjects(&CollectClass, &classes);
        CHECK(classes.size() == 5);  // simulator (1) + budget (3) + zonedev (1)
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_ZoneDeveloper) == 1);

        std::vector<uint8_t> record;
        cISC4ZoneDeveloper* zd = NewZoneDeveloper(director);
        CHECK(zd != nullptr);
        if (zd != nullptr) {
            // §1: +0 for 1 and its IID, the lot manager at +4, the message
            // target at +8, the serializable at +0xC; nothing else.
            const uintptr_t base = reinterpret_cast<uintptr_t>(zd);
            void* p = nullptr;
            CHECK(zd->QueryInterface(GZIID_cIGZUnknown, &p) && p == static_cast<void*>(zd));
            if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
            p = nullptr;
            CHECK(zd->QueryInterface(kIID_ZoneDeveloper, &p) && p == static_cast<void*>(zd));
            if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
            p = nullptr;
            CHECK(zd->QueryInterface(kIID_LotManager, &p) && reinterpret_cast<uintptr_t>(p) == base + 4);
            if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
            cIGZMessageTarget2* mt = nullptr;
            CHECK(zd->QueryInterface(0x452294AA, reinterpret_cast<void**>(&mt)) &&
                  reinterpret_cast<uintptr_t>(mt) == base + 8);
            cIGZSerializable* ser = nullptr;
            CHECK(zd->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) &&
                  reinterpret_cast<uintptr_t>(ser) == base + 0xC);
            if (ser != nullptr) {
                CHECK(ser->GetGZCLSID() == kCLSID_ZoneDeveloper);
                void* back = nullptr;  // every sub-object answers the others (one object)
                CHECK(static_cast<cIGZUnknown*>(ser)->QueryInterface(kIID_LotManager, &back) &&
                      reinterpret_cast<uintptr_t>(back) == base + 4);
                if (back != nullptr) static_cast<cIGZUnknown*>(back)->Release();
            }
            p = reinterpret_cast<void*>(0x1234);
            CHECK(!zd->QueryInterface(0x12345678, &p) && p == reinterpret_cast<void*>(0x1234));
            CHECK(!zd->QueryInterface(kIID_ZdLot, &p) && p == reinterpret_cast<void*>(0x1234));
            cFC4ISC4LotManager* lm = ZdLotManager(zd);
            CHECK(lm != nullptr);
            if (lm != nullptr) CheckZoneDevNeutral(lm);
            // The record before Init (§10.2): version 1, an empty 0 x 0 map.
            int nullRefs = -1;
            record = ZdWrite(zd, &nullRefs);
            const std::vector<uint8_t> want = {1, 0, 0, 0, 0, 0, 0, 0, 0, 0};
            CHECK(record == want && nullRefs == 0);
            // Init without a city: no services (logged), still neutral.
            CHECK(!zd->Init());
            if (lm != nullptr) CheckZoneDevNeutral(lm);
            seam(kFC4SeamObserverMessage, reinterpret_cast<void*>(static_cast<uintptr_t>(kMsgSimNewMonth)),
                 reinterpret_cast<void*>(static_cast<intptr_t>(kJulian20000201)));
            if (mt != nullptr) {
                TypeMessage month(kMsgSimNewMonth);
                CHECK(mt->DoMessage(&month));
            }
            // The shadow: no city -> -1.
            FC4ZoneDevTestInput sh{};
            sh.op = kFC4ZoneDevTestShadow;
            sh.julian = kJulian20000201;
            CHECK(seam(kFC4SeamZoneDev, &sh, nullptr) == -1);
            if (ser != nullptr) ser->Release();
            if (mt != nullptr) mt->Release();
            if (lm != nullptr) lm->Release();
            CHECK(zd->Shutdown());
            CHECK(zd->Release() == 0);
        }
        // Read runs before Init (§10.2): the record loads into a new object,
        // which writes it back byte for byte; another version does not load.
        for (int variant = 0; variant < 2; ++variant) {
            cISC4ZoneDeveloper* fresh = NewZoneDeveloper(director);
            CHECK(fresh != nullptr);
            if (fresh == nullptr || record.empty()) continue;
            std::vector<uint8_t> bytes = record;
            if (variant == 1) bytes[0] = 2;
            CHECK(ZdRead(fresh, bytes) == (variant == 0));
            if (variant == 0) CHECK(ZdWrite(fresh) == record);
            CHECK(fresh->Release() == 0);  // the final Release shuts a never-initialised one down
        }
        ZoneDevStubTest(seam, director);
        CHECK(ZdHooks(seam, nullptr, nullptr, false) == 0);  // the game's allocators again
        CHECK(g_zdBlocks.load() == 0);
        CheckCw("zonedev");
    }
    FreeLibrary(hd);
    DeleteFileA(ini.c_str());
    const std::string text = ReadFile(log);
    const auto npos = std::string::npos;
    CHECK(text.find("subsystem 'zonedev': enabled (1 class replaced) by FreeCity4.ini (enable=)") != npos);
    CHECK(text.find("session seed 0x0000002A (zonedev_seed)") != npos);
    CHECK(text.find("zonedev Write -> 1") != npos);
    CHECK(text.find("zonedev: no city; services not acquired") != npos);
    CHECK(text.find("zonedev Init FAILED: version=1 init=0") != npos);
    CHECK(text.find("zonedev month 2000-02-01: version=1") != npos);
    CHECK(text.find("zonedev: CanCreatePloppedLot(1,1..2,2, facing 1) = 0") != npos);
    CHECK(text.find("zonedev Read -> 1") != npos);
    CHECK(text.find("zonedev Read -> 0") != npos);
    CHECK(text.find("zonedev: test hooks installed") != npos);
    CHECK(text.find("zonedev Init: version=1 init=1 loaded=0 real=") != npos);
    CHECK(text.find("zonedev: CreateLot(1, 1, 1, 1, facing 0) refused: the lot pool / constructor is not available") !=
          npos);
    CHECK(text.find("zonedev parcellize: type=7 rect=1,1..2,2 cells=4 flag=0 lots=0 failed=4 deleted=0 streets=0/0 "
                    "demolish=0 [lots refused, see CreateLot]") != npos);
    CHECK(text.find("zonedev parcellize: type=7 rect=1,1..2,2 cells=4 flag=0 lots=4 failed=0 deleted=0") != npos);
    CHECK(text.find("zonedev shadow (lots) 2000-02-01 load: mismatches=0 count=5/5") != npos);
    CHECK(text.find("zonedev shadow (lots) 2000-02-01 month: mismatches=1 count=6/5") != npos);
    CHECK(text.find("zonedev shadow (cells) 2000-02-01 load: cells=64 covered=6 origins=5 mismatches=0") != npos);
    CHECK(text.find("zonedev shadow (lists) 2000-02-01 load: calls=32 mismatches=0") != npos);
    CHECK(text.find("zonedev shadow (CanCreateLot) 2000-02-01 load: calls=192 mismatches=0") != npos);
    CHECK(text.find("the city's zone developer is FreeCity4's") != npos);
    CHECK(text.find("the city has no zone developer or lot manager") != npos);
    CHECK(text.find("zonedev shadow: requested (zonedev_shadow from FreeCity4.ini) but INACTIVE") != npos);
    CHECK(text.find("zonedev shadow: ON") == npos);
    CHECK(text.find("1 safe lot iterator orphaned") != npos);
    CHECK(text.find("structured exception") == npos);
    CHECK(text.find("config: WARNING unknown key") == npos);
    CHECK(text.find("WARNING: released last reference") == npos);
    CHECK(text.find("WARNING: over-release") == npos);
    CHECK(text.find("streams: WARNING") == npos);
    if (g_failures == 0) std::printf("plugin zonedev enable test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}

}  // namespace

// ---- the harness lane: sim_turbo (FC4Turbo.h) --------------------------------------------------------------

// A day agent that finishes every delivery at once.
class QuickDayAgent final : public cIGZMessageTarget2 {
public:
    uint32_t refs = 0;
    uint32_t calls = 0;
    int32_t lastJulian = 0;
    bool QueryInterface(uint32_t, void**) override { return false; }
    uint32_t AddRef() override { return ++refs; }
    uint32_t Release() override { return refs > 0 ? --refs : 0; }
    bool DoMessage(cIGZMessage2* msg) override {
        ++calls;
        lastJulian = static_cast<int32_t>(static_cast<cIGZMessage2Standard*>(msg)->GetData1());
        return true;
    }
};

// A child process: game start with one FreeCity4.ini (the settings are read once per process).
int TurboChild(const std::string& dll, const std::string& ini, const std::string& scenario) {
    if (scenario == "disabled") {
        WriteTextFile(ini, "[subsystems]\ndisable = simulator\n[harness]\nsim_turbo = 1\nsim_turbo_until = 2001-01-01\n");
    } else {  // bad values
        WriteTextFile(ini, "[harness]\nsim_turbo = yes\nsim_turbo_frame_ms = 0\nsim_turbo_until = 2000-02-30\n"
                           "sim_turbo_wait_days = -1\n");
    }
    HMODULE h = LoadLibraryA(dll.c_str());
    if (h == nullptr) return 4;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(h, "GZDllGetGZCOMDirector"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    if (director == nullptr) return 5;
    std::vector<uint32_t> classes;
    director->EnumClassObjects(&CollectClass, &classes);  // game start: the configuration
    FreeLibrary(h);
    return 0;
}

int TurboIniTest(const std::string& dll, const std::string& ini, const std::string& log) {
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    SetEnvironmentVariableA("FC4_SIM_TURBO", nullptr);
    SetEnvironmentVariableA("FC4_SIM_TURBO_FRAME_MS", nullptr);
    SetEnvironmentVariableA("FC4_SIM_TURBO_UNTIL", nullptr);
    DWORD ms = 0;
    const std::string dllArg = "\"" + dll + "\" ";

    // 1. disable = simulator: one WARNING, turbo never switched on.
    CHECK(RunChild(dllArg + "--turbo-child-disabled", 30000, &ms) == 0);
    std::string text = ReadFile(log);
    CHECK(text.find("simulator: WARNING sim_turbo = 1 has no effect: our simulator is disabled (disable = simulator), "
                    "the original's clock runs") != std::string::npos);
    CHECK(CountOf(text, "turbo") == 1);
    CHECK(text.find("config: WARNING unknown key") == std::string::npos);

    // 2. Bad values: reported, then the defaults (250 ms, no end date).
    CHECK(RunChild(dllArg + "--turbo-child-bad", 30000, &ms) == 0);
    text = ReadFile(log);
    CHECK(text.find("simulator: WARNING sim_turbo_frame_ms=0 is not 1..10000; using 250") != std::string::npos);
    CHECK(text.find("simulator: WARNING sim_turbo_until='2000-02-30' is not a date YYYY-MM-DD; turbo has no end date") !=
          std::string::npos);
    CHECK(text.find("simulator: turbo ON (sim_turbo = 1; tooling deviation from simulator.md section 5): days not "
                    "paced by msPerDay, frame budget 250 ms at any speed; no end date") != std::string::npos);
    CHECK(text.find("simulator: WARNING sim_turbo_wait_days=-1 is not 0..3650; using 30") != std::string::npos);
    CHECK(text.find("; engages at each city's first ConnectionsReady, or after 30 game day(s) without it "
                    "(sim_turbo_wait_days)") != std::string::npos);

    // 3. In this process: speed 0 (5000 ms per day) with turbo until 2000-03-01.
    // The request at Init is HELD (days paced as usual) until the city's first
    // ConnectionsReady reaches the observer; then the 60 days to 2000-03-01 at
    // once, and no more within the test's ~0.5 s.
    WriteTextFile(ini, "[harness]\nsim_turbo = 1\nsim_turbo_frame_ms = 50\nsim_turbo_until = 2000-03-01\n"
                       "sim_turbo_wait_days = 3\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr && seam != nullptr);
    std::string held;  // the log after the city became ready (before the second load)
    if (director != nullptr && seam != nullptr) {
        std::vector<uint32_t> classes;
        director->EnumClassObjects(&CollectClass, &classes);
        cISC4Simulator* sim = nullptr;
        CHECK(director->GetClassObject(kCLSID_Simulator, kIID_cISC4Simulator, reinterpret_cast<void**>(&sim)));
        if (sim != nullptr) {
            CHECK(sim->Init());
            CHECK(sim->GetSimSpeed() == 0);
            CHECK(sim->HiddenResume());
            QuickDayAgent agent;
            CHECK(sim->AddAgent(&agent, cISC4Simulator::AgentTypeSimNewDay, cRZBaseString("turbo day agent"),
                                static_cast<cISC4Simulator::eAgentFlags>(3)));
            cIGZSystemService* service = nullptr;
            CHECK(sim->QueryInterface(GZIID_cIGZSystemService, reinterpret_cast<void**>(&service)));
            if (service != nullptr) {
                // a. Held: ~300 ms of ticks at 5000 ms per day: no day passes.
                DWORD start = GetTickCount();
                while (GetTickCount() - start < 300) service->OnTick(0);
                const int32_t heldJulian = sim->GetSimDateNumber();
                std::printf("turbo: julian %d after ~300 ms held (before ConnectionsReady)\n", heldJulian);
                CHECK(heldJulian < 2451547);
                // b. ConnectionsReady 0x6AC284F3 reaches the observer: turbo engages.
                seam(kFC4SeamObserverMessage, reinterpret_cast<void*>(static_cast<uintptr_t>(0x6AC284F3u)), nullptr);
                start = GetTickCount();
                while (GetTickCount() - start < 500) service->OnTick(0);
                // A second ConnectionsReady of the same load changes nothing.
                seam(kFC4SeamObserverMessage, reinterpret_cast<void*>(static_cast<uintptr_t>(0x6AC284F3u)), nullptr);
                service->Release();
            }
            std::printf("turbo: julian %d after ConnectionsReady and ~500 ms at speed 0; agent called %u times\n",
                        sim->GetSimDateNumber(), agent.calls);
            CHECK(sim->GetSimDateNumber() == 2451605);  // 2000-03-01, not one day more
            CHECK(agent.lastJulian == 2451605);
            CHECK(sim->Shutdown());  // releases the agent
            CHECK(agent.refs == 0);
            held = ReadFile(log);

            // c. A second city load where ConnectionsReady never comes: held
            // again, then the fallback after sim_turbo_wait_days = 3 game days
            // (speed 2: 100 ms per day), with a WARNING.
            CHECK(sim->Init());
            CHECK(sim->HiddenResume());
            CHECK(sim->SetSimSpeed(2));
            const int32_t reloadJulian = sim->GetSimDateNumber();
            if (sim->QueryInterface(GZIID_cIGZSystemService, reinterpret_cast<void**>(&service)) && service) {
                const DWORD start = GetTickCount();
                while (GetTickCount() - start < 1500) service->OnTick(0);
                service->Release();
            }
            std::printf("turbo: second load: julian %d -> %d without ConnectionsReady\n", reloadJulian,
                        sim->GetSimDateNumber());
            CHECK(sim->GetSimDateNumber() >= reloadJulian + 3);
            CHECK(sim->Shutdown());
            sim->Release();
        }
    }
    FreeLibrary(hd);
    DeleteFileA(ini.c_str());
    CHECK(held.find("simulator: turbo ON (sim_turbo = 1; tooling deviation from simulator.md section 5): days not "
                    "paced by msPerDay, frame budget 50 ms at any speed; until 2000-03-01 (julian 2451605); engages at "
                    "each city's first ConnectionsReady, or after 3 game day(s) without it (sim_turbo_wait_days)") !=
          std::string::npos);
    // The ordering: requested (held) at Init, ON only after ConnectionsReady, once.
    const size_t requested = held.find("simulator: turbo requested (sim_turbo) at 2000-01-01: held, days paced by "
                                       "msPerDay, until the city's first ConnectionsReady (or 3 game day(s) without "
                                       "it, sim_turbo_wait_days)");
    const size_t engaged =
        held.find("simulator: turbo ON at 2000-01-01 (after ConnectionsReady; sim_turbo): frame budget 50 ms, until "
                  "2000-03-01");
    CHECK(requested != std::string::npos);
    CHECK(engaged != std::string::npos);
    CHECK(requested < engaged);
    CHECK(CountOf(held, "(after ConnectionsReady") == 1);
    CHECK(held.find("turbo ON (sim_turbo) at") == std::string::npos);  // never engaged at Init
    CHECK(held.find("simulator: WARNING no ConnectionsReady") == std::string::npos);
    CHECK(held.find("simulator: turbo month 2000-02-01: 31 day(s) in ") != std::string::npos);
    CHECK(held.find(" days/s, ") != std::string::npos);
    CHECK(CountOf(held, "simulator: turbo month") == 1);  // switched off on 2000-03-01 itself
    CHECK(held.find("simulator: turbo OFF at 2000-03-01 (sim_turbo_until reached); days paced by msPerDay again") !=
          std::string::npos);
    CHECK(CountOf(held, "simulator: turbo OFF") == 1);
    text = ReadFile(log);
    CHECK(CountOf(text, "simulator: turbo requested (sim_turbo) at ") == 2);  // one per city load
    CHECK(text.find("simulator: WARNING no ConnectionsReady within 3 game day(s) of the turbo request (sim_turbo); "
                    "turbo ON at ") != std::string::npos);
    CHECK(text.find("config: WARNING unknown key") == std::string::npos);
    if (g_failures == 0) std::printf("plugin turbo test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}

// ---- the harness lane: script (FC4Script.h) ----------------------------------------------------------------

// SC4CellRegion<long> as it crosses PlaceZone (zones.md §11.4): the inclusive
// rectangle, then a cRZCellMap<bool> {vtable, width, height, words per row,
// rows}; bit (dx, dz) is bit dz % 32 of rows[dx][dz / 32].
struct ScriptRegionLayout {
    int32_t x1, z1, x2, z2;
    void* mapVtbl;
    uint32_t width, height, words;
    uint32_t** rows;
};
static_assert(sizeof(ScriptRegionLayout) == 0x24, "zones.md 11.4: 0x24 bytes");

struct ScriptPlaceCall {
    int32_t x1, z1, x2, z2;
    uint32_t width, height, words;
    bool rows;        // a row array was there
    uint32_t bitsIn;  // bits set on entry
    int32_t type;
    bool commit, skipFunds, checkSlope, skipSize, lotInside;
    bool outCost, outError, outLots;  // non-null
};

struct FakeScriptZones {
    FakeVtable zmVt;
    FakeObject zm{};
    std::vector<ScriptPlaceCall> calls;
};
FakeScriptZones* g_scriptZones = nullptr;

uint32_t ScriptRegionBits(const ScriptRegionLayout& r) {
    if (r.rows == nullptr) return 0;
    uint32_t n = 0;
    for (uint32_t dx = 0; dx < r.width; ++dx) {
        for (uint32_t dz = 0; dz < r.height; ++dz) n += (r.rows[dx][dz / 32] >> (dz % 32)) & 1u;
    }
    return n;
}

void* __fastcall ScriptCityGetZoneManager(void*, void*) { return &g_scriptZones->zm; }  // cISC4City 36 (borrowed)

// cISC4ZoneManager 11 PlaceZone (pop 40, AL): records the arguments and the
// region; then, as the game does (zones.md §13.2), only clears bits: the cell
// (x1, z1) is "rejected" by a committed zoning of a type other than 0. Type 7
// fails for funds (error 1). The cost is 10 per cell left set.
bool __fastcall ScriptZmPlaceZone(void*, void*, void* region, int32_t type, bool commit, bool skipFunds,
                                  bool checkSlope, bool skipSize, bool lotInside, int64_t* outCost, int32_t* outError,
                                  void* outLots) {
    auto* r = static_cast<ScriptRegionLayout*>(region);
    ScriptPlaceCall c{};
    c.x1 = r->x1;
    c.z1 = r->z1;
    c.x2 = r->x2;
    c.z2 = r->z2;
    c.width = r->width;
    c.height = r->height;
    c.words = r->words;
    c.rows = r->rows != nullptr;
    c.bitsIn = ScriptRegionBits(*r);
    c.type = type;
    c.commit = commit;
    c.skipFunds = skipFunds;
    c.checkSlope = checkSlope;
    c.skipSize = skipSize;
    c.lotInside = lotInside;
    c.outCost = outCost != nullptr;
    c.outError = outError != nullptr;
    c.outLots = outLots != nullptr;
    g_scriptZones->calls.push_back(c);
    if (type == 7) {
        if (outCost != nullptr) *outCost = 999999;
        if (outError != nullptr) *outError = 1;
        return false;
    }
    if (commit && type != 0 && r->rows != nullptr && r->width > 0 && r->height > 0) r->rows[0][0] &= ~1u;
    if (outCost != nullptr) *outCost = 10 * static_cast<int64_t>(ScriptRegionBits(*r));
    if (outError != nullptr) *outError = 0;
    return true;
}

// The region a call saw, its flags as a user's zoning (FC4Script.cpp), and
// the out-pointers: cost and error given, no lot set.
bool ScriptCallIs(const ScriptPlaceCall& c, int32_t x1, int32_t z1, int32_t x2, int32_t z2, uint32_t words,
                  int32_t type, bool commit) {
    const uint32_t w = static_cast<uint32_t>(x2 - x1 + 1), h = static_cast<uint32_t>(z2 - z1 + 1);
    return c.x1 == x1 && c.z1 == z1 && c.x2 == x2 && c.z2 == z2 && c.width == w && c.height == h &&
           c.words == words && c.rows && c.bitsIn == w * h && c.type == type && c.commit == commit && !c.skipFunds &&
           c.checkSlope && !c.skipSize && c.lotInside && c.outCost && c.outError && !c.outLots;
}

int ScriptIniTest(const std::string& dll, const std::string& ini, const std::string& log) {
    for (const char* v : {"FC4_DISABLE", "FC4_ENABLE", "FC4_SCRIPT", "FC4_SIM_TURBO", "FC4_AUTOTEST_SECONDS",
                          "FC4_AUTOTEST_END_DATE"}) {
        SetEnvironmentVariableA(v, nullptr);
    }
    const std::string dir = ini.substr(0, ini.find_last_of("\\/") + 1);
    const std::string script = dir + "fc4_script_test.txt";
    const std::string bad = dir + "fc4_script_bad.txt";
    const std::string empty = dir + "fc4_script_empty.txt";
    const std::string late = dir + "fc4_script_late.txt";
    const std::string saves = dir + "fc4_script_save.txt";
    WriteTextFile(script,
                  "# FreeCity4 script test (the harness lane)\n"
                  "2000-01-03 zone 10 20 12 22 1        # a 3x3 residential block\n"
                  "2000-01-03 pricezone 30 20 32 22 4   # price a commercial one\n"
                  "2000-01-03 mark three on one day\n"
                  "2000-01-05 dezone 10 20 11 22        # part of the first\n"
                  "2000-01-05 zone 40 40 41 41 7        # the stub refuses it (funds)\n"
                  "2000-01-05 zone 50 0 51 39 2         # 40 rows: two words per row\n"
                  "2000-01-06 speed 2\n"
                  "2000-01-06 pause\n"
                  "2000-01-06 resume\n"
                  "2000-01-07 turbo on\n"
                  "2000-01-08 turbo off\n"
                  "2000-01-08 digest\n"
                  "1999-12-01 mark already past at load\n");
    WriteTextFile(bad, "# two errors\n2000-01-10 zonee 1 1 2 2 1\n2000-01-10 zone 1 1 2 2 16\n2000-01-10 mark fine\n");
    WriteTextFile(empty, "# nothing to do\n\n");
    WriteTextFile(late, "2000-01-01 dezone 5 6 5 6\n");
    WriteTextFile(saves,
                  "2000-01-12 mark before the save\n"
                  "2000-01-12 save\n"
                  "2000-01-12 mark after the save, on its day\n"
                  "2000-01-13 save fast\n"
                  "2000-01-14 mark a later day\n");
    WriteTextFile(ini, "[harness]\nscript = fc4_script_test.txt   # relative: next to FreeCity4.ini\n");

    static FakeWorld world;
    InitFakeWorld(world);
    static FakeScriptZones zones;
    g_scriptZones = &zones;
    zones.zmVt.slots[11] = reinterpret_cast<void*>(&ScriptZmPlaceZone);
    zones.zm.vtbl = zones.zmVt.slots.data();
    world.cityVt.slots[36] = reinterpret_cast<void*>(&ScriptCityGetZoneManager);

    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr && seam != nullptr);
    if (director == nullptr || seam == nullptr) return 1;
    std::vector<uint32_t> classes;
    director->EnumClassObjects(&CollectClass, &classes);  // game start: the script is read and parsed
    cISC4Simulator* sim = nullptr;
    CHECK(director->GetClassObject(kCLSID_Simulator, kIID_cISC4Simulator, reinterpret_cast<void**>(&sim)));
    if (sim == nullptr) return 1;
    CHECK(sim->Init());  // our simulator, live: the script's turbo has something to set
    world.sim = sim;
    seam(kFC4SeamObserverCity, &world.city, nullptr);

    auto newDay = [&](intptr_t julian) {
        seam(kFC4SeamObserverMessage, reinterpret_cast<void*>(static_cast<uintptr_t>(kMsgSimNewDay)),
             reinterpret_cast<void*>(julian));
    };
    // 2000-01-01 .. 2000-01-02: only the action already past at load.
    newDay(2451545);
    newDay(2451546);
    CHECK(zones.calls.empty());
    // 2000-01-03: zone, pricezone, mark (file order).
    newDay(2451547);
    CHECK(zones.calls.size() == 2);
    newDay(2451548);
    CHECK(zones.calls.size() == 2);
    // 2000-01-05: dezone, a refused zone, a 40-row zone.
    newDay(2451549);
    CHECK(zones.calls.size() == 5);
    if (zones.calls.size() == 5) {
        CHECK(ScriptCallIs(zones.calls[0], 10, 20, 12, 22, 1, 1, true));   // zone
        CHECK(ScriptCallIs(zones.calls[1], 30, 20, 32, 22, 1, 4, false));  // pricezone: commit = 0
        CHECK(ScriptCallIs(zones.calls[2], 10, 20, 11, 22, 1, 0, true));   // dezone: type 0
        CHECK(ScriptCallIs(zones.calls[3], 40, 40, 41, 41, 1, 7, true));
        CHECK(ScriptCallIs(zones.calls[4], 50, 0, 51, 39, 2, 2, true));    // ceil(40 / 32) = 2 words
    }
    // 2000-01-06: speed 2, pause, resume through our simulator.
    CHECK(sim->GetSimSpeed() == 0);
    newDay(2451550);
    CHECK(sim->GetSimSpeed() == 2);
    CHECK(!sim->IsPaused());
    // 2000-01-07: turbo on, held: this city load has not seen ConnectionsReady;
    // it engages when ConnectionsReady comes. 2000-01-08: turbo off, the digest.
    newDay(2451551);
    seam(kFC4SeamObserverMessage, reinterpret_cast<void*>(static_cast<uintptr_t>(0x6AC284F3u)), nullptr);
    newDay(2451552);
    std::string text = ReadFile(log);
    const size_t linesBefore = CountOf(text, "script: line ");
    // Each runs once: the same day again and the next produce nothing.
    newDay(2451552);
    newDay(2451553);
    CHECK(zones.calls.size() == 5);

    // Refused: two parse errors, nothing runs (test seam op 46).
    CHECK(seam(kFC4SeamScript, const_cast<char*>("fc4_script_bad.txt"), nullptr) == -2);
    newDay(2451554);
    // Unreadable, and empty.
    CHECK(seam(kFC4SeamScript, const_cast<char*>("fc4_no_such_script.txt"), nullptr) == -1);
    CHECK(seam(kFC4SeamScript, const_cast<char*>("fc4_script_empty.txt"), nullptr) == 0);
    // An absolute path, its date long past: it runs at the next new day.
    CHECK(seam(kFC4SeamScript, const_cast<char*>(late.c_str()), nullptr) == 1);
    CHECK(zones.calls.size() == 5);
    newDay(2451555);
    CHECK(zones.calls.size() == 6);
    if (zones.calls.size() == 6) CHECK(ScriptCallIs(zones.calls[5], 5, 6, 5, 6, 1, 0, true));

    // save: cISC4App 16 (the fake returns true for a full save). It waits for
    // the observer's next tick, and every action due after it waits behind it.
    seam(kFC4SeamObserverApp, &world.app, nullptr);
    CHECK(seam(kFC4SeamScript, const_cast<char*>("fc4_script_save.txt"), nullptr) == 5);
    newDay(2451556);  // 2000-01-12: the first mark runs; the save and the mark after it wait
    newDay(2451557);  // 2000-01-13: the fast save queues behind them
    newDay(2451558);  // 2000-01-14: so does the last mark (file order holds)
    CHECK(world.saveCalls == 0);
    seam(kFC4SeamObserverTick, nullptr, nullptr);  // all four, in order
    CHECK(world.saveCalls == 2);
    seam(kFC4SeamObserverTick, nullptr, nullptr);  // nothing left
    newDay(2451559);
    CHECK(world.saveCalls == 2);
    seam(kFC4SeamObserverApp, nullptr, nullptr);

    seam(kFC4SeamObserverCity, nullptr, nullptr);
    CHECK(sim->Shutdown());
    sim->Release();
    FreeLibrary(hd);
    for (const std::string& f : {script, bad, empty, late, saves, ini}) DeleteFileA(f.c_str());

    text = ReadFile(log);
    // A relative path is resolved against FreeCity4.ini's folder as the DLL
    // reports it (GetModuleFileName: backslashes).
    std::string winDir = dir;
    for (char& ch : winDir) ch = ch == '/' ? '\\' : ch;
    const std::string winScript = winDir + "fc4_script_test.txt";
    CHECK(text.find("script: loaded '" + winScript + "' (script setting; tooling): 13 action(s), 1999-12-01 to "
                    "2000-01-08; each runs once at the first new day on or after its date") != std::string::npos);
    const char* const expected[] = {
        "script: line 14 1999-12-01 mark already past at load -> marked (run on 2000-01-01)",
        "script: line 2 2000-01-03 zone 10 20 12 22 1 -> ok=1 error=0 (none) cost=80 accepted=8 of 9 cell(s)",
        "script: line 3 2000-01-03 pricezone 30 20 32 22 4 -> dry run: ok=1 error=0 (none) cost=90 accepted=9 of 9 "
        "cell(s)",
        "script: line 4 2000-01-03 mark three on one day -> marked",
        "script: line 5 2000-01-05 dezone 10 20 11 22 -> ok=1 error=0 (none) cost=60 accepted=6 of 6 cell(s)",
        "script: line 6 2000-01-05 zone 40 40 41 41 7 -> ok=0 error=1 (insufficient funds) cost=999999 accepted=4 of 4 "
        "cell(s)",
        "script: line 7 2000-01-05 zone 50 0 51 39 2 -> ok=1 error=0 (none) cost=790 accepted=79 of 80 cell(s)",
        "script: line 8 2000-01-06 speed 2 -> SetSimSpeed(2) = 1; speed 0 -> 2",
        "script: line 9 2000-01-06 pause -> Pause() = 1; paused=1",
        "script: line 10 2000-01-06 resume -> Resume() = 1; paused=0",
        // our simulator's line, then the script's
        "simulator: turbo requested (script) at 2000-01-01: held, days paced by msPerDay, until the city's first "
        "ConnectionsReady (or 30 game day(s) without it, sim_turbo_wait_days)",
        "script: line 11 2000-01-07 turbo on -> turbo requested: held until the city's ConnectionsReady",
        "simulator: turbo ON at 2000-01-01 (after ConnectionsReady; script): frame budget 250 ms, until none",
        "simulator: turbo OFF (script) at ",
        "script: line 12 2000-01-08 turbo off -> turbo off",
        "script: line 13 2000-01-08 digest -> the DIGEST line follows",
        "DIGEST julian=2451552 (2000-01-08)",
        "script: all 13 action(s) of '",
    };
    size_t at = 0;
    for (const char* e : expected) {
        const size_t found = text.find(e, at);
        if (found == std::string::npos) std::printf("script test: not found (in order): %s\n", e);
        CHECK(found != std::string::npos);
        if (found != std::string::npos) at = found;
    }
    CHECK(linesBefore == 13);
    // the 13, the late script's one, the save script's 5 and its "waits" line
    CHECK(CountOf(text, "script: line ") == 20);
    CHECK(text.find("script: ERROR line 2: unknown action 'zonee' (zone, pricezone, dezone, network, speed, pause, "
                        "resume, turbo, digest, mark, save, terrain, disaster, port, tutorial, dispatch, clockmode, lightinghour, funds, establish, budgetwindow)") != std::string::npos);
    CHECK(text.find("script: ERROR line 3: zone type '16' is not 0..15") != std::string::npos);
    CHECK(text.find("script: REFUSED '" + winDir + "fc4_script_bad.txt' (test seam): 2 parse error(s); no action of "
                    "it runs") != std::string::npos);
    CHECK(text.find("script: line 4 2000-01-10 mark fine") == std::string::npos);  // refused: not even its good line
    CHECK(text.find("script: ERROR cannot read '" + winDir + "fc4_no_such_script.txt' (test seam; error 2); no "
                    "action runs") != std::string::npos);
    CHECK(text.find("script: '" + winDir + "fc4_script_empty.txt' (test seam): no actions") != std::string::npos);
    CHECK(text.find("script: loaded '" + late + "' (test seam; tooling): 1 action(s), 2000-01-01 to 2000-01-01") !=
          std::string::npos);  // an absolute path as given
    CHECK(text.find("script: line 1 2000-01-01 dezone 5 6 5 6 -> ok=1 error=0 (none) cost=10 accepted=1 of 1 "
                    "cell(s) (run on 2000-01-11)") != std::string::npos);
    {
        const std::string saveOrder[] = {
            "script: loaded '" + winDir + "fc4_script_save.txt' (test seam; tooling): 5 action(s), 2000-01-12 to "
            "2000-01-14",
            "script: line 1 2000-01-12 mark before the save -> marked\r\n",
            "script: line 2 2000-01-12 save: waits for the next framework tick (not run inside SimNewDay); the "
            "actions due after it wait behind it",
            "script: line 2 2000-01-12 save -> ok=1 (SaveCity(full) in ",
            " ms) (run on 2000-01-14)",
            "script: line 3 2000-01-12 mark after the save, on its day -> marked (run on 2000-01-14)",
            "script: line 4 2000-01-13 save fast -> ok=0 (SaveCity(fast) in ",
            " ms) (run on 2000-01-14)",
            "script: line 5 2000-01-14 mark a later day -> marked\r\n",
            "script: all 5 action(s) of '",
        };
        size_t from = 0;
        for (const std::string& e : saveOrder) {
            const size_t found = text.find(e, from);
            if (found == std::string::npos) std::printf("script save test: not found (in order): %s\n", e.c_str());
            CHECK(found != std::string::npos);
            if (found != std::string::npos) from = found;
        }
        CHECK(CountOf(text, ": waits for the next framework tick") == 1);
        CHECK(text.find("[structured exception in SaveCity]") == std::string::npos);
    }
    CHECK(text.find("config: WARNING unknown key") == std::string::npos);  // script is a known key
    if (g_failures == 0) std::printf("plugin script test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}

// ---- traffic (traffic-simulator.md, traffic-network.md, pathfinder.md; test seam op 42) ----------------
constexpr uint32_t kCLSID_Traffic = 0x6990C1AA;
constexpr uint32_t kIID_cISC4TrafficSimulatorT = 0x68027F37;
constexpr uint32_t kIID_cISC4PathFinderT = 0xAA0DB766;
constexpr uint32_t kIID_cIGZMessageTarget2T = 0x452294AA;

cFC4ISC4TrafficSimulator* NewTraffic(cIGZCOMDirector* director) {
    void* obj = nullptr;
    if (!director->GetClassObject(kCLSID_Traffic, kIID_cISC4TrafficSimulatorT, &obj)) return nullptr;
    return static_cast<cFC4ISC4TrafficSimulator*>(obj);
}

std::vector<uint8_t> TrafficWrite(cFC4ISC4TrafficSimulator* t, int* nullRefs = nullptr) {
    std::vector<uint8_t> out;
    cIGZSerializable* ser = nullptr;
    if (!t->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) || ser == nullptr) return out;
    ResidentialSegOStream s;
    if (ser->Write(s)) out = s.bytes;
    if (nullRefs != nullptr) *nullRefs = s.nullRefs;
    ser->Release();
    return out;
}

bool TrafficRead(cFC4ISC4TrafficSimulator* t, const std::vector<uint8_t>& bytes) {
    cIGZSerializable* ser = nullptr;
    if (!t->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) || ser == nullptr) return false;
    ResidentialSegIStream s(bytes);
    const bool ok = ser->Read(s);
    ser->Release();
    return ok;
}

// The caller's std::vector (begin, end, capacity end), filled with the
// test's allocator.
struct TrafficRawVector {
    void* begin = nullptr;
    void* end = nullptr;
    void* capEnd = nullptr;
};

// §7: slots 11, 14, 15, 18, 23, 24 and 27 return a float in ST0 (one value
// left on the x87 stack, popped here); 19 and 26 return nothing; 22 and 34
// return pointers in EAX. Every slot pops its arguments.
void CheckTrafficX87(void* primary, TrafficRawVector* pct, ptrdiff_t wantBytes) {
    using fc4test::CallSlot;
    struct Case {
        int slot;
        std::initializer_list<uint32_t> args;
    };
    const Case floats[] = {{11, {2, 3}}, {14, {}}, {15, {}}, {18, {}}, {23, {1, 2, 3}}, {24, {2, 3, 1}}, {27, {}}};
    for (const Case& c : floats) {
        const fc4test::SlotCall r = CallSlot(primary, c.slot, c.args);
        CHECK(r.espDelta == 0 && r.x87Pushed == 1);
        if (r.espDelta != 0 || r.x87Pushed != 1) {
            std::fprintf(stderr, "traffic slot %d: esp %d, x87 %d\n", c.slot, r.espDelta, r.x87Pushed);
        }
        for (int i = 0; i < r.x87Pushed; ++i) fc4test::PopX87();
    }
    // Slot 19 (void, pop 12): a null holder changes nothing.
    fc4test::SlotCall r = CallSlot(primary, 19, {0u, 0u, 5u});
    CHECK(r.espDelta == 0 && r.x87Pushed == 0);
    // Slot 26 (void, pop 8): three floats in the caller's vector.
    r = CallSlot(primary, 26, {1u, static_cast<uint32_t>(reinterpret_cast<uintptr_t>(pct))});
    CHECK(r.espDelta == 0 && r.x87Pushed == 0);
    CHECK(static_cast<char*>(pct->end) - static_cast<char*>(pct->begin) == wantBytes);
    // Slot 22: null; slot 34: the block (pop 0).
    r = CallSlot(primary, 22, {});
    CHECK(r.espDelta == 0 && r.x87Pushed == 0 && r.eax == 0);
    r = CallSlot(primary, 34, {});
    CHECK(r.espDelta == 0 && r.x87Pushed == 0 && r.eax != 0);
    // Slot 12 (pop 16): a record pointer, never null.
    r = CallSlot(primary, 12, {2u, 3u, 1u, 0u});
    CHECK(r.espDelta == 0 && r.x87Pushed == 0 && r.eax != 0);
}

// Getters from another thread while the main thread holds the lock inside
// DoMessage wait about 20 ms each, then answer from the snapshot (one
// fallback each): slots 14 and 10.
class TrafficLockHeldMessage final : public cIGZMessage2 {
public:
    explicit TrafficLockHeldMessage(cFC4ISC4TrafficSimulator* t) : ts(t) {}
    cFC4ISC4TrafficSimulator* ts;
    bool ran = false;
    float scale = -1.0f;
    void* map = reinterpret_cast<void*>(1);
    DWORD ms = 0;
    bool QueryInterface(uint32_t, void**) override {
        ran = true;
        const DWORD start = GetTickCount();
        std::thread th([this] {
            scale = ts->GetTripScale();
            map = ts->GetCongestionMap();
        });
        th.join();
        ms = GetTickCount() - start;
        return false;
    }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    bool Create(uint32_t, void**) const override { return false; }
    uint32_t GetType() const override { return kMsgSimNewDay; }
    void SetType(uint32_t) override {}
    bool operator==(cIGZMessage2 const&) const override { return false; }
    bool operator<(cIGZMessage2 const&) const override { return false; }
};

// A stub "game" for the slot-6 pathfinders (pathfinder.md §11 test 2): an 8 x
// 8 city with a car street of five network occupants at (0..4, 3), each
// connecting west and east; its grids and two departments. The street
// occupants trap every slot: the plugin must not call them (the stub answers
// for them).
struct TrafficGridStub {
    uint32_t kind = 0;
    int32_t tract = 1, nx = 0, nz = 0;
    std::vector<double> v;
    int refs = 0;
};

struct TrafficStub {
    std::map<uint64_t, TrafficGridStub> grids;
    uint64_t nextGrid = 0x7B000000u;
    int gridShutdowns = 0;
    FakeVtable streetVt;
    FakeObject street[5]{};
    std::map<uint64_t, int> deptRefs;
    uint64_t nextDept = 0x7B0D0000u;
    std::vector<uint32_t> subscribed;
    int agents = 0, posts = 0, rectQueries = 0, pathQueries = 0;
    std::vector<void*> blocks;  // the test allocator's blocks alive
    // create_line_item calls: department, line, name, income, only if missing (TG7).
    std::vector<std::tuple<uint64_t, uint32_t, uint32_t, bool, bool>> lineItems;
    // [TG23] update_network_costs calls (UpdateRoadsBudget /
    // UpdateMassTransitBudget): the count, which kind, and the 13 counts.
    int costUpdates = 0;
    std::vector<bool> costMassTransit;
    std::vector<std::vector<uint32_t>> costCells;
};

TrafficStub* g_tr = nullptr;

int StreetIndex(uint64_t occ) {
    for (int i = 0; i < 5; ++i) {
        if (occ == static_cast<uint64_t>(reinterpret_cast<uintptr_t>(&g_tr->street[i]))) return i;
    }
    return -1;
}

uint64_t TrGridCreate(void*, uint32_t kind, uint32_t, int32_t tract) {
    CHECK(kind == 0x49B9E602u || kind == 0x49B9E606u || kind == 0x49B9E60Au);
    TrafficGridStub g;
    g.kind = kind;
    g.tract = tract < 1 ? 1 : tract;
    g.nx = g.nz = (8 + g.tract - 1) / g.tract;
    g.v.assign(static_cast<size_t>(g.nx * g.nz), 0.0);
    g.refs = 1;
    const uint64_t id = g_tr->nextGrid++;
    g_tr->grids[id] = g;
    return id;
}
uint32_t TrGridClass(void*, uint64_t t) {
    auto it = g_tr->grids.find(t);
    return it == g_tr->grids.end() ? 0u : it->second.kind;
}
void TrGridGeometry(void*, uint64_t t, fc4_grid_geometry* out) {
    auto it = g_tr->grids.find(t);
    if (it == g_tr->grids.end()) return;
    const TrafficGridStub& g = it->second;
    int32_t shift = 0;
    while ((1 << shift) < g.tract) ++shift;
    const float w = 16.0f * static_cast<float>(g.tract);
    *out = fc4_grid_geometry{g.tract, shift, g.nx, g.nz, w, w, 1.0f / w, 1.0f / w};
}
double TrGridGet(void*, uint64_t t, uint32_t, int32_t x, int32_t z) {
    auto it = g_tr->grids.find(t);
    if (it == g_tr->grids.end()) return 0.0;
    const TrafficGridStub& g = it->second;
    return x >= 0 && x < g.nx && z >= 0 && z < g.nz ? g.v[static_cast<size_t>(x * g.nz + z)] : 0.0;
}
void TrGridSet(void*, uint64_t t, uint32_t, int32_t x, int32_t z, double v) {
    auto it = g_tr->grids.find(t);
    if (it == g_tr->grids.end()) return;
    TrafficGridStub& g = it->second;
    if (x >= 0 && x < g.nx && z >= 0 && z < g.nz) g.v[static_cast<size_t>(x * g.nz + z)] = v;
}
void TrGridFill(void*, uint64_t t, uint32_t, double v) {
    auto it = g_tr->grids.find(t);
    if (it != g_tr->grids.end()) std::fill(it->second.v.begin(), it->second.v.end(), v);
}
void TrGridShutdown(void*, uint64_t) { ++g_tr->gridShutdowns; }
void TrGridRelease(void*, uint64_t t) {
    auto it = g_tr->grids.find(t);
    CHECK(it != g_tr->grids.end());
    if (it != g_tr->grids.end()) --it->second.refs;
}
void TrAddAgent(void*, uint32_t, const uint8_t*, size_t, uint32_t) { ++g_tr->agents; }
void TrSubscribe(void*, uint32_t m) { g_tr->subscribed.push_back(m); }
void TrCellCounts(void*, int32_t* x, int32_t* z) {
    *x = 8;
    *z = 8;
}
void TrPositionToCell(void*, float x, float z, int32_t* cx, int32_t* cz) {
    *cx = static_cast<int32_t>(std::floor(x / 16.0f));
    *cz = static_cast<int32_t>(std::floor(z / 16.0f));
}
float TrCellWidth(void*) { return 16.0f; }
uint64_t TrCreateDepartment(void*, uint32_t, float) {
    const uint64_t t = g_tr->nextDept++;
    g_tr->deptRefs[t] = 1;
    return t;
}
void TrCreateLineItem(void*, uint64_t dept, uint32_t line, uint32_t name, bool income, bool onlyIfMissing) {
    g_tr->lineItems.emplace_back(dept, line, name, income, onlyIfMissing);
}
void TrReleaseDepartment(void*, uint64_t t) {
    CHECK(g_tr->deptRefs.count(t) == 1);
    --g_tr->deptRefs[t];
}
float TrDepartmentFunding(void*, uint64_t) { return 1.0f; }
void TrSetLineItemIncome(void*, uint64_t, uint32_t, int64_t) {}
// [TG23] UpdateRoadsBudget / UpdateMassTransitBudget (traffic-simulator.md
// §4.9.1): counted here. The stub's departments are fake tokens, so this
// must answer rather than let the plugin reach the game's SetFullExpenses.
void TrUpdateNetworkCosts(void*, uint64_t dept, bool massTransit, const uint32_t* cells13, const float*) {
    CHECK(g_tr->deptRefs.count(dept) == 1);
    ++g_tr->costUpdates;
    g_tr->costMassTransit.push_back(massTransit);
    // The 13 counts the core priced, for the §4.9.1 worked example shape.
    std::vector<uint32_t> c;
    if (cells13 != nullptr) c.assign(cells13, cells13 + 13);
    g_tr->costCells.push_back(std::move(c));
}
void TrNetworkOccupantsInRect(void*, const fc4_cell_rect* r, fc4_token_callback cb, void* ctx) {
    ++g_tr->rectQueries;
    for (int i = 0; i < 5; ++i) {
        if (i >= r->x1 && i <= r->x2 && 3 >= r->z1 && 3 <= r->z2) {
            cb(ctx, static_cast<uint64_t>(reinterpret_cast<uintptr_t>(&g_tr->street[i])));
        }
    }
}
void TrNoOccupants(void*, const fc4_cell_rect*, fc4_token_callback, void*) {}
bool TrIsOfType(void*, uint64_t occ, uint32_t type) { return StreetIndex(occ) >= 0 && type == 3; }  // Street
bool TrNoFlag(void*, uint64_t, uint32_t) { return false; }
bool TrPathConnection(void*, uint64_t occ, uint32_t pathType, uint32_t entry, uint32_t exit) {
    ++g_tr->pathQueries;
    return StreetIndex(occ) >= 0 && pathType == 1 && ((entry == 0 && exit == 2) || (entry == 2 && exit == 0));
}
void TrOccupantPosition(void*, uint64_t occ, float* xyz) {
    const int i = StreetIndex(occ);
    xyz[0] = static_cast<float>(i < 0 ? 0 : i) * 16.0f + 8.0f;
    xyz[1] = 0.0f;
    xyz[2] = 3.0f * 16.0f + 8.0f;
}
void TrOccupantCell(void*, uint64_t occ, int32_t* x, int32_t* z) {
    *x = StreetIndex(occ);
    *z = 3;
}
void TrOccupantBounds(void*, uint64_t occ, fc4_cell_rect* out) {
    const int i = StreetIndex(occ);
    *out = fc4_cell_rect{i, 3, i, 3};
}
bool TrOccupantProperty(void*, uint64_t, uint32_t, fc4_props*) { return false; }
uint32_t TrPropType(void*, uint64_t) { return 0; }
bool TrIsTunnel(void*, uint64_t) { return false; }
// The street pieces have no path info: no turn-around (traffic-network.md §4.2 step 3).
uint64_t TrNoPathInfo(void*, uint64_t) { return 0; }
// [TG24] With no path info the core never reaches these; answering empty
// keeps the plugin out of the game's path-info vectors and key lists.
void TrPathInfoExitsFromEntry(void*, uint64_t, uint32_t, uint32_t, fc4_traffic_exit_callback, void*) {}
void TrPathInfoEntriesNear(void*, uint64_t, uint32_t, uint64_t, uint32_t, float, fc4_traffic_u32_callback, void*) {}
// [TG25] No tunnel pair in the stub (TrIsTunnel is false), so no record.
uint64_t TrTunnelPairedPortal(void*, uint64_t) { return 0; }
void TrPostSink(void*, cIGZMessage2Standard*) { ++g_tr->posts; }
void* TrAllocate(uint32_t size) {
    void* p = std::calloc(1, size);
    g_tr->blocks.push_back(p);
    return p;
}
void TrFree(void* p) {
    auto it = std::find(g_tr->blocks.begin(), g_tr->blocks.end(), p);
    CHECK(it != g_tr->blocks.end());
    if (it != g_tr->blocks.end()) g_tr->blocks.erase(it);
    std::free(p);
}

fc4_traffic_host TrafficStubHost(TrafficStub& s) {
    fc4_traffic_host h{};
    h.struct_size = sizeof(h);
    h.user = &s;
    h.grids.struct_size = sizeof(h.grids);
    h.grids.user = &s;
    h.grids.create = &TrGridCreate;
    h.grids.class_of = &TrGridClass;
    h.grids.geometry = &TrGridGeometry;
    h.grids.get = &TrGridGet;
    h.grids.set = &TrGridSet;
    h.grids.fill = &TrGridFill;
    h.grids.shutdown = &TrGridShutdown;
    h.grids.release = &TrGridRelease;
    h.add_agent = &TrAddAgent;
    h.subscribe = &TrSubscribe;
    h.cell_counts = &TrCellCounts;
    h.position_to_cell = &TrPositionToCell;
    h.cell_width = &TrCellWidth;
    h.create_department = &TrCreateDepartment;
    h.create_line_item = &TrCreateLineItem;
    h.release_department = &TrReleaseDepartment;
    h.department_funding = &TrDepartmentFunding;
    h.set_line_item_income = &TrSetLineItemIncome;
    h.update_network_costs = &TrUpdateNetworkCosts;  // [TG23]
    h.network_occupants_in_rect = &TrNetworkOccupantsInRect;
    h.subway_occupants_in_rect = &TrNoOccupants;
    h.prop_occupants_in_rect = &TrNoOccupants;
    h.is_of_type = &TrIsOfType;
    h.has_network_flag = &TrNoFlag;
    h.has_any_network_flag = &TrNoFlag;
    h.path_connection = &TrPathConnection;
    h.occupant_position = &TrOccupantPosition;
    h.network_occupant_cell = &TrOccupantCell;
    h.occupant_bounding_cells = &TrOccupantBounds;
    h.occupant_property = &TrOccupantProperty;
    h.prop_type = &TrPropType;
    h.is_tunnel = &TrIsTunnel;
    h.path_info = &TrNoPathInfo;
    h.path_info_exits_from_entry = &TrPathInfoExitsFromEntry;  // [TG24]
    h.path_info_entries_near = &TrPathInfoEntriesNear;        // [TG24]
    h.tunnel_paired_portal = &TrTunnelPairedPortal;           // [TG25]
    return h;
}

intptr_t TrafficHooks(FC4TestSeamFn seam, cIGZUnknown* t, const fc4_traffic_host* host) {
    FC4TrafficTestInput in{};
    in.op = kFC4TrafficTestHooks;
    in.traffic = t;
    in.host = host;
    in.postSink = host ? &TrPostSink : nullptr;
    in.allocate = host ? &TrAllocate : nullptr;
    in.release = host ? &TrFree : nullptr;
    return seam(kFC4SeamTraffic, &in, nullptr);
}

intptr_t TrafficLivePathFinders(FC4TestSeamFn seam) {
    FC4TrafficTestInput in{};
    in.op = kFC4TrafficTestLivePathFinders;
    return seam(kFC4SeamTraffic, &in, nullptr);
}

// pathfinder.md §11 test 2 through slot 6: from (2, 2) to (4, 4) by car.
// The caller-owned STLport deque<RouteData> of slot 8 (pathfinder.md §7):
// ten 32-bit words, buffers of 0x7E bytes and the map from allocator B.
struct TrRouteIter {
    uint8_t* cur;
    uint8_t* first;
    uint8_t* last;
    uint8_t** node;
};
struct TrRouteDeque {
    TrRouteIter start, finish;
    uint8_t** map;
    uint32_t mapSize;
};
static_assert(sizeof(TrRouteDeque) == 0x28, "route deque: 0x28 bytes");

void TrSetNode(TrRouteIter& it, uint8_t** node) {
    it.node = node;
    it.first = *node;
    it.last = it.first + 0x7E;
}

// An empty deque as the callers' STLport constructs it, with its map of
// `mapSize` slots and its one buffer at `slot`.
TrRouteDeque TrEmptyRouteDeque(uint32_t mapSize, uint32_t slot) {
    TrRouteDeque d{};
    d.mapSize = mapSize;
    d.map = static_cast<uint8_t**>(TrAllocate(mapSize * 4));
    for (uint32_t i = 0; i < mapSize; ++i) d.map[i] = nullptr;
    d.map[slot] = static_cast<uint8_t*>(TrAllocate(0x7E));
    TrSetNode(d.start, d.map + slot);
    d.start.cur = d.start.first;
    d.finish = d.start;
    return d;
}

// The caller's walk: start.cur up to finish.cur, buffer by buffer.
std::vector<uint8_t> TrWalkRouteDeque(const TrRouteDeque& d) {
    std::vector<uint8_t> out;
    TrRouteIter it = d.start;
    while (it.cur != d.finish.cur) {
        out.insert(out.end(), it.cur, it.cur + 3);
        it.cur += 3;
        if (it.cur == it.last && it.node != d.finish.node) {
            TrSetNode(it, it.node + 1);
            it.cur = it.first;
        }
    }
    return out;
}

// The caller's destructor: every buffer from start to finish, then the map.
void TrFreeRouteDeque(TrRouteDeque& d) {
    for (uint8_t** n = d.start.node; n <= d.finish.node; ++n) TrFree(*n);
    TrFree(d.map);
    d = TrRouteDeque{};
}

void ConfigurePathFinder(cFC4ISC4PathFinder* pf) {
    pf->SetFlag(1);
    pf->SetStartRect(SC4Rect<int32_t>(2, 2, 2, 2));
    pf->SetEndRect(SC4Rect<int32_t>(4, 4, 4, 4));
    pf->SetTripDestinationFilter(3);
    pf->ClearAllTravelTypeFlags();
    pf->SetTravelTypeFlag(0x2, true);
    float costs[9] = {};
    pf->SetStartingCostArray(costs);
    pf->SetTrafficDensity(10);
}

void TrafficPathFinderTest(FC4TestSeamFn seam, cIGZCOMDirector* director) {
    static TrafficStub stub;
    g_tr = &stub;
    for (FakeObject& o : stub.street) o.vtbl = stub.streetVt.slots.data();
    const fc4_traffic_host host = TrafficStubHost(stub);
    cFC4ISC4TrafficSimulator* ts = NewTraffic(director);
    CHECK(ts != nullptr);
    if (ts == nullptr) return;
    CHECK(TrafficHooks(seam, ts, &host) == 1);
    CHECK(ts->Init());
    CHECK(!stub.grids.empty() && stub.deptRefs.size() == 2 && !stub.subscribed.empty() && stub.agents > 0);
    // TG7 (traffic-simulator.md §9.1): lines 1-3 with each new department,
    // then lines 4-6 of both when missing; the two Income lines.
    CHECK(stub.lineItems.size() == 12);
    if (stub.lineItems.size() == 12) {
        const uint64_t roads = std::get<0>(stub.lineItems[0]);
        const uint64_t transit = std::get<0>(stub.lineItems[3]);
        CHECK(roads != transit && stub.deptRefs.count(roads) == 1 && stub.deptRefs.count(transit) == 1);
        CHECK((stub.lineItems[4] == std::make_tuple(transit, 2u, 0x8A5F978Cu, true, false)));
        CHECK((stub.lineItems[6] == std::make_tuple(transit, 4u, 0x2AB7EA93u, false, true)));
        CHECK((stub.lineItems[10] == std::make_tuple(roads, 5u, 0x0C3BF3C4u, true, true)));
        CHECK((stub.lineItems[11] == std::make_tuple(roads, 6u, 0xEA5F96E7u, false, true)));
    }
    // DoPostCityInit (§1): the whole city rebuilt from the network occupants.
    cIGZMessageTarget2* mt = nullptr;
    CHECK(ts->QueryInterface(kIID_cIGZMessageTarget2T, reinterpret_cast<void**>(&mt)) && mt != nullptr);
    if (mt != nullptr) {
        TypeMessage init(kMsgPostCityInit);
        CHECK(mt->DoMessage(&init));
    }
    CHECK(stub.rectQueries > 0 && stub.pathQueries > 0);
    // [TG23] §3.5 step 14 calls UpdateMassTransitBudget then UpdateRoadsBudget,
    // so the two line-item rewrites happen in that order from PostCityInit.
    CHECK(stub.costUpdates == 2);
    if (stub.costUpdates == 2) {
        CHECK(stub.costMassTransit[0] && !stub.costMassTransit[1]);
        CHECK(stub.costCells[0].size() == 13 && stub.costCells[1].size() == 13);
    }
    // Slot 6.
    CHECK(!ts->CreatePathFinder(nullptr));
    cFC4ISC4PathFinder* pf = nullptr;
    CHECK(ts->CreatePathFinder(&pf) && pf != nullptr);
    CHECK(TrafficLivePathFinders(seam) == 1);
    if (pf != nullptr) {
        void* p = nullptr;
        CHECK(pf->QueryInterface(kIID_cISC4PathFinderT, &p) && p == pf);
        if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
        p = reinterpret_cast<void*>(0x1234);
        CHECK(!pf->QueryInterface(kIID_cISC4TrafficSimulatorT, &p) && p == reinterpret_cast<void*>(0x1234));
        CHECK(!pf->Init());  // created initialised (§1): a second Init is false
        ConfigurePathFinder(pf);
        CHECK(pf->FindPath() == 1);
        CHECK(pf->GetTripLength() > 0.0f);
        int32_t ex = -1, ez = -1;
        pf->GetEndCell(ex, ez);
        CHECK(ex == 4 && ez == 4);
        CHECK(pf->GetTrafficArrived() == 10);
        CHECK(pf->GetLastTravelType() == 1);
        CHECK(pf->GetTripLengthScaledForDisplay() > 0.0f);
        // Slots 25 and 26 return in ST0.
        for (int slot : {25, 26}) {
            const fc4test::SlotCall r = fc4test::CallSlot(pf, slot, {});
            CHECK(r.espDelta == 0 && r.x87Pushed == 1);
            for (int i = 0; i < r.x87Pushed; ++i) fc4test::PopX87();
        }
        // Slot 32: the header (type 1, x 2, z 3), then one Car run of two +x
        // steps (pathfinder.md §4.15), in the caller's vector (B).
        TrafficRawVector route;
        uint32_t mask = 0;
        pf->GetCompressedRoute(reinterpret_cast<fc4::GameVector<uint8_t>&>(route), mask);
        CHECK(mask == (1u << 1));
        const auto* b = static_cast<const uint8_t*>(route.begin);
        const std::vector<uint8_t> bytes(b, static_cast<const uint8_t*>(route.end));
        CHECK((bytes == std::vector<uint8_t>{1, 2, 3, 0x41, 2, 0x0A}));
        CHECK(stub.blocks.size() == 1);
        CHECK(pf->WasTripReversible());
        // A route deque set (slot 8): filled root -> result in place
        // (pathfinder.md §4.13 step 5, §7 and §11 test 2; TG1).
        const std::vector<uint8_t> want{2, 3, 1, 3, 3, 1, 4, 3, 1};
        {
            // An empty STLport deque: a map of 8 slots, one buffer at slot 3.
            TrRouteDeque d = TrEmptyRouteDeque(8, 3);
            pf->SetRouteInfo(&d);
            CHECK(pf->FindPath() == 1);
            CHECK(TrWalkRouteDeque(d) == want);
            // A new buffer in the slot before the start one, no regrowth.
            CHECK(d.mapSize == 8 && d.start.node == d.map + 2 && d.finish.node == d.map + 3);
            CHECK(d.start.cur == d.start.first + 0x7E - 9 && d.finish.cur == d.finish.first);
            // Again: cleared (the finish buffer freed), refilled in the start buffer.
            const size_t before = stub.blocks.size();
            CHECK(pf->FindPath() == 1);
            CHECK(TrWalkRouteDeque(d) == want);
            CHECK(d.start.node == d.map + 2 && d.finish.node == d.map + 2 && stub.blocks.size() == before - 1);
            CHECK(d.start.cur == d.start.first + 0x7E - 18);
            TrFreeRouteDeque(d);
        }
        {
            // A one-slot map: regrown to 1 + 1 + 2 = 4 slots, the used slot
            // copied to slot (4 - 2) / 2 + 1 = 2, the old map freed (§7 step 3).
            TrRouteDeque d = TrEmptyRouteDeque(1, 0);
            void* oldMap = d.map;
            pf->SetRouteInfo(&d);
            CHECK(pf->FindPath() == 1);
            CHECK(TrWalkRouteDeque(d) == want);
            CHECK(d.map != oldMap && d.mapSize == 4 && d.start.node == d.map + 1 && d.finish.node == d.map + 2);
            CHECK(std::find(stub.blocks.begin(), stub.blocks.end(), oldMap) == stub.blocks.end());
            TrFreeRouteDeque(d);
        }
        {
            // Start at slot 0 of 8: the used slot moved within the map to slot
            // (8 - 2) / 2 + 1 = 4, the new buffer at slot 3.
            TrRouteDeque d = TrEmptyRouteDeque(8, 0);
            void* const map = d.map;
            pf->SetRouteInfo(&d);
            CHECK(pf->FindPath() == 1);
            CHECK(TrWalkRouteDeque(d) == want);
            CHECK(d.map == map && d.start.node == d.map + 3 && d.finish.node == d.map + 4);
            TrFreeRouteDeque(d);
        }
        pf->SetRouteInfo(nullptr);
        CHECK(pf->FindPath() == 1);
        // The route again: the caller's buffer is kept and refilled.
        pf->GetCompressedRoute(reinterpret_cast<fc4::GameVector<uint8_t>&>(route), mask);
        CHECK(mask == (1u << 1) && stub.blocks.size() == 1);
        TrFree(route.begin);
        // Slot 7: a foreign pathfinder, a null lot.
        FakeObject foreign{stub.streetVt.slots.data()};
        CHECK(!ts->SetupPathFinderForLot(reinterpret_cast<cFC4ISC4PathFinder*>(&foreign), nullptr));
        CHECK(!ts->SetupPathFinderForLot(pf, nullptr));
        pf->ReleasePathNodes();
        CHECK(pf->Release() == 0);
    }
    CHECK(TrafficLivePathFinders(seam) == 0);
    // A pathfinder outliving its simulator: orphaned, neutral, released once.
    cFC4ISC4PathFinder* orphan = nullptr;
    CHECK(ts->CreatePathFinder(&orphan) && orphan != nullptr);
    if (mt != nullptr) mt->Release();
    CHECK(ts->Shutdown());
    CHECK(ts->Release() == 0);
    for (const auto& kv : stub.grids) CHECK(kv.second.refs == 0);  // every grid released once
    for (const auto& kv : stub.deptRefs) CHECK(kv.second == 0);    // both departments given back
    if (orphan != nullptr) {
        ConfigurePathFinder(orphan);
        CHECK(orphan->FindPath() == 0);
        const fc4test::SlotCall r = fc4test::CallSlot(orphan, 25, {});
        CHECK(r.espDelta == 0 && r.x87Pushed == 1);
        for (int i = 0; i < r.x87Pushed; ++i) fc4test::PopX87();
        CHECK(TrafficLivePathFinders(seam) == 1);
        CHECK(orphan->Release() == 0);
    }
    CHECK(TrafficLivePathFinders(seam) == 0);
    CHECK(stub.blocks.empty());
    TrafficHooks(seam, nullptr, nullptr);  // the game's allocator again
}

// ---- the traffic shadow (FC4TrafficShadow.h, test seam op 51) on a stand-in original -------------
//
// Our own simulator stands in for the original behind fake objects: the
// city's GetTrafficSimulator (slot 69) answers the fake simulator, whose
// agent forwards DoMessage and whose serializable forwards Write to ours.

struct FakeOrigTraffic {
    FakeVtable simVt, agentVt, serVt, cityVt;
    FakeObject sim{}, agent{}, ser{}, city{}, other{};
    cIGZMessageTarget2* innerTarget = nullptr;
    cIGZSerializable* innerSer = nullptr;
    int refs = 1;
    int deliveries = 0;
    int writes = 0;
};

FakeOrigTraffic* g_ot = nullptr;

bool __fastcall OtQi(void*, void*, uint32_t iid, void** out) {  // 0 (simulator, agent, serializable)
    if (out == nullptr) return false;
    if (iid == kIID_cIGZMessageTarget2T) {
        *out = &g_ot->agent;
    } else if (iid == GZIID_cIGZSerializable) {
        *out = &g_ot->ser;
    } else {
        return false;
    }
    ++g_ot->refs;
    return true;
}
uint32_t __fastcall OtAddRef(void*, void*) { return static_cast<uint32_t>(++g_ot->refs); }   // 1
uint32_t __fastcall OtRelease(void*, void*) { return static_cast<uint32_t>(--g_ot->refs); }  // 2
bool __fastcall OtWrite(void*, void*, cIGZOStream* s) {                                       // serializable 3
    ++g_ot->writes;
    return g_ot->innerSer->Write(*s);
}
uint32_t __fastcall OtClsid(void*, void*) { return kCLSID_Traffic; }  // serializable 5
bool __fastcall OtDoMessage(void*, void*, cIGZMessage2* m) {          // agent 3
    ++g_ot->deliveries;
    return g_ot->innerTarget->DoMessage(m);
}
void* __fastcall OtCitySim(void*, void*) { return &g_ot->sim; }  // city 69

void InitFakeOrigTraffic(FakeOrigTraffic& o, cIGZMessageTarget2* target, cIGZSerializable* ser) {
    g_ot = &o;
    o.innerTarget = target;
    o.innerSer = ser;
    for (FakeVtable* vt : {&o.simVt, &o.agentVt, &o.serVt}) {
        vt->slots[0] = reinterpret_cast<void*>(&OtQi);
        vt->slots[1] = reinterpret_cast<void*>(&OtAddRef);
        vt->slots[2] = reinterpret_cast<void*>(&OtRelease);
    }
    o.agentVt.slots[3] = reinterpret_cast<void*>(&OtDoMessage);
    o.serVt.slots[3] = reinterpret_cast<void*>(&OtWrite);
    o.serVt.slots[5] = reinterpret_cast<void*>(&OtClsid);
    o.cityVt.slots[69] = reinterpret_cast<void*>(&OtCitySim);
    o.sim.vtbl = o.simVt.slots.data();
    o.agent.vtbl = o.agentVt.slots.data();
    o.ser.vtbl = o.serVt.slots.data();
    o.city.vtbl = o.cityVt.slots.data();
    o.other.vtbl = o.agentVt.slots.data();  // an agent, but not the simulator's
}

intptr_t TrafficShadowOnce(FC4TestSeamFn seam, FakeObject* city, FakeObject* target, uint32_t type,
                           uint32_t fresh = 0) {
    FC4TrafficShadowTestInput in{reinterpret_cast<cISC4City*>(city), reinterpret_cast<cIGZMessageTarget2*>(target),
                                 type, 2451576, fresh};  // 2000-02-01
    return seam(kFC4SeamTrafficShadow, &in, nullptr);
}

// The shadow follows the stand-in clean through the one-shot, days and a
// month; an agent that is not the simulator's, or no city, is left to the
// caller (nothing delivered).
void TrafficShadowStandIn(FC4TestSeamFn seam, cIGZMessageTarget2* target, cIGZSerializable* ser) {
    static FakeOrigTraffic o;
    InitFakeOrigTraffic(o, target, ser);
    CHECK(TrafficShadowOnce(seam, nullptr, &o.agent, kMsgSimNewDay, 1) == -1);
    CHECK(TrafficShadowOnce(seam, &o.city, &o.other, kMsgSimNewDay) == -1);
    CHECK(o.deliveries == 0 && o.writes == 0);
    CHECK(TrafficShadowOnce(seam, &o.city, &o.agent, 0x66956810) == 0);  // the one-shot: the copy starts
    for (int d = 0; d < 3; ++d) CHECK(TrafficShadowOnce(seam, &o.city, &o.agent, kMsgSimNewDay) == 0);
    CHECK(TrafficShadowOnce(seam, &o.city, &o.agent, kMsgSimNewMonth, 2) == 0);
    CHECK(o.deliveries == 5 && o.writes == 10);
    CHECK(o.refs == 1);  // every QueryInterface released
}

int TrafficEnableTest(const std::string& dll, const std::string& ini, const std::string& log) {
    // Off by default; `enable = traffic` registers the traffic simulator.
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    SetEnvironmentVariableA("FC4_TRAFFIC_SEED", nullptr);
    SetEnvironmentVariableA("FC4_TRAFFIC_UNITS_PER_SLICE", nullptr);
    SetEnvironmentVariableA("FC4_TRAFFIC_SHADOW", nullptr);
    SetEnvironmentVariableA("FC4_TRAFFIC_SHADOW_DIFFS", nullptr);
    SetEnvironmentVariableA("FC4_TRAFFIC_SHADOW_LINES", nullptr);
    // The traffic shadow is requested too: with our traffic simulator it is inactive.
    WriteTextFile(ini, "[subsystems]\nenable = traffic\ntraffic_seed = 1234\n[tools]\ntraffic_shadow = 1\n"
                       "traffic_shadow_diffs = 8\ntraffic_shadow_lines = 8\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr && seam != nullptr);
    SetGamePrecision();  // as the game's PreFrameWorkInit does
    std::vector<uint8_t> record;
    if (director != nullptr && seam != nullptr) {
        std::vector<uint32_t> classes;
        director->EnumClassObjects(&CollectClass, &classes);
        // simulator (1) + budget (3) + traffic (1); never the game's grid classes.
        CHECK(classes.size() == 5);
        for (uint32_t c : classes) CHECK(c < 0x49B9E600 || c > 0x49B9E60B);
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_Traffic) == 1);

        cFC4ISC4TrafficSimulator* ts = NewTraffic(director);
        CHECK(ts != nullptr);
        if (ts != nullptr) {
            // §1: the primary for its IID and cIGZUnknown, the message target
            // and the serializable; nothing else (the out pointer untouched).
            void* p = nullptr;
            CHECK(ts->QueryInterface(GZIID_cIGZUnknown, &p) && p == static_cast<void*>(ts));
            if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
            p = nullptr;
            CHECK(ts->QueryInterface(kIID_cISC4TrafficSimulatorT, &p) && p == static_cast<void*>(ts));
            if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
            cIGZMessageTarget2* mt = nullptr;
            CHECK(ts->QueryInterface(kIID_cIGZMessageTarget2T, reinterpret_cast<void**>(&mt)) && mt != nullptr);
            CHECK(static_cast<void*>(mt) != static_cast<void*>(ts));
            cIGZSerializable* ser = nullptr;
            CHECK(ts->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) && ser != nullptr);
            if (ser != nullptr) {
                CHECK(ser->GetGZCLSID() == kCLSID_Traffic);
                void* back = nullptr;  // every sub-object answers the others (one object)
                CHECK(static_cast<cIGZUnknown*>(ser)->QueryInterface(kIID_cISC4TrafficSimulatorT, &back) &&
                      back == ts);
                if (back != nullptr) static_cast<cIGZUnknown*>(back)->Release();
            }
            p = reinterpret_cast<void*>(0x1234);
            CHECK(!ts->QueryInterface(0x12345678, &p) && p == reinterpret_cast<void*>(0x1234));
            CHECK(!ts->QueryInterface(kIID_cISC4PathFinderT, &p) && p == reinterpret_cast<void*>(0x1234));
            CHECK(ts->GetSimulatorType() == 0x88027F0D);
            // Neutral without a city.
            CHECK(ts->GetAirPollutingTrafficMap() == nullptr && ts->GetCongestionMap() == nullptr);
            CHECK(ts->GetTrafficStats() != nullptr);
            CHECK(ts->GetDesiredLotInsertionPoint() == nullptr);
            // The x87 slots, with the test's allocator for slot 26's vector.
            static TrafficStub allocOnly;
            g_tr = &allocOnly;
            FC4TrafficTestInput in{};
            in.op = kFC4TrafficTestHooks;
            in.allocate = &TrAllocate;
            in.release = &TrFree;
            CHECK(seam(kFC4SeamTraffic, &in, nullptr) == 0);  // the allocator only
            TrafficRawVector pct;
            CheckTrafficX87(ts, &pct, 12);
            TrFree(pct.begin);
            pct = TrafficRawVector{};
            CHECK(allocOnly.blocks.empty());
            in.allocate = nullptr;
            in.release = nullptr;
            CHECK(seam(kFC4SeamTraffic, &in, nullptr) == 0);
            // The record before Init (§6): version 41, every reference null.
            int nullRefs = 0;
            record = TrafficWrite(ts, &nullRefs);
            CHECK(record.size() > 64);
            CHECK(nullRefs > 0);
            if (record.size() > 2) CHECK(record[0] == 0x29 && record[1] == 0);
            // Init without a city: no services, no grid; still neutral.
            CHECK(ts->Init());
            // The traffic shadow (seam op 51) with this simulator as the stand-in
            // original, from its first delivery (the day of month last seen is
            // not saved, §3.3: a copy started later would see a new day).
            if (mt != nullptr && ser != nullptr) TrafficShadowStandIn(seam, mt, ser);
            CHECK(ts->GetCongestionMap() == nullptr);
            CheckTrafficX87(ts, &pct, 0);  // the game's allocator is unavailable: the vector stays empty
            CHECK(pct.begin == nullptr);
            if (mt != nullptr) {
                TypeMessage day(kMsgSimNewDay);
                CHECK(mt->DoMessage(&day));
                TypeMessage month(kMsgSimNewMonth);
                CHECK(mt->DoMessage(&month));
                TrafficLockHeldMessage held(ts);
                CHECK(mt->DoMessage(&held));
                CHECK(held.ran && held.scale == ts->GetTripScale() && held.map == nullptr);
                CHECK(held.ms >= 30 && held.ms < 5000);  // two getters waited (about 20 ms each)
            }
            if (ser != nullptr) ser->Release();
            if (mt != nullptr) mt->Release();
            CHECK(ts->Shutdown());
            CHECK(ts->Release() == 0);
        }
        // Read runs before Init (§6): the record loads into a new simulator,
        // which writes it back byte for byte; a record below version 19 does
        // not load.
        cFC4ISC4TrafficSimulator* fresh = NewTraffic(director);
        CHECK(fresh != nullptr);
        if (fresh != nullptr && !record.empty()) {
            CHECK(TrafficRead(fresh, record));
            CHECK(TrafficWrite(fresh) == record);
            CHECK(fresh->Release() == 0);  // the final Release shuts a never-initialised simulator down
        }
        fresh = NewTraffic(director);
        if (fresh != nullptr && !record.empty()) {
            std::vector<uint8_t> old = record;
            old[0] = 0x12;
            CHECK(!TrafficRead(fresh, old));
            CHECK(fresh->Release() == 0);
        }
        TrafficPathFinderTest(seam, director);
        CheckCw("traffic");
    }
    FreeLibrary(hd);
    DeleteFileA(ini.c_str());
    const std::string text = ReadFile(log);
    CHECK(text.find("subsystem 'traffic': enabled (1 class replaced) by FreeCity4.ini (enable=)") !=
          std::string::npos);
    CHECK(text.find("cFC4TrafficSimulator(") != std::string::npos);
    CHECK(text.find("session seed 1234 (traffic_seed)") != std::string::npos);
    CHECK(text.find("traffic Write -> 1") != std::string::npos);
    CHECK(text.find("traffic Write: init=0 loaded=0 state=") != std::string::npos);
    CHECK(text.find("traffic Init: units per slice ") != std::string::npos);
    CHECK(text.find("traffic Init: init=1 loaded=0") != std::string::npos);
    CHECK(text.find("traffic: no city at Init") != std::string::npos);
    CHECK(text.find("traffic cycle -: state ") != std::string::npos);
    CHECK(text.find("traffic month - (#1): trips attempted=") != std::string::npos);
    CHECK(text.find("traffic Shutdown: init=0") != std::string::npos);
    CHECK(text.find("traffic Read -> 1") != std::string::npos);
    CHECK(text.find("traffic Read: init=0 loaded=1") != std::string::npos);
    CHECK(text.find("traffic Read -> 0") != std::string::npos);
    CHECK(text.find("cFC4TrafficSimulator destroyed (2 fallbacks, 0 external pathfinders)") != std::string::npos);
    CHECK(text.find("traffic: test hooks installed") != std::string::npos);
    // The traffic shadow on the stand-in original (seam op 51). glue27:
    // `enable = traffic` with `traffic_shadow = 1` is no longer refused; the
    // shadow runs as that replacement's self-check (FC4TrafficShadow.h).
    CHECK(text.find("traffic shadow: ON over FreeCity4's replacement (traffic_shadow from FreeCity4.ini, "
                    "`enable = traffic`)") != std::string::npos);
    CHECK(text.find("traffic shadow: requested (traffic_shadow from FreeCity4.ini) but INACTIVE") ==
          std::string::npos);
    CHECK(text.find("traffic shadow: comparing with the live traffic simulator") != std::string::npos);
    CHECK(text.find("traffic shadow month to 2000-02-01: clean; deliveries=5 (day 3, month 1, other 1)") !=
          std::string::npos);
    CHECK(text.find("INCOMPLETE") == std::string::npos);
    // TG1: a deque is filled, never refused; no fill failed.
    CHECK(text.find("FindPath with a route deque refused") == std::string::npos);
    CHECK(text.find("traffic: WARNING FindPath route deque") == std::string::npos);
    CHECK(text.find("cFC4TrafficSimulator destroyed (0 fallbacks, 2 external pathfinders)") != std::string::npos);
    CHECK(text.find("structured exception") == std::string::npos);
    CHECK(text.find("config: WARNING unknown key") == std::string::npos);
    CHECK(text.find("WARNING: released last reference") == std::string::npos);
    CHECK(text.find("WARNING: over-release") == std::string::npos);
    if (g_failures == 0) std::printf("plugin traffic enable test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}

// ---- close_dialogs (the harness lane, FC4Dialogs.h, test seam op 47) --------------------------------

// A child with close_dialogs off (= 0, unset, not 0/1) or on through the
// environment alone: the fakes are installed and a dialog opens; off, nothing
// may be subscribed or called. Returns 0 if the fakes saw what the scenario
// expects; the parent checks the log.
int DialogsChild(const std::string& dll, const std::string& ini, const std::string& scenario) {
    SetEnvironmentVariableA("FC4_CLOSE_DIALOGS", scenario == "env" ? "1" : nullptr);
    if (scenario == "off") {
        WriteTextFile(ini, "[harness]\nclose_dialogs = 0\n");
    } else if (scenario == "bad") {
        WriteTextFile(ini, "[harness]\nclose_dialogs = maybe\n");
    } else {  // unset, env
        WriteTextFile(ini, "; no close_dialogs key\n");
    }
    HMODULE h = LoadLibraryA(dll.c_str());
    if (h == nullptr) return 4;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(h, "GZDllGetGZCOMDirector"));
    auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(h, "FC4TestSeam"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    if (director == nullptr || seam == nullptr) return 5;
    std::vector<uint32_t> classes;
    director->EnumClassObjects(&CollectClass, &classes);  // game start: close_dialogs is read

    FakeDialogGame& g = g_dialogGame;
    DlgSetup(seam);
    DlgServices(true);
    static FakeDialog a;
    DlgWindow(a, 0x2A5CFB2Cu);
    const intptr_t started = DlgSeam(kFC4DialogsTestStart);  // as PostAppInit
    DlgBegin(a);
    for (int i = 0; i < 40; ++i) DlgFrame();
    const intptr_t closed = DlgSeam(kFC4DialogsTestClosed);
    const intptr_t stopped = DlgSeam(kFC4DialogsTestStop);  // as PreAppShutdown
    FreeLibrary(h);

    const bool on = scenario == "env";
    int bad = 0;
    if (started != (on ? 1 : 0) || stopped != (on ? 1 : 0)) bad |= 1;
    if (g.adds != (on ? 2 : 0) || g.removes != (on ? 2 : 0)) bad |= 2;
    if (on) {
        if (a.presses != 1 || closed != 1 || g.nest != 0) bad |= 4;
    } else if (a.presses != 0 || a.endModals != 0 || a.flagReads != 0 || g.validCalls != 0 || g.nestCalls != 0 ||
               g.focusCalls != 0 || closed != 0) {
        bad |= 8;  // off: not one call
    }
    if (bad != 0) std::fprintf(stderr, "dialogs child %s: fake check bits 0x%X\n", scenario.c_str(), bad);
    return bad == 0 ? 0 : 10 + bad;
}

int DialogsIniTest(const std::string& dll, const std::string& ini, const std::string& log) {
    for (const char* v : {"FC4_DISABLE", "FC4_ENABLE", "FC4_CLOSE_DIALOGS", "FC4_AUTOTEST_SECONDS",
                          "FC4_AUTOTEST_END_DATE", "FC4_SCRIPT"}) {
        SetEnvironmentVariableA(v, nullptr);
    }
    const auto npos = std::string::npos;
    const std::string dllArg = "\"" + dll + "\" ";

    // Off: = 0, unset, not 0/1. On through the environment alone.
    for (const char* scenario : {"off", "unset", "bad", "env"}) {
        DWORD ms = 0;
        const int code = RunChild(dllArg + "--dialogs-child-" + scenario, 30000, &ms);
        std::printf("dialogs child %s: exit code %d in %lu ms\n", scenario, code, static_cast<unsigned long>(ms));
        CHECK(code == 0);
        const std::string text = ReadFile(log);
        const std::string s = scenario;
        if (s == "env") {
            CHECK(text.find("dialogs: close_dialogs=1 (FC4_CLOSE_DIALOGS): game modal dialogs are ended through the "
                            "window manager") != npos);
            CHECK(CountOf(text, "dialogs: closed ") == 1);
        } else {
            CHECK(text.find("] dialogs:") == npos && text.find("[v] dialogs:") == npos);  // zero behaviour change: no line
            CHECK(text.find("cSC4WinGameDelay") == npos);
        }
        CHECK((s == "bad") == (text.find("config: WARNING close_dialogs='maybe' (FreeCity4.ini) is not 0/1; the "
                                         "dialog closer is off") != npos));
        CHECK(text.find("config: WARNING unknown key") == npos);  // close_dialogs is a known key
    }
    SetEnvironmentVariableA("FC4_CLOSE_DIALOGS", nullptr);

    // On, in this process.
    DeleteFileA(log.c_str());
    WriteTextFile(ini, "[harness]\nclose_dialogs = 1   ; end game dialogs from inside the game\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr && seam != nullptr);
    if (director == nullptr || seam == nullptr) return 1;
    std::vector<uint32_t> classes;
    director->EnumClassObjects(&CollectClass, &classes);  // game start: close_dialogs is read
    FakeDialogGame& g = g_dialogGame;
    DlgSetup(seam);
    DlgServices(true);

    // Before PostAppInit: no subscription, nothing called.
    for (int i = 0; i < 3; ++i) DlgTick();
    CHECK(g.adds == 0 && g.validCalls == 0);
    // PostAppInit: both notices, our target.
    CHECK(DlgSeam(kFC4DialogsTestStart) == 1);
    CHECK(g.adds == 2 && g.added[0] == kDlgBegin && g.added[1] == kDlgEnd && g.target != nullptr);
    CHECK(DlgSeam(kFC4DialogsTestStart) == 1);  // once
    CHECK(g.adds == 2);

    // 1. Closed by button: nothing inside the begin notice, nothing in the
    // grace (frames 1-4), button 0 at frame 5, the end notice 3 frames later.
    static FakeDialog a;
    DlgWindow(a, 0x2A5CFB2Cu);
    DlgBegin(a);
    CHECK(g.validCalls == 0 && a.presses == 0 && a.flagReads == 0);  // not inside the notice
    CHECK(DlgSeam(kFC4DialogsTestOpen) == 1);
    for (int i = 0; i < 4; ++i) DlgFrame();
    CHECK(a.presses == 0 && g.validCalls == 0);
    DlgFrame();
    CHECK(a.presses == 1 && a.pressType == 3 && a.pressCode == 0x287259F6u && a.pressButton == 0 &&
          a.pressToggle == 0 && a.flagReads == 1);
    for (int i = 0; i < 2; ++i) DlgFrame();
    CHECK(a.endSeq != 0 && g.nest == 0 && a.endModals == 0);
    CHECK(DlgSeam(kFC4DialogsTestClosed) == 1 && DlgSeam(kFC4DialogsTestOpen) == 0);
    a.valid = false;  // the owner destroys it (§2.6)
    const int validAfterA = g.validCalls;
    for (int i = 0; i < 40; ++i) DlgFrame();  // no dialog: nothing called
    CHECK(a.presses == 1 && g.validCalls == validAfterA);

    // 2. A dialog that ignores button 0: EndModal(window, 0) 30 frames after
    // the press, then its end notice 2 frames later.
    static FakeDialog b;
    DlgWindow(b, 0x6A4D0A59u);
    b.obeysButton = false;
    DlgBegin(b);
    for (int i = 0; i < 5; ++i) DlgFrame();
    CHECK(b.presses == 1 && b.pressButton == 0 && b.endModals == 0);
    for (int i = 0; i < 29; ++i) DlgFrame();
    CHECK(b.endModals == 0);
    DlgFrame();
    CHECK(b.endModals == 1 && b.endModalResult == 0 && b.presses == 1);
    DlgFrame();
    CHECK(b.endSeq != 0 && g.nest == 0);
    CHECK(DlgSeam(kFC4DialogsTestClosed) == 2 && DlgSeam(kFC4DialogsTestOpen) == 0);
    b.valid = false;

    // 3. A stale window: gone without an end notice. IsWindowValid says so,
    // and nothing else is called (not the nest count, not GetFlag, no button).
    static FakeDialog c;
    DlgWindow(c, 0x0C0C0C0Cu);
    DlgBegin(c);
    c.valid = false;
    c.modal = false;
    --g.nest;
    g.running.pop_back();
    g.focus = nullptr;
    const int nestBefore = g.nestCalls;
    for (int i = 0; i < 40; ++i) DlgFrame();
    CHECK(c.presses == 0 && c.endModals == 0 && c.flagReads == 0 && g.nestCalls == nestBefore);
    CHECK(DlgSeam(kFC4DialogsTestOpen) == 0 && DlgSeam(kFC4DialogsTestClosed) == 2);

    // 4. Nested: d opens, e opens inside it two frames later. e is pressed and
    // closed first; then d, at the first frame after e's end notice.
    static FakeDialog d, e;
    DlgWindow(d, 0x0D0D0D0Du);
    DlgWindow(e, 0x0E0E0E0Eu);
    DlgBegin(d);
    DlgFrame();
    DlgFrame();
    DlgBegin(e);
    CHECK(DlgSeam(kFC4DialogsTestOpen) == 2);
    for (int i = 0; i < 4; ++i) DlgFrame();
    CHECK(e.presses == 0 && d.presses == 0);  // e's grace (d is not the innermost)
    DlgFrame();
    CHECK(e.presses == 1 && d.presses == 0);
    for (int i = 0; i < 2; ++i) DlgFrame();
    CHECK(e.endSeq != 0 && g.nest == 1 && d.presses == 0);
    DlgFrame();
    CHECK(d.presses == 1 && d.pressSeq > e.endSeq);
    for (int i = 0; i < 2; ++i) DlgFrame();
    CHECK(d.endSeq != 0 && g.nest == 0 && d.endModals == 0 && e.endModals == 0);
    CHECK(DlgSeam(kFC4DialogsTestClosed) == 4 && DlgSeam(kFC4DialogsTestOpen) == 0);
    d.valid = e.valid = false;

    // 5. SC4GameDelay's invisible modal (its vtable traps): left alone; it
    // ends itself, and is not counted.
    static FakeDialog gd;
    DlgWindow(gd, 0x5F914000u, true);
    DlgBegin(gd);
    for (int i = 0; i < 80; ++i) DlgFrame();
    CHECK(gd.presses == 0 && gd.endModals == 0);
    gd.endIn = 1;
    DlgFrame();
    CHECK(gd.endSeq != 0 && g.nest == 0);
    CHECK(DlgSeam(kFC4DialogsTestClosed) == 4 && DlgSeam(kFC4DialogsTestOpen) == 0);
    gd.valid = false;

    // 6. Not (yet) modal: no button while its flag is clear; one WARNING after
    // 30 frames; the button once the flag is set.
    static FakeDialog f;
    DlgWindow(f, 0x0F0F0F0Fu);
    DlgBegin(f);
    f.modal = false;
    for (int i = 0; i < 40; ++i) DlgFrame();
    CHECK(f.presses == 0 && f.flagReads >= 30);
    f.modal = true;
    DlgFrame();
    CHECK(f.presses == 1);
    for (int i = 0; i < 2; ++i) DlgFrame();
    CHECK(f.endSeq != 0 && g.nest == 0);
    CHECK(DlgSeam(kFC4DialogsTestClosed) == 5);
    f.valid = false;

    // 7. PreAppShutdown: both notices removed (the server releases our target).
    CHECK(DlgSeam(kFC4DialogsTestStop) == 1);
    CHECK(g.removes == 2 && g.target == nullptr);
    CHECK(DlgSeam(kFC4DialogsTestStop) == 0);

    // 8. A late subscription: no legacy message server at PostAppInit (a
    // WARNING), a dialog opens unseen; when the server is there the closer
    // subscribes, walks from the focused child up to the modal window, and
    // closes it.
    DlgServices(false);
    CHECK(DlgSeam(kFC4DialogsTestStart) == 0);
    static FakeDialog h, child;
    DlgWindow(h, 0x0A0A0A0Au);
    DlgWindow(child, 0x0B0B0B0Bu);
    child.parent = &h;
    DlgBegin(h);  // nobody subscribed: no notice
    g.focus = &child;
    for (int i = 0; i < 3; ++i) DlgFrame();
    CHECK(h.presses == 0 && g.adds == 2);
    DlgServices(true);
    DlgFrame();  // subscribed, h adopted at depth 0
    CHECK(g.adds == 4 && g.focusCalls == 1 && child.flagReads == 1 && h.flagReads == 1 && h.presses == 0);
    CHECK(DlgSeam(kFC4DialogsTestOpen) == 1);
    for (int i = 0; i < 5; ++i) DlgFrame();
    CHECK(h.presses == 1);
    for (int i = 0; i < 2; ++i) DlgFrame();
    CHECK(h.endSeq != 0 && g.nest == 0);
    CHECK(DlgSeam(kFC4DialogsTestClosed) == 6 && DlgSeam(kFC4DialogsTestOpen) == 0);
    CHECK(DlgSeam(kFC4DialogsTestStop) == 1);
    CHECK(g.removes == 4);
    CHECK(g.strayCalls == 0);

    FreeLibrary(hd);
    DeleteFileA(ini.c_str());

    const std::string text = ReadFile(log);
    CHECK(text.find("dialogs: close_dialogs=1 (FreeCity4.ini): game modal dialogs are ended through the window "
                    "manager (modal-dialogs.md 3.2): button 0 5 frame(s) after the begin notice, EndModal 30 frame(s) "
                    "after that if no end notice came; subscribed at PostAppInit") != npos);
    CHECK(CountOf(text, "dialogs: subscribed to the modal begin / end notices 0x62BA7B42 / 0x62BA7B43 (legacy "
                        "message server 0x64063387) at PostAppInit") == 1);
    CHECK(CountOf(text, "dialogs: closed " + DlgName(a, 0) + " by button after 3 frame(s)") == 1);
    CHECK(CountOf(text, "dialogs: closed " + DlgName(b, 0) + " by EndModal fallback after 2 frame(s)") == 1);
    CHECK(CountOf(text, "dialogs: WARNING could not close " + DlgName(c, 0) +
                            ": the window is no longer valid (IsWindowValid false) and no end notice came; record "
                            "dropped, nothing called on it") == 1);
    CHECK(CountOf(text, "dialogs: closed " + DlgName(e, 1) + " by button after 3 frame(s)") == 1);
    CHECK(CountOf(text, "dialogs: closed " + DlgName(d, 0) + " by button after 3 frame(s)") == 1);
    CHECK(CountOf(text, "dialogs: left " + DlgName(gd, 0) + " alone: SC4GameDelay's invisible modal ends itself") ==
          1);
    CHECK(text.find("dialogs: closed " + DlgName(gd, 0)) == npos);
    CHECK(CountOf(text, "dialogs: WARNING could not close " + DlgName(f, 0) +
                            ": not the current modal window for 30 frame(s) (nest count 1, expected 1; modal flag 0); "
                            "still checked each frame") == 1);
    CHECK(CountOf(text, "dialogs: closed " + DlgName(f, 0) + " by button after 3 frame(s)") == 1);
    CHECK(CountOf(text, "dialogs: unsubscribed at PreAppShutdown (5 dialog(s) closed)") == 1);
    CHECK(CountOf(text, "dialogs: WARNING could not subscribe to the modal notices at PostAppInit: no legacy message "
                        "server 0x64063387 (no framework); retried at each frame") == 1);
    CHECK(CountOf(text, "dialogs: subscribed to the modal begin / end notices 0x62BA7B42 / 0x62BA7B43 (legacy "
                        "message server 0x64063387) at a frame") == 1);
    CHECK(CountOf(text, "dialogs: adopted " + DlgName(h, 0) + ": it was open before the subscription") == 1);
    CHECK(CountOf(text, "dialogs: closed " + DlgName(h, 0) + " by button after 3 frame(s)") == 1);
    CHECK(CountOf(text, "dialogs: unsubscribed at PreAppShutdown (6 dialog(s) closed)") == 1);
    CHECK(CountOf(text, "dialogs: closed ") == 6);
    CHECK(CountOf(text, "dialogs: WARNING") == 3);  // the stale window, the flag, the late server
    CHECK(text.find("config: WARNING") == npos);
    CHECK(text.find("WARNING: released last reference") == npos && text.find("WARNING: over-release") == npos);
    if (g_failures == 0) std::printf("plugin close_dialogs test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}

// The history warehouse and the 24-hour clock (history.md, clock24.md; seam op 54).
#include "plugin_histclock_test.inc"

// ---- the tract developer (tract-developer.md; test seam op 45) ----------------------------------------------
constexpr uint32_t kCLSID_TractDeveloper = 0x2990C142;
constexpr uint32_t kIID_cISC4TractDeveloper = 0xA818ED1B;
constexpr uint32_t kMsgSimNewYear = 0x66956817;

// The exe has no game: its own tract containers (FC4TractInterfaces.h) use
// the test allocator, never the game's (this stands in for the plugin's
// compat table, which the exe does not link).
namespace fc4::compat {
bool StlNodeAllocator(StlNodeAllocateFn*, StlNodeFreeFn*) { return false; }
}  // namespace fc4::compat

void* TractTestAllocate(uint32_t size) { return std::malloc(size); }
void TractTestFree(void* p) { std::free(p); }

// The record of §6: version 3, then 19 + 4 x count bytes in all.
bool TractRecordShape(const std::vector<uint8_t>& r) {
    if (r.size() < 19 || r[0] != 3 || r[1] != 0) return false;
    uint32_t count = 0;
    std::memcpy(&count, r.data() + 11, 4);
    return r.size() == 19 + 4 * static_cast<size_t>(count);
}

int TractEnableTest(const std::string& dll, const std::string& ini, const std::string& log) {
    // Off by default; `enable = tract` registers cFC4TractDeveloper. No city:
    // the QI table (§1.1), the slots' return channels (§7.1), the record
    // (§6), Init / Shutdown and a Grow without services, the seam's filter
    // counts and the shadow's answers (since glue27 a self-check over ours).
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    SetEnvironmentVariableA("FC4_TRACT_SEED", nullptr);
    SetEnvironmentVariableA("FC4_TRACT_SHADOW", nullptr);
    SetEnvironmentVariableA("FC4_TRACT_GROW_LOG", nullptr);
    WriteTextFile(ini, "[subsystems]\nenable = tract\ntract_seed = 0x2A\n[tools]\ntract_shadow = 1\n"
                       "tract_grow_log = 1\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr && seam != nullptr);
    SetGamePrecision();
    // The game containers come from malloc here (the exe's and the DLL's
    // copies of the allocator both).
    fc4::TractNodeAllocator::TestAllocate() = &TractTestAllocate;
    fc4::TractNodeAllocator::TestFree() = &TractTestFree;
    if (director != nullptr && seam != nullptr) {
        FC4TractTestInput hooks{};
        hooks.op = kFC4TractTestHooks;
        hooks.allocate = &TractTestAllocate;
        hooks.free = &TractTestFree;
        CHECK(seam(kFC4SeamTract, &hooks, nullptr) == 0);  // the allocator only
        CHECK(seam(kFC4SeamTract, nullptr, nullptr) == -1);
        std::vector<uint32_t> classes;
        director->EnumClassObjects(&CollectClass, &classes);
        CHECK(classes.size() == 5);  // simulator (1) + budget (3) + tract (1)
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_TractDeveloper) == 1);

        std::vector<uint8_t> record;
        void* obj = nullptr;
        CHECK(director->GetClassObject(kCLSID_TractDeveloper, kIID_cISC4TractDeveloper, &obj));
        auto* td = static_cast<cFC4ISC4TractDeveloper*>(obj);
        if (td != nullptr) {
            // §1.1: the primary for 1 and its IID, the agent (+4) and the
            // serializable (+8); nothing else, the out pointer untouched.
            void* p = nullptr;
            CHECK(td->QueryInterface(GZIID_cIGZUnknown, &p) && p == static_cast<void*>(td));
            if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
            p = nullptr;
            CHECK(td->QueryInterface(kIID_cISC4TractDeveloper, &p) && p == static_cast<void*>(td));
            if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
            cIGZMessageTarget2* mt = nullptr;
            CHECK(td->QueryInterface(0x452294AA, reinterpret_cast<void**>(&mt)) && mt != nullptr &&
                  static_cast<void*>(mt) != static_cast<void*>(td));
            cIGZSerializable* ser = nullptr;
            CHECK(td->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) && ser != nullptr);
            if (ser != nullptr) CHECK(ser->GetGZCLSID() == kCLSID_TractDeveloper);
            p = reinterpret_cast<void*>(0x1234);
            CHECK(!td->QueryInterface(0x12345678, &p) && p == reinterpret_cast<void*>(0x1234));

            // §7.1: slot 7 returns 0, 12 and 14 `this`, 10 slot 9's vector.
            FC4GrowthProfile prof{};
            prof.x = 1;
            prof.z = 1;
            prof.stage = 1;
            CHECK(td->UpdatePopulation(prof) == 0);
            CHECK(td->SetYearsBetweenStyleChanges(7) == td);
            CHECK(td->GetYearsBetweenStyleChanges() == 7);
            CHECK(td->UseAllStylesAtOnce(true) == td);
            CHECK(td->IsUsingAllStylesAtOnce());
            CHECK(td->UseAllStylesAtOnce(false) == td);
            CHECK(!td->IsUsingAllStylesAtOnce());
            fc4::GameVector<uint32_t>& styles = td->GetActiveStyles();
            const size_t before = styles.size();
            CHECK(before == 0);  // §2: empty until Init (or a Read) fills it
            CHECK(td->SetActiveStyles(styles) == reinterpret_cast<intptr_t>(&styles));
            CHECK(td->GetActiveStyles().size() == before);
            {
                fc4::TractStyleVector mine;
                CHECK(mine.push_back(0x2001) && mine.push_back(0x2002));
                CHECK(td->SetActiveStyles(fc4::AsGameB(mine)) == reinterpret_cast<intptr_t>(&styles));
                CHECK(td->GetActiveStyles().size() == 2 && styles[0] == 0x2001 && styles[1] == 0x2002);
            }
            // The record before Init (§6), then Init without a city.
            if (ser != nullptr) {
                ResidentialSegOStream out;
                CHECK(ser->Write(out));
                record = out.bytes;
                CHECK(TractRecordShape(record));
            }
            CHECK(!td->Init());  // §1.3 step 3: no city services
            // Grow without services: nothing built, no outcome code (Init failed).
            {
                fc4::TractOutcomeList outcomes;
                bool flag = true;
                CHECK(td->Grow(prof, reinterpret_cast<fc4::GameList<int32_t>&>(outcomes), flag) == 0);
                CHECK(outcomes.valid() && outcomes.begin() == outcomes.end());  // not initialised: no code
            }
            CHECK(td->Decline(prof) == 0);
            if (mt != nullptr) {
                ResidentialDayMessage year(kMsgSimNewYear, kJulian20000201);
                CHECK(mt->DoMessage(&year));
                ResidentialDayMessage other(0x12345678, kJulian20000201);
                CHECK(mt->DoMessage(&other));
            }
            FC4TractFilterCounts counts{};
            FC4TractTestInput fin{};
            fin.op = kFC4TractTestFilters;
            fin.tractDeveloper = td;
            CHECK(seam(kFC4SeamTract, &fin, &counts) == 1);
            // No city: no configuration manager to register with.
            CHECK(counts.registered == 0 && counts.unregistered == 0);
            CHECK(seam(kFC4SeamTract, &fin, nullptr) == -1);
            // The shadow: no city -> -1.
            FC4TractTestInput sh{};
            sh.op = kFC4TractTestShadow;
            sh.julian = kJulian20000201;
            CHECK(seam(kFC4SeamTract, &sh, nullptr) == -1);
            if (ser != nullptr) ser->Release();
            if (mt != nullptr) mt->Release();
            CHECK(td->Shutdown());
            CHECK(seam(kFC4SeamTract, &fin, &counts) == 1);
            CHECK(counts.live == 0);
            CHECK(td->Release() == 0);
        }
        // Read runs before Init (§1.2): the record loads into a new object; a
        // truncated one does not; a newer version does not.
        for (int variant = 0; variant < 3; ++variant) {
            obj = nullptr;
            CHECK(director->GetClassObject(kCLSID_TractDeveloper, kIID_cISC4TractDeveloper, &obj));
            auto* fresh = static_cast<cFC4ISC4TractDeveloper*>(obj);
            if (fresh == nullptr) continue;
            cIGZSerializable* s = nullptr;
            CHECK(fresh->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&s)) && s != nullptr);
            if (s != nullptr && !record.empty()) {
                std::vector<uint8_t> bytes = record;
                if (variant == 1) bytes.resize(bytes.size() - 2);
                if (variant == 2) bytes[0] = 4;
                ResidentialSegIStream in(bytes);
                CHECK(s->Read(in) == (variant == 0));
                if (variant == 0) {
                    CHECK(fresh->GetYearsBetweenStyleChanges() == 7);
                    CHECK(fresh->GetActiveStyles().size() == 2);
                    ResidentialSegOStream again;
                    CHECK(s->Write(again));
                    CHECK(again.bytes == record);
                }
                s->Release();
            }
            CHECK(fresh->Release() == 0);  // the final Release shuts a never-initialised one down
        }
        FC4TractFilterCounts counts{};
        FC4TractTestInput fin{};
        fin.op = kFC4TractTestFilters;
        CHECK(seam(kFC4SeamTract, &fin, &counts) == 1 && counts.live == 0);
        CheckCw("tract");
    }
    FreeLibrary(hd);
    fc4::TractNodeAllocator::TestAllocate() = nullptr;
    fc4::TractNodeAllocator::TestFree() = nullptr;
    DeleteFileA(ini.c_str());
    const std::string text = ReadFile(log);
    const auto npos = std::string::npos;
    CHECK(text.find("subsystem 'tract': enabled (1 class replaced) by FreeCity4.ini (enable=)") != npos);
    CHECK(text.find(", session seed 0x0000002A (tract_seed), tract_grow_log on") != npos);
    CHECK(CountOf(text, "cFC4TractDeveloper(") == 4);
    CHECK(CountOf(text, "cFC4TractDeveloper destroyed (0 fallbacks)") == 4);
    CHECK(text.find("tract Write -> 1") != npos);
    CHECK(text.find("tract: no city; services not acquired") != npos);
    CHECK(text.find("tract Init FAILED: init=0 configured=1 ") != npos);
    CHECK(text.find("tract Grow dev=00000000 tract=1,1 stage=1 -> built=0") != npos);
    CHECK(text.find("tract NewYear: init=") != npos);
    CHECK(CountOf(text, "tract Read -> 1") == 1);
    CHECK(CountOf(text, "tract Read -> 0") == 2);
    CHECK(text.find("tract Shutdown: init=0") != npos);
    // glue27: `enable = tract` with `tract_shadow = 1` is no longer refused;
    // the shadow runs as that replacement's self-check (FC4TractShadow.h).
    CHECK(text.find("tract shadow: ON over FreeCity4's replacement (tract_shadow from FreeCity4.ini, "
                    "`enable = tract`)") != npos);
    CHECK(text.find("tract shadow: requested (tract_shadow from FreeCity4.ini) but INACTIVE") == npos);
    CHECK(text.find("structured exception") == npos);
    CHECK(text.find("config: WARNING unknown key") == npos);
    CHECK(text.find("WARNING: released last reference") == npos);
    CHECK(text.find("WARNING: over-release") == npos);
    CHECK(text.find("streams: WARNING") == npos);
    if (g_failures == 0) std::printf("plugin tract enable test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}

// ---- the lot developer (lot-developer.md; test seam op 52) ------------------------------------------------
constexpr uint32_t kCLSID_LotDeveloper = 0xA990BFE0;
constexpr uint32_t kIID_cISC4LotDeveloper = 0x4818E5D2;

// A stub host (seam op 52): Init's registrations are counted, the city is
// 64 x 64 cells of 16 m.
struct LotDevStub {
    std::vector<uint32_t> agents, subscribed, unsubscribed;
    uint32_t ticks = 0, unticks = 0, removeAgents = 0, servers = 0, releasedServers = 0;
};
LotDevStub g_lotdevStub;

bool LdAcquire(void*) { return true; }
void LdRelease(void*) {}
bool LdHasSim(void*) { return true; }
void LdAddAgent(void*, uint32_t level, uint32_t) { g_lotdevStub.agents.push_back(level); }
void LdRemoveAgent(void*) { ++g_lotdevStub.removeAgents; }
bool LdServer(void*) {
    ++g_lotdevStub.servers;
    return true;
}
void LdReleaseServer(void*) { ++g_lotdevStub.releasedServers; }
void LdSubscribe(void*, uint32_t m) { g_lotdevStub.subscribed.push_back(m); }
void LdUnsubscribe(void*, uint32_t m) { g_lotdevStub.unsubscribed.push_back(m); }
void LdTick(void*) { ++g_lotdevStub.ticks; }
void LdUntick(void*) { ++g_lotdevStub.unticks; }
void LdCells(void*, int32_t* out2) {
    out2[0] = 64;
    out2[1] = 64;
}
void LdWidths(void*, float* out2) {
    out2[0] = 16.0f;
    out2[1] = 16.0f;
}
float LdCellWidth(void*) { return 16.0f; }
int32_t LdLotCount(void*) { return 0; }
uint64_t LdIterator(void*) { return 0; }

// The record of §13 with an empty night deque: version 1, then 22 bytes.
bool LotDevRecordShape(const std::vector<uint8_t>& r) {
    if (r.size() != 24 || r[0] != 1 || r[1] != 0) return false;
    uint32_t n = 1;
    std::memcpy(&n, r.data() + 14, 4);
    return n == 0;
}

int LotDevEnableTest(const std::string& dll, const std::string& ini, const std::string& log) {
    // Off by default; `enable = lotdev` registers cFC4LotDeveloper. No city:
    // the QI table (§1.1), Release's 0 (§1.4), the record (§13), Init without
    // services; then Init / Shutdown on a stub host (seam op 52) and the
    // shadow's answers (inactive with ours).
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    SetEnvironmentVariableA("FC4_LOTDEV_SEED", nullptr);
    SetEnvironmentVariableA("FC4_LOTDEV_SHADOW", nullptr);
    SetEnvironmentVariableA("FC4_LOTDEV_LOT_LOG", nullptr);
    WriteTextFile(ini, "[subsystems]\nenable = lotdev\nlotdev_seed = 0x2A\n[tools]\nlotdev_shadow = 1\n"
                       "lotdev_lot_log = 1\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr && seam != nullptr);
    SetGamePrecision();
    if (director != nullptr && seam != nullptr) {
        FC4LotDevTestInput alloc{};
        alloc.op = kFC4LotDevTestHooks;
        alloc.allocate = &TractTestAllocate;
        alloc.free = &TractTestFree;
        CHECK(seam(kFC4SeamLotDev, &alloc, nullptr) == 0);  // the allocator only
        CHECK(seam(kFC4SeamLotDev, nullptr, nullptr) == -1);
        std::vector<uint32_t> classes;
        director->EnumClassObjects(&CollectClass, &classes);
        CHECK(classes.size() == 5);  // simulator (1) + budget (3) + lotdev (1)
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_LotDeveloper) == 1);

        std::vector<uint8_t> record;
        void* obj = nullptr;
        CHECK(director->GetClassObject(kCLSID_LotDeveloper, kIID_cISC4LotDeveloper, &obj));
        auto* ld = static_cast<cFC4ISC4LotDeveloper*>(obj);
        if (ld != nullptr) {
            // §1.1: the primary for 1 and its IID, the message target (+4)
            // and the serializable (+8); nothing else, the out pointer untouched.
            void* p = nullptr;
            CHECK(ld->QueryInterface(GZIID_cIGZUnknown, &p) && p == static_cast<void*>(ld));
            if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
            p = nullptr;
            CHECK(ld->QueryInterface(kIID_cISC4LotDeveloper, &p) && p == static_cast<void*>(ld));
            if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
            cIGZMessageTarget2* mt = nullptr;
            CHECK(ld->QueryInterface(0x452294AA, reinterpret_cast<void**>(&mt)) && mt != nullptr &&
                  static_cast<void*>(mt) == static_cast<void*>(reinterpret_cast<char*>(ld) + 4));
            cIGZSerializable* ser = nullptr;
            CHECK(ld->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) && ser != nullptr &&
                  static_cast<void*>(ser) == static_cast<void*>(reinterpret_cast<char*>(ld) + 8));
            if (ser != nullptr) CHECK(ser->GetGZCLSID() == kCLSID_LotDeveloper);
            p = reinterpret_cast<void*>(0x1234);
            CHECK(!ld->QueryInterface(0x12345678, &p) && p == reinterpret_cast<void*>(0x1234));
            // §1.4: Release answers 0, not the count.
            CHECK(ld->AddRef() > 1);
            CHECK(ld->Release() == 0);
            // §11.1: slot 5, the stub of slot 23, the force-empty flag.
            CHECK(ld->GetSimulatorType() == 0x6818E5C9);
            CHECK(!ld->IsAreaDevelopedStub(SC4Rect<int32_t>(0, 0, 1, 1)));
            CHECK(!ld->GetForceEmptyLotBaseTextures());
            // The record before Init (§13), then Init without a city.
            if (ser != nullptr) {
                ResidentialSegOStream out;
                CHECK(ser->Write(out));
                record = out.bytes;
                CHECK(LotDevRecordShape(record));
            }
            CHECK(!ld->Init());  // §1.3 step 3: no city services
            // The stub host: Init registers the agent at 0, 4, 6, the nine
            // messages in order and the animation tick (§1.3 steps 8-10).
            static fc4_lotdev_host stub{};
            stub.struct_size = sizeof(stub);
            stub.acquire_services = &LdAcquire;
            stub.release_services = &LdRelease;
            stub.has_simulator = &LdHasSim;
            stub.add_agent = &LdAddAgent;
            stub.remove_agent = &LdRemoveAgent;
            stub.acquire_message_server = &LdServer;
            stub.release_message_server = &LdReleaseServer;
            stub.subscribe = &LdSubscribe;
            stub.unsubscribe = &LdUnsubscribe;
            stub.register_animation_tick = &LdTick;
            stub.unregister_animation_tick = &LdUntick;
            stub.city_cell_counts = &LdCells;
            stub.city_cell_widths = &LdWidths;
            stub.terrain_cell_counts = &LdCells;
            stub.terrain_cell_width = &LdCellWidth;
            stub.lot_count = &LdLotCount;
            stub.safe_lot_iterator = &LdIterator;
            FC4LotDevTestInput hooks{};
            hooks.op = kFC4LotDevTestHooks;
            hooks.lotDeveloper = ld;
            hooks.host = &stub;
            hooks.allocate = &TractTestAllocate;
            hooks.free = &TractTestFree;
            CHECK(seam(kFC4SeamLotDev, &hooks, nullptr) == 1);
            CHECK(ld->Init());
            CHECK((g_lotdevStub.agents == std::vector<uint32_t>{0, 4, 6}));
            CHECK((g_lotdevStub.subscribed == std::vector<uint32_t>{0x44778234, 0x99EF1142, 0x99EF1143, 0xE8BDA2E7,
                                                                     0xE99B3D4D, 0x278E6003, 0x8A07AE00, 0x26D31EC4,
                                                                     0x69247DC7}));
            CHECK(g_lotdevStub.ticks == 1 && g_lotdevStub.servers == 1);
            FC4LotDevTestState st{};
            FC4LotDevTestInput sin{};
            sin.op = kFC4LotDevTestState;
            sin.lotDeveloper = ld;
            CHECK(seam(kFC4SeamLotDev, &sin, &st) == 1);
            CHECK(st.core.initialized == 1 && st.core.verts_x == 65);
            CHECK(st.core.extent_x == 1024.0f && st.core.extent_z == 1024.0f);
            CHECK(st.core.cell_count_x == 64 && st.core.cell_count_z == 64);
            CHECK(st.held == 0 && st.iterators == 0);
            CHECK(seam(kFC4SeamLotDev, &sin, nullptr) == -1);
            CHECK(ld->Init());  // already initialised: true at once, nothing registered again
            CHECK(g_lotdevStub.agents.size() == 3 && g_lotdevStub.subscribed.size() == 9);
            if (mt != nullptr) {
                ResidentialDayMessage other(0x12345678, kJulian20000201);
                CHECK(mt->DoMessage(&other));
            }
            // The shadow: no city -> -1.
            FC4LotDevTestInput sh{};
            sh.op = kFC4LotDevTestShadow;
            sh.julian = kJulian20000201;
            CHECK(seam(kFC4SeamLotDev, &sh, nullptr) == -1);
            if (ser != nullptr) ser->Release();
            if (mt != nullptr) mt->Release();
            CHECK(ld->Shutdown());
            CHECK(g_lotdevStub.removeAgents == 1 && g_lotdevStub.unticks == 1 && g_lotdevStub.releasedServers == 1);
            CHECK(g_lotdevStub.unsubscribed == g_lotdevStub.subscribed);
            CHECK(ld->Release() == 0);
        }
        // Read runs before Init (§1.2): the record loads into a new object; a
        // truncated one does not; another version does not.
        for (int variant = 0; variant < 3; ++variant) {
            obj = nullptr;
            CHECK(director->GetClassObject(kCLSID_LotDeveloper, kIID_cISC4LotDeveloper, &obj));
            auto* fresh = static_cast<cFC4ISC4LotDeveloper*>(obj);
            if (fresh == nullptr) continue;
            cIGZSerializable* s = nullptr;
            CHECK(fresh->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&s)) && s != nullptr);
            if (s != nullptr && !record.empty()) {
                std::vector<uint8_t> bytes = record;
                if (variant == 1) bytes.resize(bytes.size() - 2);
                if (variant == 2) bytes[0] = 2;
                ResidentialSegIStream in(bytes);
                CHECK(s->Read(in) == (variant == 0));
                if (variant == 0) {
                    ResidentialSegOStream again;
                    CHECK(s->Write(again));
                    CHECK(again.bytes == record);
                }
                s->Release();
            }
            CHECK(fresh->Release() == 0);  // the final Release shuts a never-initialised one down
        }
        alloc.allocate = nullptr;
        alloc.free = nullptr;
        CHECK(seam(kFC4SeamLotDev, &alloc, nullptr) == 0);
        CheckCw("lotdev");
    }
    FreeLibrary(hd);
    DeleteFileA(ini.c_str());
    const std::string text = ReadFile(log);
    const auto npos = std::string::npos;
    CHECK(text.find("subsystem 'lotdev': enabled (1 class replaced) by FreeCity4.ini (enable=)") != npos);
    CHECK(text.find(", session seed 0x0000002A (lotdev_seed), lotdev_lot_log on") != npos);
    CHECK(CountOf(text, "cFC4LotDeveloper(") == 4);
    CHECK(CountOf(text, "cFC4LotDeveloper destroyed") == 4);
    CHECK(text.find("lotdev Write -> 1 (24 bytes)") != npos);
    CHECK(text.find("lotdev: no city; services not acquired") != npos);
    CHECK(text.find("lotdev Init FAILED: init=0 ") != npos);
    CHECK(text.find("lotdev: test hooks installed") != npos);
    CHECK(text.find("lotdev Init: init=1 daily=0 visited=0 vertsX=65 extent=1024.000,1024.000 cells=64,64") != npos);
    CHECK(CountOf(text, "lotdev Read -> 1") == 1);
    CHECK(CountOf(text, "lotdev Read -> 0") == 2);
    CHECK(text.find("lotdev Shutdown: init=0") != npos);
    CHECK(text.find("lotdev shadow: requested (lotdev_shadow from FreeCity4.ini) but INACTIVE") != npos);
    CHECK(text.find("lotdev shadow: ON") == npos);
    CHECK(text.find("structured exception") == npos);
    CHECK(text.find("config: WARNING unknown key") == npos);
    CHECK(text.find("WARNING: released last reference") == npos);
    CHECK(text.find("WARNING: over-release") == npos);
    CHECK(text.find("streams: WARNING") == npos);
    if (g_failures == 0) std::printf("plugin lotdev enable test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}
// The cmglue lane: commercial / industrial, civic, parks (plugin_cmglue_test.inc).
#include "plugin_cmglue_test.inc"
// The glue12b1 lane: the prop developer and the city detail manager (plugin_propcity_test.inc).
#include "plugin_propcity_test.inc"
// The glue12b2 lane: the airport and seaport developers (plugin_ports_test.inc).
#include "plugin_ports_test.inc"// The glue16 lane: the terrain shadow (plugin_terrain_test.inc, seam op 58).
#include "FC4TerrainShadow.h"
#include "plugin_terrain_test.inc"
// The glue15 lane: the tutorial system (plugin_glue15_test.inc).
#include "cISC4TutorialSystem.h"
#include "plugin_glue15_test.inc"
#include "plugin_advisors_replace_test.inc"
#include "plugin_netrules_replace_test.inc"
#include "plugin_prop_occupant_compare_test.inc"
#include "plugin_mysims_factory_test.inc"
#include "plugin_disasters_test.inc"
#include "plugin_effects_registration_test.inc"  // Guarded registration, ownership, and section 18.2 target layout.
#include "plugin_netocc_registration_test.inc"
#include "plugin_cfgrules_registration_test.inc"
#include "plugin_lotrec_boundary_test.inc"
#include "plugin_lotconf_ownership_test.inc"
#include "plugin_netlot_ownership_test.inc"
#include "plugin_effects_resource_registration_test.inc"
#include "plugin_demolition_building_test.inc"
#include "plugin_mysims_ownership_test.inc"
#include "plugin_power_line_connections_test.inc"

int main(int argc, char** argv) {
    prop_compare_test::Check();
    if (argc < 2) {
        std::fprintf(stderr, "usage: %s <FreeCity4.dll>\n", argv[0]);
        return 2;
    }
    const std::string dll = argv[1];
    const std::string mode = argc >= 3 ? argv[2] : "--normal";
    const std::string log = dll.substr(0, dll.find_last_of("\\/") + 1) + "FreeCity4.log";
    const std::string ini = dll.substr(0, dll.find_last_of("\\/") + 1) + "FreeCity4.ini";
    DeleteFileA(log.c_str());
    DeleteFileA(ini.c_str());  // a leftover from an aborted run must not change this one
    SetEnvironmentVariableA("FC4_LOG_LEVEL", "2");
    SetEnvironmentVariableA("FC4_DISABLE_SIMULATOR", nullptr);
    SetEnvironmentVariableA("FC4_OBSERVE", "1");  // the same lines in release and checked builds
    SetEnvironmentVariableA("FC4_AUTOTEST_SAVE_AFTER_MONTHS", nullptr);

    if (mode == "--ini-enable-mysims") return MySimsFactoryBoundaryTest(dll, ini, log, MySimsIni::Alone);
    if (mode == "--ini-default-mysims") return MySimsFactoryBoundaryTest(dll, ini, log, MySimsIni::Default);
    if (mode == "--ini-mixed-mysims") return MySimsFactoryBoundaryTest(dll, ini, log, MySimsIni::Mixed);
    if (mode == "--ini-replace-mysims") return MySimsFactoryBoundaryTest(dll, ini, log, MySimsIni::Replace);
    if (mode == "--effects-enabled" || mode == "--effects-disabled" || mode == "--effects-default" ||
        mode == "--effects-guarded")
        return EffectsRegistrationTest(dll, ini, log, mode);

    if (mode == "--effectsres-default" || mode == "--effectsres-enabled" ||
        mode == "--effectsres-disabled" || mode == "--effectsres-mixed")
        return EffectsResourceRegistrationTest(dll, ini, log, mode);

    const std::string childPrefix = "--autotest-child-";
    if (mode.compare(0, childPrefix.size(), childPrefix) == 0) {
        return AutotestChild(dll, ini, mode.substr(childPrefix.size()));
    }
    // popup_dialogs: the parent runs one child per FreeCity4.ini.
    const std::string popupsPrefix = "--popups-child-";
    if (mode.compare(0, popupsPrefix.size(), popupsPrefix) == 0) {
        return PopupsChild(dll, ini, mode.substr(popupsPrefix.size()));
    }
    if (mode == "--ini-popups") return PopupsTest(dll, ini, log);
    // Protection (protection.md): the replacement, and the grid shadow.
    if (mode == "--ini-enable-protection") return ProtectionEnableTest(dll, ini, log);
    if (mode == "--ini-protection-shadow") return ProtectionShadowTest(dll, ini, log);
    // Aura (aura.md): the replacement, and the grid shadow on fake originals.
    if (mode == "--ini-enable-aura") return AuraEnableTest(dll, ini, log);
    // Flora (flora.md): the replacement, and the census shadow on fake originals.
    if (mode == "--ini-enable-flora") return FloraEnableTest(dll, ini, log);
    // Landfill (landfill.md): the replacement, and the shadow on fake originals.
    if (mode == "--ini-enable-landfill") return LandfillEnableTest(dll, ini, log);
    // Residential (residential.md): the replacement, and the shadow on fake originals.
    if (mode == "--ini-enable-residential") return ResidentialEnableTest(dll, ini, log);
    if (mode == "--ini-enable-zones") return ZonesEnableTest(dll, ini, log);
    // The zone developer / lot manager (zones.md §1-§11): the replacement, stub lots (op 53), the shadow.
    if (mode == "--ini-enable-zonedev") return ZoneDevEnableTest(dll, ini, log);
    // Neighbours (neighbors.md): the replacement, its objects, an original connection loaded.
    if (mode == "--ini-enable-neighbors") return NeighborsEnableTest(dll, ini, log);
    // Development (building-development.md): the replacement without a city.
    if (mode == "--ini-enable-development") return DevelopmentEnableTest(dll, ini, log);
    // Traffic (traffic-simulator.md): the replacement without a city, slot-6 pathfinders on a stub (op 42).
    if (mode == "--ini-enable-traffic") return TrafficEnableTest(dll, ini, log);
    // The development shadow: ON with the original unit, its list comparison (seam op 34).
    if (mode == "--ini-development-shadow") return DevelopmentShadowIniTest(dll, ini, log);
    // The tract developer (tract-developer.md): the replacement without a city (seam op 45).
    if (mode == "--ini-enable-tract") return TractEnableTest(dll, ini, log);
    // The lot developer (lot-developer.md): the replacement without a city (seam op 52).
    if (mode == "--ini-enable-lotdev") return LotDevEnableTest(dll, ini, log);
    // The cmglue lane (plugin_cmglue_test.inc): commercial / industrial, civic, parks.
    if (mode == "--ini-enable-commercial") return CommercialEnableTest(dll, ini, log);
    if (mode == "--ini-enable-civic") return CivicEnableTest(dll, ini, log);
    if (mode == "--ini-enable-parks") return ParksEnableTest(dll, ini, log);
    // The glue12b1 lane (plugin_propcity_test.inc): the prop developer, the city detail manager.
    if (mode == "--ini-enable-propdev") return PropDevEnableTest(dll, ini, log);
    if (mode == "--ini-enable-citydetail") return CityDetailEnableTest(dll, ini, log);
    // The glue12b2 lane (plugin_ports_test.inc): the airport and seaport developers.
    if (mode == "--ini-enable-ports") return PortsEnableTest(dll, ini, log);    // The glue16 lane: the terrain shadow on a stand-in original (seam op 58).
    if (mode == "--ini-terrain-shadow") return TerrainShadowIniTest(dll, ini, log);
    // Land value and flammability grid shadows on fake originals; both inactive with ours.
    if (mode == "--ini-landvalue-shadow") return LandValueShadowTest(dll, ini, log);
    if (mode == "--ini-flammability-shadow") return FlammabilityShadowTest(dll, ini, log);
    if (mode == "--ini-lvfl-shadow-inactive") return LvFlShadowInactiveTest(dll, ini, log);
    // References read from a save (in-game run 88): land value grids, a foreign ordinance.
    if (mode == "--saved-refs") return SavedRefsTest(dll, ini, log);
    // The growth calibration probe (building-development.md §12, seam op 39).
    if (mode == "--ini-growth-probe") return GrowthProbeIniTest(dll, ini, log);
    // The harness lane: sim_turbo (FC4Turbo.h).
    const std::string turboPrefix = "--turbo-child-";
    if (mode.compare(0, turboPrefix.size(), turboPrefix) == 0) return TurboChild(dll, ini, mode.substr(turboPrefix.size()));
    if (mode == "--ini-turbo") return TurboIniTest(dll, ini, log);
    // The harness lane: script (FC4Script.h, seam op 46).
    if (mode == "--ini-script") return ScriptIniTest(dll, ini, log);
    // The harness lane: close_dialogs (FC4Dialogs.h, seam op 47).
    const std::string dialogsPrefix = "--dialogs-child-";
    if (mode.compare(0, dialogsPrefix.size(), dialogsPrefix) == 0) {
        return DialogsChild(dll, ini, mode.substr(dialogsPrefix.size()));
    }
    if (mode == "--ini-dialogs") return DialogsIniTest(dll, ini, log);
    // The history warehouse (history.md): the replacement through the test hooks (seam op 54).
    if (mode == "--ini-enable-history") return HistoryEnableTest(dll, ini, log);
    // The history shadow (FC4HistoryShadow.h, seam op 54 op 1).
    if (mode == "--ini-history-shadow") return HistoryShadowIniTest(dll, ini, log);
    // The 24-hour clock (clock24.md): the replacement through the test hooks (seam op 54).
    if (mode == "--ini-enable-clock24") return Clock24EnableTest(dll, ini, log);
    // The 24-hour clock shadow (FC4Clock24Shadow.h, seam op 54 op 2).
    if (mode == "--ini-clock24-shadow") return Clock24ShadowIniTest(dll, ini, log);
    // The tutorial system (tutorial.md; the glue15 lane).
    if (mode == "--ini-enable-tutorial") return TutorialEnableTest(dll, ini, log);
    if (mode == "--ini-enable-disasters") return DisastersFactoryTest(dll, ini, true);
    if (mode == "--ini-disable-disasters") return DisastersFactoryTest(dll, ini, false);
    if (mode == "--ini-netocc-shadow-factories") return NetOccShadowRegistrationTest(dll, ini);
    if (mode == "--ini-netocc-line-factory") return NetOccLineRegistrationTest(dll, ini);
    if (mode == "--ini-netocc-pole-factory") return NetOccPoleRegistrationTest(dll, ini);
    if (mode == "--netlot-ownership") return NetLotOwnershipTest(dll, ini);
    if (mode == "--netlot-defaults") return NetLotOwnershipTest(dll, ini, 1);
    if (mode == "--cfgrules-default") return ConfigRulesRegistrationTest(dll, ini, 0);
    if (mode == "--cfgrules-enabled") return ConfigRulesRegistrationTest(dll, ini, 1);
    if (mode == "--cfgrules-disabled") return ConfigRulesRegistrationTest(dll, ini, 2);
    if (mode == "--netlot-disabled") return NetLotOwnershipTest(dll, ini, 2);
    if (mode == "--netlot-object-factories") return NetLotOwnershipTest(dll, ini, 3);
    if (mode == "--ini-enable-advisors") return AdvisorsRegistrationTest(dll, ini, log, true);
    if (mode == "--ini-default-advisors") return AdvisorsRegistrationTest(dll, ini, log, false);
    if (mode.rfind("--ini-netrules-", 0) == 0) return NetRulesRegistrationTest(dll, ini, log, mode);
    if (mode == "--ini-lotrec-boundary") return LotrecBoundaryTest(dll, ini, log, true);
    if (mode == "--ini-lotrec-default") return LotrecBoundaryTest(dll, ini, log, false);
    if (mode == "--ini-lotconf-ownership") return LotConfigurationOwnershipTest(dll, ini);

    if (mode == "--autotest") {
        // Autotest: the verdict line, then TerminateProcess(exit code 0).
        SetEnvironmentVariableA("FC4_DISABLE", nullptr);
        SetEnvironmentVariableA("FC4_AUTOTEST_SECONDS", nullptr);
        SetEnvironmentVariableA("FC4_AUTOTEST_UNPAUSE", nullptr);
        SetEnvironmentVariableA("FC4_AUTOTEST_END_DATE", nullptr);
        DWORD ms = 0;
        const std::string dllArg = "\"" + dll + "\" ";

        // 1. Clean: verdict 2 s after the first tick; the paused simulator is resumed at speed 1.
        CHECK(RunChild(dllArg + "--autotest-child-clean", 30000, &ms) == 0);
        std::printf("autotest clean child: %lu ms\n", static_cast<unsigned long>(ms));
        CHECK(ms >= 1500 && ms < 15000);
        std::string text = ReadFile(log);
        CHECK(text.find("autotest: ON - verdict 2 s after the first tick of a loaded city, then TerminateProcess; "
                        "unpause to speed 1") != std::string::npos);
        CHECK(text.find("autotest: city loaded (PostCityInit)") != std::string::npos);
        CHECK(text.find("autotest: clock started; verdict in 2 s") != std::string::npos);
        CHECK(text.find("autotest: unpause: 1 Resume call(s), speed 1") != std::string::npos);
        CHECK(text.find("paused=0 hidden=") != std::string::npos);
        // julian: our simulator publishes its own date (2000-01-01; it is not ticked here).
        // The run figures (the harness lane): no new day since the clock started.
        CHECK(NormalVerdict(text, false) ==
              "AUTOTEST VERDICT: clean (foreign_exceptions=0, watchdog_stalls=0, rtc=0, overruns=0, ref_warnings=0, "
              "julian=2451545, date=2000-01-01, real_seconds=S, days_per_second=0.00, switches_off=-, disabled=-)");
        CHECK(text.find("autotest: TerminateProcess(exit code 0) from the autotest thread") != std::string::npos);
        CHECK(text.find("end date") == std::string::npos);  // no autotest_end_date

        // 2. Corrupt: a first-chance access violation in the host; terminate ~5 s later.
        CHECK(RunChild(dllArg + "--autotest-child-corrupt", 30000, &ms) == 0);
        std::printf("autotest corrupt child: %lu ms\n", static_cast<unsigned long>(ms));
        CHECK(ms >= 4500 && ms < 20000);
        text = ReadFile(log);
        CHECK(text.find("first-chance exception OUTSIDE FreeCity4.dll in fc4_plugin_load_test.exe+0x") !=
              std::string::npos);
        CHECK(NormalVerdict(text, false) ==
              "AUTOTEST VERDICT: corrupt (foreign_exceptions=1, watchdog_stalls=0, rtc=0, overruns=0, ref_warnings=0, "
              "julian=2451600, date=2000-02-25, real_seconds=S, days_per_second=0.00, switches_off=-, disabled=-)");
        CHECK(text.find("autotest: terminating in 5000 ms (time for stall evidence)") != std::string::npos);
        CHECK(text.find("autotest: TerminateProcess(exit code 0) from the autotest thread") != std::string::npos);
        CHECK(CountOf(text, "AUTOTEST VERDICT:") == 1);

        // 3. Stalled (passthrough: our classes off, the observer alone): no tick for > 10 s.
        CHECK(RunChild(dllArg + "--autotest-child-stalled", 40000, &ms) == 0);
        std::printf("autotest stalled child: %lu ms\n", static_cast<unsigned long>(ms));
        CHECK(ms >= 9000 && ms < 30000);
        text = ReadFile(log);
        CHECK(text.find("observer: PostCityInit (passthrough)") != std::string::npos);
        CHECK(text.find("watchdog: started for main thread") != std::string::npos);
        CHECK(NormalVerdict(text, false) ==
              "AUTOTEST VERDICT: stalled (foreign_exceptions=0, watchdog_stalls=1, rtc=0, overruns=0, ref_warnings=0, "
              "julian=2451600, date=2000-02-25, real_seconds=S, days_per_second=0.00, switches_off=-, "
              "disabled=simulator,budget)");
        CHECK(text.find("autotest: TerminateProcess(exit code 0) from the watchdog thread") != std::string::npos);
        CHECK(text.find("for 4294967") == std::string::npos);  // no unsigned underflow in any duration

        // 3b. Ended (passthrough): the game shuts the city down 0.5 s into a 60 s
        // run; verdict "ended" at its PostCityShutdown, with the last day played.
        CHECK(RunChild(dllArg + "--autotest-child-ended", 30000, &ms) == 0);
        std::printf("autotest ended child: %lu ms\n", static_cast<unsigned long>(ms));
        CHECK(ms < 15000);
        text = ReadFile(log);
        CHECK(text.find("autotest: WARNING the game is shutting the city down by itself 0 s after the clock started, "
                        "before the 60 s deadline (julian 2451600)") != std::string::npos);
        CHECK(text.find("autotest: the game ended the city before the deadline (PostCityShutdown)") !=
              std::string::npos);
        CHECK(NormalVerdict(text, false) ==
              "AUTOTEST VERDICT: ended (foreign_exceptions=0, watchdog_stalls=0, rtc=0, overruns=0, ref_warnings=0, "
              "julian=2451600, date=2000-02-25, real_seconds=S, days_per_second=0.00, switches_off=-, "
              "disabled=simulator,budget)");
        CHECK(text.find("autotest: TerminateProcess(exit code 0) from the observer (city ended)") != std::string::npos);
        CHECK(CountOf(text, "AUTOTEST VERDICT:") == 1);

        // A Rust panic in the core: reported in the log (message, location,
        // backtrace, the last interface calls) before the abort.
        {
            const int code = RunChild(dllArg + "--autotest-child-panic", 30000, &ms);
            std::printf("panic child: exit code 0x%08X\n", static_cast<unsigned>(code));
            CHECK(code == static_cast<int>(0xC0000409) || code == 3);  // abort (release) or unwound
            text = ReadFile(log);
            CHECK(text.find("fail-fast reports on:") != std::string::npos);
            CHECK(text.find("Rust PANIC in FreeCity4 core: 'fc4_debug_panic: a deliberate panic (test of the panic "
                            "report)' at ") != std::string::npos);
            CHECK(text.find("panic_log.rs:") != std::string::npos);
            CHECK(text.find("Rust backtrace:") != std::string::npos);
            CHECK(text.find("FreeCity4 interface calls (oldest first):") != std::string::npos);
        }

        // 4. Save after 2 month-ends (no autotest_seconds): snapshot, SaveCity(full), verdict, terminate.
        CHECK(RunChild(dllArg + "--autotest-child-save", 30000, &ms) == 0);
        std::printf("autotest save child: %lu ms\n", static_cast<unsigned long>(ms));
        text = ReadFile(log);
        CHECK(text.find("autotest: save ON - after 2 month-end(s) of a loaded city: snapshot, SaveCity(full) in place, "
                        "verdict, TerminateProcess") != std::string::npos);
        CHECK(text.find("observe snapshot (after load): date=2000-01-01 julian=2451545 funds=- fundings=-") !=
              std::string::npos);
        CHECK(text.find("observe month 2000-02: date=2000-02-01 julian=2451576 funds=-") != std::string::npos);
        CHECK(text.find("observe month 2000-03: date=2000-03-01 julian=2451605 funds=-") != std::string::npos);
        CHECK(text.find("autotest: 2 month-end(s) since the city load; saving at the next tick") != std::string::npos);
        CHECK(text.find("observe snapshot (before save): date=2000-01-01 julian=2451545 funds=- fundings=-") !=
              std::string::npos);
        CHECK(text.find("autotest: save after 2 month(s): SaveCity(full) returned 1 in ") != std::string::npos);
        // No autotest clock (no autotest_seconds): no real seconds, no rate.
        CHECK(NormalVerdict(text, false) ==
              "AUTOTEST VERDICT: clean (foreign_exceptions=0, watchdog_stalls=0, rtc=0, overruns=0, ref_warnings=0, "
              "julian=2451605, date=2000-03-01, real_seconds=-, days_per_second=-, switches_off=-, disabled=-)");
        CHECK(text.find("autotest: TerminateProcess(exit code 0) from the observer (save autotest)") !=
              std::string::npos);
        CHECK(CountOf(text, "AUTOTEST VERDICT:") == 1);
        CHECK(text.find("config: WARNING unknown key") == std::string::npos);  // observe, autotest_save_* are known

        // 5. The harness lane: autotest_end_date = 2000-03-01 alone (no timeout):
        // one new day per tick from 2000-02-25; the verdict at 2000-03-01, on
        // that day, after 6 new days.
        CHECK(RunChild(dllArg + "--autotest-child-enddate", 30000, &ms) == 0);
        std::printf("autotest end-date child: %lu ms\n", static_cast<unsigned long>(ms));
        CHECK(ms < 15000);
        text = ReadFile(log);
        CHECK(text.find("autotest: ON - verdict at the first new day on or after 2000-03-01 (autotest_end_date; no "
                        "timeout: autotest_seconds = 0), then TerminateProcess; unpause off") != std::string::npos);
        CHECK(text.find("autotest: end date 2000-03-01 (julian 2451605): the run ends at the first new day on or "
                        "after it\r\n") != std::string::npos ||
              text.find("autotest: end date 2000-03-01 (julian 2451605): the run ends at the first new day on or "
                        "after it\n") != std::string::npos);
        CHECK(text.find("autotest: clock started; verdict at 2000-03-01 (autotest_end_date), no timeout") !=
              std::string::npos);
        CHECK(text.find("autotest: end date 2000-03-01 reached at the new day 2000-03-01 (julian 2451605), ") !=
              std::string::npos);
        CHECK(text.find("s after the clock started; verdict, then TerminateProcess") != std::string::npos);
        CHECK(text.find("autotest: end date 2000-03-01 reached (autotest_end_date)") != std::string::npos);
        CHECK(NormalVerdict(text, true) ==
              "AUTOTEST VERDICT: clean (foreign_exceptions=0, watchdog_stalls=0, rtc=0, overruns=0, ref_warnings=0, "
              "julian=2451605, date=2000-03-01, real_seconds=S, days_per_second=R, switches_off=-, "
              "disabled=simulator,budget)");
        {
            const double days = VerdictDays(text);
            std::printf("autotest end-date child: rate x seconds = %.3f days (6 expected)\n", days);
            CHECK(days > 5.8 && days < 6.2);
        }
        CHECK(text.find("autotest: TerminateProcess(exit code 0) from the observer (autotest_end_date)") !=
              std::string::npos);
        CHECK(CountOf(text, "AUTOTEST VERDICT:") == 1);
        CHECK(text.find("config: WARNING unknown key") == std::string::npos);  // autotest_end_date is known

        // 5b. The timeout comes first: autotest_seconds = 2 with an end date in 2100.
        CHECK(RunChild(dllArg + "--autotest-child-enddate-timeout", 30000, &ms) == 0);
        std::printf("autotest end-date timeout child: %lu ms\n", static_cast<unsigned long>(ms));
        CHECK(ms >= 1500 && ms < 15000);
        text = ReadFile(log);
        CHECK(text.find("autotest: ON - verdict 2 s after the first tick of a loaded city, then TerminateProcess; "
                        "unpause off") != std::string::npos);
        CHECK(text.find("autotest: end date 2100-01-01 (julian 2488070): the run ends at the first new day on or "
                        "after it, or at the autotest_seconds timeout, whichever comes first") != std::string::npos);
        CHECK(text.find("autotest: clock started; verdict in 2 s or at 2100-01-01 (autotest_end_date), whichever "
                        "comes first") != std::string::npos);
        CHECK(text.find("reached at the new day") == std::string::npos);
        CHECK(text.find("autotest: 2 s after the first tick of the loaded city") != std::string::npos);
        {
            const std::string v = NormalVerdict(text, true);
            const std::string head = "AUTOTEST VERDICT: clean (foreign_exceptions=0, watchdog_stalls=0, rtc=0, "
                                     "overruns=0, ref_warnings=0, julian=";
            CHECK(v.compare(0, head.size(), head) == 0);
            CHECK(v.find(", real_seconds=S, days_per_second=R, switches_off=-, disabled=simulator,budget)") !=
                  std::string::npos);
            // date= is the verdict's julian, and the days counted are the days played.
            const long j = std::atol(VerdictField(v, "julian").c_str());
            const fc4::harness::Date d = fc4::harness::CivilFromJulian(static_cast<int32_t>(j));
            CHECK(VerdictField(v, "date") == fc4::harness::FormatDate(d));
            const double days = VerdictDays(text);
            std::printf("autotest end-date timeout child: julian %ld, rate x seconds = %.3f days (%ld played)\n", j,
                        days, j - 2451600 + 1);
            CHECK(j > 2451620 && days > 0.95 * static_cast<double>(j - 2451600 + 1) - 1.0 &&
                  days < 1.05 * static_cast<double>(j - 2451600 + 1) + 1.0);
        }
        CHECK(CountOf(text, "AUTOTEST VERDICT:") == 1);

        // 5c. autotest_end = quit and an end date already past when the city
        // loads: the first new day ends the run; with no game to quit through
        // (tests have no framework), the verdict as at the end.
        CHECK(RunChild(dllArg + "--autotest-child-enddate-quit", 30000, &ms) == 0);
        std::printf("autotest end-date quit child: %lu ms\n", static_cast<unsigned long>(ms));
        CHECK(ms < 15000);
        text = ReadFile(log);
        CHECK(text.find("autotest: end date 2000-01-15 reached at the new day 2000-02-25 (julian 2451600), ") !=
              std::string::npos);
        CHECK(text.find("s after the clock started; quitting at the next tick (autotest_end = quit)") !=
              std::string::npos);
        CHECK(text.find("autotest: no cISC4App to quit through") != std::string::npos);
        CHECK(text.find("autotest: end date reached, but the quit could not be requested") != std::string::npos);
        CHECK(NormalVerdict(text, true) ==
              "AUTOTEST VERDICT: clean (foreign_exceptions=0, watchdog_stalls=0, rtc=0, overruns=0, ref_warnings=0, "
              "julian=2451600, date=2000-02-25, real_seconds=S, days_per_second=R, switches_off=-, "
              "disabled=simulator,budget)");
        {
            const double days = VerdictDays(text);
            std::printf("autotest end-date quit child: rate x seconds = %.3f days (1 expected)\n", days);
            CHECK(days > 0.9 && days < 1.1);
        }
        CHECK(text.find("autotest: TerminateProcess(exit code 0) from the observer (autotest_end = quit)") !=
              std::string::npos);
        CHECK(CountOf(text, "AUTOTEST VERDICT:") == 1);

        // 5e. A script save due on the end date waits for the observer's tick;
        // the verdict waits for it and comes at that tick, after the save.
        CHECK(RunChild(dllArg + "--autotest-child-enddate-save", 30000, &ms) == 0);
        std::printf("autotest end-date save child: %lu ms\n", static_cast<unsigned long>(ms));
        CHECK(ms < 15000);
        DeleteFileA((ini.substr(0, ini.find_last_of("\\/") + 1) + "fc4_script_endsave.txt").c_str());
        text = ReadFile(log);
        {
            const char* const order[] = {
                "script: line 1 2000-02-26 mark the day before -> marked",
                "script: line 2 2000-02-27 save: waits for the next framework tick (not run inside SimNewDay); the "
                "actions due after it wait behind it",
                "autotest: end date 2000-02-27 reached at the new day 2000-02-27 (julian 2451602), ",
                "s after the clock started; the script's waiting action(s) first, then the verdict and "
                "TerminateProcess at the next tick",
                "script: line 2 2000-02-27 save -> ok=1 (SaveCity(full) in ",
                "script: line 3 2000-02-27 mark after the save -> marked",
                "script: all 3 action(s) of '",
                "autotest: end date 2000-02-27 reached (autotest_end_date)",
                "AUTOTEST VERDICT:",
                "autotest: TerminateProcess(exit code 0) from the observer (autotest_end_date)",
            };
            size_t at = 0;
            for (const char* e : order) {
                const size_t found = text.find(e, at);
                if (found == std::string::npos) std::printf("end-date save: not found (in order): %s\n", e);
                CHECK(found != std::string::npos);
                if (found != std::string::npos) at = found;
            }
        }
        CHECK(text.find("s after the clock started; verdict, then TerminateProcess") == std::string::npos);
        CHECK(NormalVerdict(text, true) ==
              "AUTOTEST VERDICT: clean (foreign_exceptions=0, watchdog_stalls=0, rtc=0, overruns=0, ref_warnings=0, "
              "julian=2451602, date=2000-02-27, real_seconds=S, days_per_second=R, switches_off=-, "
              "disabled=simulator,budget)");
        {
            const double days = VerdictDays(text);
            std::printf("autotest end-date save child: rate x seconds = %.3f days (3 expected)\n", days);
            CHECK(days > 2.8 && days < 3.2);
        }
        CHECK(CountOf(text, "AUTOTEST VERDICT:") == 1);
        CHECK(text.find("config: WARNING unknown key") == std::string::npos);

        // 5d. Not a date: a WARNING, ignored; the 1 s timeout ends the run.
        CHECK(RunChild(dllArg + "--autotest-child-enddate-bad", 30000, &ms) == 0);
        text = ReadFile(log);
        CHECK(text.find("autotest: WARNING autotest_end_date='2000-02-30' is not a date YYYY-MM-DD; ignored") !=
              std::string::npos);
        CHECK(text.find("autotest: clock started; verdict in 1 s\r\n") != std::string::npos ||
              text.find("autotest: clock started; verdict in 1 s\n") != std::string::npos);
        CHECK(text.find("autotest: end date") == std::string::npos);
        CHECK(text.find("autotest: 1 s after the first tick of the loaded city") != std::string::npos);
        CHECK(CountOf(text, "AUTOTEST VERDICT: clean") == 1);

        // 5e. The harness lane: close_dialogs = 1 adds dialogs_closed= after
        // ref_warnings= (one dialog closed by button before the days run).
        CHECK(RunChild(dllArg + "--autotest-child-enddate-dialogs", 30000, &ms) == 0);
        text = ReadFile(log);
        CHECK(CountOf(text, "dialogs: closed 0x2A5CFB2C (window 0x") == 1);
        CHECK(NormalVerdict(text, true) ==
              "AUTOTEST VERDICT: clean (foreign_exceptions=0, watchdog_stalls=0, rtc=0, overruns=0, ref_warnings=0, "
              "dialogs_closed=1, julian=2451605, date=2000-03-01, real_seconds=S, days_per_second=R, switches_off=-, "
              "disabled=simulator,budget)");
        CHECK(CountOf(text, "AUTOTEST VERDICT:") == 1);
        DeleteFileA(ini.c_str());
        if (g_failures == 0) std::printf("plugin autotest test: all checks passed\n");
        return g_failures == 0 ? 0 : 1;
    }

    if (mode == "--ini-disabled") {
        // FreeCity4.ini `disable =` wins over FC4_DISABLE; switches are read at
        // game start; unknown keys are reported.
        SetEnvironmentVariableA("FC4_DISABLE", "simulator");
        WriteTextFile(ini, "; bisection test\n[subsystems]\ndisable = Budget   # the original budget\nlot_scan = off\n"
                       "bogus_key = 1\n");
        HMODULE hd = LoadLibraryA(dll.c_str());
        CHECK(hd != nullptr);
        if (hd == nullptr) return 1;
        auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
        cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
        CHECK(director != nullptr);
        if (director != nullptr) {
            std::vector<uint32_t> classes;
            director->EnumClassObjects(&CollectClass, &classes);
            CHECK(classes.size() == 1 && classes[0] == kCLSID_Simulator);
            void* obj = nullptr;
            CHECK(!director->GetClassObject(kCLSID_Budget, kIID_cISC4BudgetSimulator, &obj));
        }
        FreeLibrary(hd);
        DeleteFileA(ini.c_str());
        const std::string text = ReadFile(log);
        CHECK(text.find("subsystem 'budget': DISABLED by FreeCity4.ini (disable=)") != std::string::npos);
        CHECK(text.find("subsystem 'simulator': enabled (1 class replaced)") != std::string::npos);
        CHECK(text.find("config (game start): ") != std::string::npos);
        CHECK(text.find("FreeCity4.ini found; budget_default=1; set by ini: lot_scan=0; set by env: -") !=
              std::string::npos);
        CHECK(text.find("config (game start): budget switches OFF (1 of 219): lot_scan") != std::string::npos);
        CHECK(text.find("config: WARNING unknown key 'bogus_key' in FreeCity4.ini (ignored)") != std::string::npos);
        if (g_failures == 0) std::printf("plugin ini disable test: all checks passed\n");
        return g_failures == 0 ? 0 : 1;
    }

    if (mode == "--ini-enable" || mode == "--ini-enable-disabled") {
        // The land value subsystem is off by default; `enable = landvalue`
        // registers it (and never the game's grid classes); `disable` wins.
        const bool alsoDisabled = mode == "--ini-enable-disabled";
        SetEnvironmentVariableA("FC4_DISABLE", nullptr);
        SetEnvironmentVariableA("FC4_ENABLE", nullptr);
        WriteTextFile(ini, alsoDisabled ? "[subsystems]\nenable = LandValue\ndisable = landvalue\n"
                                        : "[subsystems]\nenable = LandValue\n");
        HMODULE hd = LoadLibraryA(dll.c_str());
        CHECK(hd != nullptr);
        if (hd == nullptr) return 1;
        auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
        cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
        CHECK(director != nullptr);
        if (director != nullptr) {
            std::vector<uint32_t> classes;
            director->EnumClassObjects(&CollectClass, &classes);
            for (uint32_t c : classes) CHECK(c < 0x49B9E600 || c > 0x49B9E60B);  // simgrid.md §6.3
            void* obj = nullptr;
            if (alsoDisabled) {
                CHECK(classes.size() == 4);
                CHECK(!director->GetClassObject(kCLSID_LandValue, kIID_cISC4LandValueSimulator, &obj));
            } else {
                CHECK(classes.size() == 5 && classes[4] == kCLSID_LandValue);
                CHECK(director->GetClassObject(kCLSID_LandValue, kIID_cISC4LandValueSimulator, &obj));
                auto* lv = static_cast<cISC4LandValueSimulator*>(obj);
                if (lv != nullptr) {
                    // §1: exactly four interfaces, one reference count.
                    void* p = nullptr;
                    CHECK(lv->QueryInterface(0x452294AA, &p) && p != nullptr);
                    if (p) static_cast<cIGZUnknown*>(p)->Release();
                    cIGZSerializable* ser = nullptr;
                    CHECK(lv->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) && ser);
                    if (ser) {
                        CHECK(ser->GetGZCLSID() == kCLSID_LandValue);
                        ser->Release();
                    }
                    CHECK(lv->QueryInterface(GZIID_cIGZUnknown, &p) && p == static_cast<cIGZUnknown*>(lv));
                    if (p) static_cast<cIGZUnknown*>(p)->Release();
                    CHECK(!lv->QueryInterface(0xC9BD939A, &p));
                    CHECK(!lv->QueryInterface(0x6772BD23, &p));
                    // Before Init: no grids, zero values.
                    CHECK(lv->GetIntrinsicLandValueMap() == nullptr && lv->GetTotalLandValueMap() == nullptr &&
                          lv->GetSlopeMap() == nullptr);
                    CHECK(lv->GetLandValue(3, 4) == 0);
                    CHECK(lv->GetAverageLandValue() == 0);
                    CHECK(lv->GetModeAltitude() == 0.0f);
                    // Init without a city or GZCOM: no grids, but a clean lifecycle.
                    CHECK(lv->Init());
                    CHECK(lv->Init());
                    CHECK(lv->GetLandValueType(0, 0) == cISC4LandValueSimulator::LandValueType::Low);
                    CHECK(lv->AddRef() == 2);
                    CHECK(lv->Release() == 1);
                    CHECK(lv->Release() == 0);  // §4.3: Shutdown, destroy
                }
            }
        }
        FreeLibrary(hd);
        DeleteFileA(ini.c_str());
        const std::string text = ReadFile(log);
        if (alsoDisabled) {
            CHECK(text.find("subsystem 'landvalue': DISABLED by FreeCity4.ini (disable=)") != std::string::npos);
            CHECK(text.find("cFC4LandValueSimulator(") == std::string::npos);
        } else {
            CHECK(text.find("subsystem 'landvalue': enabled (1 class replaced) by FreeCity4.ini (enable=)") !=
                  std::string::npos);
            CHECK(text.find("landvalue Init: ok=1") != std::string::npos);
            CHECK(text.find("landvalue Shutdown (0 passes)") != std::string::npos);
            CHECK(text.find("cFC4LandValueSimulator destroyed") != std::string::npos);
        }
        CHECK(text.find("subsystem 'budget': enabled (3 classes replaced)") != std::string::npos);
        CHECK(text.find("config: WARNING unknown key") == std::string::npos);
        if (g_failures == 0) std::printf("plugin enable test (%s): all checks passed\n", mode.c_str());
        return g_failures == 0 ? 0 : 1;
    }

    if (mode == "--ini-enable-weather") {
        // The weather subsystem is off by default; `enable = weather` registers it.
        SetEnvironmentVariableA("FC4_DISABLE", nullptr);
        SetEnvironmentVariableA("FC4_ENABLE", nullptr);
        WriteTextFile(ini, "[subsystems]\nenable = weather\n");
        HMODULE hd = LoadLibraryA(dll.c_str());
        CHECK(hd != nullptr);
        if (hd == nullptr) return 1;
        auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
        cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
        CHECK(director != nullptr);
        if (director != nullptr) {
            std::vector<uint32_t> classes;
            director->EnumClassObjects(&CollectClass, &classes);
            CHECK(classes.size() == 5 && std::count(classes.begin(), classes.end(), kCLSID_Weather) == 1);
            void* obj = nullptr;
            CHECK(director->GetClassObject(kCLSID_Weather, kIID_cISC4WeatherSimulator, &obj));
            auto* w = static_cast<cFC4ISC4WeatherSimulator*>(obj);
            if (w != nullptr) {
                // weather §1: five interfaces.
                void* p = nullptr;
                for (uint32_t iid : {0x452294AAu, 0xE98B2F57u}) {
                    p = nullptr;
                    CHECK(w->QueryInterface(iid, &p) && p != nullptr);
                    if (p) static_cast<cIGZUnknown*>(p)->Release();
                }
                cIGZSerializable* ser = nullptr;
                CHECK(w->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) && ser);
                if (ser) {
                    CHECK(ser->GetGZCLSID() == kCLSID_Weather);
                    ser->Release();
                }
                CHECK(w->QueryInterface(GZIID_cIGZUnknown, &p) && p == static_cast<cIGZUnknown*>(w));
                if (p) static_cast<cIGZUnknown*>(p)->Release();
                CHECK(!w->QueryInterface(0x6772BD23, &p));
                // Constructor values (§3): no grid, the code-default humidity.
                CHECK(w->CellCountX() == 0 && w->GetMoistureValue(10.0f, 10.0f) == 0);
                CHECK(w->GetHumidity(0.0f, 0.0f) == 0.75f);
                // Init without a city or GZCOM: an empty city, a clean lifecycle.
                CHECK(w->Init());
                CHECK(w->Init());
                CHECK(w->CellWidthX() == 16.0f && w->CellCountZ() == 0);
                CHECK(w->GetCurrentSeasonMonth() == 1 && w->GetCurrentSeason() == 0);
                float x = 0, z = 0;
                w->CellToPosition(2, 3, x, z);
                CHECK(x == 32.0f && z == 48.0f);
                {
                    // §4 (corrected 2026-10-14): slot 11 returns nothing; called the
                    // game's way it pops 16 and leaves the x87 stack as it was.
                    float gx = 0, gz = 0;
                    const fc4test::SlotCall r = fc4test::CallSlot(
                        w, 11, {2, 3, reinterpret_cast<uint32_t>(&gx), reinterpret_cast<uint32_t>(&gz)});
                    CHECK(r.espDelta == 0 && r.x87Pushed == 0 && gx == 32.0f && gz == 48.0f);
                }
                int32_t a = 0, b = 0;
                w->CellToStandardCityCell(3, 7, a, b);
                CHECK(a == 3 && b == 3);  // [quirk]
                CHECK(w->StandardCityCellToCell(5, 6, a, b) == 6 && a == 5 && b == 6);
                cS3DVector2 wind(0.0f, 0.0f);
                CHECK(w->GetAmbientWind(wind) == 3.0f && wind.fX == 3.0f && wind.fY == 0.0f);
                FC4ByteGridVector* grid = nullptr;
                w->GetMoistureValueMapPtr(grid);
                CHECK(grid != nullptr);
                if (grid != nullptr) {
                    void* const* v = reinterpret_cast<void* const*>(grid);
                    CHECK(v[0] == v[1]);  // no rows in an empty city
                }
                CHECK(w->Release() == 0);  // Shutdown, destroy
            }
        }
        FreeLibrary(hd);
        DeleteFileA(ini.c_str());
        const std::string text = ReadFile(log);
        CHECK(text.find("subsystem 'weather': enabled (1 class replaced) by FreeCity4.ini (enable=)") !=
              std::string::npos);
        CHECK(text.find("subsystem 'landvalue': off by default") != std::string::npos);
        CHECK(text.find("weather Init: ok=1") != std::string::npos);
        CHECK(text.find("weather Shutdown") != std::string::npos);
        CHECK(text.find("cFC4WeatherSimulator destroyed") != std::string::npos);
        CHECK(text.find("config: WARNING unknown key") == std::string::npos);
        if (g_failures == 0) std::printf("plugin enable test (%s): all checks passed\n", mode.c_str());
        return g_failures == 0 ? 0 : 1;
    }

    if (mode == "--ini-enable-ordinance") {
        // The ordinance subsystem is off by default; `enable = ordinance`
        // registers the simulator and the 21 ordinance classes.
        SetEnvironmentVariableA("FC4_DISABLE", nullptr);
        SetEnvironmentVariableA("FC4_ENABLE", nullptr);
        WriteTextFile(ini, "[subsystems]\nenable = ordinance\n");
        HMODULE hd = LoadLibraryA(dll.c_str());
        CHECK(hd != nullptr);
        if (hd == nullptr) return 1;
        auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
        cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
        CHECK(director != nullptr);
        if (director != nullptr) {
            std::vector<uint32_t> classes;
            director->EnumClassObjects(&CollectClass, &classes);
            CHECK(classes.size() == 26);
            CHECK(std::count(classes.begin(), classes.end(), kCLSID_OrdinanceSim) == 1);
            CHECK(std::count(classes.begin(), classes.end(), kCLSID_FreeClinics) == 1);
            CHECK(std::count(classes.begin(), classes.end(), 0x8BD4FC74u) == 1);
            void* obj = nullptr;
            CHECK(director->GetClassObject(kCLSID_OrdinanceSim, kIID_cISC4OrdinanceSimulator, &obj));
            auto* sim = static_cast<cISC4OrdinanceSimulator*>(obj);
            if (sim != nullptr) {
                // ordinance §1: four interfaces.
                void* p = nullptr;
                CHECK(sim->QueryInterface(0x452294AAu, &p) && p != nullptr);
                auto* target = static_cast<cIGZMessageTarget2*>(p);
                cIGZSerializable* ser = nullptr;
                CHECK(sim->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) && ser);
                if (ser) {
                    CHECK(ser->GetGZCLSID() == kCLSID_OrdinanceSim);
                    ser->Release();
                }
                void* u = nullptr;
                CHECK(sim->QueryInterface(GZIID_cIGZUnknown, &u) && u == static_cast<cIGZUnknown*>(sim));
                if (u) static_cast<cIGZUnknown*>(u)->Release();
                CHECK(!sim->QueryInterface(GZIID_cISC4Ordinance, &u));
                // Init without a city or GZCOM: no exemplars, so no standard ordinances.
                CHECK(sim->Init());
                CHECK(sim->Init());
                uint32_t n = 0;
                CHECK(sim->GetOrdinanceIDArray(nullptr, n) == 0 && n == 0);

                // The shadow must handle an empty record and reject null input.
                auto ordinanceShadowSeam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
                CHECK(ordinanceShadowSeam != nullptr);
                if (ordinanceShadowSeam) {
                    CHECK(ordinanceShadowSeam(kFC4SeamOrdinanceShadow, nullptr, nullptr) == -1);
                    CHECK(ordinanceShadowSeam(kFC4SeamOrdinanceShadow, sim, nullptr) == 0);
                }

                // One of our ordinances, created through GZCOM (as the class store does).
                void* o = nullptr;
                CHECK(director->GetClassObject(kCLSID_FreeClinics, GZIID_cISC4Ordinance, &o));
                cFC4ISC4Ordinance* ord = fc4::AsGame(static_cast<cISC4Ordinance*>(o));
                if (ord != nullptr) {
                    cIGZSerializable* os = nullptr;
                    CHECK(ord->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&os)) && os);
                    if (os) {
                        CHECK(os->GetGZCLSID() == kCLSID_FreeClinics);
                        os->Release();
                    }
                    CHECK(!ord->QueryInterface(0x8BD4FC74u, &u));  // [choice]
                    // No exemplar: the zero ordinance (§3.1).
                    CHECK(ord->GetID() == 0 && !ord->IsAvailable() && !ord->IsOn());
                    CHECK(ord->GetChanceAvailability() == 0 && ord->GetEnactmentIncome() == 0);
                    CHECK(ord->GetName() != nullptr && ord->GetName()->Strlen() == 0);
                    CHECK(ord->GetMiscProperties() == nullptr);
                    CHECK(sim->AddOrdinance(*fc4::AsSdk(ord)));
                    CHECK(sim->GetOrdinanceIDArray(nullptr, n) == 1);
                    CHECK(sim->GetOrdinanceByID(0) == fc4::AsSdk(ord));
                    CHECK(ord->AddRef() == 3);  // ours, the map's, this one
                    CHECK(ord->Release() == 2);
                    if (ordinanceShadowSeam) {
                        // Capturing/stepping copies cannot alter the live flags,
                        // adjusted money, pending balance or reference count.
                        CHECK(ord->ForceAvailable(true));
                        CHECK(ord->ForceOn(true));
                        CHECK(ord->ForceMonthlyAdjustedIncome(-37));
                        CHECK(ordinanceShadowSeam(kFC4SeamOrdinanceShadow, sim, nullptr) == 1);
                        CHECK(ord->IsAvailable() && ord->IsOn() && ord->GetMonthlyAdjustedIncome() == -37);
                        CHECK(ord->AddRef() == 3 && ord->Release() == 2);
                        CHECK(ord->ForceAvailable(false));
                        CHECK(ord->ForceOn(false));
                        CHECK(ord->ForceMonthlyAdjustedIncome(0));
                    }
                }

                // A foreign (DLL plugin) ordinance, available when added (§4.2).
                FakeOrdinance fake;
                CHECK(sim->AddOrdinance(*fc4::AsSdk(&fake)));
                CHECK(fake.refs == 1);
                CHECK(sim->IsOrdinanceAvailable(0x12345678) && sim->IsOrdinanceAvailableButOff(0x12345678));
                CHECK(sim->SetOrdinanceOn(0x12345678, true) && fake.on && sim->IsOrdinanceOn(0x12345678));
                CHECK(sim->GetOrdinanceOnCount() == 1 && sim->GetOrdinanceAvailableCount() == 1);
                uint32_t ids[4] = {};
                n = 4;
                CHECK(sim->GetOrdinanceIDArray(ids, n) == 2 && n == 2 && ids[0] == 0 && ids[1] == 0x12345678);
                const cRZBaseString name("FAKE ordinance");
                CHECK(sim->GetOrdinanceByName(name) == fc4::AsSdk(&fake));
                CHECK(sim->GetOrdinanceIDByName(name) == 0x12345678);
                CHECK(sim->GetCumulativeFactor(0x28ED0380, 0) == 1.0f && sim->GetCumulativeOffset(0xAA5B8407, 0) == 0);
                // Month end (§5): the enactment (-10) and the fake's -7.
                if (target != nullptr) {
                    TypeMessage month(0x66956816);
                    CHECK(target->DoMessage(&month));
                    target->Release();
                }
                CHECK(sim->GetOrdinanceMonthlyExpense() == 17 && sim->GetOrdinanceMonthlyIncome() == 0);
                CHECK(sim->RemoveOrdinance(*fc4::AsSdk(&fake)));
                CHECK(fake.refs == 0 && !fake.on && !fake.available);
                CHECK(sim->Release() == 0);  // Shutdown, destroy: the map lets go
                if (ord != nullptr) CHECK(ord->Release() == 0);
            }
        }
        FreeLibrary(hd);
        DeleteFileA(ini.c_str());
        const std::string text = ReadFile(log);
        CHECK(text.find("subsystem 'ordinance': enabled (22 classes replaced) by FreeCity4.ini (enable=)") !=
              std::string::npos);
        CHECK(text.find("subsystem 'weather': off by default") != std::string::npos);
        CHECK(text.find("ordinance Init: ok=1") != std::string::npos);
        CHECK(text.find("ordinance month: 2 ordinances (1 available, 1 on)") != std::string::npos);
        CHECK(text.find("ordinance Shutdown") != std::string::npos);
        CHECK(text.find("cFC4OrdinanceSimulator destroyed") != std::string::npos);
        CHECK(text.find("config: WARNING unknown key") == std::string::npos);
        if (g_failures == 0) std::printf("plugin enable test (%s): all checks passed\n", mode.c_str());
        return g_failures == 0 ? 0 : 1;
    }

    if (mode == "--ini-enable-demand") {
        // The demand subsystem is off by default; `enable = demand` registers
        // the simulator and the node class.
        SetEnvironmentVariableA("FC4_DISABLE", nullptr);
        SetEnvironmentVariableA("FC4_ENABLE", nullptr);
        WriteTextFile(ini, "[subsystems]\nenable = demand\n");
        HMODULE hd = LoadLibraryA(dll.c_str());
        CHECK(hd != nullptr);
        if (hd == nullptr) return 1;
        auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
        cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
        CHECK(director != nullptr);
        if (director != nullptr) {
            std::vector<uint32_t> classes;
            director->EnumClassObjects(&CollectClass, &classes);
            CHECK(classes.size() == 6);
            CHECK(std::count(classes.begin(), classes.end(), kCLSID_DemandSim) == 1);
            CHECK(std::count(classes.begin(), classes.end(), kCLSID_Demand) == 1);
            void* obj = nullptr;
            CHECK(director->GetClassObject(kCLSID_DemandSim, kIID_cISC4DemandSimulator, &obj));
            auto* sim = static_cast<cFC4ISC4DemandSimulator*>(obj);
            if (sim != nullptr) {
                // demand §1: four interfaces (not the fourth message-target IID [choice]).
                void* p = nullptr;
                CHECK(sim->QueryInterface(0x452294AAu, &p) && p != nullptr);
                auto* target = static_cast<cIGZMessageTarget2*>(p);
                cIGZSerializable* ser = nullptr;
                CHECK(sim->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) && ser);
                if (ser) {
                    CHECK(ser->GetGZCLSID() == kCLSID_DemandSim);
                    ser->Release();
                }
                void* u = nullptr;
                CHECK(sim->QueryInterface(GZIID_cIGZUnknown, &u) && u == static_cast<cIGZUnknown*>(sim));
                if (u) static_cast<cIGZUnknown*>(u)->Release();
                CHECK(!sim->QueryInterface(0x452294ABu, &u));
                CHECK(!sim->QueryInterface(kIID_cISC4Demand, &u));
                CHECK(sim->GetSimulatorType() == 0x279681D7u);
                // Init without a city or GZCOM: no demand exemplars, so no nodes.
                CHECK(sim->Init());
                CHECK(sim->Init());
                CHECK(sim->GetDemand(0x1010, 0x20000) == nullptr);
                CHECK(sim->GetJobsBySensus(0x3121) == 0);
                CHECK(sim->GetNeutralTaxRate() == 0.0f);
                // §5 (corrected 2026-10-14): four floats, jobs[1..3] zeroed (no
                // exemplar), jobs[0] never touched, nothing returned.
                float jobs[4] = {7.0f, 1.0f, 1.0f, 1.0f};
                sim->CalculateJobsPerUnitOfDemand(jobs, 0x3121, 0);
                CHECK(jobs[0] == 7.0f && jobs[1] == 0.0f && jobs[2] == 0.0f && jobs[3] == 0.0f);
                {
                    float g[4] = {7.0f, 1.0f, 1.0f, 1.0f};
                    const fc4test::SlotCall r =
                        fc4test::CallSlot(sim, 8, {reinterpret_cast<uint32_t>(g), 0x3121u, 0u});
                    CHECK(r.espDelta == 0 && r.x87Pushed == 0);
                    CHECK(g[0] == 7.0f && g[1] == 0.0f && g[2] == 0.0f && g[3] == 0.0f);
                }
                SC4Percentage full{1.0f}, half{0.5f};
                sim->UpdateOccupantEffects(0xBEEF, full, half);  // no effects: nothing happens
                // Not the identified executable: the game's map functions cannot be
                // called, so the caller's map is left untouched (logged once).
                std::map<uint32_t, int32_t> summary;
                sim->GetLocalPopulationSummary(summary);
                CHECK(summary.empty());

                // The building profile's capacity maps (seam op 20: the read the
                // census and §7.2 make through the host's building_profile).
                auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
                CHECK(seam != nullptr);
                if (seam != nullptr) {
                    FakeCapacityProfile prof;
                    std::vector<std::array<uint32_t, 3>> got;
                    FC4DemandCapacityTestInput in{prof.bytes, 0xBEEF, &CollectCapacity, &got};
                    CHECK(seam(kFC4SeamDemandCapacity, &in, nullptr) == 3);
                    CHECK(got.size() == 3);
                    if (got.size() == 3) {
                        CHECK((got[0] == std::array<uint32_t, 3>{0, 0x1010, 12}));
                        CHECK((got[1] == std::array<uint32_t, 3>{0, 0x1020, 7}));
                        CHECK((got[2] == std::array<uint32_t, 3>{0, 0x1030, 3}));
                    }
                    // No capacity property: no maps, nothing reported.
                    got.clear();
                    prof.SetVector(nullptr, nullptr, nullptr);
                    CHECK(seam(kFC4SeamDemandCapacity, &in, nullptr) == 0 && got.empty());
                    // A wrong count: refused as a whole.
                    prof.SetVector(&prof.map, &prof.map + 1, &prof.map + 1);
                    prof.map.count = 4;
                    CHECK(seam(kFC4SeamDemandCapacity, &in, nullptr) == -1 && got.empty());
                    // A cycle (the largest node's right link back at the root).
                    prof.map.count = 3;
                    prof.c.right = &prof.r;
                    CHECK(seam(kFC4SeamDemandCapacity, &in, nullptr) == -1 && got.empty());
                    prof.c.right = nullptr;
                    // A vector that points at unreadable memory: the fault is caught.
                    prof.SetVector(reinterpret_cast<void*>(0x10), reinterpret_cast<void*>(0x1C),
                                   reinterpret_cast<void*>(0x1C));
                    CHECK(seam(kFC4SeamDemandCapacity, &in, nullptr) == -1 && got.empty());
                    CHECK(seam(kFC4SeamDemandCapacity, nullptr, nullptr) == -1);
                }

                // A node, created through GZCOM (as the class store does).
                void* o = nullptr;
                CHECK(director->GetClassObject(kCLSID_Demand, kIID_cISC4Demand, &o));
                cFC4ISC4Demand* node = fc4::AsGame(static_cast<cISC4Demand*>(o));
                if (node != nullptr) {
                    cIGZSerializable* ns = nullptr;
                    CHECK(node->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ns)) && ns);
                    if (ns) {
                        CHECK(ns->GetGZCLSID() == kCLSID_Demand);
                        ns->Release();
                    }
                    CHECK(node->Init());
                    // §3.1 defaults: activeMax = +8.507059e37 (0x7E7FFFFF), min its negative.
                    const float dmax = node->QueryActiveDemandMax();
                    uint32_t bits = 0;
                    std::memcpy(&bits, &dmax, sizeof(bits));
                    CHECK(bits == 0x7E7FFFFFu && node->QueryActiveDemandMin() == -dmax);
                    CHECK(node->SetId(0x1010) && node->GetId() == 0x1010);
                    CHECK(node->SetRegionUse(1) && node->GetRegionUse() == 1);
                    // §16 test 2: min/max ±6000, demand 1000, supply 400 + 10 pending.
                    CHECK(node->SetActiveDemandMin(-6000.0f) && node->SetActiveDemandMax(6000.0f));
                    CHECK(node->SetDemandValue(1000.0f) && node->SetSupplyValue(400.0f));
                    {
                        // §3.2 (corrected 2026-10-14): slots 16/17 return true in AL and
                        // leave the x87 stack as they found it (pop 4).
                        const float ten = 10.0f, zero = 0.0f;
                        uint32_t tenBits = 0, zeroBits = 0;
                        std::memcpy(&tenBits, &ten, sizeof(tenBits));
                        std::memcpy(&zeroBits, &zero, sizeof(zeroBits));
                        const fc4test::SlotCall s16 = fc4test::CallSlot(node, 16, {tenBits});
                        CHECK(s16.espDelta == 0 && s16.x87Pushed == 0 && (s16.eax & 0xFF) == 1);
                        const fc4test::SlotCall s17 = fc4test::CallSlot(node, 17, {zeroBits});
                        CHECK(s17.espDelta == 0 && s17.x87Pushed == 0 && (s17.eax & 0xFF) == 1);
                    }
                    CHECK(node->QuerySupplyValue() == 400.0f && node->QueryNewSupply() == 10.0f);
                    CHECK(node->EndOfCycle());
                    CHECK(!node->EndOfCycle());
                    CHECK(node->QuerySupplyValue() == 410.0f && node->QueryActiveDemandValue() == 590.0f);
                    CHECK(node->SetEconomyModifier(0.03f) && node->QueryEconomyModifier() == 0.03f);
                    CHECK(!node->EndOfCycle() && std::fabs(node->QueryActiveDemandValue() - 620.0f) < 0.01f);
                    const SC4Percentage cap{0.95f};
                    CHECK(node->SetDemandCap(cap) && node->GetDemandCap() != nullptr &&
                          node->GetDemandCap()->percentage == 0.95f);
                    node->EndOfCycle();
                    CHECK(std::fabs(node->QueryActiveDemandValue() - 300.0f) < 0.01f);
                    // §16 test 3: the minimum clamps to the maximum.
                    CHECK(node->SetActiveDemandMin(7000.0f) && node->QueryActiveDemandMin() == 6000.0f);
                    CHECK(!node->DebugIsValueLocked());
                    CHECK(node->Shutdown());
                    CHECK(node->Release() == 0);
                }
                // A month (§10.2): relief and export with no city; logs the state.
                if (target != nullptr) {
                    TypeMessage month(0x66956816);
                    CHECK(target->DoMessage(&month));
                    target->Release();
                }
                CHECK(sim->Release() == 0);  // Shutdown, destroy
            }
        }
        FreeLibrary(hd);
        DeleteFileA(ini.c_str());
        const std::string text = ReadFile(log);
        CHECK(text.find("subsystem 'demand': enabled (2 classes replaced) by FreeCity4.ini (enable=)") !=
              std::string::npos);
        CHECK(text.find("subsystem 'ordinance': off by default") != std::string::npos);
        CHECK(text.find("demand Init: ok=1") != std::string::npos);
        CHECK(text.find("demand month: 0 nodes") != std::string::npos);
        // The month line: what the glue moved (the seam's reads have no simulator, so they are not counted).
        CHECK(text.find("export cities; capacity: reads=0 entries=0 units=0 (occupancy: reads=0 units=0) "
                        "refused=0; summary: calls=1 copied=0 untouched=1 failed=0") != std::string::npos);
        CHECK(text.find("demand: GetLocalPopulationSummary: 0 entries not copied (the executable is not "
                        "identified") != std::string::npos);
        CHECK(text.find("compat: direct call map operator[] NOT available (executable not identified)") !=
              std::string::npos);
        CHECK(text.find("demand: capacity map read: building type 0x0000BEEF, 1 map(s), 3 entries: 0x1010=12, "
                        "0x1020=7, 0x1030=3") != std::string::npos);
        CHECK(text.find("demand: WARNING building type 0x0000BEEF: capacity map 0 of 1 refused (node count "
                        "differs from the map's count); no capacity reported") != std::string::npos);
        CHECK(text.find("demand: WARNING building type 0x0000BEEF: capacity map 0 of 1 refused (inconsistent "
                        "link)") != std::string::npos);
        CHECK(text.find("STRUCTURED EXCEPTION caught in demand capacity map read") != std::string::npos);
        CHECK(text.find("demand: WARNING building type 0x0000BEEF: reading its capacity maps faulted") !=
              std::string::npos);
        CHECK(text.find("demand Shutdown") != std::string::npos);
        CHECK(text.find("cFC4DemandSimulator destroyed") != std::string::npos);
        CHECK(text.find("config: WARNING unknown key") == std::string::npos);
        if (g_failures == 0) std::printf("plugin enable test (%s): all checks passed\n", mode.c_str());
        return g_failures == 0 ? 0 : 1;
    }

    if (mode == "--ini-enable-plumbing") {
        // The plumbing subsystem is off by default; `enable = plumbing` registers
        // the water simulator (plumbing.md §1).
        SetEnvironmentVariableA("FC4_DISABLE", nullptr);
        SetEnvironmentVariableA("FC4_ENABLE", nullptr);
        WriteTextFile(ini, "[subsystems]\nenable = plumbing\n[tools]\nplumbing_shadow = 1\npower_shadow = 1\n");
        HMODULE hd = LoadLibraryA(dll.c_str());
        CHECK(hd != nullptr);
        if (hd == nullptr) return 1;
        auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
        cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
        CHECK(director != nullptr);
        if (director != nullptr) {
            std::vector<uint32_t> classes;
            director->EnumClassObjects(&CollectClass, &classes);
            // simulator (1) + budget (3) + plumbing (1); never the game's grid
            // classes (simgrid.md §6.3).
            CHECK(classes.size() == 5);
            for (uint32_t c : classes) CHECK(c < 0x49B9E600 || c > 0x49B9E60B);
            CHECK(std::count(classes.begin(), classes.end(), kCLSID_Plumbing) == 1);
            void* obj = nullptr;
            CHECK(director->GetClassObject(kCLSID_Plumbing, kIID_cISC4PlumbingSimulator, &obj));
            auto* pl = static_cast<cFC4ISC4PlumbingSimulator*>(obj);
            if (pl != nullptr) {
                // §1: exactly four IIDs, one reference count, and the out pointer
                // is left untouched for any other IID.
                void* p = nullptr;
                CHECK(pl->QueryInterface(0x452294AAu, &p) && p != nullptr);
                if (p) static_cast<cIGZUnknown*>(p)->Release();
                cIGZSerializable* ser = nullptr;
                CHECK(pl->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) && ser);
                if (ser) {
                    CHECK(ser->GetGZCLSID() == kCLSID_Plumbing);
                    ser->Release();
                }
                // cIGZUnknown is answered with the primary interface (§1).
                void* u = nullptr;
                CHECK(pl->QueryInterface(GZIID_cIGZUnknown, &u) && u == static_cast<cIGZUnknown*>(pl));
                if (u) static_cast<cIGZUnknown*>(u)->Release();
                void* untouched = reinterpret_cast<void*>(0xDEADBEEF);
                CHECK(!pl->QueryInterface(0x452294ABu, &untouched));
                CHECK(untouched == reinterpret_cast<void*>(0xDEADBEEF));
                CHECK(!pl->QueryInterface(0xE98B2F57u, &untouched));  // cIGZMessageTarget: not answered
                // Constructor values (§3): idle state 9, no city, no grids, so
                // every query reads the neutral value and nothing crashes.
                CHECK(pl->GetNumWaterSources() == 0);
                CHECK(pl->GetNumPollutedWaterSources() == 0);
                CHECK(pl->GetWaterProductionCapacity() == 0);
                CHECK(pl->GetWaterConsumed() == 0);
                CHECK(pl->GetWaterImported() == 0);
                CHECK(pl->GetWaterExported() == 0);
                CHECK(pl->GetWaterDemand() == 0);
                CHECK(pl->GetNumPipes() == 0 && pl->GetNumDistressedPipes() == 0 && pl->GetNumBurstPipes() == 0);
                CHECK(pl->GetNumWateredBuildings() == 0 && pl->GetNumUnwateredBuildings() == 0);
                CHECK(pl->GetWateredGrid() == nullptr);
                CHECK(pl->GetSupplyValue(3, 4) == 0);
                CHECK(!pl->IsCellWatered(3, 4));
                CHECK(!pl->AreWateredBuildingsInCell(3, 4));
                CHECK(pl->GetWaterProduction(nullptr) == 0);
                SC4Percentage pct{0.5f};
                CHECK(!pl->GetWaterProducerConditionPercent(nullptr, pct));
                CHECK(pct.percentage == 0.5f);  // untouched
                int64_t cost = 7;
                CHECK(!pl->GetOccupantMonthlyUpkeepCost(nullptr, cost));
                CHECK(cost == 7);  // untouched
                // Slot 24: a null producer is not a producer record, so AL = 0
                // and no output is written (§10.1).
                uint32_t curCap = 99, maxCap = 99, usedCap = 99;
                SC4Percentage eff{0.5f};
                bool polluted = true;
                CHECK(!pl->GetWaterProducerInfo(nullptr, &curCap, &maxCap, &eff, &polluted, &usedCap));
                CHECK(curCap == 99 && maxCap == 99 && usedCap == 99 && polluted && eff.percentage == 0.5f);
                cISC4SimGrid<uint8_t>* g = reinterpret_cast<cISC4SimGrid<uint8_t>*>(0x1);
                CHECK(pl->GetSupplyValueMapPtr(g) && g == nullptr);
                // Slot 28 (GetWaterProducers) is not exercised here: it fills an
                // ilist whose root node comes from the framework allocator
                // service, which is not running in this harness.
                // Init without a city or GZCOM: an empty city, a clean lifecycle.
                CHECK(pl->Init());
                CHECK(pl->Init());
                CHECK(pl->Shutdown());
                CHECK(pl->Release() == 0);
            }
        }
        FreeLibrary(hd);
        DeleteFileA(ini.c_str());
        const std::string text = ReadFile(log);
        CHECK(text.find("subsystem 'plumbing': enabled (1 class replaced) by FreeCity4.ini (enable=)") !=
              std::string::npos);
        CHECK(text.find("subsystem 'power': off by default") != std::string::npos);
        // The power shadow can run here (our simulator, the original power).
        CHECK(text.find("power shadow: ON (power_shadow from FreeCity4.ini): at each SimNewMonth, SimNewDay and "
                        "working SimIdle of the ORIGINAL power simulator") != std::string::npos);
        CHECK(text.find("power shadow: requested") == std::string::npos);
        CHECK(text.find("cFC4PlumbingSimulator(") != std::string::npos);
        CHECK(text.find("plumbing Init:") != std::string::npos);
        CHECK(text.find("plumbing Shutdown") != std::string::npos);
        CHECK(text.find("cFC4PlumbingSimulator destroyed") != std::string::npos);
        // The shadow compares against the ORIGINAL plumbing simulator: requested, inactive.
        CHECK(text.find("plumbing shadow: requested (plumbing_shadow from FreeCity4.ini) but INACTIVE: it needs "
                        "FreeCity4's simulator and the original plumbing simulator (do not enable plumbing); here the "
                        "simulator is ours and plumbing is ours") != std::string::npos);
        CHECK(text.find("plumbing shadow: ON") == std::string::npos);
        CHECK(text.find("config: WARNING unknown key") == std::string::npos);
        if (g_failures == 0) std::printf("plugin enable test (%s): all checks passed\n", mode.c_str());
        return g_failures == 0 ? 0 : 1;
    }

    if (mode == "--ini-enable-pollution") {
        // The pollution subsystem is off by default; `enable = pollution`
        // registers the pollution simulator (pollution.md §1).
        SetEnvironmentVariableA("FC4_DISABLE", nullptr);
        SetEnvironmentVariableA("FC4_ENABLE", nullptr);
        WriteTextFile(ini, "[subsystems]\nenable = pollution\npollution_shadow = 1\n");
        HMODULE hd = LoadLibraryA(dll.c_str());
        CHECK(hd != nullptr);
        if (hd == nullptr) return 1;
        auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
        cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
        CHECK(director != nullptr);
        if (director != nullptr) {
            std::vector<uint32_t> classes;
            director->EnumClassObjects(&CollectClass, &classes);
            // simulator (1) + budget (3) + pollution (1); never the game's grid
            // classes (simgrid.md §6.3).
            CHECK(classes.size() == 5);
            for (uint32_t c : classes) CHECK(c < 0x49B9E600 || c > 0x49B9E60B);
            CHECK(std::count(classes.begin(), classes.end(), kCLSID_Pollution) == 1);
            void* obj = nullptr;
            CHECK(director->GetClassObject(kCLSID_Pollution, kIID_cISC4PollutionSimulator, &obj));
            auto* po = static_cast<cFC4ISC4PollutionSimulator*>(obj);
            if (po != nullptr) {
                // §1: the four interfaces and cIGZUnknown (the primary one); the
                // out pointer is left untouched for any other IID.
                void* p = nullptr;
                CHECK(po->QueryInterface(0x452294AAu, &p) && p != nullptr);
                if (p) static_cast<cIGZUnknown*>(p)->Release();
                cIGZSerializable* ser = nullptr;
                CHECK(po->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) && ser);
                if (ser) {
                    CHECK(ser->GetGZCLSID() == kCLSID_Pollution);
                    ser->Release();
                }
                cFC4ISC4PropRequester* req = nullptr;
                CHECK(po->QueryInterface(kIID_cISC4PropRequester, reinterpret_cast<void**>(&req)) && req);
                if (req) {
                    // §8.4: the satisfaction id, the standard selector.
                    CHECK(req->GetRequesterSatisfactionId() == 0xE9A4300Bu);
                    CHECK(!req->UsesCustomPropSelector());
                    const SC4Rect<int32_t> cells(0, 0, 3, 3);
                    CHECK(req->GetRequestedShare(cells) == 0);  // no city, no tracts
                    req->Release();
                }
                void* u = nullptr;
                CHECK(po->QueryInterface(GZIID_cIGZUnknown, &u) && u == static_cast<cIGZUnknown*>(po));
                if (u) static_cast<cIGZUnknown*>(u)->Release();
                void* untouched = reinterpret_cast<void*>(0xDEADBEEF);
                CHECK(!po->QueryInterface(0x452294ABu, &untouched));
                CHECK(untouched == reinterpret_cast<void*>(0xDEADBEEF));
                CHECK(!po->QueryInterface(0xE98B2F57u, &untouched));  // cIGZMessageTarget: not answered
                // No city, no grids: every query reads the neutral value and
                // nothing crashes.
                for (uint32_t kind = 0; kind < 5; ++kind) CHECK(po->GetPollutionGrid(kind) == nullptr);
                CHECK(po->GetPollutionValue(0, 3, 4) == 0);
                int32_t v = 7;
                CHECK(po->GetAirValue(3, 4, v) && v == 0);
                v = 7;
                CHECK(po->GetWaterValue(3, 4, v) && v == 0);
                v = 7;
                CHECK(po->GetGarbageValue(3, 4, v) && v == 0);
                CHECK(po->GetAverageAirValue() == 0 && po->GetAverageWaterValue() == 0);
                CHECK(!po->IsAirPolluted(3, 4) && !po->IsWaterPolluted(3, 4));
                CHECK(!po->IsGarbageAccumulated(3, 4) && !po->IsRadioactive(3, 4));
                CHECK(!po->IsActiveLandfill(3, 4));
                CHECK(po->SetAirValue(3, 4, 10));  // no grid: nothing written, still true (§8.1)
                CHECK(po->GetPollutionValue(0, 3, 4) == 0);
                CHECK(po->GetIncineratorCount() == 0 && po->GetWasteToEnergyBuildingCount() == 0);
                CHECK(po->GetRecyclingCenterCount() == 0 && po->GetWaterTreatmentPlantCount() == 0);
                CHECK(po->GetGarbageProducedLastMonth() == 0 && po->GetGarbageLandfilledLastMonth() == 0);
                CHECK(po->GetBuildingGarbageCapacity(nullptr) == 0);
                SC4Percentage* effect = po->GetRecyclingCenterEffect();  // slot 33: an address
                CHECK(effect != nullptr);
                CHECK(po->GetRecyclingCenterEffect() == effect);
                // Slot 46 (GetGarbageBuildings) is not exercised here: it fills an
                // ilist whose root node comes from the framework allocator
                // service, which is not running in this harness.
                // Init without a city or GZCOM: an empty city, a clean lifecycle.
                CHECK(po->Init());
                CHECK(po->Init());
                CHECK(po->Shutdown());
                CHECK(po->Release() == 0);
            }
        }
        FreeLibrary(hd);
        DeleteFileA(ini.c_str());
        const std::string text = ReadFile(log);
        CHECK(text.find("subsystem 'pollution': enabled (1 class replaced) by FreeCity4.ini (enable=)") !=
              std::string::npos);
        CHECK(text.find("subsystem 'plumbing': off by default") != std::string::npos);
        CHECK(text.find("cFC4PollutionSimulator(") != std::string::npos);
        CHECK(text.find("pollution Init: init=") != std::string::npos);
        CHECK(text.find("pollution Shutdown (0 days, 0 months)") != std::string::npos);
        CHECK(text.find("cFC4PollutionSimulator destroyed") != std::string::npos);
        // The shadow needs the ORIGINAL pollution simulator: requested, inactive.
        CHECK(text.find("pollution shadow: requested (pollution_shadow from FreeCity4.ini) but INACTIVE") !=
              std::string::npos);
        CHECK(text.find("config: WARNING unknown key") == std::string::npos);
        if (g_failures == 0) std::printf("plugin enable test (%s): all checks passed\n", mode.c_str());
        return g_failures == 0 ? 0 : 1;
    }

    if (mode == "--ini-enable-crime") {
        // The crime subsystem is off by default; `enable = crime` registers the
        // crime simulator (crime.md §1). No framework and no city here: the
        // no-city path, where every query reads the neutral value.
        SetEnvironmentVariableA("FC4_DISABLE", nullptr);
        SetEnvironmentVariableA("FC4_ENABLE", nullptr);
        SetEnvironmentVariableA("FC4_CRIME_SEED", nullptr);
        SetEnvironmentVariableA("FC4_CRIME_SHADOW", nullptr);
        WriteTextFile(ini, "[subsystems]\nenable = crime\ncrime_shadow = 1\ncrime_seed = 0x1234\n");
        HMODULE hd = LoadLibraryA(dll.c_str());
        CHECK(hd != nullptr);
        if (hd == nullptr) return 1;
        auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
        cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
        CHECK(director != nullptr);
        if (director != nullptr) {
            std::vector<uint32_t> classes;
            director->EnumClassObjects(&CollectClass, &classes);
            // simulator (1) + budget (3) + crime (1); never the game's grid
            // classes (simgrid.md §6.3).
            CHECK(classes.size() == 5);
            for (uint32_t c : classes) CHECK(c < 0x49B9E600 || c > 0x49B9E60B);
            CHECK(std::count(classes.begin(), classes.end(), kCLSID_Crime) == 1);
            void* obj = nullptr;
            CHECK(director->GetClassObject(kCLSID_Crime, kIID_cISC4CrimeSimulator, &obj));
            auto* cr = static_cast<cFC4ISC4CrimeSimulator*>(obj);
            if (cr != nullptr) {
                // §1: four interfaces and cIGZUnknown (the primary one), one
                // reference count; any other IID: false and the out pointer
                // untouched (CQ14).
                void* p = nullptr;
                CHECK(cr->QueryInterface(0x452294AAu, &p) && p != nullptr);
                if (p) static_cast<cIGZUnknown*>(p)->Release();
                cIGZSerializable* ser = nullptr;
                CHECK(cr->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) && ser);
                if (ser) {
                    CHECK(ser->GetGZCLSID() == kCLSID_Crime);
                    ser->Release();
                }
                cFC4CrimePropRequester* req = nullptr;
                CHECK(cr->QueryInterface(kIID_cISC4PropRequester, reinterpret_cast<void**>(&req)) && req);
                if (req) {
                    // §11.6: the satisfaction id, the custom selector; no grid:
                    // the empty request curve asks for nothing; with no prop
                    // manager every prop is generic, so one of the list is drawn.
                    CHECK(req->GetRequesterSatisfactionId() == 0x49A1E996u);
                    CHECK(req->UsesCustomPropSelector());
                    const SC4Rect<int32_t> cells(0, 0, 3, 3);
                    CHECK(req->GetRequestedShare(cells) == 0);
                    const uint32_t props[2] = {0x111, 0x222};
                    const FC4U32VectorView none{props, props, props};
                    CHECK(req->SelectProp(none, 1, 1) == 0);
                    const FC4U32VectorView two{props, props + 2, props + 2};
                    const uint32_t chosen = req->SelectProp(two, 1, 1);
                    CHECK(chosen == 0x111 || chosen == 0x222);
                    req->Release();
                }
                void* u = nullptr;
                CHECK(cr->QueryInterface(GZIID_cIGZUnknown, &u) && u == static_cast<cIGZUnknown*>(cr));
                if (u) static_cast<cIGZUnknown*>(u)->Release();
                void* untouched = reinterpret_cast<void*>(0xDEADBEEF);
                CHECK(!cr->QueryInterface(0xE98B2F57u, &untouched));  // cIGZMessageTarget: not answered
                CHECK(untouched == reinterpret_cast<void*>(0xDEADBEEF));
                CHECK(!cr->QueryInterface(0x12345678u, &untouched));  // any other IID
                CHECK(untouched == reinterpret_cast<void*>(0xDEADBEEF));
                // Class slot 15 (class-private, after the destructor at 14):
                // GetSimulatorType.
                using TypeFn = uint32_t(__thiscall*)(void*);
                void** vt = *reinterpret_cast<void***>(cr);
                CHECK(reinterpret_cast<TypeFn>(vt[15])(cr) == 0x27567222u);
                // No city, no grids: neutral answers, nothing crashes.
                CHECK(cr->GetCrimeGrid() == nullptr);
                CHECK(cr->GetAverageCrime() == 0);
                CHECK(cr->GetMonthlyCrimeCount() == 0);
                uint8_t hv = 7;
                int32_t hx = 7, hz = 7;
                CHECK(!cr->GetCrimeHotspot(&hv, &hx, &hz));
                const cS3DVector3 pos(10.0f, 0.0f, 10.0f);
                CHECK(cr->GetNearestCrimeSim(pos, 100.0f) == nullptr);
                CHECK(cr->GetNearestCrimeProp(pos, 100.0f) == nullptr);
                {
                    fc4::GameVector<FC4CrimeEvent, TestHeap> v;
                    CHECK(!cr->GetCrimeIncidentArray(reinterpret_cast<fc4::GameVector<FC4CrimeEvent>&>(v)));
                    CHECK(v.size() == 0);
                }
                {
                    // §11.4 (CQ1): the incident array grows like the game's
                    // vector resize: exactly the count when empty, else
                    // size + max(size, added); within the capacity, kept.
                    fc4::GameVector<FC4CrimeEvent, TestHeap> g;
                    CHECK(g.reserve_for_resize(3) && g.capacity() == 3);
                    for (uint32_t i = 0; i < 3; ++i) {
                        FC4CrimeEvent e{};
                        e.typeID = 100 + i;
                        CHECK(g.push_back(e));
                    }
                    FC4CrimeEvent* const first = g.begin();
                    CHECK(g.reserve_for_resize(2) && g.capacity() == 3 && g.begin() == first);
                    CHECK(g.reserve_for_resize(3) && g.capacity() == 3 && g.begin() == first);
                    CHECK(g.reserve_for_resize(4) && g.capacity() == 6);  // 3 + max(3, 1)
                    CHECK(g.size() == 3 && g[0].typeID == 100 && g[2].typeID == 102);
                    CHECK(g.reserve_for_resize(13) && g.capacity() == 13);  // 3 + max(3, 10)
                    CHECK(g.size() == 3 && g[1].typeID == 101);
                }
                CHECK(!cr->SetVisualsCreated(0, true));
                CHECK(!cr->GetVisualsCreated(0));
                // Init without a city or GZCOM: no agent, a clean lifecycle.
                CHECK(cr->Init());
                CHECK(cr->Init());
                CHECK(cr->Shutdown());
                CHECK(cr->Release() == 0);
            }
        }
        // Test seam op 26. §6.10 (CQ13): the crime effect is the property's raw storage (the
        // variant's data pointer, slot 95, whatever its type), no conversion.
        auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
        CHECK(seam != nullptr);
        if (seam != nullptr) {
            FakeHolder holder;
            auto put = [&holder](uint32_t id, auto fill) {
                cRZBaseVariant v;
                fill(v);
                holder.AddPropertyVariant(id, fc4::AsGame(static_cast<cIGZVariant const*>(&v)), false);
            };
            uint8_t u8s[2] = {100, 8};
            uint32_t u32s[2] = {100, 8};
            float f32s[2] = {100.0f, 8.0f};
            uint16_t u16s[1] = {0x0864};
            put(1u, [&](cRZBaseVariant& v) { v.RefUint8(u8s, 2); });
            put(2u, [&](cRZBaseVariant& v) { v.RefUint32(u32s, 2); });
            put(3u, [&](cRZBaseVariant& v) { v.RefFloat32(f32s, 2); });
            put(4u, [&](cRZBaseVariant& v) { v.RefUint16(u16s, 1); });
            put(5u, [](cRZBaseVariant& v) { v.SetValUint8(100); });
            char ab[2] = {'A', 'B'};
            char a1[1] = {'A'};
            put(7u, [&](cRZBaseVariant& v) { v.RefRZChar(ab, 2); });
            put(8u, [&](cRZBaseVariant& v) { v.RefChar(ab, 2); });
            put(9u, [&](cRZBaseVariant& v) { v.RefRZChar(a1, 1); });
            FC4RawArrayBytesTestIo io{};
            auto read = [&](uint32_t id) {
                io = FC4RawArrayBytesTestIo{};
                io.id = id;
                return seam(kFC4SeamRawArrayBytes, holder.AsSdk(), &io);
            };
            // Uint8 {100, 8}: m = 100, r = 8.
            CHECK(read(1) == 1 && io.len == 2 && io.bytes[0] == 100 && io.bytes[1] == 8);
            // Uint32 {100, 8}: 64 00 00 00 08 00 00 00, so m = 100, r = 0.
            CHECK(read(2) == 1 && io.len == 8 && io.bytes[0] == 100 && io.bytes[1] == 0 && io.bytes[4] == 8);
            // Float32 {100, 8}: 00 00 C8 42 ..., so m = 0, r = 0.
            CHECK(read(3) == 1 && io.len == 8 && io.bytes[0] == 0 && io.bytes[1] == 0 && io.bytes[2] == 0xC8 &&
                  io.bytes[3] == 0x42);
            // One Uint16 0x0864: 64 08, so m = 100, r = 8.
            CHECK(read(4) == 1 && io.len == 2 && io.bytes[0] == 0x64 && io.bytes[1] == 0x08);
            CHECK(read(5) == 0);  // [choice] a scalar: no bytes
            CHECK(read(6) == 0);  // absent
            // CQ22: a string is its stored bytes, so "AB" gives m = 65, r = 66
            // (RZChar and Char arrays alike).
            CHECK(read(7) == 1 && io.len == 2 && io.bytes[0] == 65 && io.bytes[1] == 66);
            CHECK(read(8) == 1 && io.len == 2 && io.bytes[0] == 65 && io.bytes[1] == 66);
            // One character: one byte, which the core skips ([choice]).
            CHECK(read(9) == 1 && io.len == 1 && io.bytes[0] == 65);
            CHECK(seam(kFC4SeamRawArrayBytes, nullptr, &io) == -1);

            // Test seam op 27. §13 (CQ3): lot GetCenter (slot 15) answers in the out
            // parameter with the x87 stack unchanged; the thunk counts a depth
            // change and no longer pops anything (a value left behind is the
            // caller's, here the test's, to pop).
            void* vt[16] = {};
            vt[15] = reinterpret_cast<void*>(&FakeLotGetCenter);
            FakeLotObject lot{vt};
            float c[3] = {0.0f, 0.0f, 0.0f};
            const unsigned top = X87Top();
            CHECK(seam(kFC4SeamLotCenter, &lot, c) == 0);
            CHECK(c[0] == 256.5f && c[1] == 10.0f && c[2] == 512.25f);
            CHECK(X87Top() == top);
            vt[15] = reinterpret_cast<void*>(&FakeLotGetCenterSt0);
            const intptr_t left = seam(kFC4SeamLotCenter, &lot, c);
            const unsigned topLeft = X87Top();
            fc4test::PopX87();  // the 7.0f the fake left: not popped by the thunk
            CHECK(left == 1 && topLeft == ((top - 1u) & 7u));
            CHECK(c[0] == 1.0f && c[2] == 3.0f);
            CHECK(X87Top() == top);
            CHECK(seam(kFC4SeamLotCenter, nullptr, c) == -1);
        }
        FreeLibrary(hd);
        DeleteFileA(ini.c_str());
        const std::string text = ReadFile(log);
        CHECK(text.find("subsystem 'crime': enabled (1 class replaced) by FreeCity4.ini (enable=)") !=
              std::string::npos);
        CHECK(text.find("subsystem 'pollution': off by default") != std::string::npos);
        CHECK(text.find("cFC4CrimeSimulator(") != std::string::npos);
        CHECK(text.find(", session seed 0x00001234 (crime_seed)") != std::string::npos);
        CHECK(text.find("crime: no simulator; agent not registered") != std::string::npos);
        CHECK(text.find("crime Init: init=1") != std::string::npos);
        CHECK(text.find("crime Shutdown (0 days, 0 months)") != std::string::npos);
        CHECK(text.find("cFC4CrimeSimulator destroyed") != std::string::npos);
        // The shadow needs the ORIGINAL crime simulator: requested, inactive.
        CHECK(text.find("crime shadow: requested (crime_shadow from FreeCity4.ini) but INACTIVE") !=
              std::string::npos);
        // No GZCOM here: the grids cannot be created (§4.1 step 7), so the
        // getters stay neutral. Nothing else is worth a warning.
        CHECK(text.find("crime: WARNING could not create grid 0x89D5B895") != std::string::npos);
        CHECK(text.find("crime: WARNING GetCrimeIncidentArray") == std::string::npos);
        CHECK(text.find("off the main thread") == std::string::npos);
        CHECK(text.find("config: WARNING unknown key") == std::string::npos);
        if (g_failures == 0) std::printf("plugin enable test (%s): all checks passed\n", mode.c_str());
        return g_failures == 0 ? 0 : 1;
    }

    if (mode == "--disabled") {
        // FC4_DISABLE switches subsystems off: nothing is registered or created.
        SetEnvironmentVariableA("FC4_DISABLE", "Budget, Simulator");
        HMODULE hd = LoadLibraryA(dll.c_str());
        CHECK(hd != nullptr);
        if (hd == nullptr) return 1;
        auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
        cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
        CHECK(director != nullptr);
        if (director != nullptr) {
            std::vector<uint32_t> classes;
            director->EnumClassObjects(&CollectClass, &classes);
            CHECK(classes.empty());
            void* obj = nullptr;
            CHECK(!director->GetClassObject(kCLSID_Simulator, kIID_cISC4Simulator, &obj));
            CHECK(!director->GetClassObject(kCLSID_Budget, kIID_cISC4BudgetSimulator, &obj));
        }
        // The observer still runs (passthrough): the digest, read from the city's
        // simulators (here a fake city without a budget simulator).
        auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
        CHECK(seam != nullptr);
        if (seam != nullptr) {
            static FakeWorld world;
            InitFakeWorld(world);
            seam(kFC4SeamObserverCity, &world.city, nullptr);
            seam(kFC4SeamObserverMessage, reinterpret_cast<void*>(static_cast<uintptr_t>(kMsgPostCityInit)), nullptr);
            seam(kFC4SeamObserverTick, nullptr, nullptr);  // the "after load" snapshot
            seam(kFC4SeamObserverMessage, reinterpret_cast<void*>(static_cast<uintptr_t>(kMsgSimNewMonth)),
                 reinterpret_cast<void*>(static_cast<intptr_t>(2451576)));
            CHECK(world.demandCalls == 28);  // digest and observe: 12 groups + 2 census nodes each
            // A second month without a residential simulator.
            world.noRes = true;
            seam(kFC4SeamObserverMessage, reinterpret_cast<void*>(static_cast<uintptr_t>(kMsgSimNewMonth)),
                 reinterpret_cast<void*>(static_cast<intptr_t>(2451605)));
            seam(kFC4SeamObserverCity, nullptr, nullptr);
        }
        FreeLibrary(hd);
        const std::string text = ReadFile(log);
        CHECK(text.find("observer: PostCityInit (passthrough)") != std::string::npos);
        CHECK(text.find("DIGEST julian=2451576 (2000-02-01) funds=- population R/C/I=1234/480/160 demand "
                        "R$/R$$/R$$$/Cs$/Cs$$/Cs$$$/Co$$/Co$$$/I-R/I-D/I-M/I-HT=250.5/-/-/-12.0/-/-/40.0/-/7.5/-/-/0.0 "
                        "budget income/expense=-/-") != std::string::npos);
        CHECK(text.find("observe snapshot (after load): date=- julian=- funds=- fundings=-") != std::string::npos);
        CHECK(text.find("observe month 2000-02: date=2000-02-01 julian=2451576 funds=- income=- expense=- "
                        "tax[0..11]=- dept_expense=- demand R$/R$$/R$$$/Cs$/Cs$$/Cs$$$/Co$$/Co$$$/I-R/I-D/I-M/I-HT="
                        "250.5/-/-/-12.0/-/-/40.0/-/7.5/-/-/0.0 population R/C/I=1234/480/160 power=- "
                        "residential EQ/HQ/LE=12.5000/34.2500/75\r\n") != std::string::npos);
        CHECK(text.find("observe month 2000-03: date=2000-03-01 julian=2451605 funds=- ") != std::string::npos);
        CHECK(text.find(" population R/C/I=-/480/160 power=- residential EQ/HQ/LE=-\r\n") != std::string::npos);
        CHECK(text.find("subsystem 'simulator': DISABLED by FC4_DISABLE") != std::string::npos);
        CHECK(text.find("subsystem 'budget': DISABLED by FC4_DISABLE") != std::string::npos);
        CHECK(text.find("object created") == std::string::npos);
        if (g_failures == 0) std::printf("plugin disable test: all checks passed\n");
        return g_failures == 0 ? 0 : 1;
    }
    SetEnvironmentVariableA("FC4_DIAG_SELFTEST", "1");
    SetEnvironmentVariableA("FC4_CHECKED_SELFTEST", "1");
    SetEnvironmentVariableA("FC4_STATUS_EVERY", "5");
    SetEnvironmentVariableA("FC4_HEARTBEAT_MS", "2000");
    // The plumbing shadow on (our simulator, the original plumbing simulator):
    // the game-start line says ON; deliveries to other agents pass through.
    SetEnvironmentVariableA("FC4_PLUMBING_SHADOW", "1");

    SetGamePrecision();  // as the game's PreFrameWorkInit does
    HMODULE h = LoadLibraryA(dll.c_str());
    CHECK(h != nullptr);
    if (h == nullptr) return 1;
    CheckCw("LoadLibrary (CRT, Rust and DllMain initialisation)");

    using GetDirectorFn = cIGZCOMDirector* (*)();
    auto getDirector = reinterpret_cast<GetDirectorFn>(GetProcAddress(h, "GZDllGetGZCOMDirector"));
    CHECK(getDirector != nullptr);
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr);
    if (director != nullptr) {
        CHECK(director->GetDirectorID() == kDirectorID);

        std::vector<uint32_t> classes;
        director->EnumClassObjects(&CollectClass, &classes);
        CHECK(classes.size() == 4);
        CHECK(classes.size() == 4 && classes[0] == kCLSID_Simulator && classes[1] == kCLSID_Budget &&
              classes[2] == kCLSID_Department && classes[3] == kCLSID_LineItem);

        // Another process (an editor, a tail) holding the log open must not stop the logging.
        HANDLE reader = CreateFileA(log.c_str(), GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                                    nullptr, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
        CHECK(reader != INVALID_HANDLE_VALUE);

        // Our class with an interface it does not implement: refused, object destroyed.
        void* nope = reinterpret_cast<void*>(1);
        CHECK(!director->GetClassObject(kCLSID_Simulator, 0x0BADF00D, &nope));
        CHECK(nope == nullptr);

        cISC4Simulator* sim = nullptr;
        CHECK(director->GetClassObject(kCLSID_Simulator, kIID_cISC4Simulator, reinterpret_cast<void**>(&sim)));
        CHECK(sim != nullptr);
        CHECK(!director->GetClassObject(0x12345678, kIID_cISC4Simulator, reinterpret_cast<void**>(&sim)) ||
              sim != nullptr);

        if (sim != nullptr) {
            CHECK(sim->Init());
            CHECK(sim->IsHiddenPaused());
            CHECK(sim->GetSimDateNumber() == 2451545);
            CHECK(sim->SetSimSpeed(2));
            CHECK(sim->HiddenResume());
            CHECK(!sim->IsAnyPaused());

            SlowAgent agent;
            CHECK(sim->AddAgent(&agent, cISC4Simulator::AgentTypeSimNewDay, cRZBaseString("test day agent"),
                                static_cast<cISC4Simulator::eAgentFlags>(3)));
            CHECK(agent.refs == 1);

            cIGZSystemService* service = nullptr;
            CHECK(sim->QueryInterface(GZIID_cIGZSystemService, reinterpret_cast<void**>(&service)));
            if (service != nullptr) {
                CHECK(service->GetServiceID() == 0x46956659);
                CHECK(service->GetServicePriority() == -2000000);
                // Default tuning: 100 ms per day at speed 2. Tick for ~600 ms.
                const DWORD start = GetTickCount();
                while (GetTickCount() - start < 600) service->OnTick(0);
                service->Release();
            }
            const int32_t day = sim->GetSimDateNumber();
            std::printf("simulated %d day(s) in ~600 ms; agent called %u times\n", day - 2451545, agent.calls);
            CHECK(day > 2451545 + 2);
            CHECK(agent.calls > 150);
            CHECK(agent.badMessages == 0);
            CHECK(agent.lastJulian == day);
            // Overloaded methods are only ever called through the game's slots (foundations §16).
            cIGZDate* date = reinterpret_cast<cIGZDate*>(fc4test::CallSlot(sim, 7, {}).eax);  // GetSimDate()
            CHECK(date != nullptr && date->Year() == 2000 && static_cast<int32_t>(date->DayNumber()) == day);
            if (date != nullptr) {
                // cIGZDate slots 33-36 (foundations §17.4): a uint32 day number in
                // EAX, one stack argument, nothing on the x87 stack, and the date
                // itself unchanged.
                using fc4test::CallSlot;
                const uint32_t d = static_cast<uint32_t>(day);
                cIGZDate* other = nullptr;
                CHECK(CallSlot(date, 9, {reinterpret_cast<uint32_t>(&other)}).eax == 1);  // DT3 full EAX
                CHECK(other != nullptr);
                if (other != nullptr) {
                    CHECK(CallSlot(other, 1, {}).eax == 2); // fresh Clone owns exactly one ref
                    CHECK(CallSlot(other, 2, {}).eax == 1);
                    CHECK(CallSlot(other, 4, {0x12345600u}).eax == 0x12345600u); // valid low AL zero
                    CHECK(other->DayNumber() == 0x12345600u);
                    void* sentinel = reinterpret_cast<void*>(0xDEADBEEF);
                    CHECK(!other->QueryInterface(0xDEAD1234u, &sentinel));
                    CHECK(sentinel == reinterpret_cast<void*>(0xDEADBEEF));
                    CHECK(CallSlot(other, 6, {2, 30, 2000}).eax == 0); // invalid setter clears day
                    CHECK(other->DayNumber() == 0);
                    CHECK(CallSlot(other, 4, {100}).eax == 100);  // published new day number
                    const fc4test::SlotCall m1 = CallSlot(date, 33, {7});  // operator-(int32)
                    const fc4test::SlotCall m2 = CallSlot(date, 34, {reinterpret_cast<uint32_t>(other)});  // operator-(date)
                    const fc4test::SlotCall p1 = CallSlot(date, 35, {7});  // operator+(int32)
                    const fc4test::SlotCall p2 = CallSlot(date, 36, {reinterpret_cast<uint32_t>(other)});  // operator+(date)
                    std::printf("date slots 33-36: %u %u %u %u (day %u)\n", m1.eax, m2.eax, p1.eax, p2.eax, d);
                    CHECK(m1.eax == d - 7 && m2.eax == d - 100 && p1.eax == d + 7 && p2.eax == d + 100);
                    for (const fc4test::SlotCall* c : {&m1, &m2, &p1, &p2}) {
                        CHECK(c->espDelta == 0 && c->x87Pushed == 0);
                    }
                    CHECK(date->DayNumber() == d && other->DayNumber() == 100u);  // neither date modified
                    // Slots 16/17: FirstDayOfMonth(month) / FirstDayOfMonth().
                    const fc4test::SlotCall f16 = CallSlot(date, 16, {3});
                    const fc4test::SlotCall f17 = CallSlot(date, 17, {});
                    CHECK(f16.espDelta == 0 && f17.espDelta == 0);
                    CHECK(f16.eax == 61u);  // dates.md: day of year, 2000-03-01
                    CHECK(f17.eax == 1u);   // the sim date is in January 2000
                    other->Release();
                }
            }
            {
                // Overloads through the game's slots (simulator §5.6), with the
                // game's argument counts: the stack must balance.
                using fc4test::CallSlot;
                uint32_t gy = 0, gm = 0, gd = 0, gdoy = 0, gwd = 0;
                const fc4test::SlotCall s6 = CallSlot(sim, 6, {reinterpret_cast<uint32_t>(&gy), reinterpret_cast<uint32_t>(&gm),
                                                         reinterpret_cast<uint32_t>(&gd), reinterpret_cast<uint32_t>(&gdoy),
                                                         reinterpret_cast<uint32_t>(&gwd)});
                std::printf("slot 6: esp %d, year %u\n", s6.espDelta, gy);
                CHECK(s6.espDelta == 0 && gy == 2000);
                const fc4test::SlotCall s7 = CallSlot(sim, 7, {});
                std::printf("slot 7: esp %d, eax %08X\n", s7.espDelta, s7.eax);
                CHECK(s7.espDelta == 0 && s7.eax != 0);
                SlowAgent other;
                const fc4test::SlotCall s20 = CallSlot(sim, 20, {reinterpret_cast<uint32_t>(static_cast<cIGZMessageTarget2*>(&other))});
                const fc4test::SlotCall s21 = CallSlot(sim, 21, {reinterpret_cast<uint32_t>(static_cast<cIGZMessageTarget2*>(&other)), 4});
                const fc4test::SlotCall s22 = CallSlot(sim, 22, {9});
                std::printf("slots 20/21/22: esp %d %d %d\n", s20.espDelta, s21.espDelta, s22.espDelta);
                CHECK(s20.espDelta == 0 && s21.espDelta == 0 && s22.espDelta == 0);
            }
            {
                // RemoveAgent(agent, level) at slot 21 removes the day registration.
                const fc4test::SlotCall r = fc4test::CallSlot(
                    sim, 21, {reinterpret_cast<uint32_t>(static_cast<cIGZMessageTarget2*>(&agent)), 4});
                CHECK(r.espDelta == 0 && (r.eax & 0xFF) == 1);
            }
            CHECK(agent.refs == 0);
            {
                // RemoveAllAgents() at slot 23.
                const fc4test::SlotCall r = fc4test::CallSlot(sim, 23, {});
                CHECK(r.espDelta == 0);
            }

            uint32_t y = 0, m = 0, d = 0, doy = 0, wd = 0;
            const fc4test::SlotCall r6 = fc4test::CallSlot(
                sim, 6,
                {reinterpret_cast<uint32_t>(&y), reinterpret_cast<uint32_t>(&m), reinterpret_cast<uint32_t>(&d),
                 reinterpret_cast<uint32_t>(&doy), reinterpret_cast<uint32_t>(&wd)});
            CHECK(r6.espDelta == 0);
            CHECK(y == 2000 && m == 1 && doy == static_cast<uint32_t>(day - 2451545 + 1));

            {
                // Watchdog: a day agent that blocks for 6.5 s inside OnTick must
                // produce a stall report (activity, call ring, main-thread sample)
                // and an "over" line; heartbeats run meanwhile (FC4_HEARTBEAT_MS=2000).
                BlockingAgent blocker;
                CHECK(sim->AddAgent(&blocker, cISC4Simulator::AgentTypeSimNewDay, cRZBaseString("blocking test agent"),
                                    static_cast<cISC4Simulator::eAgentFlags>(3)));
                cIGZSystemService* svc = nullptr;
                CHECK(sim->QueryInterface(GZIID_cIGZSystemService, reinterpret_cast<void**>(&svc)));
                if (svc != nullptr) {
                    const DWORD start = GetTickCount();
                    while (blocker.calls == 0 && GetTickCount() - start < 3000) svc->OnTick(0);
                    const DWORD after = GetTickCount();
                    while (GetTickCount() - after < 2500) svc->OnTick(0);  // let the watchdog see the end
                    svc->Release();
                }
                CHECK(blocker.calls >= 1);
                fc4test::CallSlot(sim, 20, {reinterpret_cast<uint32_t>(static_cast<cIGZMessageTarget2*>(&blocker))});
            }

            CHECK(sim->Shutdown());
            CHECK(sim->GetSimDateNumber() == 2451545);
            CHECK(sim->Release() == 0);
        }

        // The budget without a game: no city, no message server, no exemplar
        // (code defaults; difficulty 0 without a city).
        cISC4BudgetSimulator* budget = nullptr;
        CHECK(director->GetClassObject(kCLSID_Budget, kIID_cISC4BudgetSimulator, reinterpret_cast<void**>(&budget)));
        auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(h, "FC4TestSeam"));
        CHECK(seam != nullptr);
        std::vector<PostedMessage> posts;  // budget §10: every message the budget posts
        if (seam != nullptr) seam(kFC4SeamBudgetPostSink, reinterpret_cast<void*>(&RecordPost), &posts);
        if (budget != nullptr) {
            CHECK(budget->Init());
            CHECK(budget->GetTotalFunds() == 500000);
            CHECK(budget->DepositFunds(100));
            CHECK(!budget->WithdrawFunds(600000));
            CHECK(budget->GetTotalFunds() == 500100);
            CHECK(budget->GetTaxRate(0) == 9.0f);

            cISC4DepartmentBudget* dept = budget->CreateDepartmentBudget(0x28F55A9F, 7);
            CHECK(dept != nullptr);
            if (dept != nullptr) {
                CHECK(dept->GetDepartmentID() == 0x28F55A9F);
                CHECK(fc4test::CallSlot(budget, 28, {0x28F55A9F}).eax == reinterpret_cast<uint32_t>(dept));
                cISC4DepartmentBudget* viaClsid = nullptr;
                CHECK(dept->QueryInterface(kCLSID_Department, reinterpret_cast<void**>(&viaClsid)));
                CHECK(viaClsid == dept);
                if (viaClsid != nullptr) viaClsid->Release();
                cISC4DepartmentBudget* viaIid = nullptr;
                CHECK(dept->QueryInterface(kIID_cISC4DepartmentBudget, reinterpret_cast<void**>(&viaIid)));
                if (viaIid != nullptr) viaIid->Release();

                cISC4LineItem* line = dept->CreateLineItem(1, false);
                CHECK(line != nullptr);
                if (line != nullptr) {
                    cISC4LineItem* viaLineClsid = nullptr;
                    CHECK(line->QueryInterface(kCLSID_LineItem, reinterpret_cast<void**>(&viaLineClsid)));
                    if (viaLineClsid != nullptr) viaLineClsid->Release();
                    CHECK(line->GetID() == 1);
                    CHECK(line->SetFullExpenses(1000));
                    SC4Percentage half{0.5f};
                    CHECK(dept->SetFundingPercentage(half, 0));
                    CHECK(dept->GetFundingPercentage()->percentage == 0.5f);
                    CHECK(line->GetCurrentExpenses() == 500);
                    CHECK(dept->GetLineItem(1) == line);
                }
                CHECK(budget->GetTotalMonthlyExpense() == 500u);  // low 32 bits of the 64-bit value
                {
                    // Overloads through the game's slots (budget "Verified slot order").
                    using fc4test::CallSlot;
                    const fc4test::SlotCall s18 = CallSlot(budget, 18, {0});  // GetTaxIncome(int32)
                    const fc4test::SlotCall s19 = CallSlot(budget, 19, {});   // GetTaxIncome()
                    const fc4test::SlotCall s34 = CallSlot(budget, 34, {0x28F55A9F});  // GetFunding(uint32)
                    const fc4test::SlotCall s35 = CallSlot(budget, 35, {reinterpret_cast<uint32_t>(dept)});
                    CHECK(s34.espDelta == 0 && s35.espDelta == 0 && s34.eax == 0 && s35.eax == 0);
                    std::printf("budget slots 18/19: esp %d %d\n", s18.espDelta, s19.espDelta);
                    CHECK(s18.espDelta == 0 && s19.espDelta == 0);
                    cRZBaseString empty("");
                    const fc4test::SlotCall s27 = CallSlot(budget, 27, {reinterpret_cast<uint32_t>(static_cast<cIGZString*>(&empty))});
                    const fc4test::SlotCall s28 = CallSlot(budget, 28, {0x28F55A9F});
                    std::printf("budget slots 27/28: esp %d %d, %08X %08X (dept %p)\n", s27.espDelta, s28.espDelta, s27.eax,
                                s28.eax, static_cast<void*>(dept));
                    CHECK(s27.espDelta == 0 && s28.espDelta == 0);
                    CHECK(s27.eax == reinterpret_cast<uint32_t>(dept));  // "" is a prefix of every name
                    CHECK(s28.eax == reinterpret_cast<uint32_t>(dept));
                }
                CHECK(budget->IssueBond(10000));
                CHECK(budget->GetTotalBorrowed() == 10000);
                CHECK(budget->GetTotalFunds() == 510100);
                CHECK(budget->GetTotalMonthlyExpense() == 500u + static_cast<uint32_t>(budget->GetMonthlyPaymentForLoan(10000)));
                {
                    // Slot 26, GetLocalFundingPercent, returns a POINTER to a float
                    // in EAX (budget erratum): the game dereferences it at once.
                    FakeHolder holder;
                    holder.Set<uint32_t>(0xEA54D283, 0x28F55A9F);  // department
                    holder.Set<uint32_t>(0xEA54D284, 2);           // line item
                    holder.Set<uint32_t>(0xEA54D285, 7);           // purpose
                    holder.Set<int64_t>(0xEA54D286, 200);          // cost
                    CHECK(dept->CreateLineItem(2, true) != nullptr);
                    CHECK(dept->AddLocallyFundedObject(holder.AsSdk(), 7));
                    CHECK(holder.refs == 2);
                    const uint32_t h = reinterpret_cast<uint32_t>(holder.AsSdk());
                    using fc4test::CallSlot;
                    const fc4test::SlotCall found = CallSlot(dept, 26, {h, 7});
                    const fc4test::SlotCall otherPurpose = CallSlot(dept, 26, {h, 8});
                    const fc4test::SlotCall noHolder = CallSlot(dept, 26, {0, 7});
                    for (const fc4test::SlotCall* c : {&found, &otherPurpose, &noHolder}) {
                        for (int i = 0; i < c->x87Pushed; ++i) fc4test::PopX87();
                    }
                    std::printf("department slot 26: found %08X (%d x87), not found %08X / %08X\n", found.eax,
                                found.x87Pushed, otherPurpose.eax, noHolder.eax);
                    CHECK(found.espDelta == 0 && otherPurpose.espDelta == 0 && noHolder.espDelta == 0);
                    CHECK(found.x87Pushed == 0 && otherPurpose.x87Pushed == 0 && noHolder.x87Pushed == 0);
                    CHECK(found.eax != 0 && otherPurpose.eax != 0 && noHolder.eax != 0);
                    if (found.eax != 0) {
                        // The item's local-funding fraction: float32(property value × 0.01f)
                        // (budget.md §6, RG13: a multiplication, not pct / 100).
                        const float q = reinterpret_cast<const float*>(found.eax)[0];
                        CHECK(q == holder.Get32f(0xEA54D287) * 0.01f);
                        CHECK(q == 0.5f);  // AddLocallyFundedObject wrote the department's 50 %
                    }
                    if (otherPurpose.eax != 0) CHECK(reinterpret_cast<const float*>(otherPurpose.eax)[0] == 0.0f);
                    CHECK(noHolder.eax == otherPurpose.eax);  // the shared constant 0.0
                    CHECK(found.eax != otherPurpose.eax);
                    CHECK(dept->RemoveLocallyFundedObject(holder.AsSdk(), 7));
                    CHECK(holder.refs == 1);
                    const fc4test::SlotCall gone = CallSlot(dept, 26, {h, 7});
                    CHECK(gone.eax == otherPurpose.eax && gone.espDelta == 0 && gone.x87Pushed == 0);
                }
            }
            {
                // Budget §10.1/§10.2 [compat]: void1 = IGZUnknown = the sender's
                // primary interface pointer; data1 shares void1's slot.
                if (seam != nullptr) seam(kFC4SeamBudgetPostSink, nullptr, nullptr);
                std::printf("budget posted %u message(s)\n", static_cast<unsigned>(posts.size()));
                void* const b = static_cast<void*>(budget);
                void* const d = static_cast<void*>(dept);
                CHECK(CountPosts(posts, 0x0772FAD4) >= 1);  // FundsChanged (DepositFunds)
                CHECK(CountPosts(posts, 0xC772FADA) >= 1);  // BondIssued
                CHECK(CountPosts(posts, 0x08DAC3E7) >= 1);  // FundingPercentageChanged
                CHECK(CountPosts(posts, 0x08DAC3E8) >= 1);  // IdealMonthlyFundingChanged
                for (const PostedMessage& p : posts) {
                    std::printf("  post 0x%08X void1=%p unknown=%p d1=0x%08X d2=%d d3=%d\n", p.type, p.void1, p.unknown,
                                static_cast<unsigned>(p.d1), static_cast<int>(p.d2), static_cast<int>(p.d3));
                    if (p.type == 0x0772FAD4 || p.type == 0xC772FADA || p.type == 0xCA3FBDCE) {
                        CHECK(p.void1 == b && p.unknown == b);
                        CHECK(p.d1 == reinterpret_cast<intptr_t>(b));
                    }
                    if (p.type == 0x08DAC3E7 || p.type == 0x08DAC3E8) {
                        CHECK(p.void1 == d && p.unknown == d);
                        CHECK(p.d1 == reinterpret_cast<intptr_t>(d));
                        CHECK(p.d2 == 0x28F55A9F);
                    }
                    if (p.type == 0x08DAC3E8) CHECK(p.d3 == 0);
                    if (p.type == 0x0A3E7624) {  // BudgetGroupChanged: data only, no sender (§10)
                        CHECK(p.unknown == nullptr && p.d1 == 7);
                    }
                }
                bool sawPercentage = false;  // SetFundingPercentage(half, 0): data3 = the caller's argument
                for (const PostedMessage& p : posts) sawPercentage |= p.type == 0x08DAC3E7 && p.d3 == 0;
                CHECK(sawPercentage);
            }
            CHECK(budget->Shutdown());
            CHECK(fc4test::CallSlot(budget, 28, {0x28F55A9F}).eax == 0);
            CHECK(budget->GetTotalFunds() == 510100);  // kept across Shutdown
            CHECK(budget->Release() == 0);
        }

        // Bisection switches: FreeCity4.ini is re-read at city load (budget
        // Init) and wins over the environment; a switch that is off makes its
        // behaviour a no-op.
        SetEnvironmentVariableA("FC4_BUDGET_CHEATS", "0");    // overridden by the ini
        SetEnvironmentVariableA("FC4_BUDGET_DISASTER", "0");  // not in the ini: applies
        WriteTextFile(ini, "[budget]\r\nholder_refs = 0\r\nproperty_writes=off ; no writes\r\ncheats = 1\r\n");
        cISC4BudgetSimulator* budget2 = nullptr;
        CHECK(director->GetClassObject(kCLSID_Budget, kIID_cISC4BudgetSimulator, reinterpret_cast<void**>(&budget2)));
        if (budget2 != nullptr) {
            CHECK(budget2->Init());
            cISC4DepartmentBudget* dept = budget2->CreateDepartmentBudget(0x28F55A9F, 7);
            CHECK(dept != nullptr);
            if (dept != nullptr) {
                CHECK(dept->CreateLineItem(2, true) != nullptr);
                FakeHolder holder;
                holder.Set<uint32_t>(0xEA54D283, 0x28F55A9F);
                holder.Set<uint32_t>(0xEA54D284, 2);
                holder.Set<uint32_t>(0xEA54D285, 7);
                holder.Set<int64_t>(0xEA54D286, 200);
                CHECK(dept->AddLocallyFundedObject(holder.AsSdk(), 7));
                CHECK(holder.refs == 1);                             // holder_refs off: no reference held
                CHECK(holder.Get32f(0xEA54D287) == -1.0f);           // property_writes off: nothing written
                CHECK(dept->RemoveLocallyFundedObject(holder.AsSdk(), 7));
                CHECK(holder.refs == 1);
            }
            CHECK(budget2->Shutdown());
            CHECK(budget2->Release() == 0);
        }
        DeleteFileA(ini.c_str());
        SetEnvironmentVariableA("FC4_BUDGET_CHEATS", nullptr);
        SetEnvironmentVariableA("FC4_BUDGET_DISASTER", nullptr);

        // The flammability ignition path (§6.8): the layer's manager is
        // borrowed, only the QueryInterface result is released. Run 37
        // released the manager as well, once per IgniteCell and once per
        // SetCause, and freed the game's fire manager (refs would be -1).
        if (seam != nullptr) {
            static FakeFireCity fire;
            InitFakeFireCity(fire);
            CHECK(seam(kFC4SeamFlammabilityFire, &fire.city, nullptr) == 0);
            CHECK(fire.ignites == 1);
            CHECK(fire.causes == 1);
            CHECK(fire.cause == 5);
            CHECK(fire.qis == 2);
            CHECK(fire.refs == 1);
        }

        // The x87 return channels (docs/abi corrected 2026-10-14), called the
        // game's way: nothing may be left on, or taken off, the x87 stack.
        if (seam != nullptr) {
            static FakePipeCity pipe;
            InitFakePipeCity(pipe);
            // City slot 97 is void: protection and landfill call it so (op 28).
            FC4CellCenterTestIo io{3, 5, 0.0f, 0.0f, 0.0f, 0.0f};
            const unsigned top = X87Top();
            CHECK(seam(kFC4SeamCellCenter, &pipe.city, &io) == 1);
            CHECK(X87Top() == top);  // a float declaration popped an empty register here
            CHECK(io.protectionX == 56.0f && io.protectionZ == 88.0f);
            CHECK(io.landfillX == 56.0f && io.landfillZ == 88.0f);
            CHECK(seam(kFC4SeamCellCenter, nullptr, &io) == -1);
            // Plumbing §7.1 step 4: the AttemptDisasterStart result is ignored,
            // never released (op 29). The old glue released it (refs 0).
            const int32_t xz[2] = {12, 34};
            CHECK(seam(kFC4SeamPipeBurst, &pipe.city, const_cast<int32_t*>(xz)) == 1);
            CHECK(pipe.attempts == 1 && pipe.x == 12 && pipe.z == 34);
            CHECK(pipe.instRefs == 1);
            CHECK(seam(kFC4SeamPipeBurst, nullptr, const_cast<int32_t*>(xz)) == -1);
            // cIGZVariant slot 41 SetValFloat64(double) is void (op 30): pops 8,
            // nothing in ST0.
            cFC4IGZVariant* v = nullptr;
            CHECK(seam(kFC4SeamVariant, &v, nullptr) == 1 && v != nullptr);
            if (v != nullptr) {
                const double d = 2.5;
                uint32_t w[2] = {0, 0};
                std::memcpy(w, &d, sizeof(w));
                const fc4test::SlotCall r = fc4test::CallSlot(v, 41, {w[0], w[1]});
                CHECK(r.espDelta == 0 && r.x87Pushed == 0);
                CHECK(v->GetValFloat64() == 2.5);
                CHECK(v->Release() == 0);  // the heap copy is deleted
            }
            CHECK(seam(kFC4SeamVariant, nullptr, nullptr) == -1);
            // The x87 leftover guard (op 31, FC4Fpu.h): a clean stack is 0 and
            // writes nothing; two values left are found (and logged once for
            // the site this game month, counted every time); nothing is popped.
            CHECK(seam(kFC4SeamX87Check, nullptr, nullptr) == 0);
            __asm {
                fld1;
                fldz;
            }
            const intptr_t found = seam(kFC4SeamX87Check, nullptr, nullptr);
            const intptr_t again = seam(kFC4SeamX87Check, nullptr, nullptr);
            const unsigned topLeft = X87Top();
            __asm {
                fstp st(0)
                fstp st(0)
            }
            CHECK(found == 2 && again == 2);
            CHECK(topLeft == ((top - 2u) & 7u));
            CHECK(X87Top() == top);
            CHECK(seam(kFC4SeamX87Check, nullptr, nullptr) == 0);
        }

        // The budget shadow next to an "original" budget: the same values
        // give no mismatch; the original counting the not-livable CS$ lot
        // gives one (type 3), attributed to that lot. Any write to the fake
        // (or any call outside its getters) would abort.
        if (seam != nullptr) {
            static FakeOrigBudget orig;
            InitFakeOrigBudget(orig);
            cIGZMessageTarget2* agent = reinterpret_cast<cIGZMessageTarget2*>(&orig.agent);
            CHECK(ShadowMonth(seam, orig, agent, 2000, 1500000) == 0);
            CHECK(orig.deliveries == 1);
            CHECK(ShadowMonth(seam, orig, agent, 2500, 1875000) == 1);
            CHECK(orig.deliveries == 2);
            // Not the budget's agent: left to the caller, nothing delivered.
            CHECK(ShadowMonth(seam, orig, reinterpret_cast<cIGZMessageTarget2*>(&orig.other), 2000, 1500000) == -1);
            CHECK(orig.deliveries == 2);
            CHECK(orig.refs == 1);  // every QueryInterface released
            CheckCw("budget shadow");
        }

        // The demand shadow next to an "original" demand simulator: FreeCity4's
        // propagation over the states read before NewDay gives the states the
        // original leaves (0 mismatches); a skewed total gives 2 (demand and
        // active); a census applied inside the delivery is inferred (0); the
        // modifiers set in a NewDay or a SimIdle are compared with §8.1; a
        // SimIdle that changes nothing compares nothing (-2). Any write to the
        // fake (or any call outside its getters) would abort.
        if (seam != nullptr) {
            static FakeOrigDemand od;
            InitFakeOrigDemand(od);
            cIGZMessageTarget2* agent = reinterpret_cast<cIGZMessageTarget2*>(&od.agent);
            OdPre(od);
            OdPost(od, 0.0f, 0.0f, 0.0f, 0.0f);
            CHECK(DemandShadowOnce(seam, od, agent, kMsgSimNewDay, 1.0f, 0.0f) == 0);
            CHECK(od.deliveries == 1);
            OdPre(od);
            OdPost(od, 0.0f, 0.0f, 0.0f, 0.5f);
            CHECK(DemandShadowOnce(seam, od, agent, kMsgSimNewDay, 1.0f, 0.0f) == 2);
            OdPre(od);
            OdPost(od, 5.0f, 0.0f, 0.0f, 0.0f);  // a census added 5 inside DoIdle(5)
            CHECK(DemandShadowOnce(seam, od, agent, kMsgSimNewDay, 1.0f, 0.0f) == 0);
            OdPre(od);
            OdPost(od, 0.0f, 0.25f, 0.25f, 0.0f);  // census state 0 inside DoIdle(5): §8.1 = 1.25 - 1, 0.25
            CHECK(DemandShadowOnce(seam, od, agent, kMsgSimNewDay, 1.25f, 0.25f) == 0);
            CHECK(od.deliveries == 4);
            // SimIdle: census state 0 sets 0x21010's tax 0.25 / economy 0.06.
            OdPre(od);
            for (int i = 0; i < 3; ++i) od.post[i] = od.nodes[i];
            od.post[2].tax = 0.25f;
            od.post[2].econ = 0.06f;
            CHECK(DemandShadowOnce(seam, od, agent, kMsgSimIdle, 1.25f, 0.06f) == 0);
            OdPre(od);
            od.post[2].tax = 0.3f;  // not what §8.1 gives
            CHECK(DemandShadowOnce(seam, od, agent, kMsgSimIdle, 1.25f, 0.06f) == 1);
            OdPre(od);
            for (int i = 0; i < 3; ++i) od.post[i] = od.nodes[i];
            CHECK(DemandShadowOnce(seam, od, agent, kMsgSimIdle, 1.25f, 0.06f) == -2);
            CHECK(od.deliveries == 7);
            // Not the demand simulator's agent: left to the caller, nothing delivered.
            CHECK(DemandShadowOnce(seam, od, reinterpret_cast<cIGZMessageTarget2*>(&od.other), kMsgSimNewDay, 1.0f,
                                   0.0f) == -1);
            CHECK(od.deliveries == 7);
            CHECK(od.refs == 1);  // every QueryInterface released
            CheckCw("demand shadow");
        }

        // The pollution shadow (op 16) on its neutral paths: no city, a city
        // without a pollution simulator and an agent that is not the
        // original's are left to the caller (-1, nothing delivered); the
        // original's agent gets its delivery exactly once, its record is
        // captured before and after (its own Write), and a record without the
        // grid references is reported INCOMPLETE (-2, nothing compared). The
        // month delivery ends with the month line. Any other call aborts.
        if (seam != nullptr) {
            static FakeOrigPollution op;
            InitFakeOrigPollution(op);
            cIGZMessageTarget2* agent = reinterpret_cast<cIGZMessageTarget2*>(&op.agent);
            CHECK(PollutionShadowOnce(seam, nullptr, agent, kMsgSimNewMonth) == -1);
            CHECK(PollutionShadowOnce(seam, &op.noCity, agent, kMsgSimNewMonth) == -1);
            CHECK(op.deliveries == 0 && op.writes == 0);
            CHECK(PollutionShadowOnce(seam, &op.city, agent, kMsgSimNewMonth) == -2);
            CHECK(op.deliveries == 1);
            CHECK(op.writes == 2);
            CHECK(PollutionShadowOnce(seam, &op.city, agent, kMsgSimNewDay) == -2);
            CHECK(op.deliveries == 2 && op.writes == 4);
            CHECK(PollutionShadowOnce(seam, &op.city, reinterpret_cast<cIGZMessageTarget2*>(&op.other),
                                      kMsgSimNewMonth) == -1);
            CHECK(op.deliveries == 2 && op.writes == 4);
            CHECK(op.refs == 1);  // every QueryInterface released
            CheckCw("pollution shadow");
        }

        // The power shadow (op 21) next to an "original" power simulator on
        // an empty 32 x 32 city. Clean month: the original advanced the ring
        // and asked for the recompute and the ageing, as FreeCity4's §7.5
        // does (0). Skewed month: its ring index is one further (1). The
        // neighbours step (state 2 -> 3) and the statistics step (3 -> 4) are
        // clean (0), then the original is idle: a SimIdle passes through
        // (-3) while a SimNewDay is wrapped (0). A neighbours step whose
        // update also wrote a powered value into an unwired cell: one grid
        // cell differs (1), reported in the cycle line of the statistics step
        // that follows (0). Not the original's agent, no city: left to the
        // caller (-1, nothing delivered).
        if (seam != nullptr) {
            static FakeOrigPower ow;
            InitFakeOrigPower(ow);
            cIGZMessageTarget2* agent = reinterpret_cast<cIGZMessageTarget2*>(&ow.agent);
            CHECK(PowerShadowOnce(seam, nullptr, agent, kMsgSimNewMonth, true) == -1);
            CHECK(ow.deliveries == 0 && ow.writes == 0);
            OwSet(ow, 4, FakePowerRecord{1, 1, 1, 4});
            CHECK(PowerShadowOnce(seam, &ow.city, agent, kMsgSimNewMonth, true) == 0);
            CHECK(ow.deliveries == 1 && ow.writes == 2);
            OwSet(ow, 4, FakePowerRecord{2, 1, 1, 4});
            CHECK(PowerShadowOnce(seam, &ow.city, agent, kMsgSimNewMonth, true) == 1);
            OwSet(ow, 2, FakePowerRecord{0, 0, 0, 3});
            CHECK(PowerShadowOnce(seam, &ow.city, agent, kMsgSimIdle, true) == 0);
            OwSet(ow, 3, FakePowerRecord{0, 0, 0, 4});
            CHECK(PowerShadowOnce(seam, &ow.city, agent, kMsgSimIdle, false) == 0);
            OwSet(ow, 4, FakePowerRecord{0, 0, 0, 4});
            CHECK(PowerShadowOnce(seam, &ow.city, agent, kMsgSimIdle, false) == -3);
            CHECK(ow.deliveries == 5 && ow.writes == 8);  // passed through: delivered, not captured
            CHECK(PowerShadowOnce(seam, &ow.city, agent, kMsgSimNewDay, false) == 0);
            OwSet(ow, 2, FakePowerRecord{0, 0, 0, 3});
            ow.poke = 5 * 32 + 7;
            ow.pokeValue = 2;
            CHECK(PowerShadowOnce(seam, &ow.city, agent, kMsgSimIdle, true) == 1);
            OwSet(ow, 3, FakePowerRecord{0, 0, 0, 4});
            CHECK(PowerShadowOnce(seam, &ow.city, agent, kMsgSimIdle, false) == 0);
            // The same on a shadow switched on mid-city: the original's
            // ordinance factor is not known until a month start (power.md
            // §8, PW7), so the neighbours step's difference is not exact (0).
            std::memset(ow.cells, 0, sizeof(ow.cells));
            OwSet(ow, 2, FakePowerRecord{0, 0, 0, 3});
            ow.poke = 5 * 32 + 7;
            ow.pokeValue = 2;
            CHECK(PowerShadowOnce(seam, &ow.city, agent, kMsgSimIdle, true, true) == 0);
            OwSet(ow, 3, FakePowerRecord{0, 0, 0, 4});
            CHECK(PowerShadowOnce(seam, &ow.city, agent, kMsgSimIdle, false) == 0);
            std::memset(ow.cells, 0, sizeof(ow.cells));
            CHECK(PowerShadowOnce(seam, &ow.city, reinterpret_cast<cIGZMessageTarget2*>(&ow.other), kMsgSimIdle,
                                  true) == -1);
            CHECK(ow.deliveries == 10 && ow.writes == 18);
            CHECK(ow.refs == 1);  // every QueryInterface released
            CheckCw("power shadow");
        }

        // The crime shadow (op 13) next to an "original" crime simulator: one
        // month each (SimNewMonth, then SimNewDay until its pipeline state 4).
        // Clean: the original did what FreeCity4's state 4 does (0
        // mismatches). Skewed: one tract and the count differ (2). Unchanged:
        // state 4 left everything as it was, so it was not observed (-2,
        // nothing compared). Not the original's agent: left to the caller (-1).
        if (seam != nullptr) {
            static FakeOrigCrime oc;
            InitFakeOrigCrime(oc);
            cIGZMessageTarget2* agent = reinterpret_cast<cIGZMessageTarget2*>(&oc.agent);
            OcMonth(oc, 0, 0);
            CHECK(CrimeShadowOnce(seam, oc, agent) == 0);
            CHECK(oc.deliveries == 6 && oc.committed && oc.state == 5);
            OcMonth(oc, 1, 1);
            CHECK(CrimeShadowOnce(seam, oc, agent) == 2);
            CHECK(oc.deliveries == 12);
            OcMonth(oc, 0, 0, true);
            CHECK(CrimeShadowOnce(seam, oc, agent) == -2);
            CHECK(oc.deliveries == 18);
            OcMonth(oc, 0, 0);
            CHECK(CrimeShadowOnce(seam, oc, reinterpret_cast<cIGZMessageTarget2*>(&oc.other)) == -1);
            CHECK(oc.deliveries == 25);  // all seven delivered by the caller
            CHECK(oc.refs == 1);         // every QueryInterface released
            CheckCw("crime shadow");
        }

        // The plumbing shadow (op 22) next to an "original" plumbing simulator:
        // runs of 25 deliveries, a fresh shadow each. No city, a city without a
        // plumbing simulator, or an agent that is not the original's: left to
        // the caller (-1), every delivery made once. Clean: two passes, each
        // seen from its start (the passCounter moved) and exact, and a clean
        // month (0). One published cell wrong in the second pass (1); one
        // scalar (the watered buildings) wrong in both passes (2). Made
        // mid-city: clean, the month's distressed count not exact (0).
        if (seam != nullptr) {
            static FakeOrigPlumbing opb;
            InitFakeOrigPlumbing(opb);
            cIGZMessageTarget2* agent = reinterpret_cast<cIGZMessageTarget2*>(&opb.agent);
            OpbReset(opb, false, 0);
            CHECK(PlumbingShadowRun(seam, nullptr, agent) == -1);
            CHECK(PlumbingShadowRun(seam, &opb.noCity, agent) == -1);
            OpbReset(opb, false, 0);
            CHECK(PlumbingShadowRun(seam, &opb.city, reinterpret_cast<cIGZMessageTarget2*>(&opb.other)) == -1);
            CHECK(opb.deliveries == 75 && opb.writes == 0);
            OpbReset(opb, false, 0);
            const int writesBefore = opb.writes;
            CHECK(PlumbingShadowRun(seam, &opb.city, agent) == 0);
            CHECK(opb.deliveries == 100);
            CHECK(opb.pass == 2 && opb.state == 9);
            CHECK(opb.writes > writesBefore);
            OpbReset(opb, true, 0);
            CHECK(PlumbingShadowRun(seam, &opb.city, agent) == 1);
            OpbReset(opb, false, 3);
            CHECK(PlumbingShadowRun(seam, &opb.city, agent) == 2);
            CHECK(opb.deliveries == 150);
            OpbReset(opb, false, 0);
            CHECK(PlumbingShadowRun(seam, &opb.city, agent, true) == 0);
            CHECK(opb.deliveries == 175);
            CHECK(opb.refs == 1);  // every QueryInterface released
            CheckCw("plumbing shadow");
        }

        // The reference audit (run 42, ForEachExemplarKey): freeing a
        // temporary the game created for us (acquired, then
        // FC4_RELEASE_CREATED) is not a warning; the same last-reference
        // release through FC4_RELEASE still is (a release of a reference the
        // audited sites never acquired still warns as an over-release; not
        // exercised here: this log must stay free of over-releases).
        if (seam != nullptr) {
            static FakeVtable tempVt;
            tempVt.slots[0] = reinterpret_cast<void*>(&TempQi);
            tempVt.slots[2] = reinterpret_cast<void*>(&TempRelease);
            static FakeObject temps[2] = {{tempVt.slots.data()}, {tempVt.slots.data()}};
            CHECK(seam(kFC4SeamRefAuditRelease, &temps[0], reinterpret_cast<void*>(intptr_t{3})) == 0);
            CHECK(seam(kFC4SeamRefAuditRelease, &temps[1], reinterpret_cast<void*>(intptr_t{1})) == 1);
            // The growth developer's special-building copy released in its
            // destructor (growth §1.3, DG28; run 102's five warnings): the
            // last reference of an occupant the load's object table already
            // dropped is the destructor's to free, as the original's is.
            static FakeObject special{tempVt.slots.data()};
            CHECK(seam(kFC4SeamRefAuditRelease, &special, reinterpret_cast<void*>(intptr_t{5})) == 0);

            // The controller manager's destruction (run sg4-c1, 238
            // warnings): the last reference of a controller the manager
            // acquired is the manager's to free at its end (automata 3.4,
            // 7.1, foundations 2.7 SV1); a release outside the destruction
            // that reaches 0 (the replaced clock reference) still warns.
            static FakeVtable countedVt;
            countedVt.slots[0] = reinterpret_cast<void*>(&TempQi);
            countedVt.slots[1] = reinterpret_cast<void*>(&CountedAddRef);
            countedVt.slots[2] = reinterpret_cast<void*>(&CountedRelease);
            static CountedFake held{countedVt.slots.data(), 0};
            static CountedFake held2{countedVt.slots.data(), 0};
            static CountedFake replaced{countedVt.slots.data(), 0};
            CHECK(seam(kFC4SeamAutomataCmTeardown, &held, nullptr) == 0);
            CHECK(held.refs == 0);  // released, not leaked
            CHECK(seam(kFC4SeamAutomataCmTeardown, &held2, &replaced) == 1);
            CHECK(held2.refs == 0 && replaced.refs == 0);
            // Still held by the load's table (a city not saved since): no warning either way.
            held.refs = 1;
            CHECK(seam(kFC4SeamAutomataCmTeardown, &held, nullptr) == 0);
            CHECK(held.refs == 1);
        }

        // The monthly digest (observer), read through the city's public
        // interfaces: here a fake city with our budget simulator.
        cISC4BudgetSimulator* budget3 = nullptr;
        CHECK(director->GetClassObject(kCLSID_Budget, kIID_cISC4BudgetSimulator, reinterpret_cast<void**>(&budget3)));
        if (seam != nullptr && budget3 != nullptr) {
            CHECK(budget3->Init());
            // The power glue's department / line-item path on our budget
            // classes, with a department reference read back from a save
            // (run 21: the token was the department's cIGZSerializable).
            CHECK(seam(kFC4SeamPowerBudgetPath, budget3, nullptr) == 0);
            static FakeWorld world;
            InitFakeWorld(world);
            world.budget = budget3;
            seam(kFC4SeamObserverCity, &world.city, nullptr);
            seam(kFC4SeamObserverMessage, reinterpret_cast<void*>(static_cast<uintptr_t>(kMsgPostCityInit)), nullptr);
            seam(kFC4SeamObserverMessage, reinterpret_cast<void*>(static_cast<uintptr_t>(kMsgSimNewMonth)),
                 reinterpret_cast<void*>(static_cast<intptr_t>(2451576)));
            seam(kFC4SeamObserverCity, nullptr, nullptr);
            CHECK(world.demandCalls == 28);  // digest and observe: 12 groups + 2 census nodes each
            CHECK(world.demandBadIndex == 0);
            CheckCw("power budget path, observer month, our budget");

            // glue-exemplars4 (foundations §2.7, §3.7): a grid reference read
            // in a Read is not touched until the store load ends (PostLoad /
            // PreCityInit). Address 0x10 is unmapped: any grid call would fault.
            CHECK(seam(kFC4SeamUnreadRefs, reinterpret_cast<void*>(0x10), nullptr) == 31);
            DemolitionBuildingInterfaceChecks(seam);
            MySimsActiveOwnershipChecks(seam);
            PowerLineConnectionChecks(seam);
            DemolitionFilterChecks(seam);
            DemolitionPreDemolishChecks(seam);

            // The always-on guard: the reference as PreAppInit takes it, a
            // quiet tick, then a 53-bit main thread is reported once.
            // (FpuTick alone: a full observer tick would start the watchdog,
            // whose thread pins the DLL.)
            void* const fpuTick = reinterpret_cast<void*>(1);
            seam(kFC4SeamFpuReference, nullptr, nullptr);
            seam(kFC4SeamFpuReference, fpuTick, nullptr);
            unsigned int unused = 0;
            _controlfp_s(&unused, _PC_53, _MCW_PC);
            seam(kFC4SeamFpuReference, fpuTick, nullptr);
            seam(kFC4SeamFpuReference, fpuTick, nullptr);
            SetGamePrecision();
            CHECK(budget3->Shutdown());
        }
        if (budget3 != nullptr) CHECK(budget3->Release() == 0);
        if (reader != INVALID_HANDLE_VALUE) CloseHandle(reader);
    }

    // The same faulting address three times: reported at most twice.
    CHECK(ForeignFault() == 1);
    CHECK(ForeignFault() == 1);
    CHECK(ForeignFault() == 1);
    CHECK(ForeignFaultX87() == 1);
    const std::string text = ReadFile(log);
    // The harness switches are off by default: no turbo, no end date, no script (FC4Turbo.h).
    CHECK(text.find("turbo") == std::string::npos);
    CHECK(text.find("end date") == std::string::npos);
    CHECK(text.find("script:") == std::string::npos);
    CHECK(CountOf(text, "first-chance exception OUTSIDE FreeCity4.dll in fc4_plugin_load_test.exe+0x") == 3);
    CHECK(text.find("(report 2 of at most 2 for this address)") != std::string::npos);
    // ... with the x87 state at the fault (an empty stack here) and the raw
    // dwords at esp (FC4Diag.cpp, FC4DiagContext.h).
    CHECK(text.find("  fpu: CW=0x") != std::string::npos);
    CHECK(text.find(") TW=0x") != std::string::npos);
    CHECK(text.find("[FXSAVE FTW=0x00 occupied=0] indefinite NaN: ") != std::string::npos);
    // ForeignFaultX87: 1.0 in ST0 (physical register 7). Under WOW64 the
    // context's TagWord is the full tag (0x3FFF), not the abridged byte.
    CHECK(text.find("(TOP=7 SF=0 IE=0 C1=0) TW=0x3FFF occupied=1 [FXSAVE FTW=0x80 occupied=1] indefinite NaN: none") !=
          std::string::npos);
    CHECK(text.find("  esp dwords: 0x") != std::string::npos);
    // The x87 leftover guard (test seam op 31): the first finding at a site
    // this game month is logged, the second only counted; the heartbeat
    // carries the total.
    CHECK(CountOf(text, "x87 leftovers at 'test seam': 2 register(s) occupied (FSW=0x") == 1);
    CHECK(text.find("previous check 'test seam' had 0; game month ") != std::string::npos);
    CHECK(text.find("| x87 leftovers: ") != std::string::npos);
    CHECK(text.find("x87 leftovers at 'test seam'") == text.find("x87 leftovers at '"));  // no other site
    // Watchdog (hang diagnostics).
    CHECK(text.find("watchdog: started for main thread") != std::string::npos);
    CHECK(text.find("WATCHDOG: main thread (tid ") != std::string::npos);
    CHECK(text.find("stalled - inside FreeCity4 for ") != std::string::npos);
    CHECK(text.find("  activity: OnTick (") != std::string::npos);
    CHECK(text.find("> core fc4_sim_on_tick (") != std::string::npos);
    CHECK(text.find("| delivering msg 0x66956814 (level 4) to agent ") != std::string::npos);
    CHECK(text.find("'blocking test agent' for ") != std::string::npos);
    CHECK(text.find("  context: eip=0x") != std::string::npos);
    CHECK(text.find("] FreeCity4.dll+0x") != std::string::npos);  // a return address into our image
    CHECK(text.find("] fc4_plugin_load_test.exe+0x") != std::string::npos);  // ... and into the executable
    CHECK(text.find("  cpu: process ") != std::string::npos);
    CHECK(text.find("WATCHDOG: stall over after ") != std::string::npos);
    CHECK(text.find("heartbeat (periodic): 2000-01-") != std::string::npos);
    CHECK(text.find("watchdog: stopped") != std::string::npos);
    CHECK(text.find("SetSimSpeed(2) -> 1 (julian 2451545") != std::string::npos);
    CHECK(text.find("HiddenResume() -> 1 (pause=0 hidden=0 emergency=0; julian 2451545 speed 2)") != std::string::npos);
    CHECK(text.find("first-chance exception OUTSIDE FreeCity4.dll in fc4_plugin_load_test.exe+0x") != std::string::npos);
    CHECK(text.find("FreeCity4 interface calls (oldest first):") != std::string::npos);
    CHECK(text.find("] BudgetSimulator::Shutdown") != std::string::npos);
    CHECK(text.find("call BudgetSimulator::GetTaxIncomeOfType (#1)") != std::string::npos);
    // The budget shadow (test seam op 10): a clean month, then type 3 attributed to the not-livable lot.
    CHECK(text.find("budget shadow 2000-02-01: tax orig/ours=750000/750000,0/0,0/0,1500000/1500000,0/0,0/0,"
                    "2250000/2250000,0/0,0/0,0/0,0/0,0/0; total orig/ours=4500000/4500000; income orig/ours=4500000/-; "
                    "expense orig/ours=0/-; net orig/ours=4500000/-; lots=5; mismatches=0; months=1 clean=1; "
                    "departments unreadable; tuning: code defaults") != std::string::npos);
    CHECK(text.find("budget shadow 2000-02-01: tax orig/ours=750000/750000,0/0,0/0,1875000/1500000!,") !=
          std::string::npos);
    CHECK(text.find("; lots=5; mismatches=1 [tax 3]; months=2 clean=1;") != std::string::npos);
    CHECK(text.find("budget shadow 2000-02-01 type 3: assessed orig/ours=2500/2000 (orig-ours=500); "
                    "tax orig/ours=1875000/1500000; rate=9.00 mult=100000.000; ours: counted=1 (building wealth != "
                    "lot wealth: 0), not-livable=1 (assessed 500)") != std::string::npos);
    CHECK(text.find("budget shadow 2000-02-01 type 3 lot (2,3) 1x1: not-livable purpose=2 wealth b1/l1 value=500 lv=0 "
                    "assessed=500 tax=0.000 [assessed = |orig-ours|]") != std::string::npos);
    CHECK(text.find("budget shadow 2000-02-01 type 3 lot (1,2) 2x1: counted purpose=2 wealth b1/l1 value=2000 lv=0 "
                    "assessed=2000 tax=1500000.000") != std::string::npos);
    CHECK(CountOf(text, "budget shadow 2000-02-01 type ") == 3);  // one type line, two lot lines
    CHECK(text.find("budget shadow: the original budget ") != std::string::npos);
    CHECK(text.find("budget shadow: ON") == std::string::npos);  // off by default
    // The pollution shadow (test seam op 16): two incomplete deliveries, one month line.
    CHECK(text.find("pollution shadow: the original pollution simulator ") != std::string::npos);
    CHECK(text.find("pollution shadow: comparing with the original pollution simulator ") != std::string::npos);
    CHECK(text.find("pollution shadow 2000-02-01 month: INCOMPLETE (before: the record has no grid references); "
                    "not compared") != std::string::npos);
    CHECK(text.find("pollution shadow 2000-02-01 day: INCOMPLETE (before: the record has no grid references); "
                    "not compared") != std::string::npos);
    CHECK(text.find("pollution shadow month to 2000-02-01: deliveries=1 (days 0, months 1) clean=0 incomplete=1 "
                    "reruns=0 uncached=0; cells=0 mismatches=0; airSrc 0/0 max|d|=0, waterSrc 0/0 max|d|=0,") !=
          std::string::npos);
    CHECK(text.find("; first differing cells: none; fields: none; props compared=0 mismatched=0; deals compared=0 "
                    "mismatched=0; incomplete because: before: the record has no grid references x1") !=
          std::string::npos);
    CHECK(CountOf(text, "pollution shadow month to ") == 1);
    CHECK(text.find("pollution shadow: ON") == std::string::npos);  // off by default
    // The power shadow (test seam op 21): two months (clean, ring index
    // skewed), three cycles (clean; one grid cell off; the same mid-city, not
    // exact).
    CHECK(text.find("power shadow: the original power simulator ") != std::string::npos);
    CHECK(text.find("power shadow 2000-02-01 month: 1024 cells compared; production=0/0 consumption=0/0 import=0/0 "
                    "export=0/0 demand=0/0 requirement=0/0 consumers=0/0 powered=0/0 unpowered=0/0 plants=0/0 "
                    "aged=0/0 distressed=0/0 overworked=0/0 deficit=0/0 poles=0/0 broken=0/0; brokenFraction from 0 "
                    "at the city start, as the original's (power.md §7.6; first step 2000-02-01); not_exact=0; "
                    "mismatches=0") != std::string::npos);
    CHECK(text.find("broken=0/0; fields orig/ours: ringIndex=2/1; brokenFraction from 0 at the city start, as the "
                    "original's (power.md §7.6; first step 2000-02-01); not_exact=0; mismatches=1") !=
          std::string::npos);
    CHECK(text.find("power shadow: comparing with the original power simulator ") != std::string::npos);
    CHECK(text.find(") from the city start (its unsaved values known: ") != std::string::npos);
    CHECK(text.find(") from mid-city (not known: the ordinance factor until the first month") != std::string::npos);
    CHECK(text.find("NOT EXACT: the ordinance factor is not known until the first month (the shadow started "
                    "mid-city; power.md §8)") != std::string::npos);
    CHECK(CountOf(text, "power shadow 2000-02-01 bookkeeping: ") == 2);
    CHECK(CountOf(text, "consumption map 0/1024 cells differ max|d|=0; wire 0 cells differ;") == 2);
    // Each a new shadow's first line: the bookkeeping baseline is the first
    // line equal to the one before it (power.md §5.2 case 2 at the first).
    // The wire bits are listed and not counted (power.md §5.3, PW5).
    CHECK(CountOf(text, "wire 0 cells differ; baseline not set yet (the first line since the city start; the "
                        "baseline is the first line equal to the one before it, nothing counted until then): 0 "
                        "differences; wire bits, not counted (the original's running bitmap after its "
                        "dirty-rectangle rebuilds vs one rebuild of the whole map, power.md §5.3: PW5, unclipped "
                        "bits, no insertion filters; each rebuild is compared on the cycle lines) 0; "
                        "mismatches=0") == 2);
    CHECK(CountOf(text, "power shadow month to 2000-02-01: deliveries=1 steps=0 idle passed through=0 not "
                        "enabled=0 cycles=0") == 2);
    CHECK(text.find("power shadow 2000-02-01 cycle: steps 2 3 (cycle start not observed) on 32x32 cells; grid 2048 "
                    "cells compared, max|d|=0; first differing cells: none;") != std::string::npos);
    CHECK(CountOf(text, "power shadow 2000-02-01 cycle: ") == 3);
    CHECK(text.find("power shadow: ON") == std::string::npos);  // off by default
    // The crime shadow (test seam op 13): a clean month, a skewed one, an
    // unobserved state 4; four runs, each a new "original".
    CHECK(CountOf(text, "crime shadow: tracking the original crime simulator ") == 4);
    CHECK(text.find("crime shadow: FreeCity4's crime model: 3 crime types, tuning from its exemplar") !=
          std::string::npos);
    CHECK(text.find("crime shadow 2000-02-05: state 4 compared on 4x4 tracts (S=4; decay, hotspot, stamps of the "
                    "original's incidents, count, average): cells mismatched=0 max|orig-ours|=0; hotspot "
                    "orig/ours=(0,0)=60/(0,0)=60 (before: (4,4)=100, replayed from read); count orig/ours=3/3; "
                    "average orig/ours=21/21; incidents=4 (severity sum 140, no lot at the cell 1, unknown types 0, ") !=
          std::string::npos);
    CHECK(text.find("; mismatches=0; months=1 clean=1; not compared: crimes to commit") != std::string::npos);
    CHECK(text.find("crime shadow 2000-02-05: state 4 compared on 4x4 tracts (S=4; decay, hotspot, stamps of the "
                    "original's incidents, count, average): cells mismatched=1 max|orig-ours|=1 [cells (x,z) "
                    "orig/ours: (2,3) 44/43 pre 7]; hotspot orig/ours=(0,0)=60/(0,0)=60 (before: (4,4)=100, "
                    "replayed from read); count orig/ours=4/3!; average orig/ours=21/21;") != std::string::npos);
    CHECK(text.find("; mismatches=2; months=2 clean=1; not compared: crimes to commit") != std::string::npos);
    CHECK(text.find("crime shadow 2000-02-05: state 4 NOT OBSERVED in this delivery") != std::string::npos);
    CHECK(text.find("(unobserved=1)") != std::string::npos);
    CHECK(text.find("crime shadow 2000-02-05: INCOMPLETE") == std::string::npos);
    CHECK(text.find("crime shadow: ON") == std::string::npos);  // off by default
    CHECK(text.find("crime shadow: requested") == std::string::npos);
    // The plumbing shadow (test seam op 22): four runs taken (clean, one
    // wrong cell, one wrong scalar, clean made mid-city), each two passes
    // seen from their start (the passCounter, plumbing.md §6.1 PS6) and
    // exact, and a month. The fake's record names no department: the funding
    // is the department found by its ID (PS5), noted on the month line.
    CHECK(CountOf(text, "plumbing shadow: comparing with the original plumbing simulator ") == 4);
    CHECK(CountOf(text, "; made at the city start (the original's pipe condition is known: 0)") == 3);
    CHECK(CountOf(text, "; made mid-city (the original's pipe condition is not known: the distressed count is not "
                        "exact)") == 1);
    CHECK(text.find("plumbing shadow 2000-02-01 pass 1: aligned=start exact=1; grid 4x4 cells=16 differing=0 "
                    "max|diff|=0; producers 0; ") != std::string::npos);
    CHECK(text.find("plumbing shadow 2000-02-01 pass 1: aligned=swap") == std::string::npos);
    CHECK(text.find("plumbing shadow 2000-02-01 month: pipes=0 distressedBefore=0 burstBefore=0; distressed "
                    "orig/ours=0/0; burst orig/ours=+0/+0; pipeCondition ours=0.0000 (orig not readable; its "
                    "distressed count implies 0 (no pipes)); funding from department 0xaa4014b4 by ID (the record "
                    "names no department); funding=1.000; mismatches=0; months=1 clean=1; passes since the last "
                    "month line: seen=1 compared=1 clean=1 notExact=0") != std::string::npos);
    CHECK(text.find("plumbing shadow 2000-02-01 month: pipes=0 distressedBefore=0 burstBefore=0; distressed "
                    "orig/ours=0/0 burst +0 (ours allows 0..0) (not exact: the original's pipe condition is not "
                    "known, the shadow started mid-city; plumbing.md §7.1 PS3); ") != std::string::npos);
    CHECK(CountOf(text, "plumbing shadow 2000-02-01 month: ") == 4);
    CHECK(CountOf(text, "; funding=1.000; mismatches=0; months=1 clean=1; passes since the last month line: ") == 4);
    CHECK(text.find("plumbing shadow 2000-02-01 pass 2: aligned=start exact=1; grid 4x4 cells=16 differing=0 "
                    "max|diff|=0; ") != std::string::npos);
    CHECK(text.find("; produced orig/ours=0/0; consumed orig/ours=0/0; imported orig/ours=0/0; exported "
                    "orig/ours=0/0; wateredCells orig/ours=0/0; unwateredCells orig/ours=0/0; cost orig/ours=0/0; "
                    "demand orig/ours=0/0; pipes orig/ours=0/0; distressed orig/ours=0/0; burst orig/ours=0/0; "
                    "sources orig/ours=0/0; maintenance compared=0 differing=0; ageing=yes; condition orig/ours=[]; "
                    "deliveries=5 days=0; mismatches=0; passes compared=2 clean=2") != std::string::npos);
    CHECK(text.find("plumbing shadow 2000-02-01 pass 2: aligned=start exact=1; grid 4x4 cells=16 differing=1 "
                    "max|diff|=1 first (x,z) orig/ours: (2,2) 1/0; ") != std::string::npos);
    CHECK(CountOf(text, "wateredCells orig/ours=3/0!; ") == 2);
    CHECK(CountOf(text, "; ageing=no; deliveries=5 days=0; mismatches=1; passes compared=1 clean=0") == 1);
    CHECK(CountOf(text, "; mismatches=1; passes compared=2 clean=1") == 1);  // the wrong cell
    CHECK(CountOf(text, "; mismatches=1; passes compared=2 clean=0") == 1);  // the wrong scalar, again
    // Made mid-city around no month: whether the first state 8 aged shows in
    // the producers' condition (none here: not aged; PS7).
    CHECK(text.find("; ageing=no (inferred: no producer's condition changed; plumbing.md §6.7, PS7); deliveries=5 "
                    "days=0; mismatches=0; passes compared=1 clean=1") != std::string::npos);
    CHECK(text.find("plumbing shadow: ON (plumbing_shadow from FC4_PLUMBING_SHADOW): at each SimIdle and SimNewDay "
                    "of the ORIGINAL plumbing simulator (one pass state each), FreeCity4's water pass replayed") !=
          std::string::npos);
    CHECK(text.find("differing cells listed per line: 8; passes replayed per game day: 4 (0 = all). The record is "
                    "read through the original's own Write into memory. Not compared (not readable read-only): pipe "
                    "watered states") != std::string::npos);
    CHECK(text.find("plumbing shadow: requested") == std::string::npos);
    CHECK(CountOf(text, "plumbing shadow 2000-02-01 pass ") == 8);  // no INCOMPLETE pass line
    CHECK(text.find("DLL_PROCESS_ATTACH") != std::string::npos);
    CHECK(text.find("cFC4Director constructed") != std::string::npos);
    CHECK(text.find("EnumClassObjects: reporting clsid 0x2990C1E5") != std::string::npos);
    CHECK(text.find("subsystem 'landvalue': off by default") != std::string::npos);
    CHECK(text.find("cFC4LandValueSimulator(") == std::string::npos);
    CHECK(text.find("subsystem 'weather': off by default") != std::string::npos);
    CHECK(text.find("cFC4WeatherSimulator(") == std::string::npos);
    CHECK(text.find("subsystem 'ordinance': off by default") != std::string::npos);
    CHECK(text.find("cFC4OrdinanceSimulator(") == std::string::npos);
    CHECK(text.find("subsystem 'demand': off by default") != std::string::npos);
    CHECK(text.find("cFC4DemandSimulator(") == std::string::npos);
    CHECK(text.find("subsystem 'aura': off by default") != std::string::npos);
    CHECK(text.find("cFC4AuraSimulator(") == std::string::npos);
    CHECK(text.find("aura shadow: ON") == std::string::npos);  // off by default
    CHECK(text.find("aura shadow: requested") == std::string::npos);
    CHECK(text.find("landvalue shadow") == std::string::npos);     // off by default: not a word
    CHECK(text.find("flammability shadow") == std::string::npos);  // off by default: not a word
    CHECK(text.find("GetClassObject enter (clsid=0x2990C1E5, iid=0x8695664E") != std::string::npos);
    CHECK(text.find("GetClassObject exit (clsid=0x2990C1E5, iid=0x8695664E) -> 1") != std::string::npos);
    CHECK(text.find("GetClassObject exit (clsid=0x2990C1E5, iid=0x0BADF00D) -> 0") != std::string::npos);
    CHECK(text.find("GetClassObject exit (clsid=0x12345678, iid=0x8695664E) -> 0") != std::string::npos);
    CHECK(text.find("factory enter: cSC4Simulator (0x2990C1E5), factory call #1") != std::string::npos);
    CHECK(text.find("constructor enter") != std::string::npos);
    CHECK(text.find("calling fc4_sim_create") != std::string::npos);
    CHECK(text.find("fc4_sim_create returned") != std::string::npos);
    CHECK(text.find("cSC4Simulator object created") != std::string::npos);
    CHECK(text.find("CreateOurs: after QueryInterface -> 1 (clean=1)") != std::string::npos);
    CHECK(text.find("vectored handler installed") != std::string::npos);
    CHECK(text.find("first-chance exception in FreeCity4.dll: code 0xC0000005 ACCESS_VIOLATION (writing address "
                    "0x00000000) at") != std::string::npos);
    CHECK(text.find("STRUCTURED EXCEPTION caught in diagnostics self-test: code 0xC0000005") != std::string::npos);
    CHECK(text.find("diagnostics self-test: guard returned 0 (expected 0)") != std::string::npos);
    CHECK(text.find("could not be written") == std::string::npos);
    CHECK(text.find("AddAgent(") != std::string::npos);
    CHECK(text.find("deliver #1: level 4 type 0x66956814 d1=2451546 d2=0 d3=2 -> agent") != std::string::npos);
    CHECK(text.find("'test day agent' returned 0") != std::string::npos);
    CHECK(text.find("'test day agent' (level 4) has now returned false 100 times in a row") != std::string::npos);
    CHECK(text.find("status @tick 5:") != std::string::npos);
    CHECK(text.find("DAY_GATE_OPEN=") != std::string::npos);
    CHECK(text.find("  level 4: 1 agent(s), cursor") != std::string::npos);
    CHECK(text.find("post SimNewDay 0x66956814 d1=2451546 (2000-01-02) d2=0 d3=2 -> MessagePost skipped") !=
          std::string::npos);
    CHECK(text.find("post 0x4A7FB7E9 d1=2") != std::string::npos);  // SpeedChanged
    CHECK(text.find("GetSimDate() #1 -> date object") != std::string::npos);
    CHECK(text.find("cFC4Date(") != std::string::npos); // capped trace; behavior checked above
    CHECK(text.find("GetSimDateNumber() #1 -> 2451545") != std::string::npos);
    // Limit-timer calibration (spec 5.5): installed and published, but this test
    // process is not SimCity 4, so the compat guard must refuse to write.
    CHECK(text.find("calibration: QPC ") != std::string::npos);
    CHECK(text.find("installed=1") != std::string::npos);
    CHECK(text.find("compat: host executable ") != std::string::npos);
    CHECK(text.find("compat: WARNING unrecognised executable") != std::string::npos);
    CHECK(text.find("written to the game=yes") == std::string::npos);
    CHECK(text.find("calibration (Init): installed") != std::string::npos);
    CHECK(text.find("calibration (status): installed") != std::string::npos);
    CHECK(text.find("QueryInterface(0x8695664E) -> 1") != std::string::npos);
    CHECK(text.find("Init() called") != std::string::npos);
    CHECK(text.find("OnTick #1 ") != std::string::npos);
    CHECK(text.find("HiddenResume() -> 1 (pause=0 hidden=0") != std::string::npos);
    CHECK(text.find("cFC4Simulator destroyed") != std::string::npos);
    CHECK(text.find("subsystem 'budget': enabled (3 classes replaced)") != std::string::npos);
    CHECK(text.find("cSC4BudgetSimulator object created") != std::string::npos);
    CHECK(text.find("budget: tuning exemplar unavailable; using the code defaults") != std::string::npos);
    CHECK(text.find("budget: no simulator; agents not registered") != std::string::npos);
    CHECK(text.find("budget: game-data registry node 'budget' not published (stub)") != std::string::npos);
    CHECK(text.find("budget Init: ok=1") != std::string::npos);
    CHECK(text.find("budget post 0x0772FAD4") != std::string::npos);  // FundsChanged (skipped: no server)
    CHECK(text.find("budget IssueBond(10000) -> 1 (borrowed 10000)") != std::string::npos);
    CHECK(text.find("budget Shutdown") != std::string::npos);
    CHECK(text.find("cFC4BudgetSimulator destroyed") != std::string::npos);

    // Budget §10: the verbose post line shows what receivers read.
    CHECK(text.find("test seam: budget post sink installed") != std::string::npos);
    CHECK(text.find("budget post 0x0772FAD4 d1=0x") != std::string::npos);
    CHECK(text.find("not posted (no message server; test sink only)") != std::string::npos);
    CHECK(text.find(" void1=") != std::string::npos && text.find(" unknown=") != std::string::npos);
    // Observer (replacement mode) and the monthly digest.
    CHECK(text.find("autotest: off") != std::string::npos);
    CHECK(text.find("observer: PostCityInit (replacement)") != std::string::npos);
    CHECK(text.find("DIGEST julian=2451576 (2000-02-01) funds=500000 population R/C/I=1234/480/160 demand "
                    "R$/R$$/R$$$/Cs$/Cs$$/Cs$$$/Co$$/Co$$$/I-R/I-D/I-M/I-HT=250.5/-/-/-12.0/-/-/40.0/-/7.5/-/-/0.0 "
                    "budget income/expense=") != std::string::npos);
    CHECK(text.find("power budget path test: passed") != std::string::npos);
    // The x87 control word: logged at attach, the reference, a change seen by
    // a tick, and the guard's two warnings, each once.
    CHECK(text.find("fpu DLL attach: x87 CW=0x") != std::string::npos);
    CHECK(text.find("(PC 24-bit, RC nearest, masks 0x3F) MXCSR=0x") != std::string::npos);
    CHECK(text.find("[thread ") != std::string::npos && text.find(", main] = the reference for the guard") != std::string::npos);
    CHECK(text.find(": control word changed from 0x") != std::string::npos);
    CHECK(CountOf(text, "fpu: WARNING the main thread's control word changed (tick ") == 1);
    CHECK(CountOf(text, "fpu: WARNING the main thread is not at 24-bit x87 precision (tick ") == 1);
    CHECK(text.find("(PC 53-bit") != std::string::npos);
    CHECK(text.find("power budget path test: step") == std::string::npos);
    // The observe line (our budget; no game allocator here, so no departments).
    CHECK(text.find("observer: digest on, observe on") != std::string::npos);
    CHECK(text.find("observe month 2000-02: date=2000-02-01 julian=2451576 funds=500000 income=") != std::string::npos);
    CHECK(text.find(" dept_expense=- demand R$/R$$/R$$$/Cs$/Cs$$/Cs$$$/Co$$/Co$$$/I-R/I-D/I-M/I-HT="
                    "250.5/-/-/-12.0/-/-/40.0/-/7.5/-/-/0.0 population R/C/I=1234/480/160 power=- "
                    "residential EQ/HQ/LE=12.5000/34.2500/75\r\n") != std::string::npos);
    CHECK(text.find("[INCOMPLETE") == std::string::npos);
    // Bisection configuration.
    CHECK(text.find("config (game start): all 219 budget switches ON") != std::string::npos);
    CHECK(text.find("config (city load: budget Init): all 219 budget switches ON") != std::string::npos);
    CHECK(text.find("set by ini: property_writes=0, cheats=1, holder_refs=0; set by env: disaster=0") !=
          std::string::npos);
    CHECK(text.find("config (city load: budget Init): budget switches OFF (3 of 219): property_writes, disaster, "
                    "holder_refs") != std::string::npos);
    CHECK(text.find("switch 'holder_refs' off: AddRef of a game object for the budget core skipped") !=
          std::string::npos);
    CHECK(text.find("switch 'property_writes' off: AddPropertyVariant (reported as written) skipped") !=
          std::string::npos);
    CHECK(text.find("switches (budget Shutdown): calls skipped since the last report: property_writes ") !=
          std::string::npos);
    CHECK(text.find("budget core (before budget Shutdown): holds 0 reference(s)") != std::string::npos);
    // Reference-count audit: the fake holder and the test agents are "game" objects.
    CHECK(text.find("refaudit: on (verbose line cap 2000") != std::string::npos);
    CHECK(text.find("ref +1 AddRef ") != std::string::npos);
    CHECK(text.find("ref -1 Release ") != std::string::npos);
    CHECK(text.find("[class ?, vtable fc4_plugin_load_test.exe+0x") != std::string::npos);
    CHECK(text.find("at fc4::`anonymous-namespace'::AddRefToken:") != std::string::npos);
    CHECK(text.find("refaudit (budget Shutdown): ") != std::string::npos);
    CHECK(text.find("refaudit (simulator Shutdown): ") != std::string::npos);
    CHECK(text.find("objects we still hold references to: 0") != std::string::npos);
    CHECK(text.find("WARNING: released last reference to game object ") != std::string::npos);  // test agents
    CHECK(text.find("WARNING: over-release") == std::string::npos);
    // Checked build: RTC failures and out-parameter overruns reach the log.
    if (mode == "--checked") {
        CHECK(text.find("build: CHECKED (/Od /RTC1") != std::string::npos);
        CHECK(text.find("RTC ERROR #1 (report 1 for this call site): ESP not preserved across a call") !=
              std::string::npos);
        CHECK(text.find("  call site: FreeCity4.dll+0x") != std::string::npos);
        CHECK(text.find("ProvokeEspMismatch") != std::string::npos);  // our function name (FreeCity4.pdb)
        CHECK(text.find("OUT-PARAM OVERRUN #1: the callee wrote outside the 1-byte self-test uint8 out-parameter") !=
              std::string::npos);
        CHECK(text.find("(canary C5: before C5C5C5C5C5C5C5C5C5C5C5C5C5C5C5C5 | after 332211C5") != std::string::npos);
    } else {
        CHECK(text.find("build: normal") != std::string::npos);
        CHECK(text.find("FC4_CHECKED_SELFTEST ignored: this is not a checked build") != std::string::npos);
        CHECK(text.find("RTC ERROR") == std::string::npos);
    }
    CHECK(text.find("OUT-PARAM OVERRUN #2") == std::string::npos);

    CheckCw("the whole normal run");
    FreeLibrary(h);
    CheckCw("FreeLibrary");
    CHECK(ReadFile(log).find("DLL_PROCESS_DETACH (FreeLibrary)") != std::string::npos);

    if (g_failures == 0) std::printf("plugin load test: all checks passed (log: %s)\n", log.c_str());
    return g_failures == 0 ? 0 : 1;
}
