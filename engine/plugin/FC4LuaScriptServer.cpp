// SPDX-License-Identifier: GPL-3.0-or-later
// Bounded native boundary for lua-host.md 1.4.1 / 6.3 (LH1/LH2).
// This owner is intentionally NOT installed or registered as a class. Insertion
// timing is unproved and the execution/list boundary is incomplete. An enabled
// diagnostic must never turn a partial adapter into the city's scheduler.
#include "FC4LuaScriptServerBridge.h"
#include "FC4Subsystems.h"
#include "FC4Log.h"
#include "FC4RefAudit.h"
#include "cIGZCOM.h"
#include "cIGZCommandGenerator.h"
#include "cIGZCommandServer.h"
#include "cIGZFrameWork.h"
#include "cIGZLua5.h"
#include "cIGZLuaScriptServer.h"
#include "cIGZSystemService.h"
#include "cRZBaseString.h"
#include "cRZCOMDllDirector.h"
#include <atomic>
#include <cstddef>
#include <new>
#include <string_view>

namespace {
constexpr uint32_t kClass = 0xCBD101A9, kService = 0x4BD10133, kIID = 0x4BD10129;
constexpr int32_t kPriority = -999999;

// LH2: separate from cIGZCommandGenerator, whose slot 3 has a different ABI.
struct LuaGenerator : cIGZUnknown {
    virtual cIGZCommandGenerator* AsIGZCommandGenerator() = 0; // borrowed
    virtual bool Init(cIGZCommandServer*, cIGZLua5*, const cIGZString&) = 0;
};
// 6.3.1: unnamed +8 view has the three Unknown slots and deleting destructor.
// No IID is invented for this view; the owner uses it only for destruction.
struct DestructionView : cIGZUnknown {
    virtual void* Destroy(uint32_t flags) = 0;
};

class BoundedOwner final : public cIGZLuaScriptServer,
                           public cIGZSystemService, public DestructionView {
public:
    BoundedOwner() : core_(fc4_luascripts_new()) {}
    bool Valid() const { return core_ != nullptr; }
    bool Layout() {
        auto* primary = static_cast<cIGZLuaScriptServer*>(this);
        auto* service = static_cast<cIGZSystemService*>(this);
        auto* destruction = static_cast<DestructionView*>(this);
        return reinterpret_cast<char*>(service) - reinterpret_cast<char*>(primary) == 4 &&
               reinterpret_cast<char*>(destruction) - reinterpret_cast<char*>(primary) == 8;
    }
    bool QueryInterface(uint32_t iid, void** out) override {
        if (!out) return false;
        *out = nullptr;
        if (iid == 1 || iid == kIID) *out = static_cast<cIGZLuaScriptServer*>(this);
        else if (iid == GZIID_cIGZSystemService) *out = static_cast<cIGZSystemService*>(this);
        else return false;
        if (AddRef() == 0) { *out = nullptr; return false; }
        return true;
    }
    uint32_t AddRef() override { return static_cast<uint32_t>(Call(1)); }
    uint32_t Release() override {
        int64_t result = 0;
        if (!Invoke(2, -1, 0, &result)) return 0;
        if (result == 0) delete this;
        return static_cast<uint32_t>(result);
    }
    void* Destroy(uint32_t flags) override {
        auto* primary = static_cast<cIGZLuaScriptServer*>(this);
        int64_t ignored = 0;
        if (Invoke(5, -1, flags, &ignored) && (flags & 1)) delete this;
        return primary;
    }
    bool Init() override { return Call(3) != 0; }
    bool Shutdown() override { return Call(4) != 0; }
    cIGZLua5* GetLua() override { return reinterpret_cast<cIGZLua5*>(static_cast<uintptr_t>(Call(11))); }
    int32_t RunScriptString(const cIGZString&, const cIGZString&, const cIGZString&) override {
        Refuse("RunScriptString"); return 0;
    }
    int32_t RunScriptFile(const cIGZString&, const cIGZString&) override { Refuse("RunScriptFile"); return 0; }
    bool KillScript(int32_t) override { Refuse("KillScript"); return false; }
    int32_t PauseScript(int32_t) override { Refuse("PauseScript"); return -1; }
    int32_t ResumeScript(int32_t) override { Refuse("ResumeScript"); return -1; }
    bool GetBreakpointExists(int32_t, const cIGZString&, int32_t) override { Refuse("GetBreakpointExists"); return false; }
    bool SetBreakpointExists(int32_t, const cIGZString&, int32_t, bool) override { Refuse("SetBreakpointExists"); return false; }
    bool ToggleBreakpointExists(int32_t, const cIGZString&, int32_t) override { Refuse("ToggleBreakpointExists"); return false; }
    bool GetBreakpointEnabled(int32_t, const cIGZString&, int32_t) override { Refuse("GetBreakpointEnabled"); return false; }
    bool SetBreakpointEnabled(int32_t, const cIGZString&, int32_t, bool) override { Refuse("SetBreakpointEnabled"); return false; }
    bool GetBreakpointCondition(int32_t, const cIGZString&, int32_t, cIGZString&) override { Refuse("GetBreakpointCondition"); return false; }
    bool SetBreakpointCondition(int32_t, const cIGZString&, int32_t, const cIGZString&) override { Refuse("SetBreakpointCondition"); return false; }
    bool GetBreakpointEntryList(int32_t, const cIGZString&, SC4List<BreakpointEntry>&) override {
        Refuse("GetBreakpointEntryList (adapter incomplete)"); return false;
    }
    void EnumerateScriptIDs(SC4Vector<int32_t>&) override { Refuse("EnumerateScriptIDs"); }
    bool GetScriptInfo(int32_t, cIGZLua5Thread**, cIGZString*, cIGZString*, int32_t*, int32_t*, bool*) override {
        Refuse("GetScriptInfo"); return false;
    }
    int32_t GetCurrentRunningScriptID() override { return static_cast<int32_t>(Call(6)); }
    bool GetScriptAutoYieldEnabled(int32_t id) const override {
        return const_cast<BoundedOwner*>(this)->Call(9, id) != 0;
    }
    bool SetScriptAutoYieldEnabled(int32_t id, bool enabled) override { return Call(10, id, enabled) != 0; }
    int32_t GetLastCreatedScriptID() override { return static_cast<int32_t>(Call(7)); }
    uint32_t GetServiceID() override { return service_.load(); }
    cIGZSystemService* SetServiceID(uint32_t id) override { service_ = id; return this; }
    int32_t GetServicePriority() override { return kPriority; }
    bool IsServiceRunning() override { return running_.load(); }
    cIGZSystemService* SetServiceRunning(bool value) override { running_ = value; return this; }
    bool OnTick(uint32_t) override { Refuse("OnTick"); return false; }
    bool OnIdle(uint32_t) override { Refuse("OnIdle"); return false; }
    int32_t GetServiceTickPriority() override { return kPriority; }
private:
    ~BoundedOwner() { fc4_luascripts_free(core_); }
    void Refuse(const char* method) {
        ++refusals_;
        fc4::Log("LUASCRIPTS refusal method=%s count=%u installed=0", method, refusals_.load());
    }
    bool Invoke(uint32_t op, int32_t id, uint32_t flag, int64_t* result) {
        if (fc4_luascripts_call(core_, &Foreign, this, op, id, flag, result)) return true;
        Refuse("owner transport/foreign/reentrant failure"); return false;
    }
    int64_t Call(uint32_t op, int32_t id = -1, uint32_t flag = 0) {
        int64_t value = 0; Invoke(op, id, flag, &value); return value;
    }
    static bool Foreign(void* context, const FC4LuaCall* call, uint64_t* reply) {
        auto* owner = static_cast<BoundedOwner*>(context);
        auto* unknown = reinterpret_cast<cIGZUnknown*>(static_cast<uintptr_t>(call->handle));
        cIGZFrameWork* fw = RZGetFrameWork(); // borrowed
        *reply = 0;
        switch (call->op) {
        case 1: case 4: {
            // Public COM route only. Factory acquisitions are owned references.
            auto* com = fw ? fw->GetCOMObject() : nullptr;
            void* result = nullptr;
            if (com && com->GetClassObject(call->number, static_cast<uint32_t>(call->other), &result)) {
                if (!result) return false; // true-with-null violates the foreign contract
                FC4_ACQUIRED(static_cast<cIGZUnknown*>(result), "Lua factory");
                *reply = reinterpret_cast<uintptr_t>(result);
            }
            return true;
        }
        case 2:
            // [abi-ret] lua-host.md 3.1 returns the open flag; 6.3.2 specifies Boolean status in AL.
            *reply = static_cast<cIGZLua5*>(unknown)->Open(static_cast<cIGZLua5::OpenFlags>(call->number)); return true;
        case 3: static_cast<cIGZLua5*>(unknown)->Close(); return true;
        case 5: {
            cIGZCommandServer* command = nullptr;
            if (fw && fw->GetSystemService(0xEB903A32, 0xCB90253A, reinterpret_cast<void**>(&command)) && command)
                FC4_ACQUIRED(command, "CommandServer service");
            cRZBaseString name("sc4app");
            *reply = static_cast<LuaGenerator*>(unknown)->Init(command,
                reinterpret_cast<cIGZLua5*>(static_cast<uintptr_t>(call->other)), name);
            if (command) FC4_RELEASE(command);
            return true;
        }
        case 6: *reply = reinterpret_cast<uintptr_t>(static_cast<LuaGenerator*>(unknown)->AsIGZCommandGenerator()); return *reply != 0;
        case 7: static_cast<cIGZCommandGenerator*>(unknown)->Shutdown(); return true;
        case 8: *reply = fw != nullptr; return true;
        case 9: if (fw) fw->AddToTick(static_cast<cIGZSystemService*>(owner)); return true;
        case 10: if (fw) fw->RemoveFromTick(static_cast<cIGZSystemService*>(owner)); return true;
        // Only owned Lua/generator factory references reach this bounded bridge.
        case 11: if (!unknown) return false; FC4_RELEASE_CREATED(unknown); return true;
        case 12: case 13: return true; // native allocation is freed after Rust call returns
        default: owner->Refuse("unbound primitive"); return false;
        }
    }
    FC4LuaOwner* core_;
    std::atomic<uint32_t> service_{kService}, refusals_{0};
    std::atomic<bool> running_{false};
};
bool Enabled() {
    size_t count = 0;
    const auto* entries = fc4::Subsystems(&count);
    for (size_t i = 0; i < count; ++i)
        if (std::string_view(entries[i].key) == "luascripts") return fc4::SubsystemEnabled(entries[i]);
    return false;
}
}

namespace fc4 {
void LuaScriptsTrace(const char* phase) {
    if (!Enabled()) return;
    auto* fw = RZGetFrameWork();
    cIGZLuaScriptServer* original = nullptr;
    const bool acquired = fw && fw->GetSystemService(kService, kIID, reinterpret_cast<void**>(&original));
    cIGZSystemService* service = nullptr;
    if (original) {
        FC4_ACQUIRED(original, "ScriptServer service trace");
        if (original->QueryInterface(GZIID_cIGZSystemService, reinterpret_cast<void**>(&service)) && service)
            FC4_ACQUIRED(service, "ScriptServer service QI trace");
    }
    Log("LUASCRIPTS trace phase=%s fw_state=%d acquire=%d original=%p service=%p id=%08X priority=%d running=unobserved installed=0 gap=LH4",
        phase, fw ? static_cast<int>(fw->GetState()) : -1, acquired, original, service,
        service ? service->GetServiceID() : 0, service ? service->GetServicePriority() : 0);
    if (service) FC4_RELEASE(service);
    if (original) FC4_RELEASE(original);
    static bool constructed = false;
    if (!constructed) {
        constructed = true;
        auto* owner = new (std::nothrow) BoundedOwner;
        if (owner && owner->Valid()) {
            owner->AddRef();
            Log("LUASCRIPTS bounded layout=%d clsid=%08X id=%08X priority=%d current=%d last=%d callbacks=0 comparisons=0 refusals=0 resync=0 installed=0",
                owner->Layout(), kClass, owner->GetServiceID(), owner->GetServicePriority(),
                owner->GetCurrentRunningScriptID(), owner->GetLastCreatedScriptID());
            owner->Release();
        } else if (owner) owner->Release();
    }
}
}
