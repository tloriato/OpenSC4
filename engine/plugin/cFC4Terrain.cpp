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

// The terrain replacement over fc4_terrain (docs/specs/terrain.md). See
// cFC4Terrain.h for the shape, TR18 and TR9.

#include "cFC4Terrain.h"

#include "FC4Config.h"
#include "FC4Streams.h"
#include "compat/FC4Compat.h"
#include "FC4Diag.h"
#include "FC4Fpu.h"
#include "FC4GameList.h"
#include "FC4Interfaces.h"
#include "FC4Log.h"
#include "FC4RefAudit.h"
#include "cFC4Message2Standard.h"

#include "fc4.h"

#include "GZServPtrs.h"
#include "cIGZApp.h"
#include "cIGZFrameWork.h"
#include "cIGZIStream.h"
#include "cIGZMessage2.h"
#include "cIGZMessage2Standard.h"
#include "cIGZMessageServer2.h"
#include "cIGZOStream.h"
#include "cISC4App.h"
#include "cISC4City.h"
#include "cISC4WeatherSimulator.h"
#include "cISC4DBSegment.h"
#include "cIGZPersistDBSegment.h"
#include "cGZPersistResourceKey.h"
#include "cISC4Lot.h"
#include "cISC4LotManager.h"
#include "cISC4NetworkOccupant.h"
#include "cISC4Occupant.h"
#include "cISC4OccupantManager.h"
#include "cIGZCOM.h"
#include "cRZCOMDllDirector.h"
#include "SC4List.h"

#include <algorithm>
#include <atomic>
#include <cmath>
#include <cstring>
#include <vector>

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>
#include <tlhelp32.h>
#include <cstdio>

// Game calls (docs/abi/interfaces.tsv; a bool is only reliable in AL, a float
// return is x87). Reads of a game return value below carry their citation:
//   cISC4City 33 GetLotManager, 34 GetOccupantManager, 40 GetTrafficNetwork,
//   83 GetSurfaceWater, 84 GetTerrain (all borrowed pointers in EAX, the
//   table's `eax-or-void` class);
//   cISC4City 93/94 CellCountX / CellCountZ [abi-ret]: the export folds those
//   two names onto lighting/draw-context entries, but terrain.md §2.1 is
//   explicit that the terrain's Init reads the cell counts from the city
//   there, so they are counts in EAX and not a bool;
//   the traffic network map 8 FindNetworkTile (a pointer), network occupant 12
//   GetNetworkFlag and 67 GetPassageCeiling(float&) (a bool per TR19);
//   occupant manager 41 GetOccupantsByStandardCityCells (an A list);
//   lot manager 5 GetLot(cellX, cellZ, bool) (a borrowed pointer).

namespace fc4 {
namespace {

// ---- messages (terrain.md §8, §12.1) -------------------------------------------------------------

constexpr uint32_t kMsgPostCityInit = 0x26D31EC1;
constexpr uint32_t kMsgPreCityShutdown = 0x26D31EC2;
constexpr uint32_t kMsgViewUpdate = 0xA6B79602;      // §2.3, TR9
constexpr uint32_t kMsgAnimationTick = 0x86AD10EE;  // §12.1
constexpr uint32_t kMsgViewWindow = 0x470BD214;     // §12.1
constexpr uint32_t kMsgReconcile = 0x2A612643;      // §12.1
constexpr uint32_t kMsgSimNewDay = 0x66956814;      // §10.3

// §15.5.1 step 3: the 3D view is created by CLSID 0xC9B84E10 for this IID; the pointer the factory
// returns is view base + 0x0C, and slot 66 hands that same pointer back (§15.5.6).
constexpr uint32_t kIID_cISTETerrainView3D = 0x6771477D;
constexpr uintptr_t kViewInterfaceOffset = 0x0C;

// What the original's terrain subscribes to (§12.1), minus the render-only
// notifications and the script commands we do not implement.
constexpr uint32_t kSubscribed[] = {kMsgPostCityInit, kMsgPreCityShutdown, kMsgViewUpdate,
                                    kMsgAnimationTick, kMsgViewWindow,      kMsgReconcile,
                                    kMsgSimNewDay};

// Vertex flags (§7).
constexpr uint16_t kFlagEvenHidden = 0x0020;
constexpr uint16_t kFlagOddHidden = 0x0040;
constexpr uint16_t kFlagCell4 = 0x0200;
constexpr uint16_t kFlagFlipInternal = 0x1000;
constexpr uint16_t kFlagFlipExternal = 0x2000;

// The one fc4_terrain of the process (one city at a time). The surface water
// object resolves through it, so it does not matter whether the city creates
// the water through the class factory or takes the terrain's own.
std::atomic<fc4_terrain*> g_activeCore{nullptr};
std::atomic<uint32_t> g_cellsX{0};
std::atomic<uint32_t> g_cellsZ{0};
std::atomic<bool> g_mirrorDirty{false};  // set by HostPost, consumed at the outermost Leave()

// ---- bounded terrain call trace (diagnostic for the gt3-r2 June stall) ----------------------------------
// A lock-free ring of the last kTraceRing terrain events (entry / exit of the locked slots, DoMessage,
// HostPost) with the thread id, the owner of the terrain lock, and a monitor thread that, when no
// SimNewDay reached the terrain for kStallMs, logs the ring once and samples every other thread's stack
// (return addresses into the game or our image). Evidence only: it changes no behaviour.
namespace trace {
struct Ev {
    DWORD ms;
    DWORD tid;
    const char* what;
    int32_t phase;  // +1 enter, -1 leave, 0 point event; 2 = waited for the lock
    uint32_t arg;
};
constexpr unsigned kTraceRing = 256;
constexpr DWORD kStallMs = 2500;
Ev g_ring[kTraceRing];
std::atomic<unsigned> g_next{0};
std::atomic<DWORD> g_lockOwner{0};
std::atomic<const char*> g_lockSlot{nullptr};
std::atomic<DWORD> g_lockSince{0};
std::atomic<DWORD> g_lastNewDay{0};
std::atomic<bool> g_stop{false};
HANDLE g_monitor = nullptr;

void Note(const char* what, int32_t phase, uint32_t arg = 0) {
    const unsigned i = g_next.fetch_add(1, std::memory_order_relaxed) % kTraceRing;
    g_ring[i] = Ev{GetTickCount(), GetCurrentThreadId(), what, phase, arg};
}

void DumpRing(const char* why, unsigned count) {
    const unsigned n = g_next.load(std::memory_order_relaxed);
    const unsigned c = n < count ? n : count;
    LogWait(1000, "terrain trace: %s; lock owner tid %lu in %s since %lu ms ago; last %u of %u events:", why,
            static_cast<unsigned long>(g_lockOwner.load()), g_lockSlot.load() ? g_lockSlot.load() : "-",
            static_cast<unsigned long>(g_lockOwner.load() ? GetTickCount() - g_lockSince.load() : 0), c, n);
    for (unsigned k = n - c; k != n; ++k) {
        const Ev e = g_ring[k % kTraceRing];
        LogWait(1000, "terrain trace:   [%lu ms] tid %lu %s %s 0x%08X", static_cast<unsigned long>(e.ms),
                static_cast<unsigned long>(e.tid),
                e.phase > 1 ? "WAITED" : (e.phase > 0 ? "enter" : (e.phase < 0 ? "leave" : "event")),
                e.what ? e.what : "?", e.arg);
    }
}

uintptr_t ModuleEnd(HMODULE m) {
    const auto* b = reinterpret_cast<const uint8_t*>(m);
    const auto* dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(b);
    const auto* nt = reinterpret_cast<const IMAGE_NT_HEADERS*>(b + dos->e_lfanew);
    return reinterpret_cast<uintptr_t>(b) + nt->OptionalHeader.SizeOfImage;
}

// Suspends each other thread of the process in turn, copies up to 4 KB of its stack and logs the values
// that point into the game or our image (candidate return addresses). Formatting runs after ResumeThread.
void SampleThreads() {
#if defined(_M_IX86)
    HMODULE self = nullptr;
    GetModuleHandleExA(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                       reinterpret_cast<LPCSTR>(&SampleThreads), &self);
    const HMODULE exe = GetModuleHandleA(nullptr);
    const uintptr_t exeB = reinterpret_cast<uintptr_t>(exe), exeE = ModuleEnd(exe);
    const uintptr_t ourB = reinterpret_cast<uintptr_t>(self), ourE = self ? ModuleEnd(self) : 0;
    HANDLE snap = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
    if (snap == INVALID_HANDLE_VALUE) return;
    THREADENTRY32 te{};
    te.dwSize = sizeof(te);
    const DWORD pid = GetCurrentProcessId(), me = GetCurrentThreadId();
    static DWORD stack[1024];
    unsigned threads = 0;
    for (BOOL ok = Thread32First(snap, &te); ok && threads < 32; ok = Thread32Next(snap, &te)) {
        if (te.th32OwnerProcessID != pid || te.th32ThreadID == me) continue;
        HANDLE th = OpenThread(THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT | THREAD_QUERY_INFORMATION, FALSE,
                               te.th32ThreadID);
        if (th == nullptr) continue;
        ++threads;
        CONTEXT ctx{};
        ctx.ContextFlags = CONTEXT_CONTROL | CONTEXT_INTEGER;
        SIZE_T got = 0;
        bool have = false;
        if (SuspendThread(th) != static_cast<DWORD>(-1)) {
            have = GetThreadContext(th, &ctx) != FALSE;
            if (have && !ReadProcessMemory(GetCurrentProcess(), reinterpret_cast<const void*>(ctx.Esp), stack,
                                           sizeof(stack), &got)) {
                // the stack may end within 4 KB: retry with 1 KB
                if (!ReadProcessMemory(GetCurrentProcess(), reinterpret_cast<const void*>(ctx.Esp), stack, 1024,
                                       &got)) {
                    got = 0;
                }
            }
            ResumeThread(th);
        }
        CloseHandle(th);
        if (!have) continue;
        char line[1024];
        int len = snprintf(line, sizeof(line), "terrain trace: thread %lu eip=%s+0x%lX:",
                           static_cast<unsigned long>(te.th32ThreadID),
                           (ctx.Eip >= exeB && ctx.Eip < exeE) ? "exe" : ((ctx.Eip >= ourB && ctx.Eip < ourE) ? "dll" : "other"),
                           static_cast<unsigned long>((ctx.Eip >= exeB && ctx.Eip < exeE) ? ctx.Eip - exeB
                                                      : (ctx.Eip >= ourB && ctx.Eip < ourE) ? ctx.Eip - ourB : ctx.Eip));
        unsigned found = 0;
        for (unsigned i = 0; i < got / 4 && found < 28 && len > 0 && len < static_cast<int>(sizeof(line)) - 24; ++i) {
            const uintptr_t v = stack[i];
            if (v >= exeB + 0x1000 && v < exeE) {
                len += snprintf(line + len, sizeof(line) - len, " exe+0x%X", static_cast<unsigned>(v - exeB));
                ++found;
            } else if (v >= ourB + 0x1000 && v < ourE) {
                len += snprintf(line + len, sizeof(line) - len, " dll+0x%X", static_cast<unsigned>(v - ourB));
                ++found;
            }
        }
        LogWait(1000, "%s", line);
    }
    CloseHandle(snap);
#endif
}

DWORD WINAPI Monitor(void*) {
    bool reported = false;
    while (!g_stop.load(std::memory_order_acquire)) {
        Sleep(250);
        const DWORD last = g_lastNewDay.load(std::memory_order_acquire);
        if (last == 0 || reported) continue;
        const DWORD idle = GetTickCount() - last;
        if (idle < kStallMs) continue;
        reported = true;
        char why[96];
        snprintf(why, sizeof(why), "no SimNewDay reached the terrain for %lu ms", static_cast<unsigned long>(idle));
        DumpRing(why, 96);
        SampleThreads();
    }
    return 0;
}

void StartMonitor() {
    if (g_monitor != nullptr) return;
    g_stop.store(false, std::memory_order_release);
    g_monitor = CreateThread(nullptr, 64 * 1024, &Monitor, nullptr, 0, nullptr);
}

void StopMonitor() {
    if (g_monitor == nullptr) return;
    g_stop.store(true, std::memory_order_release);
    WaitForSingleObject(g_monitor, 2000);
    CloseHandle(g_monitor);
    g_monitor = nullptr;
    g_lastNewDay.store(0, std::memory_order_release);
}
}  // namespace trace

// ---- the host context -----------------------------------------------------------------------------

// TR18 (terrain.md §15.4) is enforced here as well as in the core. `inside` is
// non-zero while one of our entry points is inside fc4_terrain, so a host
// callback that reaches back into the same handle is refused and counted. Only
// touched under the object's critical section.
struct HostContext {
    cISC4City* city = nullptr;  // borrowed for one call
    cIGZUnknown* self = nullptr;  // the terrain, for the messages' void1/IGZUnknown
    int inside = 0;
    std::atomic<uint32_t> guardRefused{0};
    std::atomic<uint32_t> posts{0};
    std::atomic<uint32_t> randomDraws{0};
    std::atomic<uint32_t> fireCalls{0};
    bool testMode = false;
};

// The single TR18 gate every host callback goes through.
bool HostMayAct(HostContext* c, const char* what) {
    if (c == nullptr) return false;
    if (c->inside > 0) {
        const uint32_t n = c->guardRefused.fetch_add(1, std::memory_order_relaxed) + 1;
        Log("terrain: TR18 refused host callback '%s' from inside the terrain handle (guard count %u)", what, n);
        return false;
    }
    return true;
}

uint32_t HostRandomUniform(void* u, uint32_t n) {
    auto* c = static_cast<HostContext*>(u);
    if (!HostMayAct(c, "random_uniform") || n == 0) return 0;
    c->randomDraws.fetch_add(1, std::memory_order_relaxed);
    // §9.1: the walk angle draws from the terrain's own generator. [choice] a
    // process counter stands in for it; only the random-walk brushes consume
    // it, and every shipped brush is oriented so the walk angle is unused
    // (quirk 12), so this cannot change a height.
    static std::atomic<uint32_t> s_draw{0};
    return s_draw.fetch_add(1, std::memory_order_relaxed) % n;
}

// Navigability §10.4 steps 4 and 5: read the game only, never this handle.
int32_t HostBridgeAt(void* u, int32_t x, int32_t z, float* deck) {
    auto* c = static_cast<HostContext*>(u);
    if (!HostMayAct(c, "bridge_at") || c->city == nullptr) return FC4_TERRAIN_BRIDGE_NONE;
    auto* map = reinterpret_cast<cFC4ISC4TrafficNetworkMap*>(c->city->GetTrafficNetwork());
    if (map == nullptr) return FC4_TERRAIN_BRIDGE_NONE;
    void* tile = map->FindNetworkTile(x, z, 0x1F4F, true);
    if (tile == nullptr) return FC4_TERRAIN_BRIDGE_NONE;
    auto* n = static_cast<cISC4NetworkOccupant*>(tile);
    if ((n->GetNetworkFlag() & 0x40000u) == 0) return FC4_TERRAIN_BRIDGE_NONE;
    float h = 0.0f;
    if (!n->GetPassageCeiling(h)) return FC4_TERRAIN_BRIDGE_UNKNOWN_DECK;  // TR19
    if (deck != nullptr) *deck = h;
    return FC4_TERRAIN_BRIDGE_DECK;
}

bool HostPowerPole(void* u, int32_t x, int32_t z) {
    auto* c = static_cast<HostContext*>(u);
    if (!HostMayAct(c, "has_power_pole") || c->city == nullptr) return false;
    cISC4OccupantManager* om = c->city->GetOccupantManager();
    if (om == nullptr) return false;
    SC4List<cISC4Occupant*> list;
    const SC4Point<int> cell(x, z);
    om->GetOccupantsByStandardCityCells(list, &cell, &cell, 0x2890D4DEu, 0xFFFFFFFFu);
    bool any = false;
    for (cISC4Occupant*& o : list) {
        if (o == nullptr) continue;
        FC4_ACQUIRED(o, "GetOccupantsByStandardCityCells list entry");
        any = true;
        FC4_RELEASE(o);
        o = nullptr;
    }
    return any;
}

// §10.4 step 3: the lot at the cell (its centre, y = the sea level). The core
// compares the returned token with the one the caller passed in, so both are
// the lot pointer's bits. The y of the query is not part of the comparison.
uint64_t HostLotAtCell(void* u, int32_t x, int32_t z) {
    auto* c = static_cast<HostContext*>(u);
    if (!HostMayAct(c, "lot_at_cell") || c->city == nullptr) return 0;
    cISC4LotManager* lm = c->city->GetLotManager();
    if (lm == nullptr) return 0;
    cISC4Lot* lot = lm->GetLot(x, z, false);  // borrowed
    return static_cast<uint64_t>(reinterpret_cast<uintptr_t>(lot));
}

// The lava's flammability and fire calls (§11.4, §11.5) drive the game's fire
// manager and flammability grid and must not come back here (§15.4). No
// flammability host is wired yet: nothing ignites. [choice]
bool HostCellFlammable(void* u, int32_t x, int32_t z) {
    auto* c = static_cast<HostContext*>(u);
    (void)x;
    (void)z;
    return HostMayAct(c, "is_cell_flammable") ? false : false;
}

void HostIgniteCell(void* u, int32_t x, int32_t z) {
    auto* c = static_cast<HostContext*>(u);
    (void)x;
    (void)z;
    if (!HostMayAct(c, "ignite_cell")) return;
    c->fireCalls.fetch_add(1, std::memory_order_relaxed);
}

void HostQuickBurnCell(void* u, int32_t x, int32_t z) {
    auto* c = static_cast<HostContext*>(u);
    (void)x;
    (void)z;
    if (!HostMayAct(c, "quick_burn_cell")) return;
    c->fireCalls.fetch_add(1, std::memory_order_relaxed);
}

void HostSetCause(void* u, uint32_t cause) {
    auto* c = static_cast<HostContext*>(u);
    (void)cause;
    if (!HostMayAct(c, "set_cause")) return;
    c->fireCalls.fetch_add(1, std::memory_order_relaxed);
}

bool HostNextBurnCell(void* u, int32_t* x, int32_t* z) {
    auto* c = static_cast<HostContext*>(u);
    (void)x;
    (void)z;
    return HostMayAct(c, "next_burn_cell") ? false : false;  // exhausted
}

// §8.1: MessagePost(msg, false) with four data words. The core hands these
// over after each entry point released the handle, so this is never re-entrant
// and never runs under the terrain's lock.
void HostPost(void* u, const fc4_terrain_post* p) {
    auto* c = static_cast<HostContext*>(u);
    if (c == nullptr || p == nullptr) return;
    c->posts.fetch_add(1, std::memory_order_relaxed);
    trace::Note("HostPost", 0, p->msg_type);
    g_mirrorDirty.store(true, std::memory_order_release);  // §15.5.8: refresh the raw arrays first
    if (c->testMode) return;
    // §15.5.11 item 19 / §15.5.12.1 (TR38): the view clamps nothing, so a heights / water rectangle
    // outside [0, cellsX] x [0, cellsZ] (or inverted) is not posted at all.
    if (p->msg_type == 0x278E6003u || p->msg_type == 0x678E6E3Au) {
        const uint32_t d1 = static_cast<uint32_t>(p->data1), d2 = static_cast<uint32_t>(p->data2);
        const uint32_t x0 = d1 >> 16, x1 = d1 & 0xFFFFu, z0 = d2 >> 16, z1 = d2 & 0xFFFFu;
        const uint32_t cx = g_cellsX.load(std::memory_order_acquire), cz = g_cellsZ.load(std::memory_order_acquire);
        if (x0 > x1 || z0 > z1 || x1 > cx || z1 > cz) {
            Log("terrain: WARNING post 0x%08X refused: rectangle x %u..%u z %u..%u outside 0..%u x 0..%u (TR38)",
                p->msg_type, x0, x1, z0, z1, cx, cz);
            return;
        }
    }
    cIGZMessageServer2Ptr ms;
    if (!ms) return;
    // §15.5.12.1 (TR38): a fresh message object per post, type + data1..4 only, never touched after
    // MessagePost. gt3-repl-4's fault was SetVoid1(sender) here: void1 IS data1 (+0x10), so the view
    // unpacked the sender's address as the rectangle. The original's helper sets no sender.
    cFC4Message2Standard* m =
        FC4MakeMessage(p->msg_type, static_cast<intptr_t>(p->data1), static_cast<intptr_t>(p->data2),
                       static_cast<intptr_t>(p->data3));
    if (m == nullptr) return;
    m->SetData4(static_cast<intptr_t>(p->data4));
    ms->MessagePost(m, false);
    m->Release();
}

fc4_terrain_host MakeHost(HostContext* c) {
    fc4_terrain_host h{};
    h.struct_size = sizeof(h);
    h.user = c;
    h.post = &HostPost;
    h.random_uniform = &HostRandomUniform;
    h.lot_at_cell = &HostLotAtCell;
    h.bridge_at = &HostBridgeAt;
    h.has_power_pole = &HostPowerPole;
    h.is_cell_flammable = &HostCellFlammable;
    h.ignite_cell = &HostIgniteCell;
    h.quick_burn_cell = &HostQuickBurnCell;
    h.set_cause = &HostSetCause;
    h.next_burn_cell = &HostNextBurnCell;
    return h;
}

// ---- the city (borrowed) --------------------------------------------------------------------------

cISC4City* CurrentCity() {
    cIGZFrameWork* fw = RZGetFrameWork();
    cIGZApp* app = fw ? fw->Application() : nullptr;
    cISC4App* sc4 = nullptr;
    if (app == nullptr || !app->QueryInterface(GZIID_cISC4App, reinterpret_cast<void**>(&sc4)) || sc4 == nullptr) {
        return nullptr;
    }
    FC4_ACQUIRED(sc4, "QueryInterface");
    cISC4City* city = sc4->GetCity();  // borrowed
    FC4_RELEASE(sc4);
    return city;
}

void MirrorFlags(uint32_t x, uint32_t z, uint16_t flags);  // the raw vertex mirror, below

uint16_t VertexFlags(uint32_t x, uint32_t z) {
    fc4_terrain* core = g_activeCore.load(std::memory_order_acquire);
    return core != nullptr ? fc4_terrain_vertex_flags(core, static_cast<int32_t>(x), static_cast<int32_t>(z))
                           : uint16_t{0};
}

void SetFlagBit(uint32_t x, uint32_t z, uint16_t bit, bool on) {
    fc4_terrain* core = g_activeCore.load(std::memory_order_acquire);
    if (core == nullptr) return;
    uint16_t f = fc4_terrain_vertex_flags(core, static_cast<int32_t>(x), static_cast<int32_t>(z));
    f = on ? static_cast<uint16_t>(f | bit) : static_cast<uint16_t>(f & ~bit);
    fc4_terrain_set_vertex_flags(core, static_cast<int32_t>(x), static_cast<int32_t>(z), f);
    MirrorFlags(x, z, f);
}

// ---- the raw arrays the 3D view reads directly (terrain.md §15.5.5, §15.5.11 items 12-14) --------
//
// The game's terrain view walks terrain +0x6C (0x24-byte vertex records, altitude +0x00, flags +0x22,
// surface texture id +0x16) and water +0x18 (0x24-byte records, depth +0x00) without going through any
// slot. The core owns the real state; these are mirrors of it, refreshed at the outermost Leave() after
// anything posted a redisplay (the only channel the view rebuilds from, §15.5.8), and once in
// DoPostCityInit before the view's Init.
#pragma pack(push, 1)
struct VertexRecord {
    float altitude;         // +0x00
    float normal[3];        // +0x04..+0x0F: the view does not read it (§15.5.5); left 0
    uint8_t pad10[6];       // +0x10..+0x15
    uint16_t texture;       // +0x16: [choice] 0; the spec names no source for it (question TR36)
    uint8_t pad18[0x0A];    // +0x18..+0x21
    uint16_t flags;         // +0x22: §7 flags
};
struct WaterRecord {
    float depth;            // +0x00
    uint8_t pad[0x20];
};
#pragma pack(pop)
static_assert(sizeof(VertexRecord) == 0x24 && offsetof(VertexRecord, texture) == 0x16 &&
                  offsetof(VertexRecord, flags) == 0x22,
              "§15.5.5: 0x24-byte vertex records, texture +0x16, flags +0x22");
static_assert(sizeof(WaterRecord) == 0x24, "§15.5.5: 0x24-byte water records");

// Flag bits the view itself writes into the mirror (slot 16 LevelCell ORs 0x60, slot 17 clears them,
// §15.5.11 item 13). A mirror word that differs from what we last wrote only in these bits is the
// view's edit, and is pushed into the core rather than overwritten.
constexpr uint16_t kViewWrittenFlags = kFlagEvenHidden | kFlagOddHidden;

struct RawMirror {
    std::vector<VertexRecord> vertices;
    std::vector<WaterRecord> water;
    std::vector<uint16_t> lastFlags;
    std::vector<float> scratch;
    uint32_t vertsX = 0;
    uint32_t viewFlagEdits = 0;
    uint32_t syncs = 0;
    const void* owner = nullptr;  // the terrain the arrays belong to (one city at a time)
};
RawMirror g_mirror;

void MirrorFlags(uint32_t x, uint32_t z, uint16_t flags) {
    if (g_mirror.vertsX == 0 || x >= g_mirror.vertsX) return;
    const size_t i = static_cast<size_t>(z) * g_mirror.vertsX + x;
    if (i >= g_mirror.vertices.size()) return;
    g_mirror.vertices[i].flags = flags;
    g_mirror.lastFlags[i] = flags;
}

// The water objects alive in the process: each one exposes the water mirror at +0x18.
std::vector<void**> g_waterRecordSlots;

void PointWatersAtMirror() {
    void* p = g_mirror.water.empty() ? nullptr : g_mirror.water.data();
    for (void** slot : g_waterRecordSlots) *slot = p;
}

void AllocateMirror(const void* owner, uint32_t vertsX, uint32_t vertexCount) {
    g_mirror.owner = owner;
    g_mirror.vertices.assign(vertexCount, VertexRecord{});
    g_mirror.water.assign(vertexCount, WaterRecord{});
    g_mirror.lastFlags.assign(vertexCount, 0);
    g_mirror.scratch.assign(vertexCount, 0.0f);
    g_mirror.vertsX = vertsX;
    PointWatersAtMirror();
}

void FreeMirror(const void* owner) {
    if (g_mirror.owner != owner) return;
    g_mirror = RawMirror{};
    PointWatersAtMirror();
}

void SyncMirror(fc4_terrain* core) {
    if (core == nullptr || g_mirror.vertices.empty() || g_mirror.vertsX == 0) return;
    const size_t n = g_mirror.vertices.size();
    const size_t got = fc4_terrain_altitudes(core, g_mirror.scratch.data(), n);
    const uint32_t vx = g_mirror.vertsX;
    for (size_t i = 0; i < n; ++i) {
        VertexRecord& r = g_mirror.vertices[i];
        const int32_t x = static_cast<int32_t>(i % vx), z = static_cast<int32_t>(i / vx);
        uint16_t coreFlags = fc4_terrain_vertex_flags(core, x, z);
        const uint16_t changed = static_cast<uint16_t>(r.flags ^ g_mirror.lastFlags[i]);
        if (changed != 0 && (changed & ~kViewWrittenFlags) == 0) {
            // §15.5.11 item 13: the view's LevelCell / RemoveCellLevelling write.
            coreFlags = static_cast<uint16_t>((coreFlags & ~changed) | (r.flags & changed));
            fc4_terrain_set_vertex_flags(core, x, z, coreFlags);
            ++g_mirror.viewFlagEdits;
        }
        r.flags = coreFlags;
        g_mirror.lastFlags[i] = coreFlags;
        if (i < got) r.altitude = g_mirror.scratch[i];
        g_mirror.water[i].depth = fc4_terrain_water_depth_at_index(core, static_cast<uint32_t>(i));
    }
    ++g_mirror.syncs;
}

}  // namespace


// foundations §2.7: a `ref` is `uint32 objectID` and, only when that is non-zero,
// `uint32 classID`. The referenced object's own data is NOT here. Read consumes
// exactly those bytes and leaves the pointer null: §13.2's terrain class record
// is "u16 version 2 and a reference to the terrain view", and §13.3's water
// record is "u16 1 and a reference (the terrain)". Under-reading either one
// desynchronises the component stream and every later Read gets garbage.
bool ReadRef(cIGZIStream& in) {
    uint32_t objectId = 0;
    if (!in.GetUint32(objectId)) return false;
    if (objectId == 0) return true;
    uint32_t classId = 0;
    return in.GetUint32(classId);
}

bool WriteNullRef(cIGZOStream& out) { return out.SetUint32(0); }

// ---- cFC4STESurfaceWater (§10) --------------------------------------------------------------------

class cFC4STESurfaceWater final : public cFC4ISTESurfaceWater, public cIGZSerializable {
public:
    // §15.5.11 item 14: the view reads water +0x18 directly; keep it pointing at the water mirror.
    cFC4STESurfaceWater() {
        static_assert(sizeof(void*) == 4, "the +0x18 layout is the 32-bit game's");
        g_waterRecordSlots.push_back(&raw_.records);
        PointWatersAtMirror();
    }

    bool QueryInterface(uint32_t riid, void** ppvObj) override {
        if (ppvObj == nullptr) return false;
        // The surface water's own IID is 0xAB8DA759 (observed: the city QIs
        // for it when it builds the object; not named in terrain.md or the SDK
        // header). terrain.md §1 also lists the generic 0x00000001 among the
        // terrain's IIDs, so it is answered here too.
        if (riid == GZIID_cIGZUnknown || riid == kFC4_IID_cISTESurfaceWater || riid == 0x00000001u ||
            riid == 0xAB953253u) {
            *ppvObj = Primary();
        } else if (riid == GZIID_cIGZSerializable) {
            *ppvObj = static_cast<cIGZSerializable*>(this);
        } else {
            return false;
        }
        AddRef();
        return true;
    }
    uint32_t AddRef() override { return ++refCount_; }
    uint32_t Release() override {
        if (refCount_ > 1) return --refCount_;
        refCount_ = 0;
        if (destroying_) return 0;
        destroying_ = true;
        delete this;
        return 0;
    }

    bool Init() override { return true; }
    bool Shutdown() override { return true; }

    float GetWaterDepthAtVertex(int32_t index) const override;                  // 5
    float GetWaterDepthForCell(int32_t x, int32_t z) const override;            // 6
    bool IsNavigableAtPosition(uint32_t, uint32_t, uint32_t, uint32_t, uint32_t) const override;  // 7
    bool IsNavigable(int32_t x, int32_t z, int32_t r, cISC4Lot* lot) const override;  // 8
    void* GetBodyOfWaterMap() const override;                                   // 9
    float AltitudeAtWhichFloodingWillOccur(int32_t x, int32_t z) override;      // 10
    void* GetWaterVelocity(int32_t x, int32_t z) override;                      // 11
    int32_t GetWaterType(int32_t x, int32_t z) override;                        // 12
    bool HasDraftClearance(int32_t x, int32_t z) override;                      // 13

    // §13.3: u16 1 and a reference to the terrain. The water array is rebuilt
    // from the sea level on every load (§10.1) and lakes are never saved
    // (quirk 9), so there is nothing else to write.
    // §13.3: u16 1 and a reference (read with IID 0xC70BD37E, the terrain).
    bool Write(cIGZOStream& out) override {
        Log("terrain: water Write (§13.3)");
        return out.SetUint16(1) && WriteNullRef(out);
    }
    bool Read(cIGZIStream& in) override {
        uint16_t version = 0;
        const bool ok = in.GetUint16(version) && ReadRef(in);
        Log("terrain: water Read (version + ref, §13.3) -> ok=%d version=%u", ok ? 1 : 0, version);
        return ok;
    }
    uint32_t GetGZCLSID() override { return kFC4_CLSID_cSTESurfaceWater; }

    cIGZUnknown* Primary() {
        return static_cast<cIGZUnknown*>(static_cast<cFC4ISTESurfaceWater*>(this));
    }

private:
    ~cFC4STESurfaceWater() {
        for (size_t i = 0; i < g_waterRecordSlots.size(); ++i) {
            if (g_waterRecordSlots[i] == &raw_.records) {
                g_waterRecordSlots.erase(g_waterRecordSlots.begin() + static_cast<std::ptrdiff_t>(i));
                break;
            }
        }
    }

    // First data member, after the two vptrs (+0x00, +0x04): records lands at +0x18 (§15.5.5).
    struct WaterFields {
        uint8_t pad[0x10];  // +0x08..+0x17
        void* records = nullptr;  // +0x18: the 0x24-byte water array
    } raw_{};

public:
    static void CheckLayout() {
        static_assert(offsetof(cFC4STESurfaceWater, raw_) + offsetof(WaterFields, records) == 0x18,
                      "§15.5.11 item 14: water +0x18");
    }

private:
    uint32_t refCount_ = 0;
    bool destroying_ = false;
    mutable std::vector<uint16_t> bodyMap_;              // slot 9's backing grid
    mutable float velocity_[3] = {0.0f, 0.0f, 0.0f};     // slot 11's shared vector
};

float cFC4STESurfaceWater::GetWaterDepthAtVertex(int32_t index) const {
    fc4_terrain* core = g_activeCore.load(std::memory_order_acquire);
    return core != nullptr ? fc4_terrain_water_depth_at_index(core, static_cast<uint32_t>(index)) : 0.0f;
}

float cFC4STESurfaceWater::GetWaterDepthForCell(int32_t x, int32_t z) const {
    fc4_terrain* core = g_activeCore.load(std::memory_order_acquire);
    return core != nullptr ? fc4_terrain_water_depth_for_cell(core, x, z) : 0.0f;
}

bool cFC4STESurfaceWater::IsNavigableAtPosition(uint32_t, uint32_t, uint32_t, uint32_t,
                                                uint32_t) const {
    // §10.2: converts the position to a cell (city slot 95) and calls slot 8.
    // The five argument dwords are not spelled out in the spec and the plugin
    // never calls this slot (FC4Interfaces.h), so it is refused rather than
    // guessed. See the lane report's numbered question.
    return false;
}

bool cFC4STESurfaceWater::IsNavigable(int32_t x, int32_t z, int32_t r, cISC4Lot* lot) const {
    fc4_terrain* core = g_activeCore.load(std::memory_order_acquire);
    if (core == nullptr) return false;
    return fc4_terrain_is_navigable(core, x, z, r, static_cast<uint64_t>(reinterpret_cast<uintptr_t>(lot)));
}

void* cFC4STESurfaceWater::GetBodyOfWaterMap() const {
    fc4_terrain* core = g_activeCore.load(std::memory_order_acquire);
    if (core == nullptr) return nullptr;
    const size_t n = static_cast<size_t>(g_cellsX.load(std::memory_order_acquire)) *
                     static_cast<size_t>(g_cellsZ.load(std::memory_order_acquire));
    bodyMap_.assign(n == 0 ? 1 : n, 0);
    if (n != 0) fc4_terrain_bodies_map(core, bodyMap_.data(), n);
    return bodyMap_.data();
}

float cFC4STESurfaceWater::AltitudeAtWhichFloodingWillOccur(int32_t x, int32_t z) {
    fc4_terrain* core = g_activeCore.load(std::memory_order_acquire);
    return core != nullptr ? fc4_terrain_flooding_altitude(core, x, z) : 0.0f;
}

void* cFC4STESurfaceWater::GetWaterVelocity(int32_t x, int32_t z) {
    fc4_terrain* core = g_activeCore.load(std::memory_order_acquire);
    if (core == nullptr || !fc4_terrain_water_velocity(core, x, z, velocity_)) {
        // §10.5: out of the array, a shared zero vector.
        velocity_[0] = velocity_[1] = velocity_[2] = 0.0f;
    }
    return velocity_;
}

int32_t cFC4STESurfaceWater::GetWaterType(int32_t x, int32_t z) {
    fc4_terrain* core = g_activeCore.load(std::memory_order_acquire);
    return core != nullptr ? static_cast<int32_t>(fc4_terrain_water_type(core, x, z)) : 0;
}

bool cFC4STESurfaceWater::HasDraftClearance(int32_t x, int32_t z) {
    fc4_terrain* core = g_activeCore.load(std::memory_order_acquire);
    return core != nullptr ? fc4_terrain_has_draft_clearance(core, x, z) : false;
}

// ---- cFC4STETerrain (§2-§9, §15.1) ---------------------------------------------------------------

class cFC4STETerrain final : public cFC4ISTETerrain, public cIGZMessageTarget2, public cIGZSerializable {
public:
    // MUST be the first data member: the game reads the original's fields at
    // the offsets of terrain.md §2.3 (FC4TerrainInterfaces.h TerrainFields).
    TerrainFields f_{};

    cFC4STETerrain();
    ~cFC4STETerrain();

    bool QueryInterface(uint32_t riid, void** ppvObj) override;
    uint32_t AddRef() override { return ++refCount_; }
    uint32_t Release() override;

    bool Init() override;      // 3
    bool Shutdown() override;  // 4

    bool IsInitialized() override { return f_.initialized != 0; }               // 5
    float CellWidth() override { return 16.0f; }                               // 6
    uint32_t CellCountX() override;                                            // 7
    uint32_t CellCountZ() override;                                            // 8
    int32_t GetVertIndex(int32_t x, int32_t z) const override;                 // 9
    void GetVertex(int32_t index, int32_t& x, int32_t& z) const override;      // 10
    bool LocationIsInBounds(float x, float z) const override;                  // 11
    float GetAltitudeAtFlatIndex(int32_t index) override;                      // 12
    float GetAltitudeAt(float x, float z) override;                            // 13
    float GetAltitudeAtVertex(int32_t x, int32_t z) override;                  // 14
    void GetAltitudes(SC4Rect<int32_t> const& r, float* out, int32_t rowStride,
                      int32_t colStride) override;                            // 15
    void GetWaterDepths(SC4Rect<int32_t> const& r, float* out, int32_t rowStride,
                        int32_t colStride) override;                          // 16
    void GetCellCornerAltitudes(uint32_t x, uint32_t z, float* out) override;  // 17
    float GetMaxSurfaceLevel(uint32_t x, uint32_t z) override;                 // 18
    float GetAltitudeAtNearestGrid(float x, float z) override;                 // 19
    float GetAltitudeCheckSum(SC4Rect<int32_t> const& r) const override;       // 20
    float MinHeight() override;                                                // 21
    float MaxHeight() override;                                                // 22
    float MinAllowedHeight() override;                                         // 23
    float MaxAllowedHeight() override;                                         // 24
    int32_t GetTerrainGridIndex(float x, float z) override;                    // 25
    void GetFaceNormal(float x, float z, cS3DVector3& out) override;           // 26
    void GetLerpedNormal(float, float, cS3DVector3&) override {}               // 27  render
    void GetVertexNormal(uint32_t, uint32_t, cS3DVector3&) override {}         // 28  render
    float GetSeaLevel() const override;                                        // 29
    float GetWaterLevelAt(float x, float z) override;                          // 30
    float GetWaterLevelAtIndex(int32_t index) override;                        // 31
    float GetLerpedWaterLevel(float x, float z) override;                      // 32
    float GetWaterDepthAtVertexXZ(int32_t x, int32_t z) override;              // 33
    bool CellHasWater(uint32_t x, uint32_t z) const override;                  // 34
    bool CellHasLand(uint32_t x, uint32_t z) const override;                   // 35
    bool IsLandVertex(uint32_t x, uint32_t z) override;                        // 36
    bool IsWaterVertex(uint32_t x, uint32_t z) override;                       // 37
    bool IsSeaVertex(uint32_t x, uint32_t z) override;                         // 38
    bool AreSeaVerticesIn(SC4Rect<int32_t> const& r) const override;           // 39
    void DoPostCityInit() override;                                            // 40
    bool SetTerrainType(int32_t t) override;                                   // 41
    int32_t TerrainType() override;                                            // 42
    void CreateFlatAreas(bool b) override;                                     // 43
    bool CreateShoreCliffs(bool b) override;                                   // 44
    bool SetSeaLevel(float h) override;                                        // 45
    bool ChangeSeaLevel(bool up) override;                                     // 46
    float GetDefaultTerrainRaiseLowerAmount() const override { return 20.0f; }  // 47
    void ChangeTerrainLevel(bool up, float amount) override;                   // 48
    void SetAltitudeRect(SC4Rect<int32_t> const& r, float h, bool redisplay, uint32_t data4) override;  // 49
    void SetAltitudeAtVertex(int32_t index, float h) override;                 // 50
    void SetAltitudeXZ(int32_t x, int32_t z, float h) override;                // 51
    void LockAltitude(SC4Rect<int32_t> const& r, bool lock) override;          // 52
    uint32_t NumberOfWaterVerticesIn(SC4Rect<int32_t> const& r) override;      // 53
    cISTETerrainView* GetRenderObject() override {
        return static_cast<cISTETerrainView*>(f_.renderObject);
    }                                                                           // 54
    float GetSedimentValue(float x, float z) override;                         // 55
    void StartFlood() override;                                                // 56
    bool ApplyBrush(uint32_t brushId, TBrushParameters& p) override;           // 57
    bool GetBrushExemplarID(int32_t, uint32_t&) override { return false; }     // 58
    void BeginBrushScrub(uint32_t a, uint32_t b) override;                   // 59
    void EndBrushScrub(uint32_t a, uint32_t b) override;                       // 60
    float BrushRadius(uint32_t) const override { return 0.0f; }                // 61
    BrushCost GetCostPerBrushApplication(uint32_t) override { return 0; }      // 62
    bool GetUIElementsForBrush(uint32_t, uint32_t*) const override { return false; }  // 63
    void UpdateWaveProducers() override {}                                     // 64  render
    bool IsGoodWaveProducerLocation(float, float, float) const override { return false; }  // 65  render
    cISTETerrainView* GetView() override {  // 66: *(T+0xE8) + 0x0C, never the base (§15.5.6)
        return f_.view == nullptr ? nullptr
                                  : reinterpret_cast<cISTETerrainView*>(static_cast<uint8_t*>(f_.view) +
                                                                        kViewInterfaceOffset);
    }
    void DoLighting(SC4Rect<int32_t> const&) override {}                       // 67  render
    void RedisplayTerrainRect(bool terrain, bool water, SC4Rect<int32_t> const& r,
                              uint32_t data4) override;                        // 68
    void RedisplayTerrainPoints(bool terrain, bool water, SC4Point<int32_t> const& a,
                                SC4Point<int32_t> const& b, uint32_t data4) override;  // 69
    bool IsTriangulationFlipped(uint32_t x, uint32_t z) override;              // 70
    void HideTerrainCell(uint32_t x, uint32_t z, bool hide) override;          // 71
    void HideEvenTriangle(uint32_t x, uint32_t z, bool hide) override;         // 72
    void HideOddTriangle(uint32_t x, uint32_t z, bool hide) override;          // 73
    bool IsEvenTriangleHidden(uint32_t x, uint32_t z) const override;          // 74
    bool IsOddTriangleHidden(uint32_t x, uint32_t z) const override;           // 75
    bool IsWholeCellHidden(uint32_t x, uint32_t z) const override;             // 76
    bool IsPointHidden(float x, float z) const override;                       // 77
    void StartAccumulatingTextureUpdates() override { f_.accumulateTexture = 1; }     // 78
    void StopAccumulatingTextureUpdates() override;                            // 79
    bool IsAccumulatingTextureUpdates() override { return f_.accumulateTexture != 0; }       // 80
    bool SetTerrainCellFlags(uint32_t x, uint32_t z, uint32_t flags) override; // 81
    uint32_t GetTerrainCellFlags(uint32_t x, uint32_t z) override;             // 82
    bool GetWaterSurfaceDisplayState() override;                               // 83
    void SetWaterSurfaceDisplayState(bool on) override;                        // 84
    bool GetPlumbingInfoDisplayState() override { return plumbingDisplay_; }   // 85
    void SetPlumbingInfoDisplayState(bool on) override { plumbingDisplay_ = on; }  // 86
    void BeginLongTerrainOperation() override;                                 // 87
    void EndLongTerrainOperation() override;                                   // 88
    void ReconcileEdgesWithNeighboringCities(bool, bool, bool) override {}     // 89
    bool SaveObliterated(cIGZPersistDBSegment*) override { return false; }     // 90
    void DeletingDestructor(uint32_t flags) override;                          // 91
    float GetWaterDepthAtFlatIndex(int32_t index) override;                    // 92
    bool FlipInternalTriangulation(uint32_t x, uint32_t z, bool flip) override;  // 93
    bool FlipExternalTriangulation(uint32_t x, uint32_t z, bool flip) override;  // 94
    bool LoadObliterated(cIGZPersistDBSegment*) override { return false; }     // 95
    bool Load(cIGZPersistDBSegment*) override;                                 // 96
    bool Save(cIGZPersistDBSegment*) override;                                 // 97

    bool DoMessage(cIGZMessage2* msg) override;  // cIGZMessageTarget2 (§12.1)

    // §13.2: u16 version 2 and a reference to the terrain view (class
    // 0xC9B84E10). We hold no view, so the reference is read and dropped.
    bool Write(cIGZOStream& out) override {
        Log("terrain: Write (class record, §13.2)");
        return out.SetUint16(2) && WriteNullRef(out);
    }
    bool Read(cIGZIStream& in) override {
        uint16_t version = 0;
        const bool ok = in.GetUint16(version) && version <= 2 && ReadRef(in);
        Log("terrain: Read (class record = version + ref, §13.2) -> ok=%d version=%u", ok ? 1 : 0, version);
        if (!ok) return false;
        // §13.2 (TR39): Read then loads the §13.1 records itself: segment = SC4GetIGZDBSegment(stream),
        // then slot 96 Load(segment), and returns Load's result. §15.5.13.5 [choice]: the saved view
        // reference is dropped (a fresh view is built in DoPostCityInit); its own record Read restores
        // the only view state, the static grid flag.
        const uintptr_t get = compat::DirectCall(compat::Direct::GetIGZDBSegment);
        if (get == 0) {
            Log("terrain: WARNING SC4GetIGZDBSegment unavailable; the saved grid is not loaded");
            return true;
        }
        using GetSegmentFn = cIGZPersistDBSegment*(__cdecl*)(cIGZIStream*);
        // Borrowed: the spec does not say the result carries a reference, so it is not released (§5.1b).
        cIGZPersistDBSegment* segment = reinterpret_cast<GetSegmentFn>(get)(&in);
        return Load(segment);
    }
    uint32_t GetGZCLSID() override { return kFC4_CLSID_cSTETerrain; }

    cIGZUnknown* Primary() { return static_cast<cIGZUnknown*>(static_cast<cFC4ISTETerrain*>(this)); }
    cIGZMessageTarget2* Target() { return static_cast<cIGZMessageTarget2*>(this); }

    // For the test seam.
    fc4_terrain* Core() const { return core_; }
    HostContext* Host() { return &host_; }

private:
    bool Enter(const char* what = __builtin_FUNCTION()) const;
    void Leave() const;
    bool CreateCore(uint32_t cellsX, uint32_t cellsZ);
    bool InstallCore(fc4_terrain* created);
    void DestroyCore();
    void StepSevenWholeCity();                                        // §15.5.13.3 "Order after Init"
    void CalculateNormalsAndAssignCliffs(const int32_t rect[4]);     // §15.5.13.2 (0x00743B60)
    void MarkCliff(uint32_t v, bool steep0, bool steep1, float t);   // §15.5.13.6
    void Subscribe();
    void Unsubscribe();
    void OnViewUpdate(cIGZMessage2Standard* s);

    cFC4STESurfaceWater* water_ = nullptr;  // the terrain's own (§1, +0x24)
    HostContext host_{};
    fc4_terrain_host hostTable_{};
    fc4_terrain* core_ = nullptr;
    uint32_t refCount_ = 0;
    bool destroying_ = false;
    bool subscribed_ = false;
    bool plumbingDisplay_ = false;  // §15.1: a global byte, not a field
    int32_t scrubbing_ = 0;

    mutable CRITICAL_SECTION lock_;
    mutable DWORD mainTid_ = 0;
    mutable int depth_ = 0;
};

// The three base sub-objects are three vptrs (§1: QueryInterface returns this,
// this + 4, this + 8), so the fields begin at +0x0C as terrain.md §2.3 says.
// offsetof on a polymorphic class is conditionally supported; MSVC accepts it,
// and this is exactly the property that has to hold.
static_assert(offsetof(cFC4STETerrain, f_) == 0x0C, "terrain fields start at +0x0C (§2.3)");

cFC4STETerrain::cFC4STETerrain() {
    InitializeCriticalSection(&lock_);
    // host_ default-constructs (its atomics make it non-assignable); only the
    // table has to be filled.
    hostTable_ = MakeHost(&host_);
    host_.self = Primary();
    // §1: the terrain owns the surface water at +0x24. The city may also build
    // one through the class factory; both are views over the same core.
    water_ = new cFC4STESurfaceWater();
    water_->AddRef();  // ours
    f_.surfaceWater = water_->Primary();
    f_.saved78 = 308.0f;  // §2.4: tuning 0x48CD7B20, shipped 308
    f_.neighbourOffset[0] = -1;              // §11.3: W, NW, N, NE, E, SE, S, SW
    f_.neighbourOffset[1] = -1;              // filled with vertsX once known
    f_.neighbourOffset[2] = 0;
    f_.neighbourOffset[3] = 1;
    f_.neighbourOffset[4] = 1;
    f_.neighbourOffset[5] = 1;
    f_.neighbourOffset[6] = 0;
    f_.neighbourOffset[7] = -1;
}

cFC4STETerrain::~cFC4STETerrain() {
    trace::StopMonitor();
    DestroyCore();
    if (f_.view != nullptr) {
        // The view came from GZCOM()->GetClassObject: its QI took one reference, on base + 0x0C.
        // [choice] its Shutdown (0x007560F0) is not called: the spec gives no caller (question TR35).
        FC4_RELEASE_CREATED(reinterpret_cast<cIGZUnknown*>(static_cast<uint8_t*>(f_.view) + kViewInterfaceOffset));
        f_.view = nullptr;
        f_.renderObject = nullptr;
    }
    f_.vertices = nullptr;
    FreeMirror(this);
    if (water_ != nullptr) {
        f_.surfaceWater = nullptr;
        water_->Release();
        water_ = nullptr;
    }
    DeleteCriticalSection(&lock_);
}

bool cFC4STETerrain::Enter(const char* what) const {
    if (!TryEnterCriticalSection(&lock_)) {
        // Diagnostic only: record that this thread had to wait, and for whom; then wait as before.
        trace::Note(what, 2, trace::g_lockOwner.load(std::memory_order_relaxed));
        EnterCriticalSection(&lock_);
    }
    if (mainTid_ == 0) mainTid_ = GetCurrentThreadId();
    if (++depth_ == 1) {
        trace::g_lockOwner.store(GetCurrentThreadId(), std::memory_order_relaxed);
        trace::g_lockSlot.store(what, std::memory_order_relaxed);
        trace::g_lockSince.store(GetTickCount(), std::memory_order_relaxed);
    }
    trace::Note(what, 1, static_cast<uint32_t>(depth_));
    return true;
}

void cFC4STETerrain::Leave() const {
    // Outermost exit: anything that posted a redisplay changed what the view will copy when the posted
    // message arrives (§15.5.8), so refresh the raw arrays now, before the message pump delivers it.
    if (depth_ == 1 && g_mirrorDirty.exchange(false, std::memory_order_acq_rel)) SyncMirror(core_);
    trace::Note(trace::g_lockSlot.load(std::memory_order_relaxed), -1, static_cast<uint32_t>(depth_));
    if (--depth_ == 0) {
        trace::g_lockOwner.store(0, std::memory_order_relaxed);
        trace::g_lockSlot.store(nullptr, std::memory_order_relaxed);
    }
    LeaveCriticalSection(&lock_);
}

bool cFC4STETerrain::QueryInterface(uint32_t riid, void** ppvObj) {
    if (ppvObj == nullptr) return false;
    if (riid == GZIID_cIGZUnknown || riid == kFC4_IID_cISTETerrain || riid == 0x00000001u ||
        riid == 0xAB953253u) {
        *ppvObj = Primary();
    } else if (riid == kFC4_IID_cIGZMessageTarget2) {
        *ppvObj = Target();
    } else if (riid == kFC4_IID_cIGZSerializable) {
        *ppvObj = static_cast<cIGZSerializable*>(this);
    } else {
        return false;
    }
    AddRef();
    return true;
}

uint32_t cFC4STETerrain::Release() {
    if (refCount_ > 1) return --refCount_;
    refCount_ = 0;
    if (destroying_) return 0;
    destroying_ = true;
    if (f_.initialized != 0) Shutdown();
    delete this;
    return 0;
}

bool cFC4STETerrain::CreateCore(uint32_t cellsX, uint32_t cellsZ) {
    if (core_ != nullptr) return true;
    fc4_terrain_tuning tuning{};
    tuning.struct_size = sizeof(tuning);
    if (!fc4_terrain_tuning_shipped(&tuning)) {
        tuning = fc4_terrain_tuning{};
        tuning.struct_size = sizeof(tuning);
    }
    fc4_terrain* created = fc4_terrain_create(&hostTable_, &tuning, cellsX, cellsZ);
    if (created == nullptr) {
        Log("terrain: could not create the terrain core (%u x %u cells)", cellsX, cellsZ);
        return false;
    }
    return InstallCore(created);
}

// Takes ownership of a fresh core (created or loaded) and fills the §2.3 fields from it.
bool cFC4STETerrain::InstallCore(fc4_terrain* created) {
    if (created == nullptr) return false;
    core_ = created;
    const uint32_t cellsX = fc4_terrain_cell_count(core_, 0), cellsZ = fc4_terrain_cell_count(core_, 1);
    fc4_terrain_set_default_brush_level(core_, f_.defaultBrushLevel);
    // §2.3: the grid fields the game reads directly.
    const uint32_t vx = cellsX + 1, vz = cellsZ + 1;
    f_.cellsX = cellsX;
    f_.cellsZ = cellsZ;
    f_.cellsXm1 = cellsX - 1;
    f_.cellsZm1 = cellsZ - 1;
    f_.vertsX = vx;
    f_.vertsZ = vz;
    f_.cityRect[0] = 0;
    f_.cityRect[1] = 0;
    f_.cityRect[2] = static_cast<int32_t>(cellsX);
    f_.cityRect[3] = static_cast<int32_t>(cellsZ);
    f_.sizeX = 16.0f * static_cast<float>(cellsX);
    f_.sizeZ = 16.0f * static_cast<float>(cellsZ);
    f_.vertexCount = vx * vz;
    f_.seaLevel = fc4_terrain_sea_level(core_);
    f_.cachedMin = fc4_terrain_cached_height(core_, 0);
    f_.cachedMax = fc4_terrain_cached_height(core_, 1);
    // §11.3: the 8 neighbour index offsets, W NW N NE E SE S SW.
    const int32_t vsx = static_cast<int32_t>(vx);
    f_.neighbourOffset[0] = -1;
    f_.neighbourOffset[1] = -1 - vsx;
    f_.neighbourOffset[2] = -vsx;
    f_.neighbourOffset[3] = 1 - vsx;
    f_.neighbourOffset[4] = 1;
    f_.neighbourOffset[5] = vsx + 1;
    f_.neighbourOffset[6] = vsx;
    f_.neighbourOffset[7] = vsx - 1;
    // §15.5.11 item 12: T+0x6C is a live 0x24-byte vertex array before anything draws.
    AllocateMirror(this, vx, vx * vz);
    f_.vertices = g_mirror.vertices.data();
    g_activeCore.store(core_, std::memory_order_release);
    g_cellsX.store(cellsX, std::memory_order_release);
    g_cellsZ.store(cellsZ, std::memory_order_release);
    return true;
}

void cFC4STETerrain::DestroyCore() {
    if (core_ == nullptr) return;
    g_activeCore.store(nullptr, std::memory_order_release);
    // The raw arrays stay allocated (and T+0x6C live) until the terrain object itself goes: the view
    // reads T+0x6C through its own terrain reference and may still draw after PreCityShutdown.
    fc4_terrain_destroy(core_);
    core_ = nullptr;
    g_cellsX.store(0, std::memory_order_release);
    g_cellsZ.store(0, std::memory_order_release);
}

bool cFC4STETerrain::Init() {
    Enter();
    trace::StartMonitor();  // diagnostic (gt3-r2 June stall); reports only after a SimNewDay
    bool ok = false;
    do {
        if (f_.initialized != 0) {
            ok = true;
            break;
        }
        f_.city = CurrentCity();  // borrowed (§2.3 +0x1C)
        cISC4City* city = static_cast<cISC4City*>(f_.city);
        // Slot 54 (§15.1: `+0xE4`, exported as the folded GetSelectedCity)
        // is read as the city by the game: cSC4City::Init calls a method on
        // what it returns and faults when it is null. terrain.md calls it a
        // render object; the in-game evidence says city. [choice]
        f_.renderObject = city;
        uint32_t cx = 64, cz = 64;
        if (city != nullptr) {
            // [abi-ret] city slots 93/94 CellCountX / CellCountZ (terrain.md
            // §2.1: "the city has cellsX x cellsZ cells ... read in Init"; the
            // export folds those two names). A count in EAX, never a bool.
            cx = city->CellCountX();
            cz = city->CellCountZ();
            if (cx == 0 || cz == 0 || cx > 4096 || cz > 4096) {
                Log("terrain: implausible city cell counts %u x %u; using 64 x 64", cx, cz);
                cx = cz = 64;
            }
        }
        // §15.5.12.2 / §15.5.11 item 18 (TR40): the terrain's per-process StaticInit fills the texture-id
        // count and the per-type texture-id lists the view's own StaticInit sizes its tables from. The
        // original calls it from Init, before DoPostCityInit creates the view; it guards itself.
        if (const uintptr_t si = compat::DirectCall(compat::Direct::TerrainStaticInit)) {
            using StaticInitFn = bool(__cdecl*)();
            const bool r = reinterpret_cast<StaticInitFn>(si)();
            FC4_X87_CHECK("cSTETerrain::StaticInit");
            Log("terrain: cSTETerrain::StaticInit -> %d (TR40)", r ? 1 : 0);
        } else {
            Log("terrain: WARNING cSTETerrain::StaticInit unavailable; the view's texture tables stay empty");
        }
        // §13.2 (TR39): on a city load Read already ran slot 96 Load, which installed the saved grid.
        if (core_ != nullptr && f_.cellsX == cx && f_.cellsZ == cz) {
            Log("terrain: Init keeps the grid Load installed (%u x %u cells)", cx, cz);
        } else {
            if (core_ != nullptr) {
                Log("terrain: WARNING the loaded grid %u x %u does not match the city %u x %u; rebuilt flat",
                    f_.cellsX, f_.cellsZ, cx, cz);
                DestroyCore();
            }
            if (!CreateCore(cx, cz)) break;
        }
        f_.initialized = 1;
        Subscribe();
        Log("terrain: replacement ON (enable = terrain): %u x %u cells, sea=%.9g, default brush level=%d", cx,
            cz, fc4_terrain_sea_level(core_), f_.defaultBrushLevel);
        ok = true;
    } while (false);
    Leave();
    return ok;
}

bool cFC4STETerrain::Shutdown() {
    trace::StopMonitor();
    Enter();
    if (core_ != nullptr) {
        fc4_terrain_state st{};
        st.struct_size = sizeof(st);
        if (fc4_terrain_get_state(core_, &st)) {
            Log("terrain: shutdown: posts_pending=%u brushes=%u reentrant refused core=%u guard=%u "
                "(both expected 0 outside god-mode brush kinds 1 and 8-10)",
                st.posts_pending, st.brush_count, st.reentrant_refused,
                host_.guardRefused.load(std::memory_order_relaxed));
        }
    }
    Unsubscribe();
    // §15.5.11 item 15a (TR35): the view's Shutdown (thiscall on the view base), then Release the one
    // reference the terrain owns, then +0xE8 := 0 and the view-ready byte +0xEC := 0.
    if (f_.view != nullptr) {
        if (const uintptr_t vs = compat::DirectCall(compat::Direct::TerrainView3DShutdown)) {
            using ViewShutdownFn = void(__fastcall*)(void* viewBase, void* edx);
            reinterpret_cast<ViewShutdownFn>(vs)(f_.view, nullptr);
            FC4_X87_CHECK("terrain view Shutdown");
        }
        FC4_RELEASE_CREATED(reinterpret_cast<cIGZUnknown*>(static_cast<uint8_t*>(f_.view) + kViewInterfaceOffset));
        f_.view = nullptr;
        Log("terrain: shutdown: the terrain 3D view shut down and released (TR35)");
    }
    f_.viewReady = 0;
    DestroyCore();
    f_.initialized = 0;
    f_.city = nullptr;
    Leave();
    return true;
}

void cFC4STETerrain::Subscribe() {
    if (subscribed_) return;
    cIGZMessageServer2Ptr ms;
    if (!ms) return;
    for (uint32_t type : kSubscribed) {
        if (ms->AddNotification(Target(), type)) subscribed_ = true;
    }
}

void cFC4STETerrain::Unsubscribe() {
    if (!subscribed_) return;
    cIGZMessageServer2Ptr ms;
    if (ms) {
        for (uint32_t type : kSubscribed) ms->RemoveNotification(Target(), type);
    }
    subscribed_ = false;
}

// terrain.md §15.5.1 / §15.5.7 / §15.5.11: DoPostCityInit (slot 40) creates and Inits the game's own 3D
// terrain view, and must finish before any other PostCityInit consumer calls slot 66 (item 17). The
// terrain subscribes to PostCityInit in its Init, before the zone developer does (§15.5.1).
void cFC4STETerrain::DoPostCityInit() {
    Enter();
    do {
        // Step 1: once (the guard byte at terrain +0x14, struct offset 0x08).
        if (f_._p01[0x07] != 0) break;
        f_._p01[0x07] = 1;
        if (core_ == nullptr) break;
        // Step 2 (TR9, §2.3): the original copies the 3D camera's zoom (global 0x00B43DD8, +0xC) into
        // +0xBC here. The plugin has no camera interface, so the level keeps its constructor value 0
        // until the first view-update message 0xA6B79602 arrives (conformance test 20).
        fc4_terrain_set_default_brush_level(core_, f_.defaultBrushLevel);
        // The view's Init and its first message handlers copy from T+0x6C / W+0x18: fill them first.
        g_mirrorDirty.store(false, std::memory_order_release);
        SyncMirror(core_);
        bool viewInited = false;
        if (f_.view == nullptr) {
            // Step 3: the GZCOM class factory, CLSID 0xC9B84E10, IID 0x6771477D; store out - 0x0C.
            // Step 4 (operator new + the ctor) is not taken: the factory is the game's own and the
            // plugin does not construct game objects by hand. A failure is logged, not faked.
            cIGZCOM* com = GZCOM();
            void* out = nullptr;
            if (com != nullptr && com->GetClassObject(kFC4_CLSID_cSTETerrainView, kIID_cISTETerrainView3D, &out) &&
                out != nullptr) {
                f_.view = static_cast<uint8_t*>(out) - kViewInterfaceOffset;
                Log("terrain: created the terrain 3D view (class 0x%08X, IID 0x%08X) at DoPostCityInit",
                    kFC4_CLSID_cSTETerrainView, kIID_cISTETerrainView3D);
            } else {
                Log("terrain: WARNING the terrain 3D view (class 0x%08X) could not be created; the zone "
                    "developer's PostCityInit path reads it (§15.5.1)",
                    kFC4_CLSID_cSTETerrainView);
            }
            // Step 5: view->Init(terrain), thiscall on the view base, one stack argument (§15.5.4).
            // Init dereferences the surface water global (filled from city slot 83) without a check:
            // refuse to call it when the city has no surface water. The preferences global 0x00B43CD8
            // is the game's own app object and is not reachable through an interface (§15.5.4).
            cISC4City* city = static_cast<cISC4City*>(f_.city);
            const uintptr_t init = compat::DirectCall(compat::Direct::TerrainView3DInit);
            if (f_.view != nullptr && init != 0 && city != nullptr && city->GetSurfaceWater() != nullptr) {
                using ViewInitFn = uint32_t(__fastcall*)(void* viewBase, void* edx, void* terrain);
                const uint32_t r = reinterpret_cast<ViewInitFn>(init)(f_.view, nullptr, Primary());
                FC4_X87_CHECK("terrain view Init");
                viewInited = r != 0;
                Log("terrain: terrain 3D view Init(terrain) -> %u (1 ok, 0 already, §15.5.4)", r);
            } else if (f_.view != nullptr) {
                Log("terrain: WARNING terrain 3D view Init NOT called (init=%d city=%d water=%d)", init != 0 ? 1 : 0,
                    city != nullptr ? 1 : 0, (city != nullptr && city->GetSurfaceWater() != nullptr) ? 1 : 0);
            }
        }
        // Step 6: the view-ready flag +0xEC (the §8.1 gate), then (§15.5.13.1, TR34) the weather
        // simulator's slot 5 DoCompleteSimulation (the SDK header names that slot DoPostCityInit; same
        // thiscall, no arguments, AL), result ignored. The original dereferences it unchecked; the
        // plugin skips a null one.
        fc4_terrain_post_city_init(core_);
        f_.viewReady = 1;
        cISC4City* const cityNow = static_cast<cISC4City*>(f_.city);
        if (cISC4WeatherSimulator* weather = cityNow != nullptr ? cityNow->GetWeatherSimulator() : nullptr) {
            weather->DoPostCityInit();
            FC4_X87_CHECK("weather DoCompleteSimulation");
        }
        // Step 7 (§15.5.13.2-.3, TR33/TR36/TR41): normals + cliff marks, then the texture-layer update,
        // on the whole city. Not RedisplayTerrain: the view's Init already copied the whole city.
        StepSevenWholeCity();
        (void)viewInited;
        // Steps 8-9 (+0xF0/+0xF4 := view +0x70/+0x74, +0xF8 := the overlay manager) are not kept:
        // those fields are not part of TerrainFields and nothing the replacement exports reads them.
        // renderObject (+0xE4, slot 54) stays the city: Init sets it so on purpose (in-game evidence).
    } while (false);
    Leave();
}

// ---- DoPostCityInit step 7 (terrain.md §15.5.13.2-.3, §15.5.13.6; TR33, TR41) --------------------

namespace {
constexpr uint16_t kFlagCliff = 0x0008;
constexpr uint16_t kFlagCliffFlip = 0x0800;

struct Tri {
    float x, y, z;
};

// §15.5.13.2 step 3: s = 1/sqrt(x^2 + y^2 + z^2) summed in that order, unrounded, then each component
// times s stored as float32. [choice] "unrounded" is the x87's 80-bit register; MSVC has no 80-bit
// type, so the sum and s are carried in double (at most a last-bit difference; see TR41 rules).
Tri Normalise(float x, float y, float z) {
    const double s = 1.0 / std::sqrt(static_cast<double>(x) * x + static_cast<double>(y) * y +
                                     static_cast<double>(z) * z);
    return Tri{static_cast<float>(x * s), static_cast<float>(y * s), static_cast<float>(z * s)};
}
}  // namespace

void cFC4STETerrain::MarkCliff(uint32_t v, bool steep0, bool steep1, float t) {
    const uint32_t W = f_.vertsX;
    const auto alt = [](uint32_t i) { return g_mirror.vertices[i].altitude; };
    const auto mark = [W](uint32_t i) { SetFlagBit(i % W, i / W, kFlagCliff, true); };
    if (steep0 != steep1) {
        mark(v);
        mark(v + W + 1);
        mark(steep1 ? v + 1 : v + W);
        return;
    }
    if (!steep0) return;
    // Both steep: the other diagonal, origin c = alt(v+W).
    const float c = alt(v + W);
    const float p = alt(v) - c, q = alt(v + 1) - c, r = alt(v + W + 1) - c;
    const float aqx = q * -16.0f, apx = p * -16.0f;
    const float anx = aqx - apx;
    const float apz = p * 16.0f, aqz = q * 0.0f;
    const float anz = apz - aqz;
    const float brz = r * -16.0f, bqz = q * 0.0f;
    const float bnz = brz - bqz;
    const float bqx = q * 16.0f, brx = r * 16.0f;
    const float bnx = bqx - brx;
    // NaN compares false: not steep (§15.5.13.6).
    const double ya = std::fabs(1.0 / std::sqrt(static_cast<double>(anz) * anz + static_cast<double>(anx) * anx +
                                                65536.0) * 256.0);
    const double yb = std::fabs(1.0 / std::sqrt(static_cast<double>(bnx) * bnx + static_cast<double>(bnz) * bnz +
                                                65536.0) * 256.0);
    const bool steepA = ya < t, steepB = yb < t;
    if (steepA && steepB) {
        mark(v);
        mark(v + 1);
        mark(v + W);
        mark(v + W + 1);
        return;
    }
    // The flip removes at least one steep triangle: the terrain's own slot 93 (virtual), then 0x0800.
    FlipInternalTriangulation(v % W, v / W, true);
    SetFlagBit(v % W, v / W, kFlagCliffFlip, true);
    if (steepA) {
        mark(v);
        mark(v + W);
        mark(v + 1);
    }
    if (steepB) {
        mark(v + 1);
        mark(v + W);
        mark(v + W + 1);
    }
}

void cFC4STETerrain::CalculateNormalsAndAssignCliffs(const int32_t rect[4]) {
    if (core_ == nullptr || g_mirror.vertices.empty()) return;
    const int32_t cx = static_cast<int32_t>(f_.cellsX), cz = static_cast<int32_t>(f_.cellsZ);
    const uint32_t W = f_.vertsX;
    const int32_t x0 = rect[0], z0 = rect[1], x1 = rect[2], z1 = rect[3];
    // [choice] the cliff threshold T is 0x00B0D620 = tuning 0x48CD7B65 (§2.4, shipped 0.5); the core's
    // shipped tuning carries it as cliff_slope.
    fc4_terrain_tuning tuning{};
    tuning.struct_size = sizeof(tuning);
    const float t = fc4_terrain_tuning_shipped(&tuning) ? tuning.cliff_slope : 0.5f;
    // Step 1: clear 0x0008 on the ungrown rectangle.
    for (int32_t z = std::max(z0, 0); z <= std::min(z1, cz); ++z)
        for (int32_t x = std::max(x0, 0); x <= std::min(x1, cx); ++x)
            SetFlagBit(static_cast<uint32_t>(x), static_cast<uint32_t>(z), kFlagCliff, false);
    // Step 2: grow the low side by one, clamp the high side to the cell range, clear 0x0800.
    const int32_t gx0 = std::max(x0 - 1, 0), gz0 = std::max(z0 - 1, 0);
    const int32_t gx1 = std::min(x1, cx - 1), gz1 = std::min(z1, cz - 1);
    for (int32_t z = gz0; z <= gz1; ++z)
        for (int32_t x = gx0; x <= gx1; ++x)
            SetFlagBit(static_cast<uint32_t>(x), static_cast<uint32_t>(z), kFlagCliffFlip, false);
    if (gx1 < gx0 || gz1 < gz0) return;
    // Step 3: two triangle normals per cell (a two-row buffer in the original; the whole block here,
    // which changes nothing: MarkCliff writes flags only).
    const int32_t bw = gx1 - gx0 + 1;
    std::vector<Tri> tri(static_cast<size_t>(bw) * (gz1 - gz0 + 1) * 2);
    const auto alt = [&](int32_t x, int32_t z) { return g_mirror.vertices[static_cast<size_t>(z) * W + x].altitude; };
    const auto at = [&](int32_t x, int32_t z, int k) -> Tri& {
        return tri[(static_cast<size_t>(z - gz0) * bw + (x - gx0)) * 2 + k];
    };
    for (int32_t z = gz0; z <= gz1; ++z) {
        for (int32_t x = gx0; x <= gx1; ++x) {
            const float a = alt(x, z), b = alt(x + 1, z), c = alt(x, z + 1), d = alt(x + 1, z + 1);
            const float ca = c - a, da = d - a, ba = b - a;
            const float ca16 = 16.0f * ca, da16 = 16.0f * da, ba16 = 16.0f * ba, da0 = 0.0f * da;
            const Tri t0 = Normalise(ca16 - da16, 256.0f, da0 - ca16);
            const Tri t1 = Normalise(da0 - ba16, 256.0f, ba16 - da16);
            at(x, z, 0) = t0;
            at(x, z, 1) = t1;
            const bool s0 = std::fabs(t0.y) < t, s1 = std::fabs(t1.y) < t;
            if (s0 || s1) MarkCliff(static_cast<uint32_t>(z) * W + static_cast<uint32_t>(x), s0, s1, t);
        }
    }
    // Step 4: vertex normals of rows z0..gz1 (quirk: never the last vertex row z = cellsZ).
    for (int32_t z = std::max(z0, gz0); z <= gz1; ++z) {
        for (int32_t x = std::max(x0, 0); x <= std::min(x1, cx); ++x) {
            float sx = 0.0f, sy = 0.0f, sz = 0.0f;
            const auto add = [&](int32_t cxi, int32_t czi, int k) {
                if (cxi < gx0 || cxi > gx1 || czi < gz0 || czi > gz1) return;  // outside the computed block
                const Tri& n = at(cxi, czi, k);
                sx += n.x;
                sy += n.y;
                sz += n.z;
            };
            if (x > 0 && z < cz) add(x - 1, z, 1);
            if (x > 0 && z > 0) {
                add(x - 1, z - 1, 0);
                add(x - 1, z - 1, 1);
            }
            if (x < cx && z < cz) {
                add(x, z, 0);
                add(x, z, 1);
            }
            if (x < cx && z > 0) add(x, z - 1, 0);
            if (sx == 0.0f && sy == 0.0f && sz == 0.0f) sx = sy = sz = 1.0f;
            const Tri n = Normalise(sx, sy, sz);
            VertexRecord& r = g_mirror.vertices[static_cast<size_t>(z) * W + x];
            r.normal[0] = n.x;
            r.normal[1] = n.y;
            r.normal[2] = n.z;
        }
    }
}

void cFC4STETerrain::StepSevenWholeCity() {
    // Part 1 on +0x40, the whole city.
    CalculateNormalsAndAssignCliffs(f_.cityRect);
    // Part 2, the texture-layer update 0x007490B0 (§15.5.13.3 steps 1-8, §15.5.13.6): not implemented
    // in this round; the record layers stay empty (id 0).
    Log("terrain: DoPostCityInit step 7: normals and cliff marks on the whole city; texture layers NOT "
        "computed yet (§15.5.13.3)");
}

// TR9 (§2.3): "each view-update message 0xA6B79602 whose first data word
// differs from it stores that word". [choice] the literal reading is data1.
// zones.md / cFC4ZoneDeveloper read the same message's *void2 (a pointer to a
// u32); the lane run logs both so the difference can be settled. A word above
// the five brush levels is not stored: the level is an index 0-4 (§9.2).
void cFC4STETerrain::OnViewUpdate(cIGZMessage2Standard* s) {
    if (s == nullptr || core_ == nullptr) return;
    const uint32_t d1 = static_cast<uint32_t>(s->GetData1());
    uint32_t viaPtr = 0;
    bool havePtr = false;
    if (s->GetVoid2() != nullptr) {
        std::memcpy(&viaPtr, s->GetVoid2(), sizeof(viaPtr));
        havePtr = true;
    }
    const int32_t want = (d1 <= 4) ? static_cast<int32_t>(d1) : 0;
    if (havePtr && viaPtr != d1) {
        Log("terrain: TR9 view update 0xA6B79602 data1=%u *void2=%u -> brush level %d (was %d)", d1, viaPtr,
            want, f_.defaultBrushLevel);
    }
    if (want != f_.defaultBrushLevel) {
        f_.defaultBrushLevel = want;
        fc4_terrain_set_default_brush_level(core_, f_.defaultBrushLevel);
        Log("terrain: TR9 default brush level -> %d (view update)", f_.defaultBrushLevel);
    }
}

bool cFC4STETerrain::DoMessage(cIGZMessage2* msg) {
    if (msg == nullptr) return true;
    FC4_X87_SCOPE("terrain DoMessage", msg->GetType());
    const uint32_t type = msg->GetType();
    trace::Note("DoMessage", 1, type);
    if (type == kMsgSimNewDay) trace::g_lastNewDay.store(GetTickCount(), std::memory_order_release);
    cIGZMessage2Standard* s = nullptr;
    if (msg->QueryInterface(GZIID_cIGZMessage2Standard, reinterpret_cast<void**>(&s)) && s != nullptr) {
        FC4_ACQUIRED(s, "QueryInterface");
    }
    switch (type) {
        case kMsgPostCityInit: DoPostCityInit(); break;
        case kMsgPreCityShutdown: Shutdown(); break;
        case kMsgViewUpdate:
            Enter();
            OnViewUpdate(s);
            Leave();
            break;
        case kMsgViewWindow:
            // §12.1: refresh the textures, then RedisplayTerrain(1, 1, the whole
            // city, 1). Only the renderer reads the texture half.
            Enter();
            if (core_ != nullptr) {
                const int32_t cx = static_cast<int32_t>(fc4_terrain_cell_count(core_, 0));
                const int32_t cz = static_cast<int32_t>(fc4_terrain_cell_count(core_, 1));
                fc4_terrain_rect r{0, 0, cx, cz};
                fc4_terrain_redisplay(core_, true, true, r, 1);
            }
            Leave();
            break;
        case kMsgAnimationTick: break;  // §12.1: render work and the debug flood only
        case kMsgReconcile: break;      // §12.1: the deferred edge reconciliation
        case kMsgSimNewDay:
            Enter();
            if (core_ != nullptr) fc4_terrain_bodies_on_new_day(core_);  // §10.3
            Leave();
            break;
        default: break;
    }
    if (s != nullptr) FC4_RELEASE(s);
    trace::Note("DoMessage", -1, type);
    return true;
}

// ---- the height queries (§4) ----------------------------------------------------------------------

uint32_t cFC4STETerrain::CellCountX() {
    return core_ != nullptr ? fc4_terrain_cell_count(core_, 0) : 0u;
}
uint32_t cFC4STETerrain::CellCountZ() {
    return core_ != nullptr ? fc4_terrain_cell_count(core_, 1) : 0u;
}

int32_t cFC4STETerrain::GetVertIndex(int32_t x, int32_t z) const {
    return core_ != nullptr ? static_cast<int32_t>(fc4_terrain_vert_index(core_, x, z)) : 0;
}

void cFC4STETerrain::GetVertex(int32_t index, int32_t& x, int32_t& z) const {
    uint32_t ux = 0, uz = 0;
    if (core_ != nullptr && fc4_terrain_vertex_of(core_, static_cast<uint32_t>(index), &ux, &uz)) {
        x = static_cast<int32_t>(ux);
        z = static_cast<int32_t>(uz);
    }
}

bool cFC4STETerrain::LocationIsInBounds(float x, float z) const {
    return core_ != nullptr ? fc4_terrain_location_is_in_bounds(core_, x, z) : false;
}

float cFC4STETerrain::GetAltitudeAtFlatIndex(int32_t index) {
    return core_ != nullptr ? fc4_terrain_altitude_at_index(core_, static_cast<uint32_t>(index)) : 0.0f;
}

float cFC4STETerrain::GetAltitudeAt(float x, float z) {
    return core_ != nullptr ? fc4_terrain_altitude(core_, x, z) : 0.0f;
}

float cFC4STETerrain::GetAltitudeAtVertex(int32_t x, int32_t z) {
    return core_ != nullptr ? fc4_terrain_altitude_at_vertex(core_, x, z) : 0.0f;
}

void cFC4STETerrain::GetAltitudes(SC4Rect<int32_t> const& r, float* out, int32_t rowStride,
                                  int32_t colStride) {
    if (core_ == nullptr || out == nullptr) return;
    // §4.2: an inclusive vertex rectangle. *[decomp only]* for the strides;
    // row-major with the caller's strides is the literal reading. [choice]
    const int32_t w = r.bottomRightX - r.topLeftX + 1;
    const int32_t h = r.bottomRightY - r.topLeftY + 1;
    if (w <= 0 || h <= 0) return;
    std::vector<float> buf(static_cast<size_t>(w) * static_cast<size_t>(h));
    fc4_terrain_rect rect{r.topLeftX, r.topLeftY, r.bottomRightX, r.bottomRightY};
    const size_t n = fc4_terrain_altitudes_in(core_, rect, buf.data(), buf.size());
    for (int32_t z = 0; z < h; ++z) {
        for (int32_t x = 0; x < w; ++x) {
            const size_t i = static_cast<size_t>(z) * static_cast<size_t>(w) + static_cast<size_t>(x);
            if (i >= n) return;
            out[static_cast<size_t>(z) * static_cast<size_t>(rowStride) +
                static_cast<size_t>(x) * static_cast<size_t>(colStride)] = buf[i];
        }
    }
}

void cFC4STETerrain::GetWaterDepths(SC4Rect<int32_t> const& r, float* out, int32_t rowStride,
                                    int32_t colStride) {
    if (core_ == nullptr || out == nullptr) return;
    const int32_t w = r.bottomRightX - r.topLeftX + 1;
    const int32_t h = r.bottomRightY - r.topLeftY + 1;
    if (w <= 0 || h <= 0) return;
    for (int32_t z = 0; z < h; ++z) {
        for (int32_t x = 0; x < w; ++x) {
            out[static_cast<size_t>(z) * static_cast<size_t>(rowStride) +
                static_cast<size_t>(x) * static_cast<size_t>(colStride)] =
                fc4_terrain_water_depth_at_vertex(core_, r.topLeftX + x, r.topLeftY + z);
        }
    }
}

void cFC4STETerrain::GetCellCornerAltitudes(uint32_t x, uint32_t z, float* out) {
    if (core_ == nullptr || out == nullptr) return;
    fc4_terrain_cell_corner_altitudes(core_, static_cast<int32_t>(x), static_cast<int32_t>(z), out);
}

float cFC4STETerrain::GetMaxSurfaceLevel(uint32_t x, uint32_t z) {
    return core_ != nullptr ? fc4_terrain_max_surface_level(core_, static_cast<int32_t>(x),
                                                            static_cast<int32_t>(z))
                            : 0.0f;
}

float cFC4STETerrain::GetAltitudeAtNearestGrid(float x, float z) {
    return core_ != nullptr ? fc4_terrain_altitude_at_nearest_grid(core_, x, z) : 0.0f;
}

float cFC4STETerrain::GetAltitudeCheckSum(SC4Rect<int32_t> const& r) const {
    if (core_ == nullptr) return 0.0f;
    fc4_terrain_rect rect{r.topLeftX, r.topLeftY, r.bottomRightX, r.bottomRightY};
    return fc4_terrain_altitude_checksum(core_, rect);
}

float cFC4STETerrain::MinHeight() {
    return core_ != nullptr ? fc4_terrain_cached_height(core_, 0) : 0.0f;
}
float cFC4STETerrain::MaxHeight() {
    return core_ != nullptr ? fc4_terrain_cached_height(core_, 1) : 0.0f;
}
float cFC4STETerrain::MinAllowedHeight() {
    return core_ != nullptr ? fc4_terrain_allowed_height(core_, 0) : 0.0f;
}
float cFC4STETerrain::MaxAllowedHeight() {
    return core_ != nullptr ? fc4_terrain_allowed_height(core_, 1) : 0.0f;
}

int32_t cFC4STETerrain::GetTerrainGridIndex(float, float) {
    // §15.1 lists the slot with no rule and §4-§10 never defines it. Refused
    // rather than guessed. See the lane report's numbered question.
    return -1;
}

void cFC4STETerrain::GetFaceNormal(float x, float z, cS3DVector3& out) {
    if (core_ == nullptr) return;  // §4.3: out is untouched out of bounds
    float v[3] = {out.fX, out.fY, out.fZ};
    if (fc4_terrain_face_normal(core_, x, z, v)) {
        out.fX = v[0];
        out.fY = v[1];
        out.fZ = v[2];
    }
}

// ---- the water queries (§5) -----------------------------------------------------------------------

float cFC4STETerrain::GetSeaLevel() const {
    return core_ != nullptr ? fc4_terrain_sea_level(core_) : 0.0f;
}
float cFC4STETerrain::GetWaterLevelAt(float x, float z) {
    return core_ != nullptr ? fc4_terrain_water_level(core_, x, z) : 0.0f;
}
float cFC4STETerrain::GetWaterLevelAtIndex(int32_t index) {
    return core_ != nullptr ? fc4_terrain_water_level_at_index(core_, static_cast<uint32_t>(index)) : 0.0f;
}
float cFC4STETerrain::GetLerpedWaterLevel(float x, float z) {
    return core_ != nullptr ? fc4_terrain_lerped_water_level(core_, x, z) : 0.0f;
}
float cFC4STETerrain::GetWaterDepthAtVertexXZ(int32_t x, int32_t z) {
    return core_ != nullptr ? fc4_terrain_water_depth_at_vertex(core_, x, z) : 0.0f;
}
bool cFC4STETerrain::CellHasWater(uint32_t x, uint32_t z) const {
    return core_ != nullptr
               ? fc4_terrain_cell_has_water(core_, static_cast<int32_t>(x), static_cast<int32_t>(z))
               : false;
}
bool cFC4STETerrain::CellHasLand(uint32_t x, uint32_t z) const {
    return core_ != nullptr ? fc4_terrain_cell_has_land(core_, static_cast<int32_t>(x), static_cast<int32_t>(z))
                            : false;
}
bool cFC4STETerrain::IsLandVertex(uint32_t x, uint32_t z) {
    return core_ != nullptr ? fc4_terrain_is_land_vertex(core_, static_cast<int32_t>(x), static_cast<int32_t>(z))
                            : false;
}
bool cFC4STETerrain::IsWaterVertex(uint32_t x, uint32_t z) {
    return core_ != nullptr ? fc4_terrain_is_water_vertex(core_, static_cast<int32_t>(x), static_cast<int32_t>(z))
                            : false;
}
bool cFC4STETerrain::IsSeaVertex(uint32_t x, uint32_t z) {
    return core_ != nullptr ? fc4_terrain_is_sea_vertex(core_, static_cast<int32_t>(x), static_cast<int32_t>(z))
                            : false;
}

bool cFC4STETerrain::AreSeaVerticesIn(SC4Rect<int32_t> const& r) const {
    if (core_ == nullptr) return false;
    // §15.1 gives the name only. The literal reading: every vertex of the
    // inclusive rectangle is a sea vertex (§5: altitude < sea level). [choice]
    for (int32_t z = r.topLeftY; z <= r.bottomRightY; ++z) {
        for (int32_t x = r.topLeftX; x <= r.bottomRightX; ++x) {
            if (!fc4_terrain_is_sea_vertex(core_, x, z)) return false;
        }
    }
    return true;
}

float cFC4STETerrain::GetSedimentValue(float x, float z) {
    return core_ != nullptr ? fc4_terrain_sediment_at(core_, x, z) : 0.0f;
}

float cFC4STETerrain::GetWaterDepthAtFlatIndex(int32_t index) {
    return core_ != nullptr ? fc4_terrain_water_depth_at_index(core_, static_cast<uint32_t>(index)) : 0.0f;
}

// ---- the setters (§6) -----------------------------------------------------------------------------

bool cFC4STETerrain::SetTerrainType(int32_t t) {
    Enter();
    const bool ok = core_ != nullptr && fc4_terrain_set_terrain_type(core_, t);
    Leave();
    return ok;
}

int32_t cFC4STETerrain::TerrainType() {
    return core_ != nullptr ? fc4_terrain_terrain_type(core_) : 0;
}

void cFC4STETerrain::CreateFlatAreas(bool) {
    // §15.1: data4 0. The two terrain generators have no capi entry point.
    Log("terrain: CreateFlatAreas is not implemented");
}

bool cFC4STETerrain::CreateShoreCliffs(bool) {
    Log("terrain: CreateShoreCliffs is not implemented");
    return false;
}

bool cFC4STETerrain::SetSeaLevel(float h) {
    Enter();
    const bool ok = core_ != nullptr && fc4_terrain_set_sea_level(core_, h);  // §6
    Leave();
    return ok;
}

bool cFC4STETerrain::ChangeSeaLevel(bool up) {
    Enter();
    bool ok = false;
    if (core_ != nullptr) {
        // §15.1 groups this with SetSeaLevel; §6 gives no step for it. The
        // literal reading of the name: move the sea by the default raise/lower
        // amount (slot 47) in the given direction. [choice]
        const float step = GetDefaultTerrainRaiseLowerAmount();
        ok = fc4_terrain_set_sea_level(core_, fc4_terrain_sea_level(core_) + (up ? step : -step));
    }
    Leave();
    return ok;
}

void cFC4STETerrain::ChangeTerrainLevel(bool up, float amount) {
    Enter();
    if (core_ != nullptr) {
        // §8.2 assigns data4 1 to this call but §6 has no rule for its shape.
        // The literal reading of the name: raise or lower every vertex by
        // `amount` (the setters' locked-vertex and clamp rules apply). [choice]
        const uint32_t cx = fc4_terrain_cell_count(core_, 0);
        const uint32_t cz = fc4_terrain_cell_count(core_, 1);
        for (uint32_t z = 0; z <= cz; ++z) {
            for (uint32_t x = 0; x <= cx; ++x) {
                const uint32_t i = fc4_terrain_vert_index(core_, static_cast<int32_t>(x), static_cast<int32_t>(z));
                if (up) {
                    fc4_terrain_raise_vertex(core_, i, amount);
                } else {
                    fc4_terrain_lower_vertex(core_, i, amount);
                }
            }
        }
        fc4_terrain_rect r{0, 0, static_cast<int32_t>(cx), static_cast<int32_t>(cz)};
        fc4_terrain_redisplay(core_, true, true, r, 1);
    }
    Leave();
}

void cFC4STETerrain::SetAltitudeRect(SC4Rect<int32_t> const& r, float h, bool redisplay, uint32_t data4) {
    Enter();
    if (core_ != nullptr) {
        fc4_terrain_rect rect{r.topLeftX, r.topLeftY, r.bottomRightX, r.bottomRightY};
        fc4_terrain_set_altitude_rect(core_, rect, h, redisplay, data4);  // §6
    }
    Leave();
}

void cFC4STETerrain::SetAltitudeAtVertex(int32_t index, float h) {
    Enter();
    if (core_ != nullptr) fc4_terrain_set_altitude_at_vertex(core_, static_cast<uint32_t>(index), h);
    Leave();
}

void cFC4STETerrain::SetAltitudeXZ(int32_t x, int32_t z, float h) {
    Enter();
    if (core_ != nullptr) fc4_terrain_set_altitude(core_, x, z, h);
    Leave();
}

void cFC4STETerrain::LockAltitude(SC4Rect<int32_t> const& r, bool lock) {
    Enter();
    if (core_ != nullptr) {
        fc4_terrain_rect rect{r.topLeftX, r.topLeftY, r.bottomRightX, r.bottomRightY};
        fc4_terrain_lock_altitude(core_, rect, lock);
    }
    Leave();
}

uint32_t cFC4STETerrain::NumberOfWaterVerticesIn(SC4Rect<int32_t> const& r) {
    if (core_ == nullptr) return 0;
    fc4_terrain_rect rect{r.topLeftX, r.topLeftY, r.bottomRightX, r.bottomRightY};
    return fc4_terrain_number_of_water_vertices(core_, rect);
}

void cFC4STETerrain::StartFlood() {
    // §15.1: debug, +500 m of water on vertices 20..39 x 20..39. The harness
    // never drives it; refused rather than half-done.
    Log("terrain: StartFlood (debug) is not implemented");
}

// ---- the brushes (§9) -----------------------------------------------------------------------------

bool cFC4STETerrain::ApplyBrush(uint32_t brushId, TBrushParameters& p) {
    Enter();
    bool ok = false;
    if (core_ != nullptr) {
        fc4_brush_params bp{};
        bp.from_x = p.fromX;
        bp.from_z = p.fromZ;
        bp.to_x = p.toX;
        bp.to_z = p.toZ;
        bp.level = p.level;
        bp.redisplay = p.redisplay;
        bp.strength_scale = p.strength;
        bp.radius_scale = p.radiusScaling;
        bp.data4 = p.data4;
        bp.target_height = p.targetHeight;
        // TR18: a brush may call our host callbacks. Anything that comes back
        // into this handle is refused at the boundary and counted; the core
        // counts its own refusals too. Both stay 0 for the kinds the
        // simulation drives (0, 2, 5, 7), per §15.4.
        ++host_.inside;
        ok = fc4_terrain_apply_brush(core_, brushId, &bp);
        --host_.inside;
        p.outX0 = bp.out.x0;
        p.outZ0 = bp.out.z0;
        p.outX1 = bp.out.x1;
        p.outZ1 = bp.out.z1;
    }
    Leave();
    return ok;
}

void cFC4STETerrain::BeginBrushScrub(uint32_t a, uint32_t b) {
    (void)a;
    (void)b;
    Enter();
    ++scrubbing_;
    Leave();
}

void cFC4STETerrain::EndBrushScrub(uint32_t, uint32_t) {
    Enter();
    if (core_ != nullptr) {
        if (scrubbing_ > 0) --scrubbing_;
        // §9.6: EndBrushScrub makes the surface water rebuild its bodies.
        if (scrubbing_ == 0) fc4_terrain_update_bodies_of_water(core_);
    }
    Leave();
}

// ---- RedisplayTerrain and the long operations (§8) ------------------------------------------------

void cFC4STETerrain::RedisplayTerrainRect(bool terrain, bool water, SC4Rect<int32_t> const& r,
                                          uint32_t data4) {
    Enter();
    if (core_ != nullptr) {
        fc4_terrain_rect rect{r.topLeftX, r.topLeftY, r.bottomRightX, r.bottomRightY};
        fc4_terrain_redisplay(core_, terrain, water, rect, data4);  // §8.1
    }
    Leave();
}

void cFC4STETerrain::RedisplayTerrainPoints(bool terrain, bool water, SC4Point<int32_t> const& a,
                                            SC4Point<int32_t> const& b, uint32_t data4) {
    Enter();
    if (core_ != nullptr) {
        // §8.1: two cell corners, sorted, x1 := max + 1 clamped to cellsX.
        fc4_terrain_redisplay_cells(core_, terrain, water, a.x, a.y, b.x, b.y, data4);
    }
    Leave();
}

void cFC4STETerrain::BeginLongTerrainOperation() {
    Enter();
    if (core_ != nullptr) fc4_terrain_begin_long_operation(core_);  // §8.3
    Leave();
}

void cFC4STETerrain::EndLongTerrainOperation() {
    Enter();
    if (core_ != nullptr) fc4_terrain_end_long_operation(core_);  // §8.3
    Leave();
}

void cFC4STETerrain::StopAccumulatingTextureUpdates() {
    f_.accumulateTexture = 0;
    Enter();
    if (core_ != nullptr) {
        // §8.2: a texture-only redisplay (data4 0). Drawing is out of scope,
        // but the post is what the simulation can see.
        const int32_t cx = static_cast<int32_t>(fc4_terrain_cell_count(core_, 0));
        const int32_t cz = static_cast<int32_t>(fc4_terrain_cell_count(core_, 1));
        fc4_terrain_rect r{0, 0, cx, cz};
        fc4_terrain_redisplay(core_, false, false, r, 0);
    }
    Leave();
}

// ---- the flags (§7) -------------------------------------------------------------------------------

bool cFC4STETerrain::IsTriangulationFlipped(uint32_t x, uint32_t z) {
    return core_ != nullptr ? fc4_terrain_is_triangulation_flipped(core_, static_cast<int32_t>(x),
                                                                  static_cast<int32_t>(z))
                            : false;
}

void cFC4STETerrain::HideTerrainCell(uint32_t x, uint32_t z, bool hide) {
    // §7: 0x20 | 0x40 hides the whole cell.
    SetFlagBit(x, z, kFlagEvenHidden, hide);
    SetFlagBit(x, z, kFlagOddHidden, hide);
}
void cFC4STETerrain::HideEvenTriangle(uint32_t x, uint32_t z, bool hide) {
    SetFlagBit(x, z, kFlagEvenHidden, hide);
}
void cFC4STETerrain::HideOddTriangle(uint32_t x, uint32_t z, bool hide) {
    SetFlagBit(x, z, kFlagOddHidden, hide);
}
bool cFC4STETerrain::IsEvenTriangleHidden(uint32_t x, uint32_t z) const {
    return (VertexFlags(x, z) & kFlagEvenHidden) != 0;
}
bool cFC4STETerrain::IsOddTriangleHidden(uint32_t x, uint32_t z) const {
    return (VertexFlags(x, z) & kFlagOddHidden) != 0;
}
bool cFC4STETerrain::IsWholeCellHidden(uint32_t x, uint32_t z) const {
    const uint16_t f = VertexFlags(x, z);
    return (f & (kFlagEvenHidden | kFlagOddHidden)) == (kFlagEvenHidden | kFlagOddHidden);
}

bool cFC4STETerrain::IsPointHidden(float x, float z) const {
    // §7 lists the hide slots together but gives no point rule. The literal
    // reading: the whole cell the point falls in. [choice]
    const int32_t cx = static_cast<int32_t>(x / 16.0f);
    const int32_t cz = static_cast<int32_t>(z / 16.0f);
    return IsWholeCellHidden(static_cast<uint32_t>(cx < 0 ? 0 : cx), static_cast<uint32_t>(cz < 0 ? 0 : cz));
}

bool cFC4STETerrain::SetTerrainCellFlags(uint32_t x, uint32_t z, uint32_t flags) {
    // §7: cell flags 1, 2, 4, 8 pack as 0x20, 0x40, 0x200, 0x2000.
    fc4_terrain* core = g_activeCore.load(std::memory_order_acquire);
    if (core == nullptr) return false;
    uint16_t f = fc4_terrain_vertex_flags(core, static_cast<int32_t>(x), static_cast<int32_t>(z));
    f = static_cast<uint16_t>(f & ~(kFlagEvenHidden | kFlagOddHidden | kFlagCell4 | kFlagFlipExternal));
    if ((flags & 1u) != 0) f = static_cast<uint16_t>(f | kFlagEvenHidden);
    if ((flags & 2u) != 0) f = static_cast<uint16_t>(f | kFlagOddHidden);
    if ((flags & 4u) != 0) f = static_cast<uint16_t>(f | kFlagCell4);
    if ((flags & 8u) != 0) f = static_cast<uint16_t>(f | kFlagFlipExternal);
    fc4_terrain_set_vertex_flags(core, static_cast<int32_t>(x), static_cast<int32_t>(z), f);
    MirrorFlags(x, z, f);
    return true;
}

uint32_t cFC4STETerrain::GetTerrainCellFlags(uint32_t x, uint32_t z) {
    const uint16_t f = VertexFlags(x, z);
    uint32_t out = 0;
    if ((f & kFlagEvenHidden) != 0) out |= 1u;
    if ((f & kFlagOddHidden) != 0) out |= 2u;
    if ((f & kFlagCell4) != 0) out |= 4u;
    if ((f & kFlagFlipExternal) != 0) out |= 8u;
    return out;
}

bool cFC4STETerrain::FlipInternalTriangulation(uint32_t x, uint32_t z, bool flip) {
    SetFlagBit(x, z, kFlagFlipInternal, flip);
    return true;
}
bool cFC4STETerrain::FlipExternalTriangulation(uint32_t x, uint32_t z, bool flip) {
    SetFlagBit(x, z, kFlagFlipExternal, flip);
    return true;
}

// ---- display state (§8.4) -------------------------------------------------------------------------

bool cFC4STETerrain::GetWaterSurfaceDisplayState() {
    return core_ != nullptr && fc4_terrain_water_display_state(core_) != 0;
}

void cFC4STETerrain::SetWaterSurfaceDisplayState(bool on) {
    Enter();
    if (core_ != nullptr) fc4_terrain_set_water_display_state(core_, on ? 1 : 0);  // §8.4
    Leave();
}

// ---- slot 91, the deleting destructor -------------------------------------------------------------

void cFC4STETerrain::DeletingDestructor(uint32_t flags) {
    // GZCOM releases through Release() (slot 2), so this should never run; if
    // the game does call its deleting-destructor slot, destroy as it expects.
    if ((flags & 1u) != 0) Release();
}

// ---- persistence (§13) ----------------------------------------------------------------------------

// §13.1: slot 96 reads the misc (instance 3), altitude (1) and vertex property (2) records of the
// terrain's own class (group 0xE98F9525) from the DB segment, and the core rebuilds itself from them
// (the water from the sea level, §10.1). The stream has no size query, so a record is read byte by byte
// until the stream refuses [choice]; the core checks every length itself.
namespace {
constexpr uint32_t kRecordGroup = 0xE98F9525;
constexpr uint32_t kTypeMisc = 0x8A91E7E3, kTypeAltitudes = 0xA9DD6FF4, kTypeProperties = 0x8A91E7E0;
constexpr size_t kRecordCap = 2 + 4 * 4097u * 4097u;

bool ReadRecord(cISC4DBSegment* db, uint32_t type, uint32_t instance, std::vector<uint8_t>& out) {
    out.clear();
    cISC4DBSegmentIStream* s = nullptr;
    const cGZPersistResourceKey key(type, kRecordGroup, instance);
    const bool opened = db->OpenIStream(key, &s);  // slot 5
    if (s == nullptr) return false;
    auto* in = reinterpret_cast<cFC4ISC4DBSegmentIStream*>(s);
    FC4_ACQUIRED(static_cast<cIGZUnknown*>(in), opened ? "OpenIStream" : "OpenIStream (false)");
    if (opened) {
        uint8_t b = 0;
        while (out.size() < kRecordCap && in->GetUint8(b)) out.push_back(b);
        db->CloseIStream(s);  // slot 6
    }
    FC4_RELEASE_CREATED(static_cast<cIGZUnknown*>(in));
    return opened && !out.empty();
}
}  // namespace

bool cFC4STETerrain::Load(cIGZPersistDBSegment* segment) {
    if (segment == nullptr) {
        Log("terrain: Load: called with a null segment; starting from the city grid (question TR39)");
        return false;
    }
    Log("terrain: Load: slot 96 called (segment %p)", static_cast<void*>(segment));
    cISC4DBSegment* db = nullptr;
    if (!segment->QueryInterface(GZIID_cISC4DBSegment, reinterpret_cast<void**>(&db)) || db == nullptr) {
        Log("terrain: Load: the segment is not a cISC4DBSegment; starting from the city grid");
        return false;
    }
    FC4_ACQUIRED(static_cast<cIGZUnknown*>(db), "QueryInterface");
    std::vector<uint8_t> misc, alt, props;
    const bool haveMisc = ReadRecord(db, kTypeMisc, 3, misc);
    const bool haveAlt = ReadRecord(db, kTypeAltitudes, 1, alt);
    const bool haveProps = ReadRecord(db, kTypeProperties, 2, props);
    FC4_RELEASE(static_cast<cIGZUnknown*>(db));
    if (!haveMisc || !haveAlt || !haveProps) {
        Log("terrain: Load: records missing (misc=%u alt=%u props=%u bytes); starting from the city grid",
            static_cast<unsigned>(misc.size()), static_cast<unsigned>(alt.size()),
            static_cast<unsigned>(props.size()));
        return false;
    }
    Enter();
    fc4_terrain_tuning tuning{};
    tuning.struct_size = sizeof(tuning);
    if (!fc4_terrain_tuning_shipped(&tuning)) {
        tuning = fc4_terrain_tuning{};
        tuning.struct_size = sizeof(tuning);
    }
    int32_t error = 0;
    fc4_terrain* loaded = fc4_terrain_load(&hostTable_, &tuning, misc.data(), misc.size(), alt.data(), alt.size(),
                                           props.data(), props.size(), &error);
    bool ok = false;
    if (loaded == nullptr) {
        Log("terrain: Load: the core refused the records (error %d; misc=%u alt=%u props=%u bytes)", error,
            static_cast<unsigned>(misc.size()), static_cast<unsigned>(alt.size()),
            static_cast<unsigned>(props.size()));
    } else {
        DestroyCore();
        ok = InstallCore(loaded);
        Log("terrain: Load: %u x %u cells from the saved records (misc=%u alt=%u props=%u bytes), sea=%.9g, "
            "altitude checksum=%.9g",
            f_.cellsX, f_.cellsZ, static_cast<unsigned>(misc.size()), static_cast<unsigned>(alt.size()),
            static_cast<unsigned>(props.size()), fc4_terrain_sea_level(core_),
            static_cast<double>(fc4_terrain_altitude_checksum(
                core_, fc4_terrain_rect{0, 0, static_cast<int32_t>(f_.cellsX), static_cast<int32_t>(f_.cellsZ)})));
    }
    Leave();
    return ok;
}

bool cFC4STETerrain::Save(cIGZPersistDBSegment*) {
    Log("terrain: Save (records) is not wired");
    return false;
}

}  // namespace fc4

// The factories return the primary interface's pointer: the director's
// QueryInterface takes the first reference.
cIGZUnknown* CreateFC4Terrain() {
    auto* t = new fc4::cFC4STETerrain();
    return t->Primary();
}

cIGZUnknown* CreateFC4SurfaceWater() {
    // The water is a stateless view over the one fc4_terrain, so it does not
    // matter which of the two the city constructs. A second object is fine.
    fc4::Log("terrain: creating cSTESurfaceWater (enable = terrain): a view over the shared fc4_terrain handle");
    auto* w = new fc4::cFC4STESurfaceWater();
    return w->Primary();
}

// ---- test seam op 59 ------------------------------------------------------------------------------

intptr_t TerrainGlueTest(FC4TerrainGlueTestInput* in) {
    using namespace fc4;
    if (in == nullptr || in->struct_size < sizeof(FC4TerrainGlueTestInput)) return -1;
    in->slotsChecked = 0;
    in->reentrantGuardRefused = 0;
    in->coreReentrantRefused = 0;
    in->postsSeen = 0;
    in->brushLevel = 0;
    in->mismatches = 0;

    auto* t = new cFC4STETerrain();
    t->AddRef();
    t->Host()->testMode = true;
    if (!t->Init()) {
        t->Release();
        return -1;
    }

    fc4_terrain* core = t->Core();
    if (core != nullptr) {
        // A few slots that the consumers' list (§15.3) depends on.
        auto check = [&](bool ok) {
            ++in->slotsChecked;
            if (!ok) ++in->mismatches;
        };
        check(t->CellWidth() == 16.0f);  // abi: via cFC4ISTETerrain (float, not the SDK's uint32_t)
        check(t->GetAltitudeAt(0.0f, 0.0f) >= 0.0f);
        check(t->GetSeaLevel() > 0.0f);
        check(t->MinAllowedHeight() < t->MaxAllowedHeight());
        check(t->GetDefaultTerrainRaiseLowerAmount() == 20.0f);
        check(t->LocationIsInBounds(0.0f, 0.0f));
        check(!t->LocationIsInBounds(-1.0f, 0.0f));
        check(t->CellCountX() == t->CellCountX());

        // TR18: a host callback from inside a brush must be refused.
        t->Host()->inside = 1;
        (void)HostRandomUniform(t->Host(), 5);
        (void)HostBridgeAt(t->Host(), 0, 0, nullptr);
        (void)HostPowerPole(t->Host(), 0, 0);
        (void)HostLotAtCell(t->Host(), 0, 0);
        t->Host()->inside = 0;

        // A change: one message posted per RedisplayTerrain (§8.1 posts three,
        // of which the simulation reads 0x278E6003 / 0x678E6E3A).
        t->DoPostCityInit();
        SC4Rect<int32_t> r(0, 0, 1, 1);
        t->SetAltitudeRect(r, 123.0f, true, 3);

        fc4_terrain_state st{};
        st.struct_size = sizeof(st);
        if (fc4_terrain_get_state(core, &st)) {
            in->coreReentrantRefused = st.reentrant_refused;
            in->postsSeen = st.posts_pending;
        }
        in->brushLevel = static_cast<uint32_t>(fc4_terrain_default_brush_level(core));
    }

    in->reentrantGuardRefused = t->Host()->guardRefused.load(std::memory_order_relaxed);
    t->Shutdown();
    t->Release();
    return static_cast<intptr_t>(in->mismatches);
}
