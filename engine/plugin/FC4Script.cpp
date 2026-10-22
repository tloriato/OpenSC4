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

#include "FC4Script.h"

#include "FC4Config.h"
#include "FC4Diag.h"
#include "FC4DisasterShadow.h"
#include "FC4Fpu.h"
#include "FC4HarnessLogic.h"
#include "FC4Interfaces.h"
#include "FC4Log.h"
#include "FC4NetMgrShadow.h"
#include "FC4Observer.h"
#include "FC4RefAudit.h"
#include "FC4TerrainShadow.h"  // the glue16 lane: terrain
#include "FC4Turbo.h"
#include "FC4Watchdog.h"
#include "cFC4NetworkTool.h"  // the glue29 lane: NetToolRegistry
#include "cFC4PortDeveloper.h"  // the glue-portdev4 lane: the port action
#include "cFC4ZoneManager.h"
#include "cGZMessage.h"
#include "cIGZMessageTarget.h"
#include "cRZBaseString.h"

#include "SC4Rect.h"
#include "cISC4City.h"
#include "cISC4NetworkManager.h"
#include "cISC4NetworkTool.h"
#include "cISC4TutorialSystem.h"  // the glue-tutorial4 lane: the tutorial action
#include "cISTETerrain.h"

#include <cctype>
#include <cstdio>
#include <string>
#include <utility>
#include <vector>

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>

// Game calls (checked against docs/abi/interfaces.tsv; none returns in ST0,
// so the x87 rule holds with the declarations of FC4Interfaces.h):
//   cISC4City 36 GetZoneManager (borrowed), 50 GetSimulator (borrowed);
//   cISC4ZoneManager 11 PlaceZone (pop 40, AL);
//   cISC4Simulator 9 Pause, 12 Resume, 15 IsPaused, 27 GetSimSpeed,
//   28 SetSimSpeed (pop 4) - all eax-or-void;
//   network: cISC4City 38 GetNetworkManager (borrowed); cISC4NetworkManager
//   5 GetNetworkTool(type, true) (network-manager.md §7.1); cISC4NetworkTool
//   3 Init, 7 PlaceNetworkOccupants (pop 32, AL), 4 Shutdown
//   (network-tools.md §3, §14; FC4Interfaces.h cFC4ISC4NetworkTool);
//   save: cISC4App 16 SaveCity(bool) (docs/abi/vtables.tsv cSC4App::vftable_3,
//   pop 4, eax-or-void), called by the observer (ObserverSaveCity);
//   terrain: cISC4City 84 GetTerrain (borrowed); cISTETerrain 7, 8 the cell
//   counts (eax), 14 GetAltitudeAtVertex (x87), 49 SetAltitude(rect*, h,
//   redisplay, data4) (pop 16, eax-or-void).

namespace fc4 {
namespace {

harness::ScriptSchedule g_schedule;  // main thread only (game start, SimNewDay)
std::string g_path;                  // the loaded script, "" = none
// A `save` and every action due after it, waiting for the observer's next
// tick (pointers into g_schedule; cleared whenever it is reloaded).
std::vector<const harness::ScriptAction*> g_waiting;
bool g_inTick = false;  // ScriptOnTick is running the waiting actions

constexpr unsigned long long kMaxScriptBytes = 4ull << 20;  // [choice] a text file of actions, not a save

std::string Trimmed(std::string s) {
    size_t a = 0, b = s.size();
    while (a < b && std::isspace(static_cast<unsigned char>(s[a]))) ++a;
    while (b > a && std::isspace(static_cast<unsigned char>(s[b - 1]))) --b;
    return s.substr(a, b - a);
}

// [choice] A relative path is relative to the folder of FreeCity4.ini (the
// DLL's folder), not to the game's working directory.
std::string ResolvePath(const std::string& p) {
    const bool absolute = (p.size() >= 2 && p[1] == ':') || (!p.empty() && (p[0] == '\\' || p[0] == '/'));
    if (absolute) return p;
    const std::string ini = IniPath();
    const size_t slash = ini.find_last_of("\\/");
    return slash == std::string::npos ? p : ini.substr(0, slash + 1) + p;
}

bool ReadWhole(const std::string& path, std::string* out, unsigned long* error) {
    HANDLE f = CreateFileA(path.c_str(), GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                           nullptr, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (f == INVALID_HANDLE_VALUE) {
        *error = GetLastError();
        return false;
    }
    LARGE_INTEGER size{};
    if (!GetFileSizeEx(f, &size) || size.QuadPart < 0 ||
        static_cast<unsigned long long>(size.QuadPart) > kMaxScriptBytes) {
        *error = size.QuadPart > 0 ? static_cast<unsigned long>(ERROR_FILE_TOO_LARGE) : GetLastError();
        CloseHandle(f);
        return false;
    }
    out->assign(static_cast<size_t>(size.QuadPart), '\0');
    DWORD got = 0;
    const bool ok = out->empty() ||
                    (ReadFile(f, &(*out)[0], static_cast<DWORD>(out->size()), &got, nullptr) && got == out->size());
    if (!ok) *error = GetLastError();
    CloseHandle(f);
    return ok;
}

// ---- zone, pricezone, dezone: PlaceZone (zones.md §6) -----------------------------------------------

struct ZoneCall {
    cFC4ISC4ZoneManager* zm;
    SC4CellRegion<long>* region;
    int32_t type;
    bool commit;
    bool ok;
    int64_t cost;
    int32_t error;
};

// The argument list of zones.md §6, as a user's zoning ([choice] the flags):
// skipFunds = false (the normal funds rules), checkSlope = true (§6 notes:
// it "behaves as 'this is a user action'" [inferred]), skipSizeCheck =
// false, checkLotInside = true, outLots = null (a set would need the game's
// allocator B for its nodes, §13.2; nothing here needs the lots).
void ZoneStep(void* p) {
    auto* c = static_cast<ZoneCall*>(p);
    c->ok = c->zm->PlaceZone(*c->region, c->type, c->commit, false, true, false, true, &c->cost, &c->error,
                             nullptr);  // slot 11
}

const char* ZoneErrorName(int32_t e) {
    switch (e) {  // zones.md §6
        case 0: return "none";
        case 1: return "insufficient funds";
        case 2: return "landfill on water";
        case 3: return "zone too small";
        case 4: return "zone too large";
        case 5: return "blocked";
        default: return "unknown";
    }
}

std::string RunZone(const harness::ScriptAction& a, cISC4City* city) {
    if (city == nullptr) return "skipped: no city";
    cFC4ISC4ZoneManager* zm = AsGame(city->GetZoneManager());  // city slot 36, borrowed
    if (zm == nullptr) return "skipped: the city has no zone manager";
    // The region in the §11.4 layout, every cell set, in our own memory: the
    // game only clears the bits of rejected cells and never reallocates or
    // frees it (zones.md §13.2), so allocator C is not needed.
    ZoneRegionBuffer rb;
    if (!rb.BuildFull(a.x0, a.z0, a.x1, a.z1)) return "error: the rectangle is larger than a zone region can be";
    ZoneCall c{zm, &rb.Game(), a.type, a.op != harness::ScriptOp::PriceZone, false, 0, 0};
    const bool clean = SehGuard("script PlaceZone", &ZoneStep, &c);
    const uint32_t cells = static_cast<uint32_t>(a.x1 - a.x0 + 1) * static_cast<uint32_t>(a.z1 - a.z0 + 1);
    uint32_t accepted = 0;
    for (int32_t x = a.x0; x <= a.x1; ++x) {
        for (int32_t z = a.z0; z <= a.z1; ++z) accepted += rb.Bit(x, z) ? 1u : 0u;
    }
    char b[224];
    std::snprintf(b, sizeof(b), "%sok=%d error=%d (%s) cost=%lld accepted=%u of %u cell(s)%s",
                  c.commit ? "" : "dry run: ", c.ok ? 1 : 0, c.error, ZoneErrorName(c.error),
                  static_cast<long long>(c.cost), accepted, cells,
                  clean ? "" : " [structured exception in PlaceZone]");
    return b;
}

// ---- network: a straight run by code (network-manager.md §9) ---------------------------------------

struct NetworkCall {
    cFC4ISC4NetworkTool* tool;
    const harness::ScriptAction* a;
    bool ok;
};

// §9: Init; PlaceNetworkOccupants(a, b, mask 0, callback null, real 1,
// type -1) (the tool's own type); Shutdown. Tool slot 7 returns AL only.
void NetworkStep(void* p) {
    auto* c = static_cast<NetworkCall*>(p);
    const harness::ScriptAction& a = *c->a;
    c->tool->Init();  // slot 3
    const uint32_t r = c->tool->ToolSlot07(static_cast<uint32_t>(a.x0), static_cast<uint32_t>(a.z0),
                                           static_cast<uint32_t>(a.x1), static_cast<uint32_t>(a.z1), 0u, 0u, 1u,
                                           0xFFFFFFFFu);
    c->ok = (r & 0xFFu) != 0;
    c->tool->Shutdown();  // slot 4
}

// [choice] A private tool (GetNetworkTool(type, true)), as the zone developer
// takes its street tools (zones.md §5.2): AddRef'd, and released after its
// Shutdown with the reference audit's release of a tool made for us
// (FC4ZoneDevHost.cpp ReleaseNetworkTools: the game runs showed the count
// reaching 0 there).
// glue29: with `enable = nettool` the tool comes from our registry
// (NetToolRegistry::Take) and the placement runs through the fc4_nettool
// core; the borrowed shared tool from the registry is released at city
// shutdown, not here.
std::string RunNetwork(const harness::ScriptAction& a, cISC4City* city) {
    if (city == nullptr) return "skipped: no city";
    cISC4NetworkTool* t = nullptr;
    bool ours = false;
    if (NetToolRegistry::Enabled()) {
        ours = true;
        t = reinterpret_cast<cISC4NetworkTool*>(NetToolRegistry::Take(a.type, city, nullptr));  // borrowed, lives until city shutdown
        if (t == nullptr) return "skipped: nettool registry has no tool for the type";
    } else {
        cISC4NetworkManager* nm = city->GetNetworkManager();  // city slot 38, borrowed
        if (nm == nullptr) return "skipped: the city has no network manager";
        t = nm->GetNetworkTool(a.type, true);  // slot 5
        if (t == nullptr) return "skipped: no network tool for the type";
    }
    FC4_ADDREF(t);
    NetworkCall c{reinterpret_cast<cFC4ISC4NetworkTool*>(t), &a, false};
    const bool clean = SehGuard("script PlaceNetworkOccupants", &NetworkStep, &c);
    FC4_X87_CHECK("script PlaceNetworkOccupants");
    if (ours) {
        FC4_RELEASE(t);  // the registry tool lives on until city shutdown
    } else {
        FC4_RELEASE_CREATED(t);  // a private tool made for us: the free is expected
    }
    char b[160];
    std::snprintf(b, sizeof(b), "ok=%d (PlaceNetworkOccupants type %d, real, free)%s", c.ok ? 1 : 0, a.type,
                  clean ? "" : " [structured exception in PlaceNetworkOccupants]");
    return b;
}

// ---- speed, pause, resume: the city's simulator ------------------------------------------------------

std::string RunSimulator(const harness::ScriptAction& a, cISC4City* city) {
    cFC4ISC4Simulator* sim = city ? AsGame(city->GetSimulator()) : nullptr;  // city slot 50, borrowed
    if (sim == nullptr) return "skipped: no city simulator";
    char b[160];
    if (a.op == harness::ScriptOp::Speed) {
        const int32_t before = sim->GetSimSpeed();
        const bool r = sim->SetSimSpeed(a.speed);
        std::snprintf(b, sizeof(b), "SetSimSpeed(%d) = %d; speed %d -> %d", a.speed, r ? 1 : 0, before,
                      sim->GetSimSpeed());
    } else if (a.op == harness::ScriptOp::Pause) {
        const bool r = sim->Pause();
        std::snprintf(b, sizeof(b), "Pause() = %d; paused=%d", r ? 1 : 0, sim->IsPaused() ? 1 : 0);
    } else {
        const bool r = sim->Resume();
        std::snprintf(b, sizeof(b), "Resume() = %d; paused=%d", r ? 1 : 0, sim->IsPaused() ? 1 : 0);
    }
    return b;
}

// ---- save: cISC4App::SaveCity(fast), at a framework tick -------------------------------------------

std::string RunSave(const harness::ScriptAction& a, cISC4City* city) {
    // [choice] No city, no call: what SaveCity does without one is not specified.
    if (city == nullptr) return "ok=0 (skipped: no city)";
    const ObserverSave r = ObserverSaveCity(a.fast);
    if (!r.haveApp) return "ok=0 (skipped: no cISC4App)";
    char b[160];
    std::snprintf(b, sizeof(b), "ok=%d (SaveCity(%s) in %lu ms)%s", r.result ? 1 : 0, a.fast ? "fast" : "full", r.ms,
                  r.clean ? "" : " [structured exception in SaveCity]");
    return b;
}

// ---- terrain: cISTETerrain::SetAltitude(rect, h, true, kind) (terrain.md §6) ---------------------------

struct TerrainCall {
    cISTETerrain* t;
    const harness::ScriptAction* a;
    float before;
    float after;
};

void TerrainStep(void* p) {
    auto* c = static_cast<TerrainCall*>(p);
    const harness::ScriptAction& a = *c->a;
    c->before = c->t->GetAltitudeAtVertex(a.x0, a.z0);  // slot 14
    const SC4Rect<int32_t> r(a.x0, a.z0, a.x1, a.z1);
    c->t->SetAltitude(r, a.height, true, static_cast<uint32_t>(a.kind));  // slot 49
    c->after = c->t->GetAltitudeAtVertex(a.x0, a.z0);
}

std::string RunTerrain(const harness::ScriptAction& a, int32_t julian, cISC4City* city) {
    cISTETerrain* t = city ? city->GetTerrain() : nullptr;  // city slot 84, borrowed
    if (t == nullptr) return "skipped: no city terrain";
    // [choice] The original does not check the rectangle (§6): one outside the
    // vertices is refused here.
    const uint32_t cx = t->CellCountX(), cz = t->CellCountZ();  // slots 7, 8
    if (static_cast<uint32_t>(a.x1) > cx || static_cast<uint32_t>(a.z1) > cz) {
        char b[160];
        std::snprintf(b, sizeof(b), "skipped: the rectangle is outside the %ux%u vertices", cx + 1, cz + 1);
        return b;
    }
    TerrainCall c{t, &a, 0.0f, 0.0f};
    const bool clean = SehGuard("script SetAltitude", &TerrainStep, &c);
    if (clean) TerrainShadowScriptedSetAltitude(a.x0, a.z0, a.x1, a.z1, a.height, static_cast<uint32_t>(a.kind),
                                                julian);  // terrain_shadow = 1 only
    char b[224];
    std::snprintf(b, sizeof(b), "SetAltitude(%d,%d..%d,%d, %.9g, true, %d): vertex (%d,%d) %.9g -> %.9g%s", a.x0, a.z0,
                  a.x1, a.z1, a.height, a.kind, a.x0, a.z0, c.before, c.after,
                  clean ? "" : " [structured exception in SetAltitude]");
    return b;
}

// The tutorial action (glue-tutorial4): one call on the city's tutorial system
// (cISC4City slot 82, borrowed; tutorial.md 6), then its state through slots
// 8-12. Works with the original's class and with ours (enable = tutorial).
struct TutorialCall {
    const harness::ScriptAction* a;
    cFC4ISC4TutorialSystem* t;
    int32_t ret;
    uint32_t is, edge;
    int32_t task, guid;
    bool task_null;
};

void TutorialStep(void* p) {
    auto& c = *static_cast<TutorialCall*>(p);
    const harness::ScriptAction& a = *c.a;
    c.ret = -1;  // no return value (query, task, next)
    switch (a.kind) {
        case harness::kTutorialStart: c.ret = c.t->startTutorial() ? 1 : 0; break;
        case harness::kTutorialCheat: {
            cIGZMessageTarget* handler = nullptr;
            if (c.t->QueryInterface(0xE98B2F57, reinterpret_cast<void**>(&handler)) && handler) {
                FC4_ACQUIRED(handler, "tutorial cheat QueryInterface");
                cRZBaseString line(("Tutorial " + a.text).c_str());
                cGZMessage msg(0x230E27AC);
                msg.dwData1 = 0x6A382BAF;
                msg.dwData2 = static_cast<uint32_t>(reinterpret_cast<uintptr_t>(static_cast<cIGZString*>(&line)));
                c.ret = handler->DoMessage(msg) ? 1 : 0;
                FC4_RELEASE(handler);
            }
            break;
        }
        case harness::kTutorialTask: c.t->SetTutorialTaskNumber(static_cast<uint32_t>(a.x0)); break;  // slot 19
        case harness::kTutorialNext: c.t->IncrementTutorialTaskNumber(); break;                       // slot 20
        case harness::kTutorialEnd: c.ret = c.t->endTutorial(a.x0 != 0) ? 1 : 0; break;               // slot 6
        case harness::kTutorialEvent: c.ret = c.t->OnTutorialEvent(static_cast<uint32_t>(a.x0)) ? 1 : 0; break;
        case harness::kTutorialReset: c.ret = c.t->ResetTutorialNumAndIndex(a.x0, a.z0) ? 1 : 0; break;  // 17
        default: break;
    }
    c.is = c.t->IsTutorial() ? 1u : 0u;                // slot 8
    c.edge = c.t->IsEdgeScrollingAllowed() ? 1u : 0u;  // slot 9
    c.task_null = c.t->GetCurrentTask() == nullptr;    // slot 10 (borrowed)
    c.task = c.t->GetTutorialTaskNumber();             // slot 11
    c.guid = c.t->GetTutorialGUID();                   // slot 12
}

std::string RunTutorial(const harness::ScriptAction& a, cISC4City* city) {
    if (city == nullptr) return "skipped: no city";
    cISC4TutorialSystem* t = city->GetTutorialSystem();  // slot 82, borrowed
    if (t == nullptr) return "skipped: the city has no tutorial system";
    TutorialCall c{&a, fc4::AsGame(t), -1, 0, 0, 0, 0, true};
    const bool clean = SehGuard("script tutorial", &TutorialStep, &c);
    char b[200];
    std::snprintf(b, sizeof(b), "ret=%d isTutorial=%u edgeScroll=%u currentTask=%s task=%d guid=0x%08X%s", c.ret, c.is,
                  c.edge, c.task_null ? "null" : "set", c.task, static_cast<uint32_t>(c.guid),
                  clean ? "" : " [structured exception]");
    return b;
}

void RunAction(const harness::ScriptAction& a, int32_t julian, cISC4City* city) {
    using harness::ScriptOp;
    std::string result;
    bool digest = false;
    switch (a.op) {
        case ScriptOp::Zone:
        case ScriptOp::PriceZone:
        case ScriptOp::Dezone: result = RunZone(a, city); break;
        case ScriptOp::Network: result = RunNetwork(a, city); break;
        case ScriptOp::Speed:
        case ScriptOp::Pause:
        case ScriptOp::Resume: result = RunSimulator(a, city); break;
        case ScriptOp::Turbo:
            switch (TurboScriptSet(a.on)) {
                case TurboScriptResult::On: result = "turbo on"; break;
                case TurboScriptResult::Held: result = "turbo requested: held until the city's ConnectionsReady"; break;
                case TurboScriptResult::Off: result = "turbo off"; break;
                default: result = "no effect: our simulator is not running (disable = simulator?)"; break;
            }
            break;
        case ScriptOp::Digest:
            result = "the DIGEST line follows";
            digest = true;
            break;
        case ScriptOp::Mark: result = "marked"; break;
        case ScriptOp::Save:
            NetMgrShadowBeforeSave(city, julian);  // netmgr_shadow = 1 only: the records the save writes
            result = RunSave(a, city);
            break;
        case ScriptOp::Terrain: result = RunTerrain(a, julian, city); break;
        case ScriptOp::Disaster: result = DisasterScriptAction(city, a.disaster, a.x0, a.z0); break;
        case ScriptOp::Port:
            result = PortScriptBuild(static_cast<uint32_t>(a.kind), static_cast<uint32_t>(a.type), a.x0, a.z0,
                                     static_cast<uint32_t>(a.facing));
            break;
        case ScriptOp::Tutorial: result = RunTutorial(a, city); break;
    }
    std::string late;
    if (julian != a.julian) late = " (run on " + harness::FormatDate(harness::CivilFromJulian(julian)) + ")";
    Log("script: line %d %s %s%s%s -> %s%s", a.line, harness::FormatDate(a.date).c_str(), harness::ScriptOpName(a.op),
        a.text.empty() ? "" : " ", a.text.c_str(), result.c_str(), late.c_str());
    if (digest) ObserverWriteDigest(julian);
}

void NoteAllRun() {
    if (g_schedule.Size() == 0 || g_schedule.Remaining() != 0 || !g_waiting.empty()) return;
    Log("script: all %u action(s) of '%s' have run", static_cast<unsigned>(g_schedule.Size()), g_path.c_str());
}

}  // namespace

int ScriptLoad(const char* rawPath, const char* why) {
    g_waiting.clear();
    g_schedule.Load({});
    g_path.clear();
    const std::string path = ResolvePath(rawPath ? Trimmed(rawPath) : std::string());
    std::string text;
    unsigned long error = 0;
    if (path.empty() || !ReadWhole(path, &text, &error)) {
        Log("script: ERROR cannot read '%s' (%s; error %lu); no action runs", path.c_str(), why, error);
        return -1;
    }
    harness::ParsedScript parsed = harness::ParseScript(text);
    if (!parsed.errors.empty()) {
        for (const harness::ScriptError& e : parsed.errors) Log("script: ERROR line %d: %s", e.line, e.message.c_str());
        Log("script: REFUSED '%s' (%s): %u parse error(s); no action of it runs", path.c_str(), why,
            static_cast<unsigned>(parsed.errors.size()));
        return -2;
    }
    g_schedule.Load(std::move(parsed.actions));
    g_path = path;
    if (g_schedule.Size() == 0) {
        Log("script: '%s' (%s): no actions", path.c_str(), why);
        return 0;
    }
    Log("script: loaded '%s' (%s; tooling): %u action(s), %s to %s; each runs once at the first new day on or after "
        "its date",
        path.c_str(), why, static_cast<unsigned>(g_schedule.Size()),
        harness::FormatDate(g_schedule.First()->date).c_str(), harness::FormatDate(g_schedule.Last()->date).c_str());
    return static_cast<int>(g_schedule.Size());
}

void ScriptReload() {
    const std::string v = Trimmed(Setting("script", "FC4_SCRIPT"));
    if (v.empty()) {  // off: nothing read, nothing logged
        g_waiting.clear();
        g_schedule.Load({});
        g_path.clear();
        return;
    }
    ScriptLoad(v.c_str(), "script setting");
}

void ScriptOnNewDay(int32_t julian) {
    if (g_schedule.Remaining() == 0) return;
    const std::vector<const harness::ScriptAction*> due = g_schedule.Due(julian);
    if (due.empty()) return;
    // [choice] Run inside the SimNewDay message, not at the next tick; a
    // `save` and what follows it wait for the tick (FC4Script.h).
    cISC4City* city = ObserverCity();
    for (const harness::ScriptAction* a : due) {
        const bool tutorialMutation = a->op == harness::ScriptOp::Tutorial &&
            (a->kind == harness::kTutorialStart || a->kind == harness::kTutorialCheat);
        if (g_waiting.empty() && !g_inTick && a->op != harness::ScriptOp::Save && !tutorialMutation) {
            RunAction(*a, julian, city);
            continue;
        }
        if (g_waiting.empty()) {
            Log("script: line %d %s %s%s%s: waits for the next framework tick (not run inside SimNewDay); the "
                "actions due after it wait behind it",
                a->line, harness::FormatDate(a->date).c_str(), harness::ScriptOpName(a->op), a->text.empty() ? "" : " ",
                a->text.c_str());
        }
        g_waiting.push_back(a);
    }
    NoteAllRun();
}

void ScriptOnTick() {
    if (g_waiting.empty() || g_inTick) return;
    std::vector<const harness::ScriptAction*> run;
    run.swap(g_waiting);
    g_inTick = true;  // a SimNewDay during these calls queues behind them
    const int32_t julian = static_cast<int32_t>(PublishedJulian());
    cISC4City* city = ObserverCity();
    for (const harness::ScriptAction* a : run) RunAction(*a, julian, city);
    g_inTick = false;
    NoteAllRun();
}

bool ScriptWaiting() { return !g_waiting.empty(); }

}  // namespace fc4
