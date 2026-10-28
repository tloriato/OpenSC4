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

#include "cFC4Tutorial.h"

#include "FC4Diag.h"
#include "FC4Fpu.h"
#include "FC4Log.h"
#include "FC4PropCityCommon.h"
#include "FC4RefAudit.h"
#include "FC4Streams.h"
#include "compat/FC4Compat.h"

#include "GZServPtrs.h"
#include "cIGZApp.h"
#include "cIGZCheatCodeManager.h"
#include "cIGZFrameWork.h"
#include "cIGZMessage2Standard.h"
#include "cIGZMessageServer2.h"
#include "cISC4AdvisorSystem.h"
#include "cISC4App.h"
#include "cISC4City.h"
#include "cISC4View3DWin.h"
#include "cISCLua.h"
#include "cRZBaseString.h"
#include "cRZCOMDllDirector.h"

#include <mutex>
#include <set>
#include <cstring>

namespace {

constexpr uint32_t kIID_cIGZMessageTarget2 = 0x452294AA;
constexpr uint32_t kIID_cIGZMessageTarget = 0xE98B2F57;     // §1: the cheat handler at +8
constexpr uint32_t kIID_cIGZCheatCodeManager = 0xA1085722;  // as the clock24 glue
constexpr uint32_t kCheatTutorial = 0x6A382BAF;             // §4.3: "Tutorial"
constexpr uint32_t kMsgEndTutorial = 0xAA3840E6;            // §4.3: data = message slot 9 (GetData1)

cFC4TutorialSystem* Self(void* u) { return static_cast<cFC4TutorialSystem*>(u); }

cISC4App* App() {
    cIGZFrameWork* fw = RZGetFrameWork();
    cIGZApp* app = fw ? fw->Application() : nullptr;
    cISC4App* sc4 = nullptr;
    if (app != nullptr && app->QueryInterface(GZIID_cISC4App, reinterpret_cast<void**>(&sc4)) && sc4 != nullptr) {
        FC4_ACQUIRED(sc4, "QueryInterface");
        return sc4;  // the caller releases it
    }
    return nullptr;
}

// The live FreeCity4 tutorial systems (one per city; the readers check a
// pointer against it before they call the core).
std::mutex& LiveLock() {
    static std::mutex m;
    return m;
}
std::set<const void*>& Live() {
    static std::set<const void*> s;
    return s;
}

// ---- host callbacks (fc4_tutorial_host) ------------------------------------------------------------------
//
// The core runs under the lock: the game calls it asks for are queued
// (CoreLock::Defer) when they change game state; queries answer at once.

void SubscribeCb(void* u, uint32_t type) {
    cFC4TutorialSystem* s = Self(u);
    s->Lock().Defer([s, type] { s->Subscribe(type, true); });
}
void UnsubscribeCb(void* u, uint32_t type) {
    cFC4TutorialSystem* s = Self(u);
    s->Lock().Defer([s, type] { s->Subscribe(type, false); });
}
bool CheatsAvailableCb(void* u) { return Self(u)->CheatsAvailable(); }
bool HasCheatManagerCb(void* u) { return Self(u)->HasCheatManager(); }
void RegisterCheatCb(void* u) {
    cFC4TutorialSystem* s = Self(u);
    s->Lock().Defer([s] { s->CheatRegistration(true); });
}
void UnregisterCheatCb(void* u) {
    cFC4TutorialSystem* s = Self(u);
    s->Lock().Defer([s] { s->CheatRegistration(false); });
}
void AddRefCb(void* u, uint64_t token) {
    Self(u)->RetainHostObject(token);
}
void ReleaseCb(void* u, uint64_t token) {
    Self(u)->ReleaseHostObject(token);
}
// tutorial.md 4.2.1: the app getter returns the borrowed registry itself,
// not an interface and not an app-relative offset.
void* Registry() {
    cISC4App* app = App();
    if (!app) return nullptr;
    void* registry = reinterpret_cast<void*>(app->GetTutorialRegistry());
    FC4_RELEASE(app);
    return registry;
}

template<class R, class... Args> R WindowSlot(uint64_t token, unsigned slot, Args... args) {
    void* window = reinterpret_cast<void*>(static_cast<uintptr_t>(token));
    auto** table = *reinterpret_cast<void***>(window);
    using Fn = R(__thiscall*)(void*, Args...);
    return reinterpret_cast<Fn>(table[slot])(window, args...);
}

// 4.2.2: allocate on the game's heap; the base constructor starts at zero
// references. The assignment owns the first and the core takes the second.
uint64_t CreateWindowCb(void* u) {
    using Allocate = void*(__cdecl*)(uint32_t);
    using Construct = void*(__thiscall*)(void*);
    auto allocate = reinterpret_cast<Allocate>(fc4::compat::DirectCall(fc4::compat::Direct::GlobalHeapAllocate));
    auto construct = reinterpret_cast<Construct>(fc4::compat::DirectCall(fc4::compat::Direct::TutorialWindowConstruct));
    if (!allocate || !construct) return 0;
    void* memory = allocate(100);
    if (!memory) return 0;
    auto* window = static_cast<cIGZUnknown*>(construct(memory));
    FC4_ADDREF(window);
    const uint64_t token = fc4::TokenOf(window);
    Self(u)->RememberCreatedWindow(token);
    return token;
}

void InitWindowCb(void*, uint64_t window) {
    using Fn = void(__thiscall*)(void*);
    auto fn = reinterpret_cast<Fn>(fc4::compat::DirectCall(fc4::compat::Direct::TutorialWindowInit));
    if (window && fn) fn(reinterpret_cast<void*>(static_cast<uintptr_t>(window)));
}
void CloseWindowCb(void*, uint64_t window) {
    using Fn = void(__thiscall*)(void*);
    auto fn = reinterpret_cast<Fn>(fc4::compat::DirectCall(fc4::compat::Direct::TutorialWindowClose));
    if (window && fn) fn(reinterpret_cast<void*>(static_cast<uintptr_t>(window)));
}
uint64_t WindowTaskCb(void*, uint64_t window) {
    return window ? fc4::TokenOf(WindowSlot<cIGZUnknown*>(window, 9)) : 0;
}
bool TaskEdgeScrollingCb(void*, uint64_t task) {
    // tutorial.md 4.4: current task slot 25, AL Boolean.
    return !task || WindowSlot<bool>(task, 25);
}
void WindowFilterCb(void*, uint64_t window, uint32_t value) {
    if (window) WindowSlot<void>(window, 6, value);
}
void WindowHintsCb(void*, uint64_t window) {
    if (window) WindowSlot<void>(window, 7);
}
void WindowCatalogCb(void*, uint64_t window, uint64_t menu) {
    if (window) WindowSlot<void>(window, 8, reinterpret_cast<void*>(static_cast<uintptr_t>(menu)));
}
bool WindowClickCb(void*, uint64_t window, int32_t* x, int32_t* z, int32_t n) {
    return window && WindowSlot<bool>(window, 10, x, z, n);
}
int32_t WindowResetCb(void*, uint64_t window, int32_t guid, int32_t index) {
    return window ? WindowSlot<int32_t>(window, 11, guid, index) : -1;
}
void WindowEventCb(void*, uint64_t window, uint32_t event) {
    using Fn = bool(__thiscall*)(void*, uint32_t);
    auto fn = reinterpret_cast<Fn>(fc4::compat::DirectCall(fc4::compat::Direct::TutorialWindowEvent));
    if (window && fn) fn(reinterpret_cast<void*>(static_cast<uintptr_t>(window)), event);
}

bool RegistrySetActiveCb(void*, uint32_t guid, bool active) {
    using Fn = bool(__thiscall*)(void*, uint32_t, bool);
    auto fn = reinterpret_cast<Fn>(fc4::compat::DirectCall(fc4::compat::Direct::TutorialRegistrySetActive));
    void* registry = Registry();
    return fn && registry && fn(registry, guid, active);
}
bool RegistryEntryEnabledCb(void*, uint32_t guid, bool* enabled) {
    using Fn = const uint8_t*(__thiscall*)(void*, uint32_t);
    auto fn = reinterpret_cast<Fn>(fc4::compat::DirectCall(fc4::compat::Direct::TutorialRegistryGetItem));
    void* registry = Registry();
    const uint8_t* item = fn && registry ? fn(registry, guid) : nullptr;
    if (enabled) *enabled = item && item[0x10] != 0;
    return item != nullptr;
}
// §4.2 (TU2, answered): endTutorial calls the 3D view, when present, slot 60
// CloseTertiaryMenu then slot 59 CloseSecondaryMenu (docs/abi interfaces.tsv
// cISC4View3DWin 59 / 60; the SDK header has the same slots). The view is the
// city view (main window child 0x6104489A, GetChildAs 0x9A47B417), one
// reference taken and released. Deferred: a game call, made outside the lock.
void ViewRestoreCb(void* u) {
    cFC4TutorialSystem* s = Self(u);
    s->Lock().Defer([] {
        cISC4View3DWin* view = fc4::propcity::View3D();
        if (view == nullptr) {
            fc4::Log("tutorial endTutorial: no 3D view, slots 60 / 59 not called");
            return;
        }
        view->CloseTertiaryMenu();   // slot 60
        view->CloseSecondaryMenu();  // slot 59: eax-or-void in docs/abi, the result is not read
        fc4::Log("tutorial endTutorial: 3D view CloseTertiaryMenu, CloseSecondaryMenu");
        fc4::propcity::Drop(view);
    });
}
uint64_t FetchLuaCb(void* u) { return Self(u)->FetchLuaContext(); }

fc4_tutorial_host MakeHost(cFC4TutorialSystem* self) {
    fc4_tutorial_host h{};
    h.struct_size = sizeof(h);
    h.user = self;
    h.subscribe = &SubscribeCb;
    h.unsubscribe = &UnsubscribeCb;
    h.cheats_available = &CheatsAvailableCb;
    h.has_cheat_manager = &HasCheatManagerCb;
    h.register_cheat = &RegisterCheatCb;
    h.unregister_cheat = &UnregisterCheatCb;
    h.add_ref = &AddRefCb;
    h.release = &ReleaseCb;
    h.create_window = &CreateWindowCb;
    h.init_window = &InitWindowCb;
    h.close_window = &CloseWindowCb;
    h.window_current_task = &WindowTaskCb;
    h.task_edge_scrolling_allowed = &TaskEdgeScrollingCb;
    h.window_filter_value = &WindowFilterCb;
    h.window_show_hints = &WindowHintsCb;
    h.window_catalog_menu = &WindowCatalogCb;
    h.window_left_click = &WindowClickCb;
    h.window_reset = &WindowResetCb;
    h.window_event = &WindowEventCb;
    h.registry_set_active = &RegistrySetActiveCb;
    h.registry_entry_enabled = &RegistryEntryEnabledCb;
    h.view_restore_controls = &ViewRestoreCb;
    h.fetch_lua_context = &FetchLuaCb;
    return h;
}

}  // namespace

cIGZUnknown* CreateFC4TutorialSystem() {
    return static_cast<cIGZUnknown*>(static_cast<cFC4ISC4TutorialSystem*>(new cFC4TutorialSystem()));
}

// ---- the cheat handler (+8) -------------------------------------------------------------------------------

bool cFC4TutorialCheatHandler::QueryInterface(uint32_t riid, void** ppvObj) {
    return owner_->QueryInterface(riid, ppvObj);
}
uint32_t cFC4TutorialCheatHandler::AddRef() { return owner_->AddRef(); }
uint32_t cFC4TutorialCheatHandler::Release() { return owner_->Release(); }

// §4.3: the debug subcommands (reset, save, step, next, quit) of the cheat
// "Tutorial" are not specified (TU7): logged and ignored. Only registered
// when the app's debug functionality is on.
bool cFC4TutorialCheatHandler::DoMessage(cGZMessage&) {
    fc4::Log("tutorial: the debug cheat \"Tutorial\" was issued; its subcommands are not specified (TU7), ignored");
    return true;
}

// ---- object -----------------------------------------------------------------------------------------------

void cFC4TutorialSystem::RememberCreatedWindow(uint64_t token) {
    if (token != 0) createdWindowRefs_[token] = 1;
}

void cFC4TutorialSystem::RetainHostObject(uint64_t token) {
    if (cIGZUnknown* p = fc4::FromToken(token)) {
        const auto found = createdWindowRefs_.find(token);
        if (found != createdWindowRefs_.end()) ++found->second;
        FC4_ADDREF(p);
    }
}

void cFC4TutorialSystem::ReleaseHostObject(uint64_t token) {
    if (cIGZUnknown* p = fc4::FromToken(token)) {
        const auto found = createdWindowRefs_.find(token);
        const bool created = found != createdWindowRefs_.end();
        if (created && --found->second == 0) createdWindowRefs_.erase(found);
        // tutorial 4.2.2: the two owner references may free this allocation.
        // Erase our bookkeeping before Release can reenter the owner.
        if (created) FC4_RELEASE_CREATED(p);
        else FC4_RELEASE(p);
    }
}

cFC4TutorialSystem::cFC4TutorialSystem() : cheat_(this) {
    host_ = MakeHost(this);
    core_ = fc4_tutorial_create(&host_, fc4::TokenOf(Primary()));
    if (core_ == nullptr) fc4::Log("cFC4TutorialSystem: core creation failed");
    // 4.2.2: the retained original UI obtains its owner through this binding.
    if (void** slot = fc4::compat::TutorialSystemGlobal()) *slot = Primary();
    {
        std::lock_guard<std::mutex> g(LiveLock());
        Live().insert(this);
    }
    fc4::Log("cFC4TutorialSystem(%p) created, core %p", static_cast<void*>(Primary()), static_cast<void*>(core_));
}

cFC4TutorialSystem::~cFC4TutorialSystem() {
    {
        std::lock_guard<std::mutex> g(LiveLock());
        Live().erase(this);
    }
    fc4_tutorial_destroy(core_);
    core_ = nullptr;
    if (void** slot = fc4::compat::TutorialSystemGlobal()) {
        if (*slot == Primary()) *slot = nullptr;
    }
    fc4::Log("cFC4TutorialSystem destroyed (%u IsTutorial calls, %u off-main calls)", isTutorialCalls_.load(),
             lock_.OffMainCalls());
}

// §1: 0xAA3836F6 and 1 (+0), 0x452294AA (+4), 0xE98B2F57 (+8), 0xE4FDA3D4
// (+0x10). The out pointer is left untouched on a refusal.
bool cFC4TutorialSystem::QueryInterface(uint32_t riid, void** ppvObj) {
    if (ppvObj == nullptr || !fc4_tutorial_query_interface(riid)) return false;
    switch (riid) {
        case kIID_cIGZMessageTarget2: *ppvObj = Target(); break;
        case kIID_cIGZMessageTarget: *ppvObj = CheatTarget(); break;
        case GZIID_cIGZSerializable: *ppvObj = static_cast<cIGZSerializable*>(this); break;
        default: *ppvObj = Primary(); break;  // 0xAA3836F6 and 1
    }
    AddRef();
    return true;
}

uint32_t cFC4TutorialSystem::AddRef() {
    fc4::CoreLock::Section s(lock_, "tutorial AddRef");
    fc4_tutorial_add_ref(core_);
    return ++refCount_;
}

// §1 / TU4 (answered): the last Release destroys and frees the object without
// running Shutdown (the shipped city calls Shutdown first). [choice]The C++ object is freed at the last
// reference; one still subscribed (Init without Shutdown) is shut down first
// so that no subscription is left on a deleted object (as HW13's choice).
uint32_t cFC4TutorialSystem::Release() {
    {
        fc4::CoreLock::Section s(lock_, "tutorial Release");
        fc4_tutorial_release(core_);
    }
    if (refCount_ > 1) return --refCount_;
    refCount_ = 0;
    if (destroying_) return 0;
    destroying_ = true;
    if (subscribed_) Shutdown();
    delete this;
    return 0;
}

fc4_tutorial_state cFC4TutorialSystem::State() {
    fc4_tutorial_state st{};
    st.struct_size = sizeof(st);
    fc4::CoreLock::Section s(lock_, "tutorial state");
    fc4_tutorial_get_state(core_, &st);
    return st;
}

// ---- the game calls the core asks for --------------------------------------------------------------------

void cFC4TutorialSystem::Subscribe(uint32_t type, bool add) {
    cIGZMessageServer2Ptr ms;
    if (!ms) return;
    if (add) {
        ms->AddNotification(Target(), type);
    } else {
        ms->RemoveNotification(Target(), type);
    }
}

// §4.3: the cheat manager (0x00B43CAC) and cISC4App slot 21
// GetDebugFunctionalityEnabled.
bool cFC4TutorialSystem::CheatsAvailable() {
    bool on = false;
    if (cISC4App* app = App()) {
        on = app->GetCheatCodeManager() != nullptr && app->GetDebugFunctionalityEnabled();
        FC4_RELEASE(app);
    }
    return on;
}

bool cFC4TutorialSystem::HasCheatManager() {
    bool has = false;
    if (cISC4App* app = App()) {
        has = app->GetCheatCodeManager() != nullptr;
        FC4_RELEASE(app);
    }
    return has;
}

// §4.3 / TU5: the cheat "Tutorial" (0x6A382BAF) and the handler (+8, the
// cheat manager's cIGZMessageTarget notification for that cheat).
void cFC4TutorialSystem::CheatRegistration(bool add) {
    cISC4App* app = App();
    if (app == nullptr) return;
    cIGZCheatCodeManager* raw = app->GetCheatCodeManager();
    cIGZCheatCodeManager* cm = nullptr;
    if (raw != nullptr && raw->QueryInterface(kIID_cIGZCheatCodeManager, reinterpret_cast<void**>(&cm)) &&
        cm != nullptr) {
        FC4_ACQUIRED(cm, "QueryInterface");
        if (add) {
            cm->RegisterCheatCode(kCheatTutorial, cRZBaseString("Tutorial"));
            cm->AddNotification(CheatTarget(), kCheatTutorial);
        } else {
            cm->RemoveNotification(CheatTarget(), kCheatTutorial);
            cm->UnregisterCheatCode(kCheatTutorial);
        }
        FC4_RELEASE(cm);
        fc4::Log("tutorial: cheat \"Tutorial\" %s", add ? "registered" : "unregistered");
    }
    FC4_RELEASE(app);
}

// §4.4 slot 7: cISC4City slot 81 GetAdvisorSystem, then its slot 10
// GetScriptingContext; the core keeps one reference.
uint64_t cFC4TutorialSystem::FetchLuaContext() {
    uint64_t token = 0;
    if (cISC4App* app = App()) {
        cISC4City* city = app->GetCity();
        cISC4AdvisorSystem* adv = city ? city->GetAdvisorSystem() : nullptr;
        cISCLua* lua = adv ? adv->GetScriptingContext() : nullptr;
        if (lua != nullptr) {
            FC4_ADDREF(lua);
            token = fc4::TokenOf(static_cast<cIGZUnknown*>(lua));
        }
        FC4_RELEASE(app);
    }
    return token;
}

// ---- lifecycle (§4.3; no initialised flag, quirk 4) ------------------------------------------------------

bool cFC4TutorialSystem::Init() {
    FC4_TRACE("TutorialSystem::Init");
    {
        fc4::CoreLock::Section s(lock_, "tutorial Init");
        fc4_tutorial_init(core_);
    }
    subscribed_ = true;
    const fc4_tutorial_state st = State();
    fc4::Log("tutorial Init: is_tutorial=%u task=%d guid=0x%08X", st.is_tutorial, st.task_number, st.guid);
    return true;
}

bool cFC4TutorialSystem::Shutdown() {
    FC4_TRACE("TutorialSystem::Shutdown");
    if (!destroying_) AddRef();  // keep alive while the server lets go
    {
        fc4::CoreLock::Section s(lock_, "tutorial Shutdown");
        fc4_tutorial_shutdown(core_);
    }
    subscribed_ = false;
    fc4::Log("tutorial Shutdown: %u IsTutorial calls, %u off-main calls", isTutorialCalls_.load(),
             lock_.OffMainCalls());
    if (!destroying_) Release();
    return true;
}

// ---- slots 5-20 (§4.1, §4.2, §4.4) --------------------------------------------------------------------------

bool cFC4TutorialSystem::startTutorial() {
    fc4::CoreLock::Section s(lock_, "tutorial startTutorial");
    return fc4_tutorial_start(core_);
}

bool cFC4TutorialSystem::endTutorial(bool endRegistry) {
    fc4::CoreLock::Section s(lock_, "tutorial endTutorial");
    return fc4_tutorial_end(core_, endRegistry);
}

cISCLua* cFC4TutorialSystem::GetScriptingContext() {
    uint64_t t;
    {
        fc4::CoreLock::Section s(lock_, "tutorial GetScriptingContext");
        t = fc4_tutorial_scripting_context(core_);
    }
    return reinterpret_cast<cISCLua*>(static_cast<uintptr_t>(t));
}

uint32_t cFC4TutorialSystem::CoreIsTutorial() {
    ++isTutorialCalls_;
    fc4::CoreLock::Section s(lock_, "tutorial IsTutorial");
    return fc4_tutorial_is_tutorial(core_);
}

// §4.1 slot 8: EAX = 1 or 0 (the whole register, not only AL).
bool cFC4TutorialSystem::IsTutorial() { return CoreIsTutorial() != 0; }

bool cFC4TutorialSystem::IsEdgeScrollingAllowed() {
    fc4::CoreLock::Section s(lock_, "tutorial IsEdgeScrollingAllowed");
    return fc4_tutorial_is_edge_scrolling_allowed(core_);
}

cISC4TutorialTask* cFC4TutorialSystem::GetCurrentTask() {
    fc4::CoreLock::Section s(lock_, "tutorial GetCurrentTask");
    return reinterpret_cast<cISC4TutorialTask*>(static_cast<uintptr_t>(fc4_tutorial_current_task(core_)));
}

int32_t cFC4TutorialSystem::GetTutorialTaskNumber() {
    fc4::CoreLock::Section s(lock_, "tutorial GetTutorialTaskNumber");
    return fc4_tutorial_task_number(core_);
}

int32_t cFC4TutorialSystem::GetTutorialGUID() {
    fc4::CoreLock::Section s(lock_, "tutorial GetTutorialGUID");
    return static_cast<int32_t>(fc4_tutorial_guid(core_));
}

// 4.4.2: a plain four-dword prefix, not a COM message or a scalar ID.
bool cFC4TutorialSystem::FilterMessage(uint32_t message) {
    fc4::CoreLock::Section s(lock_, "tutorial FilterMessage");
    uint32_t fields[4]{};
    fc4_tutorial_state state{};
    state.struct_size = sizeof(state);
    fc4_tutorial_get_state(core_, &state);
    if (message && state.window) {
        std::memcpy(fields, reinterpret_cast<const void*>(static_cast<uintptr_t>(message)), sizeof(fields));
    }
    return fc4_tutorial_filter_message(core_, fields[0], fields[1], fields[2], fields[3]);
}

// §4.4 slot 14: no defined result ("—"); this object is returned.
cISC4TutorialSystem* cFC4TutorialSystem::ShowUIHintsForUsers() {
    {
        fc4::CoreLock::Section s(lock_, "tutorial ShowUIHintsForUsers");
        fc4_tutorial_show_ui_hints(core_);
    }
    return this;
}

bool cFC4TutorialSystem::HandleCatalogMenu(cISC4WinCatalogView* menu) {
    fc4::CoreLock::Section s(lock_, "tutorial HandleCatalogMenu");
    return fc4_tutorial_handle_catalog_menu(core_, fc4::TokenOf(menu));
}

bool cFC4TutorialSystem::FilterLeftMouseClick(int32_t& x, int32_t& z, int32_t n) {
    fc4::CoreLock::Section s(lock_, "tutorial FilterLeftMouseClick");
    return fc4_tutorial_filter_left_mouse_click(core_, &x, &z, n);
}

bool cFC4TutorialSystem::ResetTutorialNumAndIndex(int32_t a, int32_t b) {
    fc4::CoreLock::Section s(lock_, "tutorial ResetTutorialNumAndIndex");
    return fc4_tutorial_reset_num_and_index(core_, a, b);
}

bool cFC4TutorialSystem::OnTutorialEvent(uint32_t e) {
    fc4::CoreLock::Section s(lock_, "tutorial OnTutorialEvent");
    return fc4_tutorial_on_event(core_, e);
}

void cFC4TutorialSystem::SetTutorialTaskNumber(uint32_t n) {
    fc4::CoreLock::Section s(lock_, "tutorial SetTutorialTaskNumber");
    fc4_tutorial_set_task_number(core_, static_cast<int32_t>(n));
}

void cFC4TutorialSystem::IncrementTutorialTaskNumber() {
    fc4::CoreLock::Section s(lock_, "tutorial IncrementTutorialTaskNumber");
    fc4_tutorial_increment_task_number(core_);
}

// ---- messages (§4.3) ---------------------------------------------------------------------------------------

bool cFC4TutorialSystem::DoMessage(cIGZMessage2* pMessage) {
    if (pMessage == nullptr || core_ == nullptr) return true;
    FC4_X87_SCOPE("tutorial DoMessage", pMessage->GetType());
    const uint32_t type = pMessage->GetType();
    uint32_t data = 0;
    if (type == kMsgEndTutorial) {
        cIGZMessage2Standard* m = nullptr;
        if (pMessage->QueryInterface(GZIID_cIGZMessage2Standard, reinterpret_cast<void**>(&m)) && m != nullptr) {
            FC4_ACQUIRED(m, "QueryInterface");
            data = static_cast<uint32_t>(m->GetData1());  // message slot 9
            FC4_RELEASE(m);
        }
    }
    const fc4_tutorial_state before = State();
    bool r;
    {
        fc4::CoreLock::Section s(lock_, "tutorial DoMessage");
        r = fc4_tutorial_do_message(core_, type, data);
    }
    const fc4_tutorial_state after = State();
    if (before.is_tutorial != after.is_tutorial || before.start_tried != after.start_tried ||
        before.end_pending != after.end_pending) {
        fc4::Log("tutorial DoMessage 0x%08X: is_tutorial %u->%u start_tried %u->%u end_pending %u->%u", type,
                 before.is_tutorial, after.is_tutorial, before.start_tried, after.start_tried, before.end_pending,
                 after.end_pending);
    }
    return r;
}

// ---- persistence (§3) --------------------------------------------------------------------------------------

bool cFC4TutorialSystem::Write(cIGZOStream& s) {
    FC4_TRACE("TutorialSystem::Write");
    fc4::OStreamAdapter a(s);
    bool ok;
    {
        fc4::CoreLock::Section sec(lock_, "tutorial Write");
        ok = fc4_tutorial_write(core_, a.Get());
    }
    const fc4_tutorial_state st = State();
    fc4::Log("tutorial Write -> %d (is_tutorial=%u task=%d guid=0x%08X)", ok ? 1 : 0, st.is_tutorial,
             st.task_number, st.guid);
    return ok;
}

// §3: Read runs before Init (foundations §3.7).
bool cFC4TutorialSystem::Read(cIGZIStream& s) {
    FC4_TRACE("TutorialSystem::Read");
    fc4::IStreamAdapter a(s);
    bool ok;
    {
        fc4::CoreLock::Section sec(lock_, "tutorial Read");
        ok = fc4_tutorial_read(core_, a.Get());
    }
    const fc4_tutorial_state st = State();
    fc4::Log("tutorial Read -> %d (is_tutorial=%u task=%d guid=0x%08X)", ok ? 1 : 0, st.is_tutorial, st.task_number,
             st.guid);
    return ok;
}

uint32_t cFC4TutorialSystem::GetGZCLSID() { return kFC4_CLSID_cSC4TutorialSystem; }

// ---- the readers (§4.1) ----------------------------------------------------------------------------------

namespace fc4 {

cFC4TutorialSystem* OurTutorialSystem(cISC4TutorialSystem* t) {
    if (t == nullptr) return nullptr;
    auto* ours = static_cast<cFC4TutorialSystem*>(static_cast<cFC4ISC4TutorialSystem*>(t));
    std::lock_guard<std::mutex> g(LiveLock());
    return Live().count(ours) != 0 ? ours : nullptr;
}

bool TutorialIsTutorial(cISC4TutorialSystem* t, bool* ours) {
    if (ours != nullptr) *ours = false;
    if (t == nullptr) return false;
    if (cFC4TutorialSystem* f = OurTutorialSystem(t)) {
        if (ours != nullptr) *ours = true;
        return f->CoreIsTutorial() != 0;  // fc4_tutorial_is_tutorial
    }
    return t->IsTutorial();  // the original's slot 8
}

}  // namespace fc4
