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
#include "FC4Watchdog.h"
#include "FC4Diag.h"
#include "FC4DisasterShadow.h"
#include "FC4DbpfNative.h"
#include "FC4DispatchShadow.h"
#include "FC4WriteCapture.h"
#include "FC4Fpu.h"
#include "FC4HarnessLogic.h"
#include "FC4Interfaces.h"
#include "FC4Log.h"
#include "FC4NetMgrShadow.h"
#include "FC4Observer.h"
#include "FC4RefAudit.h"
#include "FC4Schedule.h"
#include "FC4TerrainShadow.h"  // the glue16 lane: terrain
#include "FC4Turbo.h"
#include "FC4Watchdog.h"
#include "cFC4NetworkTool.h"  // the glue29 lane: NetToolRegistry
#include "cFC4PortDeveloper.h"  // the glue-portdev4 lane: the port action
#include "cFC4ZoneManager.h"
#include "cFC4PropDeveloper.h"
#include "cFC4MySim.h"
#include "FC4GDriverShadow.h"
#include "FC4GZFramework.h"
#include "FC4Span565Bridge.h"

#include "SC4Rect.h"
#include "SC4String.h"
#include "cGZMessage.h"
#include "cISC4City.h"
#include "cISC4DispatchManager.h"
#include "cISC4Occupant.h"
#include "cS3DVector3.h"
#include "SC4List.h"
#include "cISC4NetworkManager.h"
#include "cISC4NetworkTool.h"
#include "cISC4TutorialSystem.h"  // the glue-tutorial4 lane: the tutorial action
#include "cISTETerrain.h"
#include "cISC4AutomataManager.h"
#include "cISC4Automaton.h"

#include <cctype>
#include <cstdio>
#include <map>
#include <string>
#include <utility>
#include <vector>

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>
#include "FC4UiDriver.h"

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
DWORD g_uiReady = 0;
bool g_uiDelay = false;
// [choice] Under the fixed schedule a UI wait counts the schedule's clock, not the wall clock
// (simulator.md §5.8, Scripted input).
DWORD UiClockMs() { return ScheduleFixed() ? ScheduleClockMs() : GetTickCount(); }
std::string g_path;                  // the loaded script, "" = none
// A `save` and every action due after it, waiting for the observer's next
// tick (pointers into g_schedule; cleared whenever it is reloaded).
std::vector<const harness::ScriptAction*> g_waiting;
bool g_inTick = false;  // ScriptOnTick is running the waiting actions
volatile LONG g_saveGraceMs = 0;  // watchdog reads; only inside an explicitly opted-in full save

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

// Record-only replay. Whitelist read callbacks rather than allowing a normal
// replacement host to commit terrain, occupants, messages or tool callbacks.
// NT27: public getters do not expose solved piece IDs/types. Never credit this
// projection as whole-owner verification (in particular bridges / power lines).
struct NetworkShadow {
    NetToolContext ctx;
    fc4_nettool* core = nullptr;
    fc4_netgeom_state geom{};
    bool ok = false;
    uint32_t failure = 0;
    fc4_powerline_selection powerline{};
    bool powerlineObserved = false;
    std::vector<fc4_netgeom_cell> cells;
    fc4_netgeom_host sourceGeom{};
    // Private grading overlay: the real-commit replay must observe its own
    // terrain writes during NT30 refresh without writing into the game.
    std::map<int32_t, float> graded;
    uint32_t changedVertices = 0;
    using CellKey = std::pair<int32_t, int32_t>;
    std::map<CellKey, fc4_netgeom_cell_info> beforeRefresh;
    std::map<CellKey, int32_t> beforeIndex;
    std::map<CellKey, bool> smoothingCells;

    void CaptureCorners() {
        LogNetToolReplayResources();
        std::vector<fc4_netgeom_cell> solved(fc4_netgeom_get_solved(ctx.geom, nullptr, 0));
        fc4_netgeom_get_solved(ctx.geom, solved.data(), static_cast<uint32_t>(solved.size()));
        for (size_t i = 0; i < solved.size(); ++i) beforeIndex[{solved[i].x, solved[i].z}] = static_cast<int32_t>(i);
        std::vector<fc4_netgeom_cell> smooth(fc4_netgeom_get_smoothing_cells(ctx.geom, nullptr, 0));
        fc4_netgeom_get_smoothing_cells(ctx.geom, smooth.data(), static_cast<uint32_t>(smooth.size()));
        for (const auto& c : smooth) smoothingCells[{c.x, c.z}] = true;
        // Read only existing infos. Include both populations, including incident
        // cells which were not solved before grading (NT30 section 6.5).
        const auto remember = [this](int32_t x, int32_t z) {
            fc4_netgeom_cell_info ci{};
            ci.struct_size = sizeof(ci);
            if (fc4_netgeom_get_cell_info(ctx.geom, x, z, &ci)) beforeRefresh[{x, z}] = ci;
        };
        for (const auto& c : solved) remember(c.x, c.z);
        for (const auto& c : smooth) remember(c.x, c.z);
        std::vector<fc4_netgeom_vertex_height> terrain(fc4_netgeom_sat_solution(ctx.geom, FC4_NETGEOM_SAT_TERRAIN, nullptr, 0));
        fc4_netgeom_sat_solution(ctx.geom, FC4_NETGEOM_SAT_TERRAIN, terrain.data(), static_cast<uint32_t>(terrain.size()));
        const int32_t stride = static_cast<int32_t>(ctx.city->CellCountX()) + 1;
        for (const auto& v : terrain) {
            const int32_t x = v.vertex % stride, z = v.vertex / stride;
            for (int32_t dz = -1; dz <= 0; ++dz) for (int32_t dx = -1; dx <= 0; ++dx) remember(x + dx, z + dz);
        }
    }

    void TraceCorners(const harness::ScriptAction& a) {
        std::vector<fc4_netgeom_cell> solved(fc4_netgeom_get_solved(ctx.geom, nullptr, 0));
        fc4_netgeom_get_solved(ctx.geom, solved.data(), static_cast<uint32_t>(solved.size()));
        uint32_t observations = 0;
        for (size_t i = 0; i < solved.size(); ++i) {
            const CellKey key{solved[i].x, solved[i].z};
            fc4_netgeom_cell_info ci{};
            ci.struct_size = sizeof(ci);
            if (!fc4_netgeom_get_cell_info(ctx.geom, key.first, key.second, &ci)) continue;
            const auto old = beforeRefresh.find(key);
            const auto oldIndex = beforeIndex.find(key);
            const bool smooth = smoothingCells.count(key) != 0;
            const char* origin = oldIndex != beforeIndex.end() ? "solve" : (smooth ? "position" : "terrain");
            for (size_t k = 0; k < 4; ++k) {
                const int32_t lookup = ci.vertices[k] + (smooth && ci.connection_node ? 100000 : 0);
                float network = 0, terrain = 0;
                const bool netHit = fc4_netgeom_sat_height(ctx.geom, FC4_NETGEOM_SAT_NETWORK, lookup, &network);
                const bool terHit = fc4_netgeom_sat_height(ctx.geom, FC4_NETGEOM_SAT_TERRAIN, ci.vertices[k], &terrain);
                const float live = sourceGeom.vertex_altitude(sourceGeom.user, ci.vertices[k]);
                Log("nettool corner %s type=%d cell=(%d,%d) index=%u old_index=%d origin=%s smooth=%d "
                    "run=%u intersection=%u placed=%u connection=%u corner=%u vertex=%d lookup=%d "
                    "old_present=%d old=%.9g retained=%.9g network_hit=%d network=%.9g terrain_hit=%d terrain=%.9g live=%.9g",
                    harness::FormatDate(a.date).c_str(), a.type, ci.x, ci.z, static_cast<uint32_t>(i),
                    oldIndex == beforeIndex.end() ? -1 : oldIndex->second, origin, smooth ? 1 : 0,
                    ci.in_run, ci.intersection, ci.placed, ci.connection_node, static_cast<uint32_t>(k), ci.vertices[k], lookup,
                    old != beforeRefresh.end() ? 1 : 0, old == beforeRefresh.end() ? 0 : old->second.corners[k],
                    ci.corners[k], netHit ? 1 : 0, network, terHit ? 1 : 0, terrain, live);
                ++observations;
            }
        }
        Log("nettool inventory %s type=%d entries=%u corners=%u pre_entries=%u smoothing=%u changed=%u contract=NT30",
            harness::FormatDate(a.date).c_str(), a.type, static_cast<uint32_t>(solved.size()), observations,
            static_cast<uint32_t>(beforeIndex.size()), static_cast<uint32_t>(smoothingCells.size()), changedVertices);
    }

    static NetworkShadow& Self(void* u) { return *static_cast<NetworkShadow*>(u); }

    fc4_netgeom_host RecordingHost() {
        fc4_netgeom_host h{};
        h.struct_size = sizeof(h);
        h.user = this;
        h.cells = [](void* u, uint32_t* x, uint32_t* z) {
            auto& s = Self(u); s.sourceGeom.cells(s.sourceGeom.user, x, z);
        };
        h.vertex_altitude = [](void* u, int32_t v) {
            auto& s = Self(u);
            const auto found = s.graded.find(v);
            return found == s.graded.end() ? s.sourceGeom.vertex_altitude(s.sourceGeom.user, v) : found->second;
        };
        h.set_altitude_at_vertex = [](void* u, int32_t v, float height) { Self(u).graded[v] = height; };
        h.cell_has_water = [](void* u, int32_t x, int32_t z) {
            auto& s = Self(u); return s.sourceGeom.cell_has_water(s.sourceGeom.user, x, z);
        };
        h.surface_water_level = [](void* u, int32_t v) {
            auto& s = Self(u); return s.sourceGeom.surface_water_level(s.sourceGeom.user, v);
        };
        h.max_allowed_height = [](void* u) {
            auto& s = Self(u); return s.sourceGeom.max_allowed_height(s.sourceGeom.user);
        };
        h.is_connection_node = [](void* u, int32_t x, int32_t z) {
            auto& s = Self(u); return s.sourceGeom.is_connection_node(s.sourceGeom.user, x, z);
        };
        h.cell_occupants = [](void* u, int32_t x, int32_t z, fc4_netgeom_occupant* out, uint32_t n) {
            auto& s = Self(u); return s.sourceGeom.cell_occupants(s.sourceGeom.user, x, z, out, n);
        };
        h.update_occupant_highlights = [](void* u, uint32_t type, const fc4_netgeom_cell* cells,
                                         uint32_t n, uint32_t valid) {
            auto& s = Self(u);
            return s.sourceGeom.update_occupant_highlights(s.sourceGeom.user, type, cells, n, valid);
        };
        h.bridge_type_count = [](void* u, uint32_t type) {
            auto& s = Self(u); return s.sourceGeom.bridge_type_count(s.sourceGeom.user, type);
        };
        // No game setters, redisplays, inserts, demolition or messages.
        return h;
    }

    ~NetworkShadow() {
        if (core) fc4_nettool_destroy(core);
        if (ctx.geom) fc4_netgeom_destroy(ctx.geom);
    }

    bool Prepare(const harness::ScriptAction& a, cISC4City* city) {
        ctx.city = city;
        sourceGeom = MakeNetGeomHost(&ctx);
        const auto gh = RecordingHost();
        ctx.geom = fc4_netgeom_create(&gh, nullptr, 0);
        if (!ctx.geom) return false;
        const auto sourceTool = MakeNetToolHost(&ctx);
        fc4_nettool_host th{};
        th.struct_size = sizeof(th);
        th.user = &ctx;
        th.draw_network_line = sourceTool.draw_network_line;
        th.validate_and_solve = sourceTool.validate_and_solve;
        th.get_network_occupant = sourceTool.get_network_occupant;
        core = fc4_nettool_create(&th, static_cast<uint32_t>(a.type));
        // Literal existing replacement Init inputs. Exemplar-dependent
        // underground/bridge/powerline setup is deliberately not inferred.
        if (!core || !fc4_nettool_init(core, city->CellCountX(), city->CellCountZ(),
                                       0, 0, 0, 0, a.type != 5)) return false;
        ok = fc4_nettool_place_network_occupants(core, a.x0, a.z0, a.x1, a.z1,
                                                0, 0, true, -1);
        failure = fc4_nettool_get_failure_state(core);
        CaptureCorners();
        if (ok) changedVertices = fc4_netgeom_update_terrain_heights(ctx.geom, nullptr);
        if (!fc4_netgeom_get_state(ctx.geom, &geom)) return false;
        if (a.type == 5) {
            powerlineObserved = ReadNetToolPowerlineSelection(&ctx, &powerline);
            if (powerlineObserved) {
                geom.valid = powerline.valid;
                failure = powerline.failure;
            }
        }
        const uint32_t count = fc4_netgeom_get_cells(ctx.geom, nullptr, 0);
        cells.resize(count);
        return fc4_netgeom_get_cells(ctx.geom, cells.data(), count) == count;
    }
};

struct NetworkCall {
    cFC4ISC4NetworkTool* tool;
    const harness::ScriptAction* a;
    bool ok;
    NetworkShadow* shadow = nullptr;
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
    if (c->shadow) {
        auto& s = *c->shadow;
        auto* t = reinterpret_cast<cFC4ISC4NetworkToolFull*>(c->tool);
        SC4Vector<SC4Point<uint32_t>> cells;
        t->GetDraggedCells(cells);
        const uint32_t failure = t->GetFailureState();
        const uint32_t valid = t->GetValidStepCount();
        const uint32_t straight = t->GetStraightStepCount();
        const uint32_t total = t->GetTotalStepCount();
        const int32_t cost = t->GetCostOfSolution();
        // Both getters must see post-commit terrain. Native real placement
        // grades it before returning; replaying against pre-commit heights
        // would count earthwork that the native getter no longer counts.
        // The solve itself still ran against the same pre-placement city.
        // Commit refresh has retained the replay's network corners. The
        // getter now reads the actual live terrain, as the native getter does.
        s.graded.clear();
        s.TraceCorners(a);
        const int32_t coreCost = s.powerlineObserved ? s.powerline.cost : fc4_netgeom_cost(s.ctx.geom, 0);
        uint32_t cellDiffs = cells.size() != s.cells.size() ? 1u : 0u;
        for (size_t i = 0; i < cells.size() && i < s.cells.size(); ++i) {
            if (static_cast<int32_t>(cells[i].x) != s.cells[i].x ||
                static_cast<int32_t>(cells[i].y) != s.cells[i].z) ++cellDiffs;
        }
        const uint32_t mismatches = cellDiffs + (c->ok != s.ok) + (failure != s.failure) +
            (valid != s.geom.valid) + (straight != s.geom.straight_count) +
            (total != s.geom.dragged_count) + (cost != coreCost);
        Log("nettool shadow %s (type=%d drag=(%d,%d)-(%d,%d) scope=drag/status/cost): mismatches=%u "
            "cells=%u/%u cell_diffs=%u ok=%d/%d failure=%08X/%08X valid=%u/%u straight=%u/%u "
            "total=%u/%u cost=%d/%d replay_changed=%u uncovered=pieces,bridges,poles,wires contract=NT27-NT31",
            harness::FormatDate(a.date).c_str(), a.type, a.x0, a.z0, a.x1, a.z1, mismatches,
            static_cast<uint32_t>(cells.size()), static_cast<uint32_t>(s.cells.size()), cellDiffs,
            c->ok ? 1 : 0, s.ok ? 1 : 0, failure, s.failure, valid, s.geom.valid,
            straight, s.geom.straight_count, total, s.geom.dragged_count, cost, coreCost, s.changedVertices);
    }
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
    NetworkShadow shadow;
    if (SettingInt("nettool_shadow", nullptr, 0) != 0) {
        if (ours) {
            Log("nettool shadow skipped: requires original nettool (disable replacement)");
        } else if (shadow.Prepare(a, city)) {
            c.shadow = &shadow;
        } else {
            Log("nettool shadow %s (type=%d setup): mismatches=1 unavailable=core",
                harness::FormatDate(a.date).c_str(), a.type);
        }
    }
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

std::string RunFunds(const harness::ScriptAction& a, cISC4City* city) {
    auto* budget = city ? AsGame(city->GetBudgetSimulator()) : nullptr;
    if (!budget) return "ok=0 (skipped: no city budget)";
    const int64_t before = budget->GetTotalFunds();
    // 00690140 has no defined bool return (including the unchanged-funds
    // path). Discard the SDK's AL declaration; qualify the write by read-back.
    budget->SetTotalFunds(a.funds);
    const int64_t after = budget->GetTotalFunds();
    char b[192];
    std::snprintf(b, sizeof(b), "ok=%d (SetTotalFunds(%d); treasury %lld -> %lld)",
                  after == a.funds ? 1 : 0, a.funds,
                  static_cast<long long>(before), static_cast<long long>(after));
    return b;
}

std::string RunBudgetWindow(cISC4City* city) {
    if (!city) return "ok=0 (skipped: no city)";
    auto* fw = RZGetFrameWork();
    auto* app = fw ? fw->Application() : nullptr; // borrowed
    cISC4App* sc4 = nullptr;
    if (!app || !app->QueryInterface(GZIID_cISC4App, reinterpret_cast<void**>(&sc4)) || !sc4)
        return "ok=0 (skipped: no app)";
    FC4_ACQUIRED(sc4, "budgetwindow app");
    auto* main = reinterpret_cast<cFC4IGZWin*>(sc4->GetMainWindow()); // borrowed
    auto* shell = main ? main->GetChildWindowFromID(0x6104489A) : nullptr; // borrowed
    cISC4View3DWin* view = nullptr;
    bool delivered = false;
    if (shell && shell->GetChildAs(0x9A47B417, kGZIID_cISC4View3DWin,
            reinterpret_cast<void**>(&view)) && view) {
        FC4_ACQUIRED(view, "budgetwindow city view");
        auto* win = reinterpret_cast<cFC4IGZWin*>(view->AsIGZWin()); // borrowed
        if (win) {
            // view3d-window §6 (advice-link payload): select budget and send
            // AA3AC40E to show tab AA3AC001. Budget slot 24 opens the
            // separate income/expense dialog (budget-dialogs §1).
            cGZMessage link(0xEA5BDFBC);
            link.dwData1 = 0x4A5BDFB8;
            win->DoMessage(link); // falls through to base; result is not visibility
            delivered = true;
            auto* panel = win->GetChildWindowFromIDRecursive(0xAA3AC001); // borrowed
            auto* taxes = win->GetChildWindowFromIDRecursive(0xAA3AC500); // borrowed
            auto* alternate = win->GetChildWindowFromIDRecursive(0xAA3AC400); // borrowed
            Log("budgetwindow observation: tab AA3AC001 visible=%u; selectors AA3AC500=%u AA3AC400=%u",
                panel && panel->GetFlag(1) ? 1u : 0u,
                taxes && taxes->GetFlag(1) ? 1u : 0u,
                alternate && alternate->GetFlag(1) ? 1u : 0u);
            // widget-toolkit section 5.3: recursive lookup prefers descendants,
            // including a hidden duplicate tab ID. Qualify the actual selector's
            // full ancestry and area with the same check used by uicontrol.
            int x = 0, y = 0;
            const bool visible = ui_driver::ControlPixel(0xAA3AC500, x, y);
            Log("budgetwindow selector ready=%u (button dispatch is deferred)", visible ? 1u : 0u);
        }
        FC4_RELEASE(view);
    }
    FC4_RELEASE(sc4);
    return delivered ? "posted budget advice link (deferred; qualify with delayed uicontrol and native reach)"
                     : "ok=0 (budget advice-link target unavailable)";
}

std::string RunEstablish(const harness::ScriptAction& a, cISC4City* city) {
    if (!city) return "ok=0 (skipped: no city)";
    if (city->GetEstablished()) return "ok=1 (already established; name unchanged)";
    // cSC4City::SetEstablished (00459a80) posts the city's establishment
    // message and calls simulator slot 33, enabling its established agents.
    // This is also the city command exposed to Maxis Lua as SetCityEstablished.
    const SC4String name(a.text.c_str());
    city->SetCityName(name);
    city->SetEstablished(true);
    SC4String actual;
    city->GetCityName(actual);
    return std::string(city->GetEstablished() ? "ok=1" : "ok=0") +
           " (SetEstablished(true); city name=" + actual.ToChar() + ")";
}

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
    long grace = a.fast ? 0 : SettingInt("script_save_grace_ms", "FC4_SCRIPT_SAVE_GRACE_MS", 0);
    if (grace < 0) grace = 0;
    if (grace > 120000) grace = 120000;
    if (grace > 0 && grace < 10000) grace = 10000;
    if (grace) Log("script: full-save watchdog grace %ld ms (script_save_grace_ms)", grace);
    InterlockedExchange(&g_saveGraceMs, grace);
    const ObserverSave r = ObserverSaveCity(a.fast);
    // We have resumed the observer's framework tick. Publish that progress
    // before dropping the allowance, so the completed save is not a stale
    // no-tick stall at the ordinary threshold.
    if (grace) NoteFrameworkTick();
    InterlockedExchange(&g_saveGraceMs, 0);
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

// Tooling: real source stations, identical calls against original/replacement.
std::string RunDispatch(const harness::ScriptAction& a, int32_t julian, cISC4City* city) {
    auto* dm = city != nullptr ? AsGame(city->GetDispatchManager()) : nullptr;
    if (dm == nullptr) return "unavailable: no dispatch manager";
    auto* source = reinterpret_cast<cFC4ISC4DispatchSource*>(dm->GetDispatchSource(a.type));
    if (source == nullptr) return "unavailable: no source (not an exercise success)";
    SC4List<cISC4Occupant*> stations;
    source->GetStations(stations);
    cS3DVector3 position;
    bool located = false;
    for (auto* station : stations) {
        if (!located && station != nullptr) located = station->GetPosition(&position);
    }
    std::string result = "unavailable: no station position (not an exercise success)";
    if (located) {
        const uint32_t freeBefore = dm->GetAvailablePylonCount(a.type);
        const uint32_t before = dm->GetDispatchCount(a.type);
        const bool can = dm->CanDispatch(a.type, position.fX, position.fZ);  // [abi-ret] dispatch.md §8 slot 5: AL
        const bool began = dm->BeginDispatchTool(a.type);  // [abi-ret] dispatch.md §8 slot 16: AL
        const uint32_t id = dm->Dispatch(a.type, position.fX, position.fZ);
        FC4DispatchInfo info{};
        const bool hasInfo = dm->GetDispatchInfo(id, info);
        WriteCapture record;
        const bool wrote = record.Capture(dm);
        Log("dispatch exercise: type=%d target=(%.3f,%.3f) can=%d begin=%d id=%u count=%u->%u "
            "free=%u->%u info=%d call=%u active=%d vehicle=%d pylon=%d view=%u record=%u bytes wrote=%d",
            a.type, position.fX, position.fZ, can, began, id, before, dm->GetDispatchCount(a.type),
            freeBefore, dm->GetAvailablePylonCount(a.type), hasInfo, info.callId, info.active,
            info.automaton != nullptr, info.pylon != nullptr, info.viewHandle,
            static_cast<unsigned>(record.Bytes().size()), wrote);
        if (DispatchShadowEnabled()) DispatchShadowTestRun(city, julian, 2);
        const bool revoked = id != 0 && dm->RevokeDispatchByID(id);  // [abi-ret] dispatch.md §8 slot 7: AL
        const bool ended = dm->EndDispatchTool(a.type);  // [abi-ret] dispatch.md §8 slot 17: AL
        if (DispatchShadowEnabled()) DispatchShadowTestRun(city, julian, 2);
        char text[240];
        std::snprintf(text, sizeof(text), "id=%u info=%d vehicle=%d revoke=%d end=%d count=%u free=%u "
                      "exercise=%s", id, hasInfo, info.automaton != nullptr, revoked, ended,
                      dm->GetDispatchCount(a.type), dm->GetAvailablePylonCount(a.type),
                      id != 0 && hasInfo && info.automaton != nullptr && revoked && ended ? "completed" : "INCOMPLETE");
        result = text;
    }
    for (auto*& station : stations) {
        // Ground-source list elements are acquired (protection.md §15.4).
        // Air-source list ownership remains DP13; never release unowned data.
        if (station != nullptr && (source->CanHandleDispatchType(0) || source->CanHandleDispatchType(1))) {
            FC4_ACQUIRED(station, "script dispatch GetStations");
            FC4_RELEASE(station);
        }
        station = nullptr;  // cleanup above owns the release, not SC4List's destructor
    }
    return result;
}

std::string RunWatercraft(const harness::ScriptAction& a, cISC4City* city) {
    auto* manager = city ? city->GetWatercraftManager() : nullptr;
    auto* terrain = city ? city->GetTerrain() : nullptr;
    if (!manager || !terrain) return "skipped: no watercraft manager or terrain";
    const int nx = static_cast<int>(city->CellCountX()), nz = static_cast<int>(city->CellCountZ());
    unsigned candidates = 0;
    const auto create = [&](int x, int z) -> std::string {
        // [choice] A 5x5 water patch keeps this fixture away from immediate grounding.
        if (x < 2 || z < 2 || x + 2 >= nx || z + 2 >= nz) return {};
        for (int dz = -2; dz <= 2; ++dz) for (int dx = -2; dx <= 2; ++dx) {
            float wx = 0, wz = 0;
            city->CellCenterToPosition(x + dx, z + dz, wx, wz);
            if (!(terrain->GetAltitude(wx, wz) < terrain->GetWaterLevel(wx, wz))) return {};
        }
        ++candidates;
        cS3DVector3 position;
        city->CellCenterToPosition(x, z, position.fX, position.fZ);
        position.fY = terrain->GetWaterLevel(position.fX, position.fZ);
        const cGZPersistResourceKey key{};
        // automata §7.2: slot 15 returns a borrowed primary interface. The
        // watercraft override ignores orientation; group/flags are §5.1.2's.
        auto* boat = manager->CreateAutomaton(0x4418, position, 0x10002, key, nullptr, 0xff, nullptr);
        if (!boat) return {};
        boat->AddRef();
        FC4_ACQUIRED(boat, "script watercraft fixture");
        boat->SetVisible(true);
        cS3DVector3 actual;
        boat->GetPosition(actual);
        FC4_RELEASE(boat);
        char text[256];
        _snprintf_s(text, sizeof(text), _TRUNCATE,
            "created group=0x4418 flags=0x10002 cell=(%d,%d) position=(%.9g,%.9g,%.9g) candidates=%u count=%u",
            x, z, actual.fX, actual.fY, actual.fZ, candidates, manager->GetAutomataCount());
        return text;
    };
    if (!a.on) {
        const auto result = create(a.x0, a.z0);
        return result.empty() ? "skipped: no water patch or native creation refused" : result;
    }
    for (int z = 2; z + 2 < nz; z += 4) for (int x = 2; x + 2 < nx; x += 4) {
        const auto result = create(x, z);
        if (!result.empty()) return result;
    }
    return "skipped: no water patch or native creation refused; candidates=" + std::to_string(candidates);
}

void RunAction(const harness::ScriptAction& a, int32_t julian, cISC4City* city) {
    using harness::ScriptOp;
    std::string result;
    bool digest = false;
    switch (a.op) {
        case ScriptOp::UiCommand: case ScriptOp::UiMouse: case ScriptOp::UiCell:
        case ScriptOp::UiKey: case ScriptOp::UiText: case ScriptOp::UiWait:
        case ScriptOp::UiControl:
            result = ui_driver::Run(a); break;
        case ScriptOp::Funds: result = RunFunds(a, city); break;
        case ScriptOp::Watercraft: result = RunWatercraft(a, city); break;
        case ScriptOp::Establish: result = RunEstablish(a, city); break;
        case ScriptOp::BudgetWindow: result = RunBudgetWindow(city); break;
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
        case ScriptOp::GZFrameworkProbe: result = GZFrameworkScriptProbe() ? "ok=1 (GZ command round trip)" : "ok=0 (GZ command round trip failed)"; break;
        case ScriptOp::MySim: result = MySimScriptAction(a.on); break;
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
        case ScriptOp::Dispatch: result = RunDispatch(a, julian, city); break;
        case ScriptOp::PortProps: result = PortScriptProps(a.x0, a.z0, static_cast<uint32_t>(a.type)); break;
        case ScriptOp::ClockMode: {
            // clock24.md section 4.5 / section 8: city slot 86 is borrowed;
            // slots 29/5 return no meaningful value (never read ST0).
            auto* clock = AsGame(city ? city->Get24HourClock() : nullptr);
            if (!clock) { result = "no 24-hour clock"; break; }
            clock->EnableDayNightTransitionNotifications(true);
            clock->SetVisualTimeMode(a.speed);
            result = "transition notifications on; visual mode " + std::to_string(clock->GetVisualTimeMode());
            break;
        }
        case ScriptOp::LightingHour:
            result = PropDevScriptLightingHour(a.speed);
            break;
        case ScriptOp::GDriverProbe:
            result = GDriverShadowProbe();
            break;
        case ScriptOp::Span565Probe:
            result = Span565ScriptProbe(harness::FormatDate(harness::CivilFromJulian(julian)).c_str());
            break;
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
    g_uiDelay = false;
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
        if (g_waiting.empty() && !g_inTick && a->op != harness::ScriptOp::Save && !ui_driver::IsAction(a->op)) {
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
    int32_t bootstrapJulian = 0;
    if (g_waiting.empty() && !g_inTick && g_schedule.Remaining()) {
        cISC4City* city = ObserverCity();
        auto* sim = city ? AsGame(city->GetSimulator()) : nullptr;
        if (sim && !city->GetEstablished()) {
            const int32_t julian = sim->GetSimDateNumber();
            if (g_schedule.BootstrapDue(julian)) {
                // An unfounded city never sends SimNewDay. Queue the explicit
                // leading establish and its same-date actions at a framework
                // tick instead; future actions still wait for real new days.
                g_inTick = true;
                ScriptOnNewDay(julian);
                g_inTick = false;
                bootstrapJulian = julian;
            }
        }
    }
    if (g_waiting.empty()) return;
    // Complete both readers on stable pre-save bytes across normal ticks.
    // A synchronous drain inside PreSave can exceed the watchdog interval.
    // Keep the save and every action behind it in their existing order.
    if (DbpfShadowPending())
        for (const auto* action : g_waiting)
            if (action->op == harness::ScriptOp::Save) {
                DbpfShadowWait();
                return;
            }
    if (g_waiting.empty()) return;
    if (g_uiDelay && static_cast<int32_t>(UiClockMs() - g_uiReady) < 0) return;
    g_uiDelay = false;
    // Modal windows pump framework ticks. Consume a UI action before calling
    // the game so a nested tick can deliver the queued close/input action.
    // Non-UI actions and nested commands stay behind the active command.
    const auto* first = g_waiting.front();
    if (ui_driver::IsAction(first->op)) {
        if (g_inTick && first->op == harness::ScriptOp::UiCommand) return;
        g_waiting.erase(g_waiting.begin());
        const bool previousTick = g_inTick;
        g_inTick = true;
        g_uiReady = UiClockMs() + (first->op == harness::ScriptOp::UiWait ? first->type : 150);
        g_uiDelay = true;
        RunAction(*first, static_cast<int32_t>(PublishedJulian()), ObserverCity());
        g_inTick = previousTick;
        NoteAllRun();
        return;
    }
    if (g_inTick) return;
    std::vector<const harness::ScriptAction*> run;
    run.swap(g_waiting);
    g_inTick = true;  // a SimNewDay during these calls queues behind them
    // No new day has published a date in an unfounded city yet.
    const int32_t julian = bootstrapJulian ? bootstrapJulian : static_cast<int32_t>(PublishedJulian());
    cISC4City* city = ObserverCity();
    for (size_t i = 0; i < run.size(); ++i) {
        if (ui_driver::IsAction(run[i]->op)) {
            g_waiting.insert(g_waiting.begin(), run.begin() + i, run.end());
            break;
        }
        RunAction(*run[i], julian, city);
    }
    g_inTick = false;
    NoteAllRun();
}

bool ScriptWaiting() {
    return g_inTick || !g_waiting.empty() ||
        (g_uiDelay && static_cast<int32_t>(UiClockMs() - g_uiReady) < 0);
}

uint32_t ScriptSaveGraceMs() { return static_cast<uint32_t>(InterlockedCompareExchange(&g_saveGraceMs, 0, 0)); }

}  // namespace fc4
