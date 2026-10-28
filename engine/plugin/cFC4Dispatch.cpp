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

#include "cFC4Dispatch.h"

#include "FC4Diag.h"
#include "FC4DispatchPylon.h"
#include "FC4Fpu.h"
#include "FC4Log.h"
#include "FC4PropCityCommon.h"
#include "FC4RefAudit.h"
#include "FC4Streams.h"
#include "compat/FC4Compat.h"
#include "cFC4Message2Standard.h"

#include "GZServPtrs.h"
#include "cIGZApp.h"
#include "cIGZFrameWork.h"
#include "cIGZMessage2Standard.h"
#include "cIGZMessageServer2.h"
#include "cISC4App.h"
#include "cISC4City.h"
#include "cISC43DRender.h"
#include "cISC4View3DWin.h"
#include "cISC4DispatchManager.h"  // cISC4DispatchSource is declared here
#include "cISC4Occupant.h"
#include "cISC4Automaton.h"
#include "cISTETerrain.h"
#include "SC4List.h"
#include "cRZBaseString.h"
#include "cRZCOMDllDirector.h"

namespace {

// §3: RemoveOccupant (data1 = the occupant) and DisasterPeriodEnded. The
// automaton notifications (0x09D92FC3) arrive at DoMessage without a
// subscription, as the vehicle's event target.
constexpr uint32_t kMsgRemoveOccupant = 0x99EF1143;
constexpr uint32_t kMsgDisasterPeriodEnded = 0xAA1B2002;
constexpr uint32_t kMsgViewChanged = 0xA6B79602;

cFC4DispatchManager* Self(void* u) { return static_cast<cFC4DispatchManager*>(u); }

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

// ---- host callbacks (fc4_dispatch_host) ----------------------------------------------------------------
//
// Wired: the message server and the subscriptions, the city, the AddRef /
// Release pair, the terrain altitude, the dispatch sources' protection-side
// slots (protection.md §8.2) and `resolve_ref` for the record's references.
// The original vehicle-view companion is retained through dispatch.md 4.1.3's
// native boundary. New pylon and vehicle creation remain unwired below.

bool AcquireMessageServer(void* u) {
    cFC4DispatchManager* s = Self(u);
    if (s->msgServerHeld) return true;
    cIGZMessageServer2Ptr ms;
    if (!ms) return false;
    s->msgServerHeld = true;
    return true;
}

void SubscribeCb(void* u, uint32_t t) {
    cFC4DispatchManager* s = Self(u);
    if (cIGZMessageServer2* ms = s->Server()) ms->AddNotification(s->Target(), t);
}
void UnsubscribeCb(void* u, uint32_t t) {
    cFC4DispatchManager* s = Self(u);
    if (cIGZMessageServer2* ms = s->Server()) ms->RemoveNotification(s->Target(), t);
}
void ReleaseMessageServerCb(void* u) { Self(u)->msgServerHeld = false; }

// §3: MessagePost (queued, priority 0) with IGZUnknown = the manager,
// data1 = the dispatch type, data2 = the ID.
void PostCb(void* u, uint32_t type, uint32_t dispatchType, uint32_t id) {
    cFC4DispatchManager* s = Self(u);
    cIGZMessageServer2* ms = s->Server();
    if (ms == nullptr) return;
    cFC4Message2Standard* msg = FC4MakeMessage(type, static_cast<intptr_t>(dispatchType),
                                               static_cast<intptr_t>(id), 0);
    msg->SetIGZUnknown(s->Primary());
    ms->MessagePost(msg, false);  // priority 0 (§3)
    FC4_X87_CHECK("dispatch MessagePost");
    msg->Release();
    s->CountPost();
}

bool AcquireCity(void* u) {
    cFC4DispatchManager* s = Self(u);
    if (s->City() != nullptr) return true;
    if (cISC4App* app = App()) {
        s->SetCity(app->GetCity());
        FC4_RELEASE(app);
    }
    return s->City() != nullptr;
}
void ReleaseCityCb(void* u) { Self(u)->SetCity(nullptr); }

void AddRefCb(void* u, uint64_t token) {
    if (token == 0) return;
    cIGZUnknown* o = reinterpret_cast<cIGZUnknown*>(static_cast<uintptr_t>(token));
    o->AddRef();
    FC4_ACQUIRED(o, "core add_ref");
}
void ReleaseCb(void* u, uint64_t token) {
    if (token == 0) return;
    cIGZUnknown* o = reinterpret_cast<cIGZUnknown*>(static_cast<uintptr_t>(token));
    FC4_RELEASE(o);  // the core's reference
}

float Altitude(void* u, float x, float z) {
    cFC4DispatchManager* s = Self(u);
    cISC4City* city = s->City();
    cISTETerrain* t = city != nullptr ? city->GetTerrain() : nullptr;
    return t != nullptr ? t->GetAltitude(x, z) : 0.0f;  // terrain slot 13
}

// The sources travel as cISC4DispatchSource*; their slots are the SDK's.
// The dispatch sources travel as cISC4DispatchSource* (the SDK's type); the
// verified declaration is cFC4ISC4DispatchSource (FC4Interfaces.h §15).
static cFC4ISC4DispatchSource* Src(cISC4DispatchSource* s) {
    return reinterpret_cast<cFC4ISC4DispatchSource*>(s);
}

bool SourceCanHandle(void* u, uint64_t source, uint32_t type) {
    cISC4DispatchSource* s = reinterpret_cast<cISC4DispatchSource*>(static_cast<uintptr_t>(source));
    return s != nullptr && Src(s)->CanHandleDispatchType(type) != 0;
}
bool SourceCanDispatch(void* u, uint64_t source, float x, float z, bool* allBusy) {
    cISC4DispatchSource* s = reinterpret_cast<cISC4DispatchSource*>(static_cast<uintptr_t>(source));
    bool busy = false;
    // [abi-ret] Slot 4 (interfaces.tsv eax-or-void) is a real AL bool:
    // protection.md §8.2 "CanDispatch(x, z, bool* allBusy) ... AL = 1", false
    // when the unit count is <= 0 or the sea level is not below the altitude.
    const bool ok = s != nullptr && Src(s)->CanDispatch(x, z, &busy);
    if (allBusy != nullptr) *allBusy = busy;
    return ok;
}
uint64_t SourceFromNearest(void* u, uint64_t source, float x, float z, uint32_t* callId) {
    cISC4DispatchSource* s = reinterpret_cast<cISC4DispatchSource*>(static_cast<uintptr_t>(source));
    uint32_t id = 0;
    cISC4Occupant* station = s != nullptr ? Src(s)->DispatchFromNearestStation(x, z, &id) : nullptr;
    if (callId != nullptr) *callId = id;
    return fc4::TokenOf(station);
}
bool SourceFromStation(void* u, uint64_t source, uint64_t station, uint32_t* callId) {
    cISC4DispatchSource* s = reinterpret_cast<cISC4DispatchSource*>(static_cast<uintptr_t>(source));
    cISC4Occupant* st = reinterpret_cast<cISC4Occupant*>(static_cast<uintptr_t>(station));
    uint32_t id = callId != nullptr ? *callId : 0;
    // [abi-ret] Slot 9 (interfaces.tsv eax-or-void) is a real AL bool:
    // protection.md §8.2 "DispatchFromStation(s, u32* id) ... AL = 1", false
    // when the unit index is past the count.
    const bool ok = s != nullptr && Src(s)->DispatchFromStation(st, &id);
    if (callId != nullptr) *callId = id;
    return ok;
}
void SourceRevoke(void* u, uint64_t source, uint64_t station, uint32_t callId) {
    cISC4DispatchSource* s = reinterpret_cast<cISC4DispatchSource*>(static_cast<uintptr_t>(source));
    cISC4Occupant* st = reinterpret_cast<cISC4Occupant*>(static_cast<uintptr_t>(station));
    if (s != nullptr) Src(s)->RevokeDispatchFromStation(st, callId);
}
// §8.2: GetStations (slot 6) fills a game SC4List; the core wants the unit
// state of each station (mask, base, count). Each station's unit state comes
// from GetDispatchesAtStation (slot 7).
size_t SourceStations(void* u, uint64_t source, fc4_station_units* out, size_t capacity) {
    cISC4DispatchSource* s = reinterpret_cast<cISC4DispatchSource*>(static_cast<uintptr_t>(source));
    if (s == nullptr) return 0;
    SC4List<cISC4Occupant*> list;
    Src(s)->GetStations(list);
    size_t n = 0;
    for (cISC4Occupant*& st : list) {
        if (out != nullptr && n < capacity) {
            uint32_t flags = 0, base = 0, count = 0;
            Src(s)->GetDispatchesAtStation(st, &flags, &base, &count);
            out[n].station = fc4::TokenOf(st);
            out[n].mask = flags;
            out[n].base = base;
            out[n].count = count;
        }
        ++n;
        // protection.md §15.4 (PR8): GetStations AddRefs every element.
        // Transfer cleanup from SC4List's releasing destructor to the audit.
        // Air-source list ownership is not published; see dispatch DP13.
        if (st != nullptr && (Src(s)->CanHandleDispatchType(0) || Src(s)->CanHandleDispatchType(1))) {
            FC4_ACQUIRED(st, "dispatch GetStations");
            FC4_RELEASE(st);
        }
        st = nullptr;  // avoid a second Release by SC4List; air ownership is DP13
    }
    return n;
}

uint64_t ResolveRef(void*, uint64_t token, uint32_t) { return token; }

bool VehicleInGroup(void*, uint64_t token, uint32_t group) {
    auto* vehicle = reinterpret_cast<cISC4Automaton*>(static_cast<uintptr_t>(token));
    auto* occupant = vehicle != nullptr ? vehicle->AsOccupant() : nullptr;
    return occupant != nullptr && occupant->IsOccupantGroup(group);
}

void AutomatonDespawn(void*, uint64_t token) {
    auto* vehicle = reinterpret_cast<cISC4Automaton*>(static_cast<uintptr_t>(token));
    if (vehicle != nullptr) vehicle->Remove(false);  // dispatch.md §3, slot 13
}

void AutomatonGoHome(void*, uint64_t token) {
    auto* vehicle = reinterpret_cast<cISC4Automaton*>(static_cast<uintptr_t>(token));
    if (vehicle != nullptr) vehicle->ReturnToSourceOccupant();  // dispatch.md §3, slot 29
}

uint64_t CreateVehicleView(void* u) {
    using Allocate = void*(__cdecl*)(uint32_t);
    using Construct = cIGZUnknown*(__thiscall*)(void*);
    using Init = bool(__thiscall*)(void*);
    auto allocate = reinterpret_cast<Allocate>(fc4::compat::DirectCall(fc4::compat::Direct::GlobalHeapAllocate));
    auto construct = reinterpret_cast<Construct>(fc4::compat::DirectCall(fc4::compat::Direct::DispatchViewConstruct));
    auto init = reinterpret_cast<Init>(fc4::compat::DirectCall(fc4::compat::Direct::DispatchViewInit));
    if (!allocate || !construct || !init) return 0;
    // Reach the renderer through the game's view interface; GetRenderer is borrowed.
    cISC4View3DWin* cityView = fc4::propcity::View3D();
    cISC43DRender* renderer = cityView ? cityView->GetRenderer() : nullptr;
    FC4_ADDREF(renderer);
    FC4_RELEASE(cityView);
    if (!renderer) {
        fc4::Log("dispatch vehicle-view: no renderer from the city view");
        return 0;
    }
    void* memory = allocate(0xA0);
    if (!memory) {
        FC4_RELEASE(renderer);
        return 0;
    }
    cIGZUnknown* view = construct(memory);
    FC4_ADDREF(view);  // the constructor starts with zero references
    init(view);       // the original owner ignores AL
    renderer->AddViewObject(reinterpret_cast<cISC4ViewObject3D*>(view), 5, 1000);
    Self(u)->SetNativeVehicleView(view, renderer);
    fc4::Log("dispatch vehicle-view: retained companion %p, renderer %p", static_cast<void*>(view),
             static_cast<void*>(renderer));
    return fc4::TokenOf(view);
}

void DestroyVehicleView(void* u, uint64_t token) {
    auto* view = reinterpret_cast<cIGZUnknown*>(static_cast<uintptr_t>(token));
    cISC43DRender* renderer = Self(u)->ViewRenderer();
    // dispatch.md 4.1.3: unregister before Shutdown, then release the owner's hold.
    if (renderer) renderer->RemoveViewObject(reinterpret_cast<cISC4ViewObject3D*>(view));
    using Shutdown = void(__thiscall*)(void*);
    auto shutdown = reinterpret_cast<Shutdown>(fc4::compat::DirectCall(fc4::compat::Direct::DispatchViewShutdown));
    if (view && shutdown) shutdown(view);
    Self(u)->SetNativeVehicleView(nullptr, nullptr);
    FC4_RELEASE_CREATED(view);  // this companion was constructed for this owner
    FC4_RELEASE(renderer);
}

void RefreshVehicleView(void*, uint64_t token) {
    using Refresh = void(__thiscall*)(void*);
    auto refresh = reinterpret_cast<Refresh>(fc4::compat::DirectCall(fc4::compat::Direct::DispatchViewRefresh));
    if (token && refresh) refresh(reinterpret_cast<void*>(static_cast<uintptr_t>(token)));
}

uint32_t TrackVehicle(void* u, uint64_t token, uint32_t kind) {
    cIGZUnknown* view = Self(u)->NativeVehicleView();
    auto* vehicle = reinterpret_cast<cISC4Automaton*>(static_cast<uintptr_t>(token));
    using Track = uint32_t(__thiscall*)(void*, cISC4Occupant*, uint32_t, uint32_t, const char*);
    auto track = reinterpret_cast<Track>(fc4::compat::DirectCall(fc4::compat::Direct::DispatchTrackOccupant));
    return view && vehicle && track ? track(view, vehicle->AsOccupant(), kind, vehicle->GetCallNumber(), nullptr) : 0;
}

void UntrackVehicle(void* u, uint32_t handle) {
    cIGZUnknown* view = Self(u)->NativeVehicleView();
    using Untrack = bool(__thiscall*)(void*, uint32_t, bool);
    auto untrack = reinterpret_cast<Untrack>(fc4::compat::DirectCall(fc4::compat::Direct::DispatchUntrackOccupant));
    // SPEC-GAP: dispatch's removal sites do not specify immediate; use the fading path.
    if (view && untrack) untrack(view, handle, false);
}

fc4_dispatch_host MakeHost(cFC4DispatchManager* self) {
    fc4_dispatch_host h{};
    h.struct_size = sizeof(h);
    h.user = self;
    h.acquire_message_server = &AcquireMessageServer;
    h.subscribe = &SubscribeCb;
    h.unsubscribe = &UnsubscribeCb;
    h.release_message_server = &ReleaseMessageServerCb;
    h.post = &PostCb;
    h.acquire_city = &AcquireCity;
    h.release_city = &ReleaseCityCb;
    h.add_ref = &AddRefCb;
    h.release = &ReleaseCb;
    h.altitude = &Altitude;
    h.source_can_handle = &SourceCanHandle;
    h.source_can_dispatch = &SourceCanDispatch;
    h.source_dispatch_from_nearest_station = &SourceFromNearest;
    h.source_dispatch_from_station = &SourceFromStation;
    h.source_revoke_dispatch_from_station = &SourceRevoke;
    h.source_stations = &SourceStations;
    h.resolve_ref = &ResolveRef;
    h.vehicle_in_group = &VehicleInGroup;
    h.automaton_despawn = &AutomatonDespawn;
    h.automaton_go_home = &AutomatonGoHome;
    h.create_vehicle_view = &CreateVehicleView;
    h.destroy_vehicle_view = &DestroyVehicleView;
    h.refresh_vehicle_view = &RefreshVehicleView;
    h.track_vehicle = &TrackVehicle;
    h.untrack_vehicle = &UntrackVehicle;
    fc4::WireDispatchPylon(h);
    // NOT wired: begin_vehicle, create_pylon, pylon_place_at_unit and pylon_return_to_station.
    return h;
}

}  // namespace

// The class's own accessors the host callbacks use (declared in the header's
// friend-free style: small inline setters).
cIGZMessageServer2* cFC4DispatchManager::Server() { return msgServerHeld ? msgServer_ : nullptr; }
void cFC4DispatchManager::SetServer(cIGZMessageServer2* ms) { msgServer_ = ms; }
cISC4City* cFC4DispatchManager::City() { return city_; }
void cFC4DispatchManager::SetCity(cISC4City* c) {
    if (c == city_) return;
    if (c != nullptr) FC4_ADDREF(c);
    if (city_ != nullptr) FC4_RELEASE(city_);
    city_ = c;
}
void cFC4DispatchManager::CountPost() { ++posts_; }

cIGZUnknown* CreateFC4DispatchManager() {
    return static_cast<cIGZUnknown*>(static_cast<cFC4ISC4DispatchManager*>(new cFC4DispatchManager()));
}

cFC4DispatchManager::cFC4DispatchManager() {
    {
        cIGZMessageServer2Ptr ms;
        if (ms) {
            SetServer(ms);
            FC4_ADDREF(msgServer_);
        }
    }
    host_ = MakeHost(this);
    core_ = fc4_dispatch_create(&host_, 0);
    if (core_ == nullptr) fc4::Log("cFC4DispatchManager: core creation failed");
    fc4::Log("cFC4DispatchManager(%p) created, core %p", static_cast<void*>(Primary()), static_cast<void*>(core_));
}

cFC4DispatchManager::~cFC4DispatchManager() {
    fc4_dispatch_destroy(core_);
    core_ = nullptr;
    SetCity(nullptr);
    FC4_RELEASE(msgServer_);
    msgServer_ = nullptr;
    fc4::Log("cFC4DispatchManager destroyed (%u posts)", posts_.load());
}

// §1: 1 and 0x694028EC at +0, 0x452294AA at +4, 0xE4FDA3D4 at +8; anything
// else leaves the out pointer untouched.
bool cFC4DispatchManager::QueryInterface(uint32_t riid, void** ppvObj) {
    if (ppvObj == nullptr) return false;
    uint32_t offset = 0;
    if (!fc4_dispatch_query_interface(riid, &offset)) return false;
    switch (offset) {
        case 0: *ppvObj = Primary(); break;
        case 4: *ppvObj = static_cast<cIGZMessageTarget2*>(this); break;
        case 8: *ppvObj = static_cast<cIGZSerializable*>(this); break;
        default: return false;
    }
    AddRef();
    return true;
}

uint32_t cFC4DispatchManager::AddRef() { return ++refCount_; }

uint32_t cFC4DispatchManager::Release() {
    if (refCount_ > 1) return --refCount_;
    refCount_ = 0;
    if (destroying_) return 0;
    destroying_ = true;
    if (core_ != nullptr) {
        fc4_dispatch_state st{};
        st.struct_size = sizeof(st);
        if (fc4_dispatch_get_state(core_, &st) && st.initialized != 0) Shutdown();
    }
    delete this;
    return 0;
}

bool cFC4DispatchManager::Init() {
    FC4_TRACE("DispatchManager::Init");
    const bool ok = fc4_dispatch_init(core_);
    fc4_dispatch_state st{};
    st.struct_size = sizeof(st);
    fc4_dispatch_get_state(core_, &st);
    fc4::Log("dispatch Init: ok=%d city=%p server=%p; %u records, nextID=%u toolType=%u", ok ? 1 : 0,
             static_cast<void*>(city_), static_cast<void*>(msgServer_), st.records, st.next_id, st.tool_type);
    return true;  // §8 slot 3 returns AL (1)
}

bool cFC4DispatchManager::Shutdown() {
    FC4_TRACE("DispatchManager::Shutdown");
    if (!destroying_) AddRef();
    fc4_dispatch_shutdown(core_);
    fc4_dispatch_state st{};
    st.struct_size = sizeof(st);
    fc4_dispatch_get_state(core_, &st);
    fc4::Log("dispatch Shutdown: %u records, nextID=%u; %u posts", st.records, st.next_id, posts_.load());
    if (!destroying_) Release();
    return true;
}

bool cFC4DispatchManager::CanDispatch(int32_t type, float x, float z) {
    return fc4_dispatch_can_dispatch(core_, static_cast<uint32_t>(type), x, z);
}

uint32_t cFC4DispatchManager::Dispatch(int32_t type, float x, float z) {
    return fc4_dispatch_dispatch(core_, static_cast<uint32_t>(type), x, z);
}

bool cFC4DispatchManager::RevokeDispatchByID(uint32_t id) { return fc4_dispatch_revoke_by_id(core_, id); }

bool cFC4DispatchManager::RevokeDispatchByPylon(cISC4PylonOccupant* pylon) {
    return fc4_dispatch_revoke_by_pylon(core_, fc4::TokenOf(pylon));
}

bool cFC4DispatchManager::RevokeAllDispatches(int32_t type) {
    return fc4_dispatch_revoke_all(core_, static_cast<uint32_t>(type));
}

bool cFC4DispatchManager::GetDispatchInfo(uint32_t id, FC4DispatchInfo& info) {
    fc4_dispatch_info s{};
    if (!fc4_dispatch_get_info(core_, id, &s)) return false;
    info.type = static_cast<int32_t>(s.dispatch_type);
    info.id = s.id;
    info.callId = s.call_id;
    info.viewHandle = s.view_handle;
    info.active = s.active != 0;
    info.x = s.target_x;
    info.y = s.target_y;
    info.z = s.target_z;
    info.station = reinterpret_cast<cISC4Occupant*>(static_cast<uintptr_t>(s.station));
    info.automaton = reinterpret_cast<cISC4Automaton*>(static_cast<uintptr_t>(s.automaton));
    info.pylon = reinterpret_cast<cISC4PylonOccupant*>(static_cast<uintptr_t>(s.pylon));
    return true;
}

uint32_t cFC4DispatchManager::GetDispatchCount(int32_t type) {
    return fc4_dispatch_get_count(core_, static_cast<uint32_t>(type));
}

uint32_t cFC4DispatchManager::GetDispatchIDArray(int32_t type, uint32_t* ids, uint32_t& capacity) {
    // §4.6: `capacity` is the limit and is never written (allocators.md).
    return static_cast<uint32_t>(
        fc4_dispatch_get_id_array(core_, static_cast<uint32_t>(type), capacity, ids, capacity));
}

bool cFC4DispatchManager::RegisterDispatchSource(cISC4DispatchSource* source) {
    const uint64_t t = fc4::TokenOf(source);
    fc4_dispatch_register_source(core_, t);
    return true;
}

bool cFC4DispatchManager::UnregisterDispatchSource(cISC4DispatchSource* source) {
    fc4_dispatch_unregister_source(core_, fc4::TokenOf(source));
    return true;
}

cISC4DispatchSource* cFC4DispatchManager::GetDispatchSource(int32_t type) {
    return reinterpret_cast<cISC4DispatchSource*>(
        static_cast<uintptr_t>(fc4_dispatch_get_source(core_, static_cast<uint32_t>(type))));
}

bool cFC4DispatchManager::BeginDispatchTool(int32_t type) {
    return fc4_dispatch_begin_tool(core_, static_cast<uint32_t>(type));
}

bool cFC4DispatchManager::EndDispatchTool(int32_t type) {
    return fc4_dispatch_end_tool(core_, static_cast<uint32_t>(type));
}

cISC4PylonOccupant* cFC4DispatchManager::GetActivePylon() {
    return reinterpret_cast<cISC4PylonOccupant*>(
        static_cast<uintptr_t>(fc4_dispatch_get_active_pylon(core_)));
}

void cFC4DispatchManager::SetActivePylon(cISC4PylonOccupant* pylon) {
    fc4_dispatch_set_active_pylon(core_, fc4::TokenOf(pylon));
}

cISC4Automaton* cFC4DispatchManager::GetAutomatonFromPylon(cISC4PylonOccupant* pylon) {
    return reinterpret_cast<cISC4Automaton*>(
        static_cast<uintptr_t>(fc4_dispatch_get_automaton_from_pylon(core_, fc4::TokenOf(pylon))));
}

uint32_t cFC4DispatchManager::GetAvailablePylonCount(int32_t type) {
    return fc4_dispatch_get_available_pylon_count(core_, static_cast<uint32_t>(type));
}

void* cFC4DispatchManager::GetDispatchVehicleView() {
    return reinterpret_cast<void*>(static_cast<uintptr_t>(fc4_dispatch_get_vehicle_view(core_)));
}

// §3: RemoveOccupant, DisasterPeriodEnded, view changed, and the automaton's
// own 0x09D92FC3 notifications (data2 = the event code, data3 = the state).
bool cFC4DispatchManager::DoMessage(cIGZMessage2* pMessage) {
    if (pMessage == nullptr || core_ == nullptr) return true;
    FC4_X87_SCOPE("dispatch DoMessage", pMessage->GetType());
    const uint32_t type = pMessage->GetType();
    uint32_t data2 = 0, data3 = 0;
    uint64_t object = 0;
    cIGZMessage2Standard* m = nullptr;
    if (pMessage->QueryInterface(GZIID_cIGZMessage2Standard, reinterpret_cast<void**>(&m)) && m != nullptr) {
        FC4_ACQUIRED(m, "QueryInterface");
        data2 = static_cast<uint32_t>(m->GetData2());
        data3 = static_cast<uint32_t>(m->GetData3());
        object = fc4::TokenOf(m->GetIGZUnknown());
        if (type == kMsgRemoveOccupant) object = fc4::TokenOf(reinterpret_cast<void*>(m->GetData1()));
        FC4_RELEASE(m);
    }
    return fc4_dispatch_do_message(core_, type, data2, data3, object);
}

bool cFC4DispatchManager::Write(cIGZOStream& s) {
    FC4_TRACE("DispatchManager::Write");
    fc4::OStreamAdapter a(s);
    const bool ok = fc4_dispatch_write(core_, a.Get());
    fc4_dispatch_state st{};
    st.struct_size = sizeof(st);
    fc4_dispatch_get_state(core_, &st);
    fc4::Log("dispatch Write -> %d (%u records, nextID=%u toolType=%u)", ok ? 1 : 0, st.records, st.next_id,
             st.tool_type);
    return ok;
}

// §7: Read runs before Init; nothing here is sized from the city.
bool cFC4DispatchManager::Read(cIGZIStream& s) {
    FC4_TRACE("DispatchManager::Read");
    fc4::IStreamAdapter a(s);
    const bool ok = fc4_dispatch_read(core_, a.Get());
    fc4_dispatch_state st{};
    st.struct_size = sizeof(st);
    fc4_dispatch_get_state(core_, &st);
    fc4::Log("dispatch Read -> %d (%u records, nextID=%u toolType=%u)", ok ? 1 : 0, st.records, st.next_id,
             st.tool_type);
    return ok;
}

uint32_t cFC4DispatchManager::GetGZCLSID() { return kFC4_CLSID_cSC4DispatchManager; }
