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

#include "FC4Config.h"

#include "FC4Log.h"

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>

#include <cctype>
#include <cstdlib>
#include <cstring>
#include <map>
#include <string>

namespace fc4 {
namespace {

constexpr unsigned kCount = static_cast<unsigned>(Sw::Count);

struct SwitchInfo {
    const char* key;  // ini key; the environment variable is FC4_BUDGET_<KEY>
    const char* env;
};

const SwitchInfo kSwitches[kCount] = {
    {"lot_scan", "FC4_BUDGET_LOT_SCAN"},
    {"property_writes", "FC4_BUDGET_PROPERTY_WRITES"},
    {"property_reads", "FC4_BUDGET_PROPERTY_READS"},
    {"message_posting", "FC4_BUDGET_MESSAGE_POSTING"},
    {"history", "FC4_BUDGET_HISTORY"},
    {"building_name", "FC4_BUDGET_BUILDING_NAME"},
    {"exemplar_loads", "FC4_BUDGET_EXEMPLAR_LOADS"},
    {"disaster", "FC4_BUDGET_DISASTER"},
    {"cheats", "FC4_BUDGET_CHEATS"},
    {"holder_refs", "FC4_BUDGET_HOLDER_REFS"},
    {"ltext", "FC4_BUDGET_LTEXT"},
    {"lot_list", "FC4_BUDGET_LOT_LIST"},
    {"lot_building", "FC4_BUDGET_LOT_BUILDING"},
    {"lot_profile", "FC4_BUDGET_LOT_PROFILE"},
    {"lot_building_value", "FC4_BUDGET_LOT_BUILDING_VALUE"},
    {"lot_land_value", "FC4_BUDGET_LOT_LAND_VALUE"},
    {"lot_size", "FC4_BUDGET_LOT_SIZE"},
    {"lot_state", "FC4_BUDGET_LOT_STATE"},
    {"lot_release", "FC4_BUDGET_LOT_RELEASE"},
    {"lot_list_free", "FC4_BUDGET_LOT_LIST_FREE"},
    {"power_occupants", "FC4_POWER_OCCUPANTS"},
    {"power_poles", "FC4_POWER_POLES"},
    {"power_lots", "FC4_POWER_LOTS"},
    {"power_neighbors", "FC4_POWER_NEIGHBORS"},
    {"power_budget", "FC4_POWER_BUDGET"},
    {"power_grid_writes", "FC4_POWER_GRID_WRITES"},
    {"power_posts", "FC4_POWER_POSTS"},
    {"power_terrain", "FC4_POWER_TERRAIN"},
    {"power_zone_tool", "FC4_POWER_ZONE_TOOL"},
    {"power_view", "FC4_POWER_VIEW"},
    {"power_radiation", "FC4_POWER_RADIATION"},
    {"power_occupant_query", "FC4_POWER_OCCUPANT_QUERY"},
    {"power_demolish", "FC4_POWER_DEMOLISH"},
    {"power_direct_calls", "FC4_POWER_DIRECT_CALLS"},
    {"plumbing_occupants", "FC4_PLUMBING_OCCUPANTS"},
    {"plumbing_pipes", "FC4_PLUMBING_PIPES"},
    {"plumbing_lots", "FC4_PLUMBING_LOTS"},
    {"plumbing_terrain", "FC4_PLUMBING_TERRAIN"},
    {"plumbing_neighbors", "FC4_PLUMBING_NEIGHBORS"},
    {"plumbing_budget", "FC4_PLUMBING_BUDGET"},
    {"plumbing_grid_writes", "FC4_PLUMBING_GRID_WRITES"},
    {"plumbing_posts", "FC4_PLUMBING_POSTS"},
    {"plumbing_pollution", "FC4_PLUMBING_POLLUTION"},
    {"plumbing_ordinance", "FC4_PLUMBING_ORDINANCE"},
    {"plumbing_disaster", "FC4_PLUMBING_DISASTER"},
    {"plumbing_history", "FC4_PLUMBING_HISTORY"},
    {"plumbing_direct_calls", "FC4_PLUMBING_DIRECT_CALLS"},
    {"flammability_occupants", "FC4_FLAMMABILITY_OCCUPANTS"},
    {"flammability_lots", "FC4_FLAMMABILITY_LOTS"},
    {"flammability_water", "FC4_FLAMMABILITY_WATER"},
    {"flammability_ordinance", "FC4_FLAMMABILITY_ORDINANCE"},
    {"flammability_season", "FC4_FLAMMABILITY_SEASON"},
    {"flammability_zoning", "FC4_FLAMMABILITY_ZONING"},
    {"flammability_fire", "FC4_FLAMMABILITY_FIRE"},
    {"flammability_grid_writes", "FC4_FLAMMABILITY_GRID_WRITES"},
    {"flammability_posts", "FC4_FLAMMABILITY_POSTS"},
    {"flammability_history", "FC4_FLAMMABILITY_HISTORY"},
    {"pollution_occupants", "FC4_POLLUTION_OCCUPANTS"},
    {"pollution_lots", "FC4_POLLUTION_LOTS"},
    {"pollution_neighbors", "FC4_POLLUTION_NEIGHBORS"},
    {"pollution_budget", "FC4_POLLUTION_BUDGET"},
    {"pollution_grid_writes", "FC4_POLLUTION_GRID_WRITES"},
    {"pollution_posts", "FC4_POLLUTION_POSTS"},
    {"pollution_history", "FC4_POLLUTION_HISTORY"},
    {"pollution_ordinance", "FC4_POLLUTION_ORDINANCE"},
    {"pollution_traffic", "FC4_POLLUTION_TRAFFIC"},
    {"pollution_zoning", "FC4_POLLUTION_ZONING"},
    {"pollution_power", "FC4_POLLUTION_POWER"},
    {"pollution_landfill", "FC4_POLLUTION_LANDFILL"},
    {"pollution_population", "FC4_POLLUTION_POPULATION"},
    {"pollution_props", "FC4_POLLUTION_PROPS"},
    {"pollution_terrain", "FC4_POLLUTION_TERRAIN"},
    {"protection_occupants", "FC4_PROTECTION_OCCUPANTS"},
    {"protection_budget", "FC4_PROTECTION_BUDGET"},
    {"protection_dispatch", "FC4_PROTECTION_DISPATCH"},
    {"protection_automata", "FC4_PROTECTION_AUTOMATA"},
    {"protection_grid_writes", "FC4_PROTECTION_GRID_WRITES"},
    {"protection_posts", "FC4_PROTECTION_POSTS"},
    {"protection_history", "FC4_PROTECTION_HISTORY"},
    {"protection_air_sources", "FC4_PROTECTION_AIR_SOURCES"},
    {"crime_lots", "FC4_CRIME_LOTS"},
    {"crime_police", "FC4_CRIME_POLICE"},
    {"crime_residential", "FC4_CRIME_RESIDENTIAL"},
    {"crime_demand", "FC4_CRIME_DEMAND"},
    {"crime_ordinance", "FC4_CRIME_ORDINANCE"},
    {"crime_occupants", "FC4_CRIME_OCCUPANTS"},
    {"crime_fire", "FC4_CRIME_FIRE"},
    {"crime_props", "FC4_CRIME_PROPS"},
    {"crime_posts", "FC4_CRIME_POSTS"},
    {"crime_grid_writes", "FC4_CRIME_GRID_WRITES"},
    {"crime_zoning", "FC4_CRIME_ZONING"},
    {"demand_capacity", "FC4_DEMAND_CAPACITY"},
    {"demand_direct_calls", "FC4_DEMAND_DIRECT_CALLS"},
    {"aura_lots", "FC4_AURA_LOTS"},
    {"aura_lot_populations", "FC4_AURA_LOT_POPULATIONS"},
    {"aura_occupants", "FC4_AURA_OCCUPANTS"},
    {"aura_budget", "FC4_AURA_BUDGET"},
    {"aura_ordinance", "FC4_AURA_ORDINANCE"},
    {"aura_land_value", "FC4_AURA_LAND_VALUE"},
    {"aura_inputs", "FC4_AURA_INPUTS"},
    {"aura_grid_writes", "FC4_AURA_GRID_WRITES"},
    {"aura_posts", "FC4_AURA_POSTS"},
    {"aura_history", "FC4_AURA_HISTORY"},
    {"aura_disaster", "FC4_AURA_DISASTER"},
    {"aura_strikes", "FC4_AURA_STRIKES"},
    {"flora_occupants", "FC4_FLORA_OCCUPANTS"},
    {"flora_create", "FC4_FLORA_CREATE"},
    {"flora_manager", "FC4_FLORA_MANAGER"},
    {"flora_queries", "FC4_FLORA_QUERIES"},
    {"flora_terrain", "FC4_FLORA_TERRAIN"},
    {"flora_weather", "FC4_FLORA_WEATHER"},
    {"flora_records", "FC4_FLORA_RECORDS"},
    {"flora_posts", "FC4_FLORA_POSTS"},
    {"flora_cheats", "FC4_FLORA_CHEATS"},
    {"flora_exemplars", "FC4_FLORA_EXEMPLARS"},
    {"flora_obliteration", "FC4_FLORA_OBLITERATION"},
    {"landfill_terrain", "FC4_LANDFILL_TERRAIN"},
    {"landfill_levelling", "FC4_LANDFILL_LEVELLING"},
    {"landfill_pollution", "FC4_LANDFILL_POLLUTION"},
    {"landfill_budget", "FC4_LANDFILL_BUDGET"},
    {"landfill_zoning", "FC4_LANDFILL_ZONING"},
    {"landfill_occupants", "FC4_LANDFILL_OCCUPANTS"},
    {"landfill_props", "FC4_LANDFILL_PROPS"},
    {"landfill_traffic", "FC4_LANDFILL_TRAFFIC"},
    {"landfill_history", "FC4_LANDFILL_HISTORY"},
    {"landfill_effects", "FC4_LANDFILL_EFFECTS"},
    {"residential_occupants", "FC4_RESIDENTIAL_OCCUPANTS"},
    {"residential_budget", "FC4_RESIDENTIAL_BUDGET"},
    {"residential_developers", "FC4_RESIDENTIAL_DEVELOPERS"},
    {"residential_pollution", "FC4_RESIDENTIAL_POLLUTION"},
    {"residential_ordinance", "FC4_RESIDENTIAL_ORDINANCE"},
    {"residential_automata", "FC4_RESIDENTIAL_AUTOMATA"},
    {"residential_grid_writes", "FC4_RESIDENTIAL_GRID_WRITES"},
    {"residential_posts", "FC4_RESIDENTIAL_POSTS"},
    {"residential_history", "FC4_RESIDENTIAL_HISTORY"},
    {"residential_lots", "FC4_RESIDENTIAL_LOTS"},
    {"residential_terrain", "FC4_RESIDENTIAL_TERRAIN"},
    {"residential_effects", "FC4_RESIDENTIAL_EFFECTS"},
    {"zones_grid_writes", "FC4_ZONES_GRID_WRITES"},
    {"zones_budget", "FC4_ZONES_BUDGET"},
    {"zones_lots", "FC4_ZONES_LOTS"},
    {"zones_out_lots", "FC4_ZONES_OUT_LOTS"},
    {"zones_demolition", "FC4_ZONES_DEMOLITION"},
    {"zones_parcellization", "FC4_ZONES_PARCELLIZATION"},
    {"zones_posts", "FC4_ZONES_POSTS"},
    {"zones_messages", "FC4_ZONES_MESSAGES"},
    {"zones_traffic", "FC4_ZONES_TRAFFIC"},
    {"zones_terrain", "FC4_ZONES_TERRAIN"},
    {"zones_occupants", "FC4_ZONES_OCCUPANTS"},
    {"zones_landfill_cost", "FC4_ZONES_LANDFILL_COST"},
    {"zones_lot_developer", "FC4_ZONES_LOT_DEVELOPER"},
    {"neighbors_region_reads", "FC4_NEIGHBORS_REGION_READS"},
    {"neighbors_region_writes", "FC4_NEIGHBORS_REGION_WRITES"},
    {"neighbors_deals", "FC4_NEIGHBORS_DEALS"},
    {"neighbors_reload", "FC4_NEIGHBORS_RELOAD"},
    {"neighbors_symmetric", "FC4_NEIGHBORS_SYMMETRIC"},
    {"neighbors_markers", "FC4_NEIGHBORS_MARKERS"},
    {"neighbors_posts", "FC4_NEIGHBORS_POSTS"},
    {"neighbors_history", "FC4_NEIGHBORS_HISTORY"},
    {"neighbors_figures", "FC4_NEIGHBORS_FIGURES"},
    {"development_map_writes", "FC4_DEVELOPMENT_MAP_WRITES"},
    {"development_occupants", "FC4_DEVELOPMENT_OCCUPANTS"},
    {"development_zoning", "FC4_DEVELOPMENT_ZONING"},
    {"development_lots", "FC4_DEVELOPMENT_LOTS"},
    {"development_lot_developer", "FC4_DEVELOPMENT_LOT_DEVELOPER"},
    {"development_demolition", "FC4_DEVELOPMENT_DEMOLITION"},
    {"development_advisor", "FC4_DEVELOPMENT_ADVISOR"},
    {"development_posts", "FC4_DEVELOPMENT_POSTS"},
    {"development_cheats", "FC4_DEVELOPMENT_CHEATS"},
    {"development_tract", "FC4_DEVELOPMENT_TRACT"},
    {"development_city", "FC4_DEVELOPMENT_CITY"},
    {"traffic_grid_writes", "FC4_TRAFFIC_GRID_WRITES"},
    {"traffic_budget", "FC4_TRAFFIC_BUDGET"},
    {"traffic_lots", "FC4_TRAFFIC_LOTS"},
    {"traffic_sources", "FC4_TRAFFIC_SOURCES"},
    {"traffic_holders", "FC4_TRAFFIC_HOLDERS"},
    {"traffic_neighbors", "FC4_TRAFFIC_NEIGHBORS"},
    {"traffic_occupants", "FC4_TRAFFIC_OCCUPANTS"},
    {"traffic_terrain", "FC4_TRAFFIC_TERRAIN"},
    {"traffic_posts", "FC4_TRAFFIC_POSTS"},
    {"traffic_history", "FC4_TRAFFIC_HISTORY"},
    {"traffic_simulators", "FC4_TRAFFIC_SIMULATORS"},
    {"traffic_messages", "FC4_TRAFFIC_MESSAGES"},
    {"tract_construction", "FC4_TRACT_CONSTRUCTION"},
    {"tract_abandon", "FC4_TRACT_ABANDON"},
    {"tract_lot_changes", "FC4_TRACT_LOT_CHANGES"},
    {"tract_demolition", "FC4_TRACT_DEMOLITION"},
    {"tract_population", "FC4_TRACT_POPULATION"},
    {"tract_resources", "FC4_TRACT_RESOURCES"},
    {"tract_filters", "FC4_TRACT_FILTERS"},
    {"tract_lot_lists", "FC4_TRACT_LOT_LISTS"},
    {"tract_traffic", "FC4_TRACT_TRAFFIC"},
    {"tract_style_map", "FC4_TRACT_STYLE_MAP"},
    {"tract_roads", "FC4_TRACT_ROADS"},
    {"tract_family_sets", "FC4_TRACT_FAMILY_SETS"},
    {"tract_agents", "FC4_TRACT_AGENTS"},
    // commercial
    {"commercial_grid_writes", "FC4_COMMERCIAL_GRID_WRITES"},
    {"zonedev_grid_writes", "FC4_ZONEDEV_GRID_WRITES"},
    {"zonedev_messages", "FC4_ZONEDEV_MESSAGES"},
    {"zonedev_network_tools", "FC4_ZONEDEV_NETWORK_TOOLS"},
    {"zonedev_posts", "FC4_ZONEDEV_POSTS"},
    {"zonedev_demolition", "FC4_ZONEDEV_DEMOLITION"},
    {"zonedev_streets", "FC4_ZONEDEV_STREETS"},
    {"zonedev_occupants", "FC4_ZONEDEV_OCCUPANTS"},
    {"zonedev_terrain", "FC4_ZONEDEV_TERRAIN"},
    {"zonedev_lot_changes", "FC4_ZONEDEV_LOT_CHANGES"},
    {"zonedev_growth", "FC4_ZONEDEV_GROWTH"},
    {"zonedev_levelling", "FC4_ZONEDEV_LEVELLING"},
    {"zonedev_lot_developer", "FC4_ZONEDEV_LOT_DEVELOPER"},
    {"zonedev_traffic", "FC4_ZONEDEV_TRAFFIC"},
    {"lotdev_lot_writes", "FC4_LOTDEV_LOT_WRITES"},
    {"lotdev_buildings", "FC4_LOTDEV_BUILDINGS"},
    {"lotdev_demolition", "FC4_LOTDEV_DEMOLITION"},
    {"lotdev_props", "FC4_LOTDEV_PROPS"},
    {"lotdev_textures", "FC4_LOTDEV_TEXTURES"},
    {"lotdev_terrain", "FC4_LOTDEV_TERRAIN"},
    {"lotdev_night", "FC4_LOTDEV_NIGHT"},
    {"lotdev_units", "FC4_LOTDEV_UNITS"},
    {"lotdev_agents", "FC4_LOTDEV_AGENTS"},
};

// Keys that are not switches.
const char* const kOtherKeys[] = {"disable", "enable", "budget_default", "refaudit", "refaudit_lines", "autotest_seconds",
                                  "autotest_unpause", "autotest_save_after_months", "autotest_save_fast",
                                  "digest", "observe", "power_report", "autoload_city", "autoload_delay_ticks",
                                  "budget_shadow", "budget_shadow_lots", "demand_shadow",
                                  "demand_shadow_nodes",
                                  "popup_dialogs", "clock_probe", "autotest_end",
                                  "pollution_shadow", "pollution_shadow_cells", "protection_shadow",
                                  "crime_shadow", "crime_shadow_cells", "crime_seed",
                                  "aura_shadow", "aura_shadow_cells",
                                  "flora_shadow",
                                  "landfill_shadow", "landfill_shadow_diffs",
                                  "power_shadow", "power_shadow_cells", "power_shadow_lines",
                                  "plumbing_shadow", "plumbing_shadow_cells", "plumbing_shadow_passes",
                                  "landvalue_shadow", "landvalue_shadow_cells",
                                  "flammability_shadow", "flammability_shadow_cells",
                                  "residential_shadow", "residential_shadow_cells", "residential_seed",
                                  "zones_shadow",
                                  "neighbours_shadow", "neighbors_seed", "neighbors_two_curves",
                                  "growth_probe",
                                  "development_growth_attempts_per_day", "development_seed",
                                  "development_shadow",
                                  "traffic_units_per_slice", "traffic_seed",
                                  // the harness lane's tooling (FC4Turbo.h, FC4Autotest.h, FC4Script.h,
                                  // FC4Dialogs.h)
                                  "sim_turbo", "sim_turbo_frame_ms", "sim_turbo_until", "autotest_end_date", "script",
                                  "sim_turbo_wait_days", "close_dialogs",
                                  "tract_shadow", "tract_seed", "tract_grow_log",
                                  // the cmglue lane (FC4CommercialShadow.h, cFC4Civic.cpp, cFC4Parks.cpp)
                                  "commercial_shadow", "commercial_shadow_cells", "civic_shadow", "civic_seed",
                                  "parks_shadow", "parks_seed",
                                  "zonedev_shadow", "zonedev_seed",
                                  // the histclock lane (FC4HistoryShadow.h, FC4Clock24Shadow.h)
                                  "history_shadow", "clock24_shadow",
                                  "lotdev_shadow", "lotdev_seed", "lotdev_lot_log",
                                  // the traffic2 lane (FC4TrafficShadow.h)
                                  "traffic_shadow", "traffic_shadow_diffs", "traffic_shadow_lines",
                                  "traffic_shadow_hypotheses",
                                  // the glue16 lane (FC4TerrainShadow.h)
                                  "terrain_shadow", "terrain_shadow_points", "terrain_shadow_diffs",
                                  "terrain_shadow_lines",
                                  // the glue22 lane (FC4EffectsShadow.h, cFC4Effects.h)
                                  "effects_shadow", "effects_finish", "effects_replace",
                                  // the glue13 lane (FC4DisasterShadow.h)
                                  "disasters_shadow",
                                  // the glue17 lane (FC4CitySitShadow.h, FC4DemolitionShadow.h,
                                  // FC4DispatchShadow.h)
                                  "citysit_shadow", "demolition_shadow", "dispatch_shadow",
                                  // the glue23 lane (FC4DemolitionPriceShadow.h)
                                  "demolition_price_shadow",
                                  // the glue15 lane (FC4PropLotShadow.h)
                                  "propmgr_shadow", "propmgr_shadow_cells", "lotcfg_shadow", "tutorial_shadow",
                                  // the glue19 lane (FC4AutomataShadow.h)
                                  "automata_shadow",                                  // the seeding lane (FC4Seed.h)
                                  "session_seed",
// the glue12b1 lane (FC4PropDevShadow.h, FC4CityDetailShadow.h)
                                  "propdev_shadow", "propdev_seed", "citydetail_shadow", "citydetail_seed",
                                  // the glue12b2 lane (FC4PortsShadow.h)
                                  "ports_shadow",
                                  // the mysims1 lane (FC4MySimsShadow.h)
                                  "mysims_shadow",
                                  // the netmgr shadow (FC4NetMgrShadow.h; read there, never registered
                                  // before verify-regress4)
                                  "netmgr_shadow", "netmgr_shadow_order", "netmgr_shadow_wide",
                                  // the glue-formats4 lane (FC4FormatsShadow.h)
                                  "formats_shadow", "formats_shadow_max", "formats_shadow_lines",
                                  // the occupant manager shadow (FC4OccMgrShadow.h; read there, never
                                  // registered before verify-occmgr4)
                                  "occmgr_shadow", "occmgr_shadow_samples", "occmgr_shadow_probe",
                                  // the glue-simgrids4 lane (FC4SimGridProbe.h)
                                  "simgrids_probe",
                                  // the glue-span4444-impl4 lane (FC4SpanRender4444.h)
                                  "span4444_shadow",
                                  // verify-occmgr4: the shadow's call follow (FC4OccMgrShadow.cpp)
                                  "occmgr_shadow_follow",
                                  // the glue-span565-impl4 lane (FC4Span565Bridge.h)
                                  "span565_trace",
                                  "span8888_shadow",
                                  // verify-ordinance4: independent original/Rust observation, off by default.
                                  "ordinance_shadow",
};
// The switches that are off, for lock-free readers (autotest verdict).
char g_offList[512] = "-";

volatile LONG g_on[kCount] = {1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
                               1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
                               1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
                               // pollution
                               1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
                               // protection
                               1, 1, 1, 1, 1, 1, 1, 1,
                               // crime
                               1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
                               // demand
                               1, 1,
                               // aura
                               1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
                               // flora
                               1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
                               // landfill
                               1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
                               // residential
                               1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
                               // zones
                               1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
                               // neighbours
                               1, 1, 1, 1, 1, 1, 1, 1, 1,
                               // development
                               1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
                               // traffic
                               1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
                               // tract
                               1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
                               // commercial
                               1,
                               // zonedev
                               1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
                               // lotdev
                               1, 1, 1, 1, 1, 1, 1, 1, 1};
static_assert(kCount == 219, "g_on: one 1 per switch");
static_assert(sizeof(kSwitches) / sizeof(kSwitches[0]) == kCount, "one entry per switch");
volatile LONG g_skipped[kCount] = {};
volatile LONG g_skipLogged[kCount] = {};

std::string Lower(std::string s) {
    for (char& c : s) c = static_cast<char>(std::tolower(static_cast<unsigned char>(c)));
    return s;
}

std::string Trim(const std::string& s) {
    size_t a = 0, b = s.size();
    while (a < b && std::isspace(static_cast<unsigned char>(s[a]))) ++a;
    while (b > a && std::isspace(static_cast<unsigned char>(s[b - 1]))) --b;
    return s.substr(a, b - a);
}

std::string Env(const char* name) {
    if (name == nullptr) return std::string();
    char value[512] = {};
    const DWORD n = GetEnvironmentVariableA(name, value, sizeof(value));
    return (n > 0 && n < sizeof(value)) ? std::string(value, n) : std::string();
}

// Parses FreeCity4.ini; `found` tells whether the file exists.
std::map<std::string, std::string> ReadIni(bool* found) {
    std::map<std::string, std::string> out;
    if (found != nullptr) *found = false;
    const std::string path = IniPath();
    if (path.empty()) return out;
    HANDLE f = CreateFileA(path.c_str(), GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                           nullptr, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (f == INVALID_HANDLE_VALUE) return out;
    if (found != nullptr) *found = true;
    std::string text;
    char buf[4096];
    DWORD got = 0;
    while (ReadFile(f, buf, sizeof(buf), &got, nullptr) && got > 0 && text.size() < 256 * 1024) text.append(buf, got);
    CloseHandle(f);
    if (text.size() >= 3 && static_cast<unsigned char>(text[0]) == 0xEF && static_cast<unsigned char>(text[1]) == 0xBB &&
        static_cast<unsigned char>(text[2]) == 0xBF) {
        text.erase(0, 3);  // UTF-8 BOM (Notepad)
    }
    size_t at = 0;
    while (at < text.size()) {
        size_t end = text.find_first_of("\r\n", at);
        if (end == std::string::npos) end = text.size();
        std::string line = text.substr(at, end - at);
        at = end + 1;
        // A comment starts the line or follows whitespace (so "a;b" lists survive).
        for (size_t i = 0; i < line.size(); ++i) {
            if ((line[i] == '#' || line[i] == ';') && (i == 0 || line[i - 1] == ' ' || line[i - 1] == '\t')) {
                line.erase(i);
                break;
            }
        }
        line = Trim(line);
        if (line.empty() || line[0] == '[') continue;
        const size_t eq = line.find('=');
        if (eq == std::string::npos) continue;
        const std::string key = Lower(Trim(line.substr(0, eq)));
        if (!key.empty()) out[key] = Trim(line.substr(eq + 1));
    }
    return out;
}

// 1 = on, 0 = off, -1 = not a boolean.
int ParseBool(const std::string& value) {
    const std::string v = Lower(Trim(value));
    if (v == "1" || v == "on" || v == "true" || v == "yes") return 1;
    if (v == "0" || v == "off" || v == "false" || v == "no") return 0;
    return -1;
}

bool KnownKey(const std::string& key) {
    for (const SwitchInfo& s : kSwitches) {
        if (key == s.key) return true;
    }
    for (const char* k : kOtherKeys) {
        if (key == k) return true;
    }
    return false;
}

}  // namespace

std::string IniPath() {
    const std::string dll = ModulePath();
    const size_t slash = dll.find_last_of("\\/");
    if (slash == std::string::npos) return std::string();
    return dll.substr(0, slash + 1) + "FreeCity4.ini";
}

bool On(Sw s) { return s < Sw::Count && g_on[static_cast<unsigned>(s)] != 0; }

const char* SwitchesOffList() { return g_offList; }

long SettingInt(const char* iniKey, const char* envName, long fallback) {
    const std::string v = Trim(Setting(iniKey, envName, nullptr));
    if (v.empty()) return fallback;
    const int b = ParseBool(v);
    if (b == 0) return 0;
    char* end = nullptr;
    const long x = strtol(v.c_str(), &end, 10);
    if (end != nullptr && *end == 0) return x;
    return b == 1 ? 1 : fallback;
}

const char* SwitchName(Sw s) { return s < Sw::Count ? kSwitches[static_cast<unsigned>(s)].key : "?"; }

void NoteSkipped(Sw s, const char* what) {
    if (s >= Sw::Count) return;
    const unsigned i = static_cast<unsigned>(s);
    InterlockedIncrement(&g_skipped[i]);
    if (InterlockedIncrement(&g_skipLogged[i]) <= 3) {
        FC4_LOGV("switch '%s' off: %s skipped", kSwitches[i].key, what);
    }
}

void LogSkipCounts(const char* why) {
    std::string list;
    for (unsigned i = 0; i < kCount; ++i) {
        const LONG n = InterlockedExchange(&g_skipped[i], 0);
        if (n == 0) continue;
        char item[64];
        _snprintf_s(item, sizeof(item), _TRUNCATE, "%s%s %ld", list.empty() ? "" : ", ", kSwitches[i].key, n);
        list += item;
    }
    if (!list.empty()) Log("switches (%s): calls skipped since the last report: %s", why, list.c_str());
}

std::string Setting(const char* iniKey, const char* envName, const char** source) {
    if (source != nullptr) *source = "";
    const std::map<std::string, std::string> ini = ReadIni(nullptr);
    auto it = ini.find(Lower(iniKey));
    if (it != ini.end()) {
        if (source != nullptr) *source = "FreeCity4.ini";
        return it->second;
    }
    std::string v = Env(envName);
    if (!v.empty() && source != nullptr) *source = envName;
    return v;
}

void ConfigReload(const char* why) {
    bool found = false;
    const std::map<std::string, std::string> ini = ReadIni(&found);
    for (const auto& kv : ini) {
        if (!KnownKey(kv.first)) Log("config: WARNING unknown key '%s' in FreeCity4.ini (ignored)", kv.first.c_str());
    }

    // budget_default (ini > env FC4_BUDGET_DEFAULT): the value of unset switches.
    int def = 1;
    {
        auto it = ini.find("budget_default");
        const std::string v = it != ini.end() ? it->second : Env("FC4_BUDGET_DEFAULT");
        if (!v.empty()) {
            const int b = ParseBool(v);
            if (b < 0) {
                Log("config: WARNING budget_default='%s' is not 0/1; using 1", v.c_str());
            } else {
                def = b;
            }
        }
    }

    std::string off, fromIni, fromEnv;
    unsigned offCount = 0;
    for (unsigned i = 0; i < kCount; ++i) {
        int value = def;
        auto it = ini.find(kSwitches[i].key);
        std::string raw;
        const char* src = nullptr;
        if (it != ini.end()) {
            raw = it->second;
            src = "ini";
        } else {
            raw = Env(kSwitches[i].env);
            if (!raw.empty()) src = "env";
        }
        if (src != nullptr) {
            const int b = ParseBool(raw);
            if (b < 0) {
                Log("config: WARNING %s='%s' (%s) is not 0/1; keeping %d", kSwitches[i].key, raw.c_str(), src, def);
            } else {
                value = b;
            }
            std::string& list = (src[0] == 'i') ? fromIni : fromEnv;
            list += list.empty() ? "" : ", ";
            list += kSwitches[i].key;
            list += value ? "=1" : "=0";
        }
        InterlockedExchange(&g_on[i], value);
        if (!value) {
            off += off.empty() ? "" : ", ";
            off += kSwitches[i].key;
            ++offCount;
        }
    }
    Log("config (%s): %s %s; budget_default=%d; set by ini: %s; set by env: %s", why, IniPath().c_str(),
        found ? "found" : "not found", def, fromIni.empty() ? "-" : fromIni.c_str(),
        fromEnv.empty() ? "-" : fromEnv.c_str());
    strncpy_s(g_offList, off.empty() ? "-" : off.c_str(), _TRUNCATE);
    for (char* p = g_offList; *p; ++p) {
        if (*p == ' ') *p = ',';  // "a, b" -> "a,,b" is avoided below
    }
    {
        // Compact "a,,b" (from ", ") to "a,b" for a single-token field.
        char* w = g_offList;
        for (const char* r = g_offList; *r; ++r) {
            if (!(*r == ',' && w > g_offList && w[-1] == ',')) *w++ = *r;
        }
        *w = 0;
    }
    if (offCount == 0) {
        Log("config (%s): all %u budget switches ON", why, kCount);
    } else {
        Log("config (%s): budget switches OFF (%u of %u): %s", why, offCount, kCount, off.c_str());
    }
}

}  // namespace fc4
