/*
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
 * Copyright (C) 2026 FreeCity4 contributors
 *
 * This program is free software: you can redistribute it and/or modify it
 * under the terms of the GNU General Public License as published by the Free
 * Software Foundation, either version 3 of the License, or (at your option)
 * any later version. This program is distributed WITHOUT ANY WARRANTY; see
 * the GNU General Public License for more details. You should have received a
 * copy of the license along with this program (see LICENSE at the repository
 * root); if not, see <https://www.gnu.org/licenses/>.
 */

/*
 * fc4.h - the C ABI of the FreeCity4 core (Rust crate fc4-capi).
 *
 * Hand-written; it must stay in sync with engine/capi/src/lib.rs. The layout
 * checks at the bottom catch drift at compile time on the C/C++ side, and
 * fc4_abi_version() lets a consumer refuse a mismatched library.
 *
 * Conventions
 *   - All functions use the platform's default C calling convention.
 *   - `bool` is C99/C++ bool (1 byte).
 *   - Strings are UTF-8, passed as pointer + length (no NUL terminator needed).
 *   - Every function is safe to call re-entrantly from inside a host callback
 *     (e.g. an agent's deliver() may add/remove agents or pause).
 *   - No Rust panic ever crosses this boundary: release builds abort, and each
 *     entry point catches unwinding and returns its documented fallback value.
 *   - A NULL `fc4_sim*` is tolerated by every function (it returns the fallback).
 *   - Single-threaded: one simulator must only be used from one thread at a time.
 */
#ifndef FC4_H
#define FC4_H

#include <stddef.h>
#include <stdint.h>
#ifndef __cplusplus
#include <stdbool.h>
#endif

#ifdef __cplusplus
extern "C" {
#endif

/* ABI history: 1 = initial; 2 = adds fc4_sim_on_post_city_init;
 * 3 = adds fc4_sim_get_debug and fc4_sim_get_agent_info (diagnostics);
 * 4 = adds the limit-timer calibration (fc4_timer_source, spec §5.5);
 * 5 = adds host streams (fc4_ostream / fc4_istream / fc4_ref);
 * 6 = adds the budget (fc4_budget / fc4_department / fc4_line_item);
 * 7 = adds fc4_department_find_local_funding_percent (budget erratum);
 * 8 = adds the land value simulator (fc4_landvalue);
 * 9 = adds host-built property holders (fc4_props) and the weather (fc4_weather);
 * 10 = adds fc4_props_set_typed and the ordinances (fc4_ordinance_sim / fc4_ordinance);
 * 11 = adds the demand simulator and demand nodes (fc4_demand_sim / fc4_demand_node);
 * 12 = adds the game's grids as host objects (fc4_grid_api) and the power simulator (fc4_power);
 * 13 = adds the plumbing simulator (fc4_plumbing);
 * 14 = adds the flammability simulator (fc4_flammability);
 * 15 = adds the budget shadow's tax scan (fc4_budget_shadow_tax);
 * 16 = adds the demand shadow (fc4_demand_shadow);
 * 17 = adds the pollution simulator (fc4_pollution) and its shadow (fc4_pollution_shadow);
 * 18 = adds the protection simulators (fc4_protection);
 * 19 = adds the crime simulator and the crime shadow (fc4_crime / fc4_crime_shadow);
 * 20 = adds the aura simulator (fc4_aura) and its shadow (fc4_aura_shadow);
 * 21 = adds the flora simulator (fc4_flora, fc4_flora_registry) and its shadow (fc4_flora_shadow);
 * 22 = adds the landfill developer (fc4_landfill) and its shadow (fc4_landfill_shadow);
 * 23 = adds the power shadow (fc4_power_shadow);
 * 24 = adds the plumbing shadow (fc4_plumbing_shadow);
 * 25 = adds the land value and flammability shadows (fc4_landvalue_shadow / fc4_flammability_shadow);
 * 26 = adds the residential simulator (fc4_residential) and its shadow (fc4_residential_shadow);
 * 27 = adds fc4_protection_shadow_unadjusted_grid (protection.md §2, PR5);
 * 28 = adds the neighbours simulator (fc4_neighbors, fc4_neighbor_deal,
 *      fc4_neighbor_connection) and its shadow (fc4_neighbors_shadow);
 * 30 = the aura shadow follows the original step by step (fc4_aura_shadow_before_step / _step / _occupant);
 *      fc4_flora_host's obliteration segment callbacks become record streams (§10.3);
 * 33 = adds fc4_power_shadow_message and the power shadow's city start (options `flags`,
 *      FC4_POWER_SHADOW_CITY_START; power.md PW3, PW4, PW5, PW7); the plumbing shadow's
 *      fc4_plumbing_shadow_city_start and fc4_plumbing_original.pass_counter (plumbing.md
 *      PS3, PS6); the landfill host's insert_landfill_tile and fc4_landfill_state's
 *      game_tiles / game_tile_failures (landfill.md LG3, LG8);
 * 34 = adds the zone manager (fc4_zones, zones.md) and its shadow mode (fc4_zones_create_shadow);
 * 35 = adds the building development simulator and growth developers (fc4_development / fc4_growth)
 *      and the development shadow (taken as 29 on its lane; renumbered at merge);
 * 41 = adds the turbo clock (tooling: fc4_sim_set_turbo / fc4_sim_get_turbo / fc4_sim_take_turbo_auto_off);
 * 44 = adds the traffic simulator, its network model and its pathfinders (fc4_traffic, fc4_traffic_pf)
 *      (taken as 37 on its lane; renumbered at merge);
 * 45 = neighbours answers (neighbors.md §15.4): foreign deals are kept as the listed deal
 *      (fc4_neighbors_add_foreign_deal, fc4_neighbors_host's foreign_deal / foreign_deal_call /
 *      release_deal, NG4); the neighbours shadow input carries the original's deals_dirty /
 *      stats_dirty / busy bytes (NG7) (taken as 47 on its lane; renumbered at merge);
 * 46 = adds the history warehouse (fc4_history) and the 24-hour clock (fc4_clock24);
 * 47 = adds the commercial simulator and industrial stub (fc4_commercial / fc4_industrial),
 *      the civic building simulator (fc4_civic) and the park manager (fc4_park);
 * 48 = adds the tract developer (fc4_tract, tract-developer.md) and its shadow mode
 *      (fc4_tract_create_shadow, the fc4_tract_probe_* functions; taken as 40 on its lane,
 *      then 45 and 47; renumbered at merge);
 * 49 = development answers DG5/DG6/DG11 (building-development.md §4.9.1, §4.9.2, §4.7.1):
 *      fc4_development_host loses load_building_occupant and gains building_type,
 *      set_default_exemplar, occupant_property_f32s and the foundation / retaining wall
 *      callbacks; adds fc4_development_add_foundation / _load_foundation /
 *      _load_retaining_wall (BDS slots 23, 26, 29); fc4_building_profile.reserved1 becomes
 *      capacities_present (taken as 56 on its lane; renumbered at merge);
 * 50 = the budget host's optional property_is_array (data-formats §5.2, EX39: the budget item
 *      arrays read a scalar as empty); fc4_props_set_u32s/i32s/i64s/f32s/bools store an array
 *      even for one value (the core's vector readers read a scalar as empty, EX39) (taken as 57
 *      on its lane; renumbered at merge);
 * 51 = adds the zone developer / lot manager (fc4_zonedev, zones.md §1.2, §7-§11.3) and its shadow mode
 *      (fc4_zonedev_create_shadow) (taken as 54 on its lane; renumbered at merge);
 * 52 = adds the history warehouse and 24-hour clock glue (docs/specs/history.md, clock24.md):
 *      their shadow modes (history_shadow, clock24_shadow) and the save sweep mode for the
 *      two records (0x89EFA536, 0xA7E7F929) (taken as 55 on its lane; renumbered at merge);
 * 53 = adds fc4_commercial_set_machine (the commercial shadow; cmglue lane) (taken as 58 on its lane;
 *      renumbered at merge);
 * 54 = adds the lot developer (fc4_lotdev, lot-developer.md) (taken as 53 on its lane; renumbered at
 *      merge);
 * 55 = zone developer re-specs11 (zones.md §5.2.1-§5.2.2, §7.2.2, §8.4.1-§8.4.3, §8.5): fc4_zonedev_host
 *      grows 17 callbacks at its end (332 -> 400 bytes on 32-bit), fc4_zonedev_cfg_object and
 *      fc4_zonedev_can_create_plopped_lot (lot manager slot 21) (taken as 54 on its lane;
 *      renumbered at merge);
 * 56 = adds the traffic shadow (fc4_traffic_shadow: tooling, a same-process differential of
 *      the original traffic simulator over the traffic host table) (taken as 52 on its lane;
 *      renumbered at merge);
 * 57 = adds the disaster layer (fc4_disasters, disasters.md): the layer, its managers (fire, riot,
 *      toxic spill, earthquake, pipe burst, volcano, effect) and their instances (taken as 64 on its
 *      lane; renumbered at merge);
 * 58 = adds the terrain (fc4_terrain, terrain.md): the height field, the surface water's depths and
 *      queries, the setters, RedisplayTerrain, kind-0 brushes, navigability, the bodies of water,
 *      the volcano lava and the records;
 * 59 = adds fc4_variant_get_u32 (holder slot 8's converting getter, neighbors.md §15.4 NG44,
 *      data-formats §5.1); fc4_props_set_typed and the neighbour connection property callback carry
 *      the void array (raw bytes, mem_type 0 with is_array; NG37) (taken as 66 on the apply12
 *      lane; renumbered at merge);
 * 60 = lot developer re-answers12 (apply13 lane): fc4_lotdev_host 178 callbacks: update_lot_object_altitudes
 *      removed (the core moves the objects, LD17), add_network_connection_nodes takes the nodes
 *      (fc4_lotdev_conn_node), lot_occupants, lot_configuration_object and occupant_moved appended (taken as
 *      71 on its lane; renumbered at merge);
 * 61 = traffic re-answers10 (traffic-simulator §9.1 TG7): fc4_traffic_host grows create_line_item at
 *      its end (404 -> 408 bytes on 32-bit) (taken as 72 on its lane; renumbered at merge);
 * 62 = adds the automata layer (fc4_automata_*, automata.md short form): the referenced objects
 *      (controllers and automata), the script system, the controller manager, the four automata
 *      managers, and the riot and fire hooks (reserved 65 on its lane; renumbered at merge);
 * 63 = adds the occupant manager (fc4_occmgr, occupant-manager.md) (taken as 67 on its lane;
 *      renumbered at merge);
 * 64 = the session seed (foundations §6.3): fc4_aura_create, fc4_flammability_create,
 *      fc4_flora_create, fc4_protection_create and fc4_weather_create take one session seed S
 *      instead of the two clock values; fc4_pollution_create's seed and the budget, plumbing
 *      and growth hosts' seed callbacks (renamed session_seed) carry S, and the core derives
 *      every generator's seed from it (derive(S, K), fc4_seed_derive); adds fc4_seed_derive
 *      (seeding lane; taken as 73 on its lane, then 59-63; renumbered at merge);
 * 65 = adds the list sinks (fc4_u64s_sink / fc4_u32s_sink / fc4_lot_cfg_object), the city detail
 *      manager (fc4_city_detail), the airport/seaport developers (fc4_port_developer) and the prop
 *      developer (fc4_prop_developer) (taken as 61 on its lane; renumbered at merge);
 * 66 = adds the network manager and network lot manager (fc4_netmgr, fc4_netlot, network-manager.md)
 *      (taken as 68 on its lane; renumbered at merge);
 * 67 = adds the disaster layer's mirror tooling (fc4_disasters_set_mirror_random,
 *      fc4_disasters_get_location, fc4_disasters_riot_locked, fc4_disasters_fire_ids): the plugin's
 *      disasters_shadow (a same-process differential of the original layer) (taken as 61, then 62
 *      to 66, on its lane; renumbered at merge);
 * 68 = adds the prop manager, lot configuration manager and tutorial system (fc4_prop_manager,
 *      fc4_lot_config, fc4_tutorial; prop-lotconfig.md, tutorial.md) (taken as 70 on its lane;
 *      renumbered at merge).
 * 69 = merge-train batch train2 (re-answers28, mergetrain-docs, re-answers29, cores12c, detpacing, cores18, apply14dis, glue12b2, coverage2, apply15c, apply15b, cores17): cores12c: adds the advisor system and the MySims (fc4_advisors, fc4_adv_obj, fc4_advisor_host, fc4_mysim_host; advisors.md, mysims.md) (taken as 62 on its lane; renumbered at merge). | detpacing: deterministic pacing (simulator.md §5.7 [choice]): fc4_pacing_set_process / fc4_pacing_process, fc4_sim_set_pacing / fc4_sim_get_pacing with fc4_pacing_hooks (the day's settle point), and every core with an idle-driven cycle runs it to completion in one settle delivery under FC4_PACING_DETERMINISTIC (taken as 65 on its lane, then 68; renumbered at merge). | cores18: adds the network rules (fc4_network_rules, fc4_network_rules_grid, network-rules.md): the rule database from resource bytes, tile rule evaluation and the rule stages of SolveNetwork over a cell grid (taken as 68 on its lane; renumbered at merge). | apply14dis: the disaster layer after re-answers14 (disasters.md): the host's dispatch-unit counts (fire_dispatch_count / police_dispatch_count) replace the effectiveness setters, the wind is a vector, network_default_cost and volcano_end_deformation are gone; fc4_disasters_set_cause returns the cause; adds fc4_disasters_manager_instances (taken as 68 on its lane). | glue12b2: adds fc4_port_developer_required_budget and fc4_port_developer_funding_ratio (the port glue's shadow; glue12b2 lane, taken as 66 on the lane, renumbered at merge). | cores17: adds the network geometry (fc4_netgeom, fc4_vhcs, network-geometry.md §3-§9): the drag, the validation passes, the cell infos, the smoothing loop, CanTilesBeSupportedOnTerrain, the grading (host terrain or a bound fc4_terrain), the height satisfier and the §10-§13 helpers (cores17 lane; taken as 63, then 64-68 on its lane; renumbered at merge).
 * 70 = merge-train batch train3 (re-integrate1, glue17, apply15d, glue14c, devperf, cores19, flakyfix, glue14b, cores12a, glue15, apply15terrain, abi-hygiene, glue12b1): merged (no C ABI history entry of its own)
 * 71 = merge-train batch train4 (re-specs20, re-answers30, re-specs19, cores22, apply15e, glue20, cores21): merged (no C ABI history entry of its own)
 * 72 = merge-train batch train5 (re-answers31, re-specs21, cores24, coverage3, glue23): merged (no C ABI history entry of its own);
 * 73 = merge-train batch train6 (glue17, apply15b, cores17, cores19, glue15, apply15terrain, glue12b1, glue12b2): adds the network geometry (fc4_netgeom, fc4_vhcs, network-geometry.md §3-§9): the drag, the validation passes, the cell infos, the smoothing loop, CanTilesBeSupportedOnTerrain, the grading (host terrain or a bound fc4_terrain), the height satisfier and the §10-§13 helpers (cores17 lane; taken as 63, then 64-68 on its lane; renumbered at merge). | adds the effects manager (fc4_effects, fc4_effects_directory; effects.md): the effect directory parser, creation, LOD and select groups, the tick, finishing (the hook or the draw-pass compat mode), the brush and game effects through host callbacks (renumbered at merge). | The lot configuration manager's opaque handle is fc4_lot_config_manager (the glue15 lane renamed it; the entry points keep their fc4_lot_config_* names, so the ABI surface is unchanged). | terrain round 2 (re-answers17, apply15terrain lane): fc4_terrain_lerped_water_level, fc4_terrain_flooding_altitude, fc4_terrain_water_velocity, fc4_terrain_sediment_at, fc4_terrain_brush_angles, fc4_terrain_set_brush_angles, fc4_terrain_talus_angle and fc4_terrain_set_talus_angle; fc4_terrain_brush_angle returns the stroke angle and fc4_terrain_add_brush's angle is the walk angle's start; set_water_display_state posts only on a change (taken as 64, then 65, 66 and 68 on its lane; renumbered at merge). | adds fc4_port_developer_required_budget and fc4_port_developer_funding_ratio (the port glue's shadow; glue12b2 lane, taken as 66 on the lane, renumbered at merge).
 * 74 = merge-train batch train8 (re-answers32, glue27, cores23, cores20): merged (no C ABI history entry of its own)
 * 75 = merge-train batch train10 (cores27): cores27: adds the network tools (fc4_nettool, network-tools.md §2-§11): the tool state, PlaceNetworkOccupants (slot 7), PlaceNetworkOccupantById (slot 8), the PlaceNetwork pipeline hand-off with the commit order, DestroyOccupantsAndResolve (slot 50) and the underground refit, over an fc4_nettool_host table (cores27 lane; taken as 75 on its lane; renumbered at merge; FC4_ABI_VERSION left at 74 for the train)
 * 76 = merge-train batch train31 (re-span4444-4, glue-formats4): merged (no C ABI history entry of its own)
 * 77 = merge-train batch train40 (re-legacycontrol4, re-view3dwin4, core-budgetwindow4, glue-tutorial4, glue-citydetail4, glue-exemplars4): merged (no C ABI history entry of its own) */
#define FC4_ABI_VERSION 77u
/* Return codes of fc4_sim_load. */
#define FC4_OK                    0
#define FC4_ERR_UNSUPPORTED_VERSION (-1)
#define FC4_ERR_TRUNCATED         (-2)
#define FC4_ERR_INVALID_ARGUMENT  (-3)
#define FC4_ERR_PANIC             (-100)

/* Agent levels (spec §2.2). */
#define FC4_LEVEL_ONE_SHOT      0u
#define FC4_LEVEL_EMERGENCY     1u
#define FC4_LEVEL_MINUTE        2u
#define FC4_LEVEL_HOUR          3u
#define FC4_LEVEL_DAY           4u
#define FC4_LEVEL_WEEK          5u
#define FC4_LEVEL_MONTH         6u
#define FC4_LEVEL_YEAR          7u
#define FC4_LEVEL_SPECIFIC_DAY  8u
#define FC4_LEVEL_SPECIFIC_TIME 9u
#define FC4_LEVEL_IDLE          10u
#define FC4_LEVEL_COUNT         11u

/* Opaque simulator instance. */
typedef struct fc4_sim fc4_sim;

/* Opaque agent identity chosen by the host (e.g. a cIGZMessageTarget2* value). */
typedef uint64_t fc4_agent;

/* A scheduler or broadcast message (the cIGZMessage2Standard fields used). */
typedef struct fc4_message {
    uint32_t type;
    uint32_t reserved; /* always 0; explicit padding */
    int64_t data1;
    int64_t data2;
    int64_t data3;
} fc4_message;

/* Host callbacks. `now_ms` is required; any other callback may be NULL
 * (a NULL deliver counts as "finished"). `user` is passed back verbatim. */
typedef struct fc4_host {
    uint32_t struct_size; /* = sizeof(fc4_host) */
    void* user;
    /* Monotonic milliseconds; only differences are used (wrapping). */
    uint32_t (*now_ms)(void* user);
    /* Deliver synchronously; return the agent's "finished" flag (spec §4). */
    bool (*deliver)(void* user, fc4_sim* sim, fc4_agent agent, const fc4_message* msg);
    /* Exactly one add_ref per registration added, one release per removal. */
    void (*add_ref)(void* user, fc4_agent agent);
    void (*release)(void* user, fc4_agent agent);
    /* Queued broadcast (MessagePost). */
    void (*post)(void* user, const fc4_message* msg);
} fc4_host;

/* Tuning inputs (spec §7). A NULL pointer means "property missing".
 * ms_per_day is used only with exactly 3 values, dilation with exactly 5. */
typedef struct fc4_tuning {
    const uint32_t* ms_per_day;
    size_t ms_per_day_count;
    const float* animation_dilation;
    size_t animation_dilation_count;
} fc4_tuning;

/* Calendar fields of one day. */
typedef struct fc4_date {
    int32_t julian;
    int32_t year;
    int32_t month;        /* 1..12 */
    int32_t day_of_month; /* 1..31 */
    int32_t day_of_year;  /* 1..366 */
    int32_t weekday;      /* 1 = Monday .. 7 = Sunday */
} fc4_date;

/* Full observable state (diagnostics / tests). */
typedef struct fc4_state {
    uint32_t struct_size; /* set by the caller to sizeof(fc4_state) */
    uint8_t initialized;
    uint8_t in_tick;
    uint8_t city_established;
    uint8_t speed_change_enabled;
    int32_t pause_count;
    int32_t hidden_pause_count;
    int32_t emergency_pause_count;
    int32_t sim_speed;
    int32_t sim_time;
    uint32_t frame_budget_ms;
    uint32_t gate_level;
    uint32_t ms_per_day;
    int32_t target_fps;
    int32_t julian_day;
    uint32_t ms_per_day_table[3];
    float animation_dilation[5];
    uint32_t level_counters[FC4_LEVEL_COUNT];
    uint8_t level_fired[FC4_LEVEL_COUNT];
    uint8_t reserved;
    uint32_t agent_counts[FC4_LEVEL_COUNT];
} fc4_state;

/* Scheduler/barrier state for diagnostics (spec §5.2 step 8). */
typedef struct fc4_debug {
    uint32_t struct_size; /* set by the caller to sizeof(fc4_debug) */
    uint32_t interval_elapsed_ms;
    uint32_t slice_elapsed_ms;
    uint32_t ms_per_day;
    uint8_t ready;                         /* interval timer >= msPerDay */
    uint8_t one_shot_or_emergency_pending; /* level 0 or 1 has agents: time suspended */
    uint8_t hours_complete;                /* levelCounter[Hour] >= 24 */
    uint8_t day_level_done;                /* cursor[Day] at end */
    uint8_t tomorrow_starts_week;
    uint8_t tomorrow_starts_month;
    uint8_t tomorrow_starts_year;
    uint8_t day_gate_open; /* every condition for the next day holds now */
    uint8_t blocked_by_week;
    uint8_t blocked_by_month;
    uint8_t blocked_by_year;
    uint8_t reserved;
    int32_t cursors[FC4_LEVEL_COUNT]; /* index in registration order, -1 = at end */
    int32_t idle_cursor;
} fc4_debug;

/* One agent registration (diagnostics). */
typedef struct fc4_agent_info {
    fc4_agent agent;
    uint32_t flags;
    uint8_t enabled;
    uint8_t reserved[3];
} fc4_agent_info;

/* Sources and sink for the CPU-cycle limit-timer calibration (spec §5.5).
 * `qpc` and `tsc` are required; `publish` may be NULL. */
typedef struct fc4_timer_source {
    uint32_t struct_size; /* = sizeof(fc4_timer_source) */
    void* user;
    uint64_t (*qpc)(void* user); /* high-resolution wall clock, ticks */
    uint64_t (*tsc)(void* user); /* processor time-stamp counter, cycles */
    /* CyclesToMillisecs (ms per cycle) and MillisecsToCycles (cycles per ms). */
    void (*publish)(void* user, float cycles_to_ms, float ms_to_cycles);
    uint64_t qpc_frequency; /* wall-clock ticks per second, > 0 */
} fc4_timer_source;

typedef void (*fc4_name_callback)(void* ctx, const char* name, size_t name_len);

/* ---- library ---------------------------------------------------------- */
uint32_t fc4_abi_version(void);

/* The session seed (foundations §6.3 [choice]). The host picks one 32-bit
 * session seed S per process (the plugin: the cRZTime wall clock at game
 * start, like the original, or `session_seed = N` in FreeCity4.ini) and passes
 * it to every create function and host callback that seeds a generator; the
 * core seeds each generator with derive(S, K) = S XOR K, 0 and 0xFFFFFFFF
 * moved by XOR 1, K by the generator's kind (a file-local generator: the
 * original's constant; a member seeded from the clock: the class's GZCLSID;
 * a temporary seeded from the clock: GZCLSID XOR its creation count). The C
 * runtime rand() of every core stays unseeded (state 1, §12.6). This returns
 * derive(S, K) (tooling: logs and tests). */
uint32_t fc4_seed_derive(uint32_t session_seed, uint32_t k);

/* Tooling: where a Rust panic is reported (message, location, thread and a
 * backtrace, as one UTF-8 block with lines separated by '\n') before the panic
 * goes on; release builds then abort with a fail-fast. Installs the panic
 * hook on the first call; a NULL sink stops the reports. Any thread. */
typedef void (*fc4_log_sink)(void* ctx, const uint8_t* text, size_t len);
void fc4_set_panic_log(fc4_log_sink sink, void* ctx);
/* Tooling: panics on purpose (tests of the report). Release builds abort the
 * process; builds that unwind return. */
void fc4_debug_panic(void);

/* ---- lifetime --------------------------------------------------------- */
/* Returns NULL if `host` is NULL, too small, or has no now_ms. */
fc4_sim* fc4_sim_create(const fc4_host* host);
/* Releases every remaining agent registration, then frees the simulator. */
void fc4_sim_destroy(fc4_sim* sim);

/* ---- lifecycle (spec §9; GZCOM registrations are the caller's job) ---- */
bool fc4_sim_init(fc4_sim* sim, const fc4_tuning* tuning /* nullable */);
bool fc4_sim_shutdown(fc4_sim* sim);
bool fc4_sim_is_initialized(const fc4_sim* sim);

/* ---- per-frame (spec §5) ----------------------------------------------- */
bool fc4_sim_on_tick(fc4_sim* sim);
void fc4_sim_simulate_time_passage(fc4_sim* sim);
void fc4_sim_simulate_agents(fc4_sim* sim);
void fc4_sim_simulate_idle_time(fc4_sim* sim);
void fc4_sim_begin_new_interval(fc4_sim* sim, uint32_t level);

/* ---- pause and speed (spec §6) ----------------------------------------- */
bool fc4_sim_pause(fc4_sim* sim);
bool fc4_sim_resume(fc4_sim* sim);
bool fc4_sim_hidden_pause(fc4_sim* sim);
bool fc4_sim_hidden_resume(fc4_sim* sim);
/* Notification 0x26D31EC1 (PostCityInit, spec §9): releases the hidden pause set
 * by Init. The caller unsubscribes from the notification itself. */
bool fc4_sim_on_post_city_init(fc4_sim* sim);
bool fc4_sim_emergency_pause(fc4_sim* sim);
bool fc4_sim_emergency_resume(fc4_sim* sim);
bool fc4_sim_is_paused(const fc4_sim* sim);
bool fc4_sim_is_hidden_paused(const fc4_sim* sim);
bool fc4_sim_is_emergency_paused(const fc4_sim* sim);
bool fc4_sim_is_any_paused(const fc4_sim* sim);
int32_t fc4_sim_get_sim_speed(const fc4_sim* sim);
bool fc4_sim_set_sim_speed(fc4_sim* sim, int32_t speed);
int32_t fc4_sim_get_sim_time(const fc4_sim* sim);
bool fc4_sim_set_sim_time(fc4_sim* sim, int32_t time);
bool fc4_sim_set_max_milliseconds_per_tick(fc4_sim* sim, uint32_t ms);
float fc4_sim_get_animation_time_dilation(const fc4_sim* sim);
bool fc4_sim_set_city_established(fc4_sim* sim, bool established);
bool fc4_sim_is_city_established(const fc4_sim* sim);

/* ---- agents (spec §4) --------------------------------------------------- */
bool fc4_sim_add_agent(fc4_sim* sim, fc4_agent agent, uint32_t level,
                       const char* name, size_t name_len, uint32_t flags);
bool fc4_sim_remove_agent(fc4_sim* sim, fc4_agent agent, uint32_t level);
bool fc4_sim_remove_agent_all_levels(fc4_sim* sim, fc4_agent agent);
bool fc4_sim_remove_all_agents(fc4_sim* sim);
bool fc4_sim_remove_all_agents_at(fc4_sim* sim, uint32_t level);
bool fc4_sim_get_agent_enabled(const fc4_sim* sim, const char* name, size_t name_len);
bool fc4_sim_set_agent_enabled(fc4_sim* sim, const char* name, size_t name_len, bool enabled);
/* Calls `cb` once per registration, level by level; returns the count. */
size_t fc4_sim_enumerate_agents(const fc4_sim* sim, fc4_name_callback cb, void* ctx);

/* ---- date ----------------------------------------------------------------- */
int32_t fc4_sim_get_julian_day(const fc4_sim* sim);
bool fc4_sim_get_date(const fc4_sim* sim, fc4_date* out);
int32_t fc4_julian_from_civil(int32_t year, int32_t month, int32_t day);
void fc4_date_from_julian(int32_t julian, fc4_date* out);

/* ---- persistence (spec §8) ------------------------------------------------ */
/* Writes the record if `capacity` suffices; always returns the record size. */
size_t fc4_sim_save(const fc4_sim* sim, uint8_t* buffer, size_t capacity);
/* Returns FC4_OK or an FC4_ERR_* code. On error the state is unchanged. */
int32_t fc4_sim_load(fc4_sim* sim, const uint8_t* buffer, size_t length);
/* Byte widths (1, 2 or 4) of each field of a record of `version`, in order,
 * for hosts with typed streams. Returns the field count, 0 if unsupported. */
size_t fc4_record_layout(uint16_t version, const uint8_t** widths);

/* ---- cheats (spec §9) ------------------------------------------------------ */
/* `text` is the full cheat line ("FPS 30"). Returns true if the id is handled. */
bool fc4_sim_handle_cheat(fc4_sim* sim, uint32_t cheat_id, const char* text, size_t text_len);

/* ---- limit-timer calibration (spec §5.5) ------------------------------------ */
/* Installs the calibration source/sink (copied). `initial_ms_per_cycle` is the
 * host's estimate of milliseconds per TSC cycle. Takes the first sample point
 * and publishes both coefficients. From then on fc4_sim_on_tick calibrates and
 * publishes before doing anything else. Returns false (nothing installed) for
 * an invalid source, a zero frequency or an estimate that is not positive. */
bool fc4_sim_set_timer_source(fc4_sim* sim, const fc4_timer_source* source, float initial_ms_per_cycle);
/* One calibration step without the rest of OnTick (tests). */
void fc4_sim_calibrate_tick(fc4_sim* sim);
/* Current coefficients; false if no timer source is installed. Pointers may be NULL. */
bool fc4_sim_get_calibration(const fc4_sim* sim, float* cycles_to_ms, float* ms_to_cycles, uint64_t* samples);

/* ---- host streams (foundations §2) ------------------------------------------
 * The core writes and reads its records through these callbacks; the GZCOM shim
 * implements them on top of the game's cIGZOStream / cIGZIStream (and the
 * segment streams for references). Every callback follows the stream model of
 * foundations §2.1: false once the stream is in its (sticky) error state. */

/* An object reference (foundations §2.7). */
typedef struct fc4_ref {
    uint64_t token;    /* host identity: an interface pointer; 0 = null reference */
    uint32_t class_id; /* the object's GetGZCLSID (writes) / its class (reads) */
    uint32_t reserved;
    void* core_handle; /* reads: the core handle if the object is one of ours (e.g. fc4_department*) */
} fc4_ref;

typedef void (*fc4_bytes_sink)(void* ctx, const uint8_t* data, size_t len);

typedef struct fc4_ostream {
    uint32_t struct_size; /* = sizeof(fc4_ostream) */
    void* user;
    bool (*put_u8)(void* user, uint8_t v);
    bool (*put_u16)(void* user, uint16_t v);
    bool (*put_u32)(void* user, uint32_t v);
    bool (*put_u64)(void* user, uint64_t v);
    bool (*put_f32)(void* user, float v);
    bool (*put_f64)(void* user, double v);
    bool (*put_bytes)(void* user, const uint8_t* data, size_t len); /* SetVoid */
    bool (*put_str)(void* user, const uint8_t* data, size_t len);   /* SetGZStr */
    bool (*put_ref)(void* user, const fc4_ref* ref);                /* WriteGZSerializable */
    int32_t (*error)(void* user);
} fc4_ostream;

typedef struct fc4_istream {
    uint32_t struct_size; /* = sizeof(fc4_istream) */
    void* user;
    bool (*get_u8)(void* user, uint8_t* out);
    bool (*get_u16)(void* user, uint16_t* out);
    bool (*get_u32)(void* user, uint32_t* out);
    bool (*get_u64)(void* user, uint64_t* out);
    bool (*get_f32)(void* user, float* out);
    bool (*get_f64)(void* user, double* out);
    bool (*get_bytes)(void* user, uint8_t* out, size_t n);           /* GetVoid */
    bool (*get_str)(void* user, fc4_bytes_sink sink, void* sink_ctx); /* GetGZStr, delivered as UTF-8 */
    /* ReadGZSerializable, then QueryInterface to `iid` (0 = keep as read). A host
     * object comes back with ONE reference owned by the caller; a non-null object
     * lacking `iid` reads as a null reference (the stream stays good). */
    bool (*get_ref)(void* user, uint32_t iid, fc4_ref* out);
    bool (*skip)(void* user, size_t n);
    int32_t (*error)(void* user);
} fc4_istream;

/* ---- budget (docs/specs/budget.md) -------------------------------------------
 * Three core objects, each fronted by one GZCOM object that owns its handle:
 * the budget simulator (fc4_budget), departments (fc4_department) and line
 * items (fc4_line_item). Objects the core hands back - departments, line items,
 * property holders, occupants - cross the ABI as TOKENS: the host identity
 * (interface pointer) the object was created with or passed in as. Returned
 * tokens are borrowed unless stated otherwise.
 *
 * Reference counting: the core calls host->add_ref / host->release on tokens it
 * keeps (departments in the budget's map, line items in a department, locally
 * funded property holders). host->create_object creates a department or line
 * item through the class registry and returns its core handle; the core then
 * owns the one reference that object was created with.
 *
 * Every function tolerates a NULL handle and returns its fallback (0 / false). */

typedef struct fc4_budget fc4_budget;
typedef struct fc4_department fc4_department;
typedef struct fc4_line_item fc4_line_item;

/* A message the budget posts (MessagePost, spec §10.1). */
typedef struct fc4_budget_message {
    uint32_t type;
    uint32_t reserved;
    int64_t data1, data2, data3;
    uint64_t void1;   /* SetVoid1: a token, 0 = none */
    uint64_t unknown; /* SetIGZUnknown: a token, 0 = none */
} fc4_budget_message;

/* One lot for ComputeTax (spec §5.2). */
typedef struct fc4_lot_info {
    uint8_t has_building;
    uint8_t livable;
    uint8_t land_value; /* 0..255 at the lot's origin cell */
    uint8_t reserved;
    uint32_t purpose;         /* building profile PurposeType */
    uint32_t building_wealth; /* building profile wealth */
    uint32_t lot_wealth;      /* cISC4Lot::GetOccupantWealth */
    int64_t building_value;   /* building property 0xE91A0B5F, 0 if absent */
    uint32_t size_x, size_z;
} fc4_lot_info;

/* cISC4BudgetSimulator::BudgetItem, with a 32-bit reserved tail. */
typedef struct fc4_budget_item {
    uint32_t department_id;
    uint32_t line_item;
    uint32_t purpose;
    int32_t index;
    int64_t cost;
    float local_funding; /* fraction (1.0 = 100 %) */
    uint32_t reserved;
} fc4_budget_item;

typedef struct fc4_loan_info {
    uint32_t months_remaining;
    uint32_t years_remaining;
    int64_t total_loan_amount;
    int64_t remaining_payment;
    int64_t monthly_payment;
} fc4_loan_info;

typedef struct fc4_bond {
    int32_t due_month, due_year;
    int64_t amount, full_cost, remaining, paid, monthly_payment, principal_left;
} fc4_bond;

typedef struct fc4_budget_group {
    uint32_t id;
    uint32_t department_count;
    int64_t total_expense;
    int64_t total_income;
} fc4_budget_group;

/* Diagnostics view of the budget simulator (spec §3.1). */
typedef struct fc4_budget_state {
    uint32_t struct_size; /* caller sets = sizeof(fc4_budget_state) */
    uint8_t initialized, loaded_from_save, agriculture_rate_fix, reserved0;
    uint32_t panel_flag;
    uint32_t bond_count;
    int64_t funds, shadow_funds, total_borrowed, last_tax_income;
    int64_t ytd_income, ytd_expense, ytd_tax_income, ytd_ordinance_income, ytd_ordinance_expense, ytd_bond_payments;
    int64_t last_month_income, last_month_expense, yearly_income, yearly_expense;
    float tax_rate[12];
    float max_tax_rate[12];
    int64_t weighted_assessed[12];
    int64_t monthly_tax[12];
    uint32_t department_count, group_count;
    uint32_t risky_money_counter, reserved1;
} fc4_budget_state;

/* One lot as the tax scan saw it (fc4_budget_shadow_tax). */
#define FC4_TAX_LOT_COUNTED 0u
#define FC4_TAX_LOT_NO_BUILDING 1u
#define FC4_TAX_LOT_NOT_LIVABLE 2u
#define FC4_TAX_LOT_UNTAXED 3u
typedef struct fc4_tax_lot {
    uint32_t status;  /* FC4_TAX_LOT_* */
    int32_t group;    /* 0..11, -1 = none; for a not-livable lot the group it would have had */
    int64_t assessed; /* §5.2 assessed value; for a not-livable lot what it would have added */
    float tax;        /* lotTax (0 unless counted) */
    uint32_t reserved;
} fc4_tax_lot;

/* The result of fc4_budget_shadow_tax (§5.2, §6 GetTaxIncome(t)). */
typedef struct fc4_tax_scan {
    uint32_t struct_size; /* caller sets = sizeof(fc4_tax_scan) */
    uint32_t lots;        /* lots visited */
    float multiplier;     /* DifficultyTaxMultiplier[difficulty] */
    float total;          /* the float32 running total, lot by lot in visit order (BQ3) */
    int64_t weighted_assessed[12];
    int64_t monthly_tax[12]; /* trunc(groupTax[t]) */
    int64_t tax_income[12];  /* GetTaxIncome(t) with the given rates */
    float group_tax[12];
    uint32_t counted[12];    /* lots counted per group */
    int64_t total_tax;       /* ComputeTax's result, trunc(total), never the sum of the lines */
} fc4_tax_scan;

typedef void (*fc4_tax_lot_callback)(void* ctx, uint32_t index, const fc4_lot_info* lot, const fc4_tax_lot* result);

typedef void (*fc4_lot_visitor)(void* ctx, const fc4_lot_info* lot);
typedef void (*fc4_token_callback)(void* ctx, uint64_t token);
typedef void (*fc4_budget_item_callback)(void* ctx, const fc4_budget_item* item);
typedef void (*fc4_loan_callback)(void* ctx, const fc4_loan_info* loan);
typedef void (*fc4_group_callback)(void* ctx, const fc4_budget_group* group, const uint8_t* name, size_t name_len,
                                   const uint32_t* department_ids);

/* What the budget needs from the game. Copied at creation. Any callback may be
 * NULL: it then reads as "nothing there" (0 / false / absent). Property and
 * query callbacks must not call back into the budget. */
typedef struct fc4_budget_host {
    uint32_t struct_size; /* = sizeof(fc4_budget_host) */
    void* user;
    void (*post)(void* user, const fc4_budget_message* msg);
    void (*show_budget_window)(void* user, uint32_t panel);
    uint32_t (*difficulty)(void* user);                       /* 0..2 */
    void (*sim_date)(void* user, int32_t* year, int32_t* month); /* month 1..12 */
    bool (*city_established)(void* user);
    void (*city_size)(void* user, float* x, float* z);
    int64_t (*ordinance_income)(void* user);
    int64_t (*ordinance_expense)(void* user);
    int64_t (*neighbor_income)(void* user);
    int64_t (*neighbor_expense)(void* user);
    /* Every lot, in the lot manager's master-list order (§5.2): the tax total
     * is a float32 running sum, so only that order matches it exactly (BQ3). */
    void (*for_each_lot)(void* user, fc4_lot_visitor visit, void* ctx);
    void (*record_history)(void* user, uint32_t series, int32_t value);
    void (*set_history_queue_length)(void* user, uint32_t series, uint32_t length);
    bool (*try_start_disaster)(void* user, uint32_t id, float x, float z); /* false if it does not exist */
    /* The session seed S (foundations §6.3): the RiskyMoney generator is seeded
     * with derive(S, 0x6C09C3D7) (ABI 64; before, this callback returned the
     * generator's seed itself). The generator is process-wide (RN3): the first
     * fc4_budget_create of the process seeds it, later budgets (later city
     * loads) continue its stream and their seed is not read again. */
    uint32_t (*session_seed)(void* user);
    bool (*ltext)(void* user, uint32_t group, uint32_t instance, fc4_bytes_sink sink, void* ctx);
    bool (*building_type_name)(void* user, uint32_t building_type, fc4_bytes_sink sink, void* ctx);
    /* Array properties of a property holder: return the number of values the
     * property has (copying at most `capacity` into `out`), or -1 if absent. */
    int32_t (*get_u32s)(void* user, uint64_t holder, uint32_t prop, uint32_t* out, size_t capacity);
    int32_t (*get_i64s)(void* user, uint64_t holder, uint32_t prop, int64_t* out, size_t capacity);
    int32_t (*get_f32s)(void* user, uint64_t holder, uint32_t prop, float* out, size_t capacity);
    bool (*set_u32s)(void* user, uint64_t holder, uint32_t prop, const uint32_t* values, size_t count);
    bool (*set_i64s)(void* user, uint64_t holder, uint32_t prop, const int64_t* values, size_t count);
    bool (*set_f32s)(void* user, uint64_t holder, uint32_t prop, const float* values, size_t count);
    bool (*copy_property)(void* user, uint64_t src, uint64_t dst, uint32_t prop);
    uint64_t (*occupant_of)(void* user, uint64_t holder); /* 0 = not an occupant */
    uint32_t (*class_of)(void* user, uint64_t token);
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    /* Creates a department (0xE990BFFC) or line item (0xAA313C9F) through the
     * class registry; returns its core handle (fc4_department* / fc4_line_item*). */
    void* (*create_object)(void* user, uint32_t class_id);
    /* ABI 50 (data-formats §5.2, EX39): 1 if the property is an array, 0 if it is a
     * scalar, -1 if absent. The budget item arrays are read with the original's vector
     * helpers, which give a scalar (count 0) an EMPTY vector, while get_u32s / get_i64s /
     * get_f32s convert a scalar to one value (the converting getters, for tuning). NULL:
     * every property counts as an array (the old one-value reading). */
    int32_t (*property_is_array)(void* user, uint64_t holder, uint32_t prop);
} fc4_budget_host;

/* Lifetime: called by the GZCOM object that fronts the core object; `token` is
 * that object's identity. Destroy drops the GZCOM object's share. */
fc4_budget* fc4_budget_create(const fc4_budget_host* host, uint64_t token);
void fc4_budget_destroy(fc4_budget* b);
fc4_department* fc4_department_create(const fc4_budget_host* host, uint64_t token);
void fc4_department_destroy(fc4_department* d);
fc4_line_item* fc4_line_item_create(const fc4_budget_host* host, uint64_t token);
void fc4_line_item_destroy(fc4_line_item* l);

/* Persistence (spec §11). References read back as our own objects arrive as
 * core handles through fc4_ref.core_handle. */
bool fc4_budget_write(const fc4_budget* b, const fc4_ostream* out);
bool fc4_budget_read(fc4_budget* b, const fc4_istream* in);
bool fc4_department_write(const fc4_department* d, const fc4_ostream* out);
bool fc4_department_read(fc4_department* d, const fc4_istream* in);
bool fc4_line_item_write(const fc4_line_item* l, const fc4_ostream* out);
bool fc4_line_item_read(fc4_line_item* l, const fc4_istream* in);

/* Budget simulator: lifecycle (§4). `tuning_holder` is the tuning exemplar as a
 * property-holder token (read through get_i64s / get_f32s), 0 = code defaults.
 * `fix_cs3_weight` enables the intended behaviour of §13.1 (off = compat). */
bool fc4_budget_init(fc4_budget* b, uint64_t tuning_holder, bool fix_cs3_weight);
bool fc4_budget_shutdown(fc4_budget* b);
bool fc4_budget_is_initialized(fc4_budget* b);
bool fc4_budget_handle_message(fc4_budget* b, uint32_t type, int64_t data1); /* DoMessage minus cheats */
bool fc4_budget_handle_cheat(fc4_budget* b, uint32_t cheat_id, const char* text, size_t text_len);
void fc4_budget_simulate_day(fc4_budget* b);   /* §5.7 */
void fc4_budget_settle_month(fc4_budget* b);   /* §5.1 */
int64_t fc4_budget_compute_tax(fc4_budget* b); /* §5.2 */

/* Budget shadow (diagnostics): ComputeTax (§5.2) without a budget object, the
 * same scan the budget simulator runs, over host->for_each_lot, with the tuning
 * read from `tuning_holder` (0 = code defaults) through get_i64s / get_f32s,
 * the given difficulty and 12 group rates (percent, `rates[0..11]`). Calls no
 * other host callback: nothing is stored, posted or written. `cb` (may be
 * NULL) sees every lot in visit order. False (and `out` untouched) on bad
 * arguments. */
bool fc4_budget_shadow_tax(const fc4_budget_host* host, uint64_t tuning_holder, bool fix_cs3_weight,
                           uint32_t difficulty, const float* rates, fc4_tax_lot_callback cb, void* ctx,
                           fc4_tax_scan* out);

/* Budget simulator: methods of cISC4BudgetSimulator (§6), in SDK order. */
bool fc4_budget_set_total_funds(fc4_budget* b, int64_t v);
int64_t fc4_budget_get_total_funds(fc4_budget* b);
bool fc4_budget_deposit_funds(fc4_budget* b, int64_t v);
bool fc4_budget_withdraw_funds(fc4_budget* b, int64_t v);
int64_t fc4_budget_get_min_allowable_funds(fc4_budget* b);
int64_t fc4_budget_get_ytd_income(fc4_budget* b);
int64_t fc4_budget_get_est_income(fc4_budget* b);
int64_t fc4_budget_get_ytd_expenses(fc4_budget* b);
int64_t fc4_budget_get_est_expenses(fc4_budget* b);
int64_t fc4_budget_get_total_monthly_expense(fc4_budget* b); /* 64-bit; see spec §6 [compat] */
int64_t fc4_budget_get_total_yearly_expense(fc4_budget* b);
int64_t fc4_budget_get_total_monthly_income(fc4_budget* b);
int64_t fc4_budget_get_total_yearly_income(fc4_budget* b);
int64_t fc4_budget_get_tax_income(fc4_budget* b);
int64_t fc4_budget_get_tax_income_for(fc4_budget* b, int32_t tax_group);
int64_t fc4_budget_set_total_monthly_expense(fc4_budget* b, int64_t v);
int64_t fc4_budget_set_total_yearly_expense(fc4_budget* b, int64_t v);
int64_t fc4_budget_set_total_monthly_income(fc4_budget* b, int64_t v);
int64_t fc4_budget_set_total_yearly_income(fc4_budget* b, int64_t v);
bool fc4_budget_show_budget_window(fc4_budget* b);
uint64_t fc4_budget_create_department(fc4_budget* b, uint32_t id, uint32_t group); /* token, 0 on failure */
bool fc4_budget_remove_department(fc4_budget* b, uint32_t id);
uint64_t fc4_budget_get_department(const fc4_budget* b, uint32_t id);
uint64_t fc4_budget_get_department_by_name(const fc4_budget* b, const uint8_t* name, size_t name_len);
size_t fc4_budget_get_all_groups(const fc4_budget* b, fc4_group_callback cb, void* ctx);
bool fc4_budget_set_group_name(fc4_budget* b, uint32_t id, const uint8_t* name, size_t name_len);
/* Department tokens in ascending ID; the caller takes its own references. */
size_t fc4_budget_get_departments_in_group(const fc4_budget* b, uint32_t id, fc4_token_callback cb, void* ctx);
void fc4_budget_needed_funding_changed(fc4_budget* b, const fc4_department* d);
void fc4_budget_funding_percentage_changed(fc4_budget* b, const fc4_department* d);
int64_t fc4_budget_get_funding(fc4_budget* b); /* always 0 [compat] */
float fc4_budget_get_tax_rate(fc4_budget* b, uint32_t tax_group);
bool fc4_budget_set_tax_rate(fc4_budget* b, uint32_t tax_group, float rate);
int64_t fc4_budget_get_weighted_assessed_tax_value(fc4_budget* b, uint32_t tax_group);
int64_t fc4_budget_get_bond_increment(fc4_budget* b);
bool fc4_budget_issue_bond(fc4_budget* b, uint32_t amount);
int64_t fc4_budget_get_total_borrowed(fc4_budget* b);
int64_t fc4_budget_get_current_borrowing_limit(fc4_budget* b);
int64_t fc4_budget_get_current_bond_limit(fc4_budget* b);
int64_t fc4_budget_get_current_max_outstanding_bonds_limit(fc4_budget* b);
int64_t fc4_budget_get_total_monthly_bond_payments(fc4_budget* b);
size_t fc4_budget_get_all_loans(const fc4_budget* b, fc4_loan_callback cb, void* ctx);
int32_t fc4_budget_get_loan_time_in_months(fc4_budget* b);
int64_t fc4_budget_get_monthly_payment_for_loan(fc4_budget* b, int64_t loan);
int64_t fc4_budget_get_full_cost_of_loan(fc4_budget* b, int64_t loan);
bool fc4_budget_get_budget_item_info(const fc4_budget* b, uint64_t holder, fc4_budget_item_callback cb, void* ctx);
bool fc4_budget_get_budget_item_for_purpose(const fc4_budget* b, uint64_t holder, uint32_t purpose,
                                            fc4_budget_item* out);
bool fc4_budget_change_budget_item_line(fc4_budget* b, uint64_t holder, uint32_t index, uint32_t line);
bool fc4_budget_change_budget_item_cost(fc4_budget* b, uint64_t holder, uint32_t index, int64_t cost);
bool fc4_budget_change_budget_item_local_funding(fc4_budget* b, uint64_t holder, uint32_t index, float fraction);
bool fc4_budget_add_budget_item_to_budget(fc4_budget* b, uint64_t holder, uint32_t index);
bool fc4_budget_remove_budget_item_from_budget(fc4_budget* b, uint64_t holder, uint32_t index);
uint64_t fc4_budget_get_line_item_from_budget_item(const fc4_budget* b, const fc4_budget_item* item);
bool fc4_budget_copy_budget_item_properties(fc4_budget* b, uint64_t src, uint64_t dst);

/* Budget simulator: inspection. */
bool fc4_budget_get_state(const fc4_budget* b, fc4_budget_state* out);
bool fc4_budget_get_bond(const fc4_budget* b, size_t index, fc4_bond* out); /* issue order */

/* Departments: cISC4DepartmentBudget (§7), in SDK order. */
uint32_t fc4_department_get_id(fc4_department* d);
bool fc4_department_get_name(const fc4_department* d, fc4_bytes_sink sink, void* ctx);
bool fc4_department_set_name(fc4_department* d, uint32_t ltext_group, uint32_t ltext_instance);
uint32_t fc4_department_get_budget_group(fc4_department* d);
bool fc4_department_set_budget_group(fc4_department* d, uint32_t group);
bool fc4_department_is_fixed_funding(fc4_department* d);
bool fc4_department_set_fixed_funding(fc4_department* d, bool fixed);
float fc4_department_get_funding_percentage(fc4_department* d);
bool fc4_department_set_funding_percentage(fc4_department* d, float q, uint32_t arg);
float fc4_department_get_max_funding_percentage(fc4_department* d);
bool fc4_department_set_max_funding_percentage(fc4_department* d, float q);
int64_t fc4_department_get_ideal_monthly_funding(fc4_department* d);
int64_t fc4_department_get_total_expenses(fc4_department* d);
int64_t fc4_department_get_total_income(fc4_department* d);
uint64_t fc4_department_create_line_item(fc4_department* d, uint32_t id, bool is_local); /* token */
uint64_t fc4_department_create_line_item_for_building_type(fc4_department* d, uint32_t building_type, bool is_local);
bool fc4_department_remove_line_item(fc4_department* d, uint32_t id);
uint64_t fc4_department_get_line_item(const fc4_department* d, uint32_t id);
size_t fc4_department_get_all_line_items(const fc4_department* d, fc4_token_callback cb, void* ctx);
bool fc4_department_add_locally_funded_object(fc4_department* d, uint64_t holder, uint32_t purpose);
bool fc4_department_remove_locally_funded_object(fc4_department* d, uint64_t holder, uint32_t purpose);
bool fc4_department_set_local_funding_percent(fc4_department* d, uint64_t holder, float q, uint32_t purpose);
bool fc4_department_set_local_full_funding(fc4_department* d, uint64_t holder, int64_t cost, uint32_t purpose);
float fc4_department_get_local_funding_percent(fc4_department* d, uint64_t holder, uint32_t purpose);
/* True and *out = the item's local funding fraction if the object is registered and its budget item exists;
 * false otherwise (*out untouched). */
bool fc4_department_find_local_funding_percent(fc4_department* d, uint64_t holder, uint32_t purpose, float* out);
int64_t fc4_department_get_local_full_funding(fc4_department* d, uint64_t holder, uint32_t purpose);
bool fc4_department_is_locally_funded_object(fc4_department* d, uint64_t holder, uint32_t purpose);
bool fc4_department_recalculate(fc4_department* d); /* RecalculateAllLineItemCurrentExpenses */
int64_t fc4_department_get_total_spending(fc4_department* d);
bool fc4_department_set_total_spending(fc4_department* d, int64_t v);
size_t fc4_department_get_locally_funded_items_by_purpose(const fc4_department* d, uint32_t purpose,
                                                          fc4_token_callback cb, void* ctx);
bool fc4_department_has_locally_funded_items_by_purpose(fc4_department* d, uint32_t purpose);

/* Line items: cISC4LineItem (§8), in SDK order. */
uint32_t fc4_line_item_get_id(fc4_line_item* l);
uint32_t fc4_line_item_get_type(fc4_line_item* l);
bool fc4_line_item_set_type(fc4_line_item* l, uint32_t type);
bool fc4_line_item_get_name(const fc4_line_item* l, fc4_bytes_sink sink, void* ctx);
bool fc4_line_item_set_name(fc4_line_item* l, uint32_t group, uint32_t instance);
int64_t fc4_line_item_get_secondary_info(fc4_line_item* l);
bool fc4_line_item_set_secondary_info(fc4_line_item* l, int64_t v);
int64_t fc4_line_item_get_full_expenses(fc4_line_item* l);
int64_t fc4_line_item_get_current_expenses(fc4_line_item* l);
bool fc4_line_item_set_full_expenses(fc4_line_item* l, int64_t v);
bool fc4_line_item_add_to_full_expenses(fc4_line_item* l, int64_t v);
int64_t fc4_line_item_get_income(fc4_line_item* l);
bool fc4_line_item_set_income(fc4_line_item* l, int64_t v);
bool fc4_line_item_add_to_income(fc4_line_item* l, int64_t v);
uint32_t fc4_line_item_get_display_flags(fc4_line_item* l);
bool fc4_line_item_set_display_flag(fc4_line_item* l, uint32_t flag, bool on);
bool fc4_line_item_is_local(fc4_line_item* l);
uint64_t fc4_line_item_get_owner(const fc4_line_item* l); /* department token or 0 */

/* ---- land value (docs/specs/landvalue.md, docs/specs/simgrid.md) --------------
 * One core object fronted by the GZCOM land value simulator. Its grids are the
 * GAME's own cSC4SimGrid objects (simgrid.md §6.3: other subsystems read their
 * memory directly), created by the host through GZCOM and held by the core as
 * TOKENS (interface pointers). The core owns one reference per grid token it
 * holds: the one create_grid returns, or the one a stream read hands over; it
 * gives it back with host->release. Grid values cross the ABI as flat arrays,
 * x outer, z inner (count_x * count_z values of the grid's geometry). */
typedef struct fc4_landvalue fc4_landvalue;

typedef struct fc4_grid_geometry {
    int32_t tract_size, shift, count_x, count_z;
    float width_x, width_z, inv_width_x, inv_width_z;
} fc4_grid_geometry;

#define FC4_GRID_U8  0u /* cSC4SimGrid<uint8_t>, CLSID 0x49B9E602 */
#define FC4_GRID_F32 1u /* cSC4SimGrid<float>,   CLSID 0x49B9E60A */

/* What land value needs from the game (landvalue.md §12). Missing callbacks
 * behave as "nothing there" (0, false, no grid). */
typedef struct fc4_landvalue_host {
    uint32_t struct_size; /* sizeof(fc4_landvalue_host) */
    void* user;
    void (*terrain_cells)(void* user, int32_t* x, int32_t* z); /* CellCountX/Z */
    bool (*is_land_vertex)(void* user, uint32_t x, uint32_t z);
    float (*altitude)(void* user, float x, float z);
    float (*min_allowed_height)(void* user);
    float (*max_allowed_height)(void* user);
    uint32_t (*zone_type)(void* user, int32_t x, int32_t z);
    /* Lot developer GetAreaSlope({x1, z1, x2, z2}, true), inclusive cells;
     * read-only in the original (landvalue.md §6, LV3). */
    float (*area_slope)(void* user, int32_t x1, int32_t z1, int32_t x2, int32_t z2);
    uint64_t (*developer)(void* user, uint32_t developer_type); /* growth developer or 0 */
    uint64_t (*desirability_map)(void* user, uint64_t developer); /* int16 grid (borrowed) or 0 */
    /* AddRef, Init, SetTractSize, SetInstanceID; returns the grid with one
     * reference for the core, or 0. */
    uint64_t (*create_grid)(void* user, uint32_t kind, int32_t tract_size, uint32_t instance_id);
    void (*grid_geometry)(void* user, uint64_t grid, fc4_grid_geometry* out);
    void (*grid_set_tract_size)(void* user, uint64_t grid, int32_t tract_size);
    void (*grid_read_u8)(void* user, uint64_t grid, uint8_t* out, size_t count);
    void (*grid_read_i16)(void* user, uint64_t grid, int16_t* out, size_t count);
    void (*grid_write_u8)(void* user, uint64_t grid, const uint8_t* values, size_t count);
    void (*grid_write_f32)(void* user, uint64_t grid, const float* values, size_t count);
    void (*grid_shutdown)(void* user, uint64_t grid);
    void (*release)(void* user, uint64_t grid);
    void (*set_history_queue_length)(void* user, uint32_t series, uint32_t length);
    void (*record_history)(void* user, uint32_t series, int32_t value);
    void (*post)(void* user, uint32_t msg_type); /* type-only MessagePost */
    /* Growth developer slot 10 GrowthAndDeclineListsAreValid (state < 17) of
     * a developer: true while its desirability map cannot change
     * (landvalue.md §7.4, LV2). Read by the land value shadow only; NULL =
     * unknown. */
    bool (*developer_lists_valid)(void* user, uint64_t developer);
} fc4_landvalue_host;

/* The tuning exemplar (§8). A NULL array means the property is absent (a
 * present empty array needs a non-NULL pointer). */
typedef struct fc4_landvalue_tuning {
    uint32_t struct_size; /* sizeof(fc4_landvalue_tuning) */
    uint32_t has_tract_size;
    int64_t tract_size;
    const int64_t* intrinsic; size_t intrinsic_count;
    const int64_t* wealth; size_t wealth_count;
    const int64_t* water; size_t water_count;
    const float* altitude_curve; size_t altitude_curve_count;
    const uint32_t* developer_ids; size_t developer_id_count;
    const float* developer_weights; size_t developer_weight_count;
    const float* desirability_curve; size_t desirability_curve_count;
} fc4_landvalue_tuning;

/* Diagnostics. */
typedef struct fc4_landvalue_state {
    uint32_t struct_size;
    uint8_t initialized, dirty, ignore_zoning, average_land_value;
    uint32_t pipeline_state; /* 0..8 running, 9 finished */
    uint32_t developer_count;
    int32_t sum, count;
    float min_alt, max_alt, mean_alt, mode_alt, pivot, low_range, high_range;
    int32_t top_index;
} fc4_landvalue_state;

/* The host table is copied. NULL if it is missing or too small. */
fc4_landvalue* fc4_landvalue_create(const fc4_landvalue_host* host);
/* Releases grid references still held (a loaded object never initialised). */
void fc4_landvalue_destroy(fc4_landvalue* lv);
/* Init / Shutdown, core part (§4.1 steps 2-7 and 10; §4.2 grids and curves).
 * tuning NULL = no exemplar (current values stay). */
bool fc4_landvalue_init(fc4_landvalue* lv, const fc4_landvalue_tuning* tuning);
bool fc4_landvalue_shutdown(fc4_landvalue* lv);
/* DoMessage (§5): always true. data3 is the SimIdle budget in ms. */
bool fc4_landvalue_handle_message(fc4_landvalue* lv, uint32_t msg_type, int64_t data3);
/* Step(budget) (§6): completes at most one pipeline state. */
void fc4_landvalue_step(fc4_landvalue* lv, uint32_t budget_ms);
/* Grid tokens, borrowed (no reference): 0 intrinsic, 1 total, 2 slope. */
uint64_t fc4_landvalue_grid(const fc4_landvalue* lv, uint32_t which);
/* GetLandValueType for a land value byte (the caller reads the total grid). */
uint32_t fc4_landvalue_type_of(const fc4_landvalue* lv, uint8_t value);
uint8_t fc4_landvalue_average(const fc4_landvalue* lv);
float fc4_landvalue_mode_altitude(const fc4_landvalue* lv);
bool fc4_landvalue_get_state(const fc4_landvalue* lv, fc4_landvalue_state* out);
/* Persistence (§10). Grid references go out as fc4_ref {token, class}; on read
 * they are requested with IID 0xC9BD939A and arrive with one reference each. */
bool fc4_landvalue_write(const fc4_landvalue* lv, const fc4_ostream* out);
bool fc4_landvalue_read(fc4_landvalue* lv, const fc4_istream* in);

/* ---- property holders ---------------------------------------------------------
 * A plain property holder the host fills with the properties of a game exemplar
 * (a subsystem's tuning) before handing it to the core, which copies it. A
 * single value is stored as a scalar, several (or none) as an array; the core
 * converts numbers between types when it reads them. */
typedef struct fc4_props fc4_props;

fc4_props* fc4_props_create(void);
void fc4_props_destroy(fc4_props* p);
/* Set (replace) property `id` to an ARRAY of `count` values (one value
 * included; ABI 50, data-formats §5.2 EX39: the core's vector readers read a
 * scalar as empty, so a scalar is set with fc4_props_set_typed). `values` may
 * be NULL when `count` is 0. */
bool fc4_props_set_u32s(fc4_props* p, uint32_t id, const uint32_t* values, size_t count);
bool fc4_props_set_i32s(fc4_props* p, uint32_t id, const int32_t* values, size_t count);
bool fc4_props_set_i64s(fc4_props* p, uint32_t id, const int64_t* values, size_t count);
bool fc4_props_set_f32s(fc4_props* p, uint32_t id, const float* values, size_t count);
bool fc4_props_set_bools(fc4_props* p, uint32_t id, const bool* values, size_t count);
/* A string property (bytes, no terminator). */
bool fc4_props_set_string(fc4_props* p, uint32_t id, const uint8_t* bytes, size_t len);
size_t fc4_props_count(const fc4_props* p);
/* Set property `id` exactly as a game variant holds it: mem_type = the
 * cIGZVariant type without the array bit (0x01 bool .. 0x0E RZChar); data =
 * `count` elements in that type's layout (bool: 1 byte). A scalar
 * (is_array false) needs count 1. ABI 59 (neighbors.md §15.4 NG37):
 * mem_type 0 with is_array = the void array (raw bytes, type 0x8000), `count`
 * bytes. False for other types or a bad count. */
bool fc4_props_set_typed(fc4_props* p, uint32_t id, uint16_t mem_type, bool is_array, const void* data,
                         size_t count);
/* WriteVariant of property `id` (foundations 9.2: u32 0, u32 type tag, the
 * value) as the game's segment streams write it, handed to `sink` in one call;
 * false when absent or not writable (ABI 56; tooling: the traffic shadow's
 * capture stream). */
bool fc4_props_write_variant(const fc4_props* p, uint32_t id, fc4_bytes_sink sink, void* ctx);
/* ABI 59 (neighbors.md §15.4 NG44, data-formats §5.1): GetProperty(id, uint32&)
 * (holder slot 8) on one value held as a game variant: var_type = the
 * cIGZVariant type WITH the array bit; data = `count` elements of an array, or
 * the scalar's one value (count 1), in that type's layout (bool: 1 byte). A
 * Uint32 scalar as is; other scalars by number (signed sign-extended, Char and
 * RZChar signed bytes, 64-bit low bits, floats truncated, NaN / inf / >= 2^63
 * give 0); arrays, strings, the void array (0x8000) and Empty (0) through the
 * text round trip (first element's text; an empty string gives true and 0).
 * True with *out set; false (out unchanged) for pointers, objects or a bad
 * layout. */
bool fc4_variant_get_u32(uint16_t var_type, const void* data, size_t count, uint32_t* out);

/* ---- weather (docs/specs/weather.md) ------------------------------------------
 * The weather simulator's core. The shim owns the GZCOM object (agents,
 * PostCityInit subscription, city/terrain references) and keeps the
 * SC4Vector-compatible mirror of the moisture grid (§11.2). */
typedef struct fc4_weather fc4_weather;

typedef struct fc4_weather_host {
    uint32_t struct_size; /* sizeof(fc4_weather_host) */
    void* user;
    /* City (§2, §4): SizeX/Z and CellWidthX/Z in world units, GetWorldHemisphere,
     * and the calendar month (1..12) of the simulation date. */
    void (*city_size)(void* user, float* x, float* z);
    void (*city_cell_width)(void* user, float* x, float* z);
    int32_t (*world_hemisphere)(void* user);
    int32_t (*sim_month)(void* user);
    /* cISC4Region::GetWaterPrefs; false = no region (defaults 60, 100). */
    bool (*water_prefs)(void* user, uint8_t* lo, uint8_t* hi);
    /* Terrain (vertex/cell indices unless named world). */
    bool (*terrain_available)(void* user);
    float (*altitude_at_vertex)(void* user, int32_t x, int32_t z);
    void (*vertex_normal)(void* user, int32_t x, int32_t z, float* xyz /* 3 floats */);
    bool (*cell_has_water)(void* user, int32_t x, int32_t z);
    float (*altitude)(void* user, float world_x, float world_z);
    float (*sea_level)(void* user);
    /* Posts a message with void1 = the weather simulator (IGZUnknown) and data2. */
    void (*post)(void* user, uint32_t msg_type, uint32_t data2);
} fc4_weather_host;

typedef struct fc4_weather_state {
    uint32_t struct_size;
    uint8_t initialized, loaded_from_save, post_init_done, pad;
    int32_t count_x, count_z, last_season, cells_per_week, wind_month;
    uint32_t water_rect_count;
    float ambient_temp, ambient_moisture, wind_speed, wind_dir_x, wind_dir_z, target_wind_speed;
} fc4_weather_state;

/* The host table is copied. `session_seed` is the session seed S (foundations
 * §6.3, FC4_ABI_VERSION 64): the generator (Seed(0xFFFFFFFF) in the original,
 * §3) is seeded with derive(S, 0x2990C1BC). NULL if the table is missing or too
 * small. */
fc4_weather* fc4_weather_create(const fc4_weather_host* host, uint32_t session_seed);
void fc4_weather_destroy(fc4_weather* w);
/* Init (§8.1, core part); tuning = the tuning exemplar's properties (copied),
 * NULL = no exemplar. Shutdown (§8.2, core part). */
bool fc4_weather_init(fc4_weather* w, const fc4_props* tuning);
bool fc4_weather_shutdown(fc4_weather* w);
bool fc4_weather_do_post_city_init(fc4_weather* w);
/* DoMessage (§5): SimNewDay (data1 = Julian day), SimNewWeek, SimNewMonth
 * (data3 = month), PostCityInit. Always true. */
bool fc4_weather_handle_message(fc4_weather* w, uint32_t msg_type, uint32_t data1, uint32_t data3);
/* cISC4WeatherSimulator methods (§4); axis 0 = x, 1 = z. */
float fc4_weather_cell_width(const fc4_weather* w, uint32_t axis);
uint32_t fc4_weather_cell_count(const fc4_weather* w, uint32_t axis);
int32_t fc4_weather_position_to_cell(const fc4_weather* w, float x, float z, int32_t* cx, int32_t* cz);
void fc4_weather_cell_to_position(const fc4_weather* w, int32_t cx, int32_t cz, float* x, float* z);
void fc4_weather_cell_to_standard_city_cell(const fc4_weather* w, int32_t cx, int32_t cz, int32_t* sx, int32_t* sz);
void fc4_weather_standard_city_cell_to_cell(const fc4_weather* w, int32_t sx, int32_t sz, int32_t* cx, int32_t* cz);
int32_t fc4_weather_season(const fc4_weather* w);
int32_t fc4_weather_season_month(const fc4_weather* w);
void fc4_weather_update_moisture(fc4_weather* w, int32_t x0, int32_t z0, int32_t x1, int32_t z1);
uint8_t fc4_weather_moisture_value(const fc4_weather* w, float x, float z);
uint8_t fc4_weather_moisture_type(const fc4_weather* w, float x, float z);
void fc4_weather_set_manual_moisture_addition(fc4_weather* w, float x0, float z0, float x1, float z1, float v);
float fc4_weather_humidity(const fc4_weather* w, float x, float z);
/* GetAmbientWind: returns the speed; v (2 floats, may be NULL) = direction x speed. */
float fc4_weather_ambient_wind(const fc4_weather* w, float* v);
/* Copies the moisture grid x-major (out[x * count_z + z], at most `capacity`
 * bytes; out may be NULL) and reports its shape. Returns the cell count. */
size_t fc4_weather_moisture_grid(const fc4_weather* w, uint8_t* out, size_t capacity, int32_t* count_x,
                                 int32_t* count_z);
bool fc4_weather_get_state(const fc4_weather* w, fc4_weather_state* out);
/* Persistence (§10). */
bool fc4_weather_write(const fc4_weather* w, const fc4_ostream* out);
bool fc4_weather_read(fc4_weather* w, const fc4_istream* in);

/* ---- ordinances (docs/specs/ordinance.md) --------------------------------------
 * The ordinance simulator and the standard ordinance classes (§3.6). The shim
 * owns the GZCOM objects: the simulator (agent, messages) and one object per
 * ordinance, each owning an fc4_ordinance. Ordinance tokens are
 * cISC4Ordinance*; the simulator's token is its IGZUnknown. Foreign
 * cISC4Ordinance objects (DLL plugins) are called back through ordinance_call,
 * ordinance_text and ordinance_misc_property. */
typedef struct fc4_ordinance_sim fc4_ordinance_sim;
typedef struct fc4_ordinance fc4_ordinance;

/* cISC4Ordinance operations (fc4_ordinance_call, the host's ordinance_call),
 * in slot order without GetName / GetDescription / GetMiscProperties.
 * Booleans are 0 / 1; CHANCE_AVAILABILITY and MONTHLY_INCOME_FACTOR return
 * the float's bits; SET_* / FORCE_* take the value in `arg`. */
enum {
    FC4_ORD_INIT = 0,
    FC4_ORD_SHUTDOWN = 1,
    FC4_ORD_CURRENT_MONTHLY_INCOME = 2,
    FC4_ORD_ID = 3,
    FC4_ORD_YEAR_FIRST_AVAILABLE = 4,
    FC4_ORD_CHANCE_AVAILABILITY = 5,
    FC4_ORD_ENACTMENT_INCOME = 6,
    FC4_ORD_RETRACMENT_INCOME = 7,
    FC4_ORD_MONTHLY_CONSTANT_INCOME = 8,
    FC4_ORD_MONTHLY_INCOME_FACTOR = 9,
    FC4_ORD_ADVISOR_ID = 10,
    FC4_ORD_IS_AVAILABLE = 11,
    FC4_ORD_IS_ON = 12,
    FC4_ORD_IS_ENABLED = 13,
    FC4_ORD_MONTHLY_ADJUSTED_INCOME = 14,
    FC4_ORD_CHECK_CONDITIONS = 15,
    FC4_ORD_IS_INCOME_ORDINANCE = 16,
    FC4_ORD_SIMULATE = 17,
    FC4_ORD_SET_AVAILABLE = 18,
    FC4_ORD_SET_ON = 19,
    FC4_ORD_SET_ENABLED = 20,
    FC4_ORD_FORCE_AVAILABLE = 21,
    FC4_ORD_FORCE_ON = 22,
    FC4_ORD_FORCE_MONTHLY_ADJUSTED_INCOME = 23
};

typedef void (*fc4_key_visitor)(void* ctx, uint32_t type_id, uint32_t group, uint32_t instance);

typedef struct fc4_ordinance_host {
    uint32_t struct_size; /* sizeof(fc4_ordinance_host) */
    void* user;
    /* Resources (§8.1, §3.2). for_each_exemplar_key visits exemplar keys (the
     * host may skip exemplars whose type property 0x10 is not 14; the core checks
     * again). exemplar returns a NEW fc4_props with all of the exemplar's
     * properties (the core destroys it), NULL if missing. ltext delivers a
     * localized LTEXT string; false if not found. */
    void (*for_each_exemplar_key)(void* user, fc4_key_visitor visit, void* ctx);
    fc4_props* (*exemplar)(void* user, uint32_t type_id, uint32_t group, uint32_t instance);
    bool (*ltext)(void* user, uint32_t type_id, uint32_t group, uint32_t instance, fc4_bytes_sink sink, void* ctx);
    /* Objects. create_ordinance: a new standard ordinance of `class_id` with one
     * reference owned by the core (its token in *token), NULL if none.
     * foreign_ordinance: the cISC4Ordinance of an object read from a save
     * (token = its IGZUnknown with one reference): on success returns the
     * ordinance token, carrying one reference, and has dropped the input
     * reference; 0 = not an ordinance (the input reference is untouched and the
     * core releases it). */
    fc4_ordinance* (*create_ordinance)(void* user, uint32_t class_id, uint64_t* token);
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    uint64_t (*foreign_ordinance)(void* user, uint64_t token);
    int64_t (*ordinance_call)(void* user, uint64_t token, uint32_t op, int64_t arg);
    /* which 0 = GetName, 1 = GetDescription. */
    bool (*ordinance_text)(void* user, uint64_t token, uint32_t which, fc4_bytes_sink sink, void* ctx);
    /* Copies property `prop` of GetMiscProperties() into `out`; false if absent. */
    bool (*ordinance_misc_property)(void* user, uint64_t token, uint32_t prop, fc4_props* out);
    /* Posts a message with void1 = the ordinance simulator (§7.1). */
    void (*post)(void* user, uint32_t msg_type, uint32_t data2, uint32_t data3);
    /* City (§3.4, §3.6). sim_year: false if there is no simulator; the expense
     * lookups: false if the department / line item is missing. */
    bool (*sim_year)(void* user, uint32_t* year);
    int32_t (*residential_population)(void* user);
    bool (*has_budget_simulator)(void* user);
    bool (*department_total_expenses)(void* user, uint32_t dept, int64_t* out);
    bool (*line_item_full_expenses)(void* user, uint32_t dept, uint32_t line, int64_t* out);
    uint32_t (*landfill_capacity)(void* user);
    int32_t (*power_plant_count)(void* user, int32_t power_type);
    uint32_t (*police_station_count)(void* user);
    /* The fraction behind GetRecyclingCenterEffect's pointer (§3.6). */
    float (*recycling_center_effect)(void* user);
    /* LegalizeGambling's SetOn(false) side effects (§3.6). */
    void (*lots_with_building_type)(void* user, uint32_t building_type, fc4_token_callback cb, void* ctx);
    void (*demolish_lots)(void* user, const uint64_t* lots, size_t count);
    void (*hide_conditional_building)(void* user, uint32_t building_type);
    void (*remove_view_input_control)(void* user, uint32_t control_id);
} fc4_ordinance_host;

typedef struct fc4_ordinance_sim_state {
    uint32_t struct_size;
    uint8_t initialized, loaded_from_save;
    uint16_t pad;
    uint32_t count; /* map entries */
    uint32_t pad2;
    int64_t monthly_expense, monthly_income, pending;
} fc4_ordinance_sim_state;

/* The simulator (§4). Host tables are copied; NULL if missing or too small. */
fc4_ordinance_sim* fc4_ordinance_sim_create(const fc4_ordinance_host* host, uint64_t token);
/* Releases every map entry (§8.2: ordinances live until the simulator dies). */
void fc4_ordinance_sim_destroy(fc4_ordinance_sim* s);
/* Init (§8.1, core part: loads the standard ordinances unless loaded from a
 * save; after a load, re-keys the entries by GetID() [choice]) and Shutdown
 * (§8.2, core part: the map is kept). */
bool fc4_ordinance_sim_init(fc4_ordinance_sim* s);
bool fc4_ordinance_sim_shutdown(fc4_ordinance_sim* s);
/* AddOrdinance / RemoveOrdinance (§4.2): token = the cISC4Ordinance*, o = its
 * fc4_ordinance when it is one of ours, NULL for a foreign one. */
bool fc4_ordinance_sim_add(fc4_ordinance_sim* s, uint64_t token, const fc4_ordinance* o);
bool fc4_ordinance_sim_remove(fc4_ordinance_sim* s, uint64_t token, const fc4_ordinance* o);
/* which: 0 IsOrdinanceAvailable, 1 IsOrdinanceOn, 2 IsOrdinanceAvailableButOff,
 * 3 IsOrdinanceEnabled. */
bool fc4_ordinance_sim_query(const fc4_ordinance_sim* s, uint32_t which, uint32_t id);
/* which: 0 SetOrdinanceAvailable (§4.3), 1 SetOrdinanceOn (§4.4), 2 SetOrdinanceEnabled. */
bool fc4_ordinance_sim_set(fc4_ordinance_sim* s, uint32_t which, uint32_t id, bool value);
/* which: 0 GetOrdinanceAvailableArray, 1 GetOrdinanceOnArray, 2 GetOrdinanceIDArray:
 * IDs in key order while fewer than *n were written (out NULL = capacity 0);
 * *n := the number written; returns the total. */
uint32_t fc4_ordinance_sim_array(const fc4_ordinance_sim* s, uint32_t which, uint32_t* out, uint32_t* n);
/* which: 0 GetOrdinanceAvailableCount, 1 GetOrdinanceOnCount. */
uint32_t fc4_ordinance_sim_count(const fc4_ordinance_sim* s, uint32_t which);
/* GetOrdinanceByID / ByName: the entry's token (no reference added), 0 if none. */
uint64_t fc4_ordinance_sim_by_id(const fc4_ordinance_sim* s, uint32_t id);
uint64_t fc4_ordinance_sim_by_name(const fc4_ordinance_sim* s, const uint8_t* name, size_t len);
uint32_t fc4_ordinance_sim_id_by_name(const fc4_ordinance_sim* s, const uint8_t* name, size_t len);
/* which: 0 GetOrdinanceMonthlyExpense, 1 GetOrdinanceMonthlyIncome. */
int64_t fc4_ordinance_sim_money(const fc4_ordinance_sim* s, uint32_t which);
/* §6.1: the offset is a 32-bit sum, sign-extended. */
int64_t fc4_ordinance_sim_cumulative_offset(const fc4_ordinance_sim* s, uint32_t prop, uint32_t index);
float fc4_ordinance_sim_cumulative_factor(const fc4_ordinance_sim* s, uint32_t prop, uint32_t index);
/* DoMessage (§5): SimNewMonth, SimBegin. Always true. */
bool fc4_ordinance_sim_handle_message(fc4_ordinance_sim* s, uint32_t msg_type);
bool fc4_ordinance_sim_get_state(const fc4_ordinance_sim* s, fc4_ordinance_sim_state* out);
/* Persistence (§9.1). Ordinances go out as references by token; on read, ours
 * arrive with their core handle (fc4_ref.core_handle = the fc4_ordinance*),
 * others with one reference offered to foreign_ordinance. */
bool fc4_ordinance_sim_write(const fc4_ordinance_sim* s, const fc4_ostream* out);
bool fc4_ordinance_sim_read(fc4_ordinance_sim* s, const fc4_istream* in);

/* A standard ordinance (§3) of class `class_id` for the GZCOM object `token`;
 * NULL for a class that is not one of §3.6. */
fc4_ordinance* fc4_ordinance_create(const fc4_ordinance_host* host, uint32_t class_id, uint64_t token);
void fc4_ordinance_destroy(fc4_ordinance* o);
uint32_t fc4_ordinance_class_id(const fc4_ordinance* o);
/* SetKey (§3.5, concrete class) and the key it holds (false if none). */
void fc4_ordinance_set_key(fc4_ordinance* o, uint32_t type_id, uint32_t group, uint32_t instance);
bool fc4_ordinance_get_key(const fc4_ordinance* o, uint32_t* type_id, uint32_t* group, uint32_t* instance);
/* One cISC4Ordinance operation (FC4_ORD_*). */
int64_t fc4_ordinance_call(fc4_ordinance* o, uint32_t op, int64_t arg);
/* which 0 = GetName, 1 = GetDescription: the bytes go to `sink`. */
bool fc4_ordinance_text(const fc4_ordinance* o, uint32_t which, fc4_bytes_sink sink, void* ctx);
/* One property of GetMiscProperties() (own, then the parent exemplar), copied
 * into `out`; false if absent. */
bool fc4_ordinance_misc_property(const fc4_ordinance* o, uint32_t prop, fc4_props* out);
/* Persistence (§9.2). Read accepts any version. */
bool fc4_ordinance_write(const fc4_ordinance* o, const fc4_ostream* out);
bool fc4_ordinance_read(fc4_ordinance* o, const fc4_istream* in);

/* ---- demand (docs/specs/demand.md) ---------------------------------------------
 * The demand simulator and demand nodes. The shim owns the GZCOM objects: the
 * simulator (agents, subscriptions, the services it reads) and one object per
 * node, each owning an fc4_demand_node. Node tokens are cISC4Demand*; lots,
 * connections and regional cities cross as their interface pointers.
 * Re-entrancy: callbacks must not call back into the demand simulator. */
typedef struct fc4_demand_sim fc4_demand_sim;
typedef struct fc4_demand_node fc4_demand_node;

/* Node values (fc4_demand_node_get / _set / _add). */
enum {
    FC4_DEMAND_SUPPLY = 0,
    FC4_DEMAND_DEMAND = 1,
    FC4_DEMAND_NEW_SUPPLY = 2,
    FC4_DEMAND_NEW_DEMAND = 3,
    FC4_DEMAND_ACTIVE_DEMAND = 4,
    FC4_DEMAND_ECONOMY_MODIFIER = 5,
    FC4_DEMAND_ACTIVE_MAX = 6,
    FC4_DEMAND_ACTIVE_MIN = 7,
    FC4_DEMAND_TAX_MODIFIER = 8,
    FC4_DEMAND_DEMAND_CAP = 9
};

typedef struct fc4_demand_connection {
    uint64_t token;     /* the cISC4NeighborConnection* */
    uint64_t city;      /* GetCityOnOtherSideOfConnection (0 = none) */
    uint32_t conn_type; /* GetType */
    uint32_t reserved;
} fc4_demand_connection;

typedef struct fc4_demand_growth_developer {
    uint32_t sub_type;    /* GetSubType (the developer type) */
    uint32_t kind;        /* GetType: 1 R, 2 C, 3 I */
    uint32_t has_source;  /* 0: GetSourceDemand returned no node */
    float supply, demand; /* the source node's committed values */
    uint32_t reserved;
} fc4_demand_growth_developer;

typedef struct fc4_demand_sim_state {
    uint32_t struct_size;
    uint8_t initialized, loaded_from_save, census_state, pad;
    uint32_t node_count, link_count;
    int32_t cursor_x, cursor_z;
    float co_economy_bonus, i_economy_bonus, commercial_relief, industrial_relief;
    uint32_t export_city_count;
} fc4_demand_sim_state;

typedef void (*fc4_capacity_visitor)(void* ctx, uint32_t map_index, uint32_t dev_type, uint16_t capacity);
typedef void (*fc4_u32_callback)(void* ctx, uint32_t value);
typedef void (*fc4_connection_callback)(void* ctx, const fc4_demand_connection* c);
typedef void (*fc4_growth_developer_callback)(void* ctx, const fc4_demand_growth_developer* g);
typedef void (*fc4_population_visitor)(void* ctx, uint32_t key, int32_t value);

/* The DemandHost trait of fc4_core::demand as callbacks (the exact calls: §14). */
typedef struct fc4_demand_host {
    uint32_t struct_size; /* sizeof(fc4_demand_host) */
    void* user;
    /* Resources (§4.2): every exemplar key (type 0x6534284A, any group; the host
     * may skip exemplars whose ExemplarType is not 3, the core checks again);
     * exemplar returns a NEW fc4_props (the core destroys it), NULL if missing. */
    void (*for_each_exemplar_key)(void* user, fc4_key_visitor visit, void* ctx);
    fc4_props* (*exemplar)(void* user, uint32_t type_id, uint32_t group, uint32_t instance);
    void (*warn)(void* user, const uint8_t* text, size_t len);
    /* Nodes. create_node: a new node (class 0x8990C12D) with one reference owned
     * by the core, its token in *token. post_active_demand: ActiveDemandChanged
     * (0x426840A0) from `node` with data1 = key and data2 (§3.4). */
    fc4_demand_node* (*create_node)(void* user, uint64_t* token);
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    void (*post_active_demand)(void* user, uint64_t node, uint32_t key, int32_t data2);
    /* City and clock: CellCountX/Z; today = the simulation date's Julian day;
     * now_ms = a millisecond wall clock (census slicing, §6.2). */
    void (*cell_counts)(void* user, int32_t* x, int32_t* z);
    uint32_t (*today)(void* user);
    uint32_t (*now_ms)(void* user);
    /* Census (§6.3). census_lot: false unless GetLot(x, z, true) exists, has
     * capacity and a building type. building_profile: false = no profile; the
     * capacity list goes to `visit` (vector index, developer type, value).
     * `visit` may be NULL: the purpose alone, no capacity walk (the census,
     * §6.3 / DQ5). */
    bool (*census_lot)(void* user, int32_t x, int32_t z, uint32_t* building_type, uint32_t* wealth);
    bool (*building_profile)(void* user, uint32_t building_type, uint32_t* purpose, fc4_capacity_visitor visit,
                             void* ctx);
    uint32_t (*developer_type)(void* user, uint32_t purpose, uint32_t wealth);
    int16_t (*desirability)(void* user, uint32_t dev_type, int32_t x, int32_t z);
    uint16_t (*lot_capacity)(void* user, int32_t x, int32_t z, uint32_t dev_type);
    float (*cell_eq)(void* user, int32_t x, int32_t z);
    float (*cell_workforce_percent)(void* user, int32_t x, int32_t z);
    /* Occupancy (§7). lot_excluded: the building has bool 0x27812844 = true. */
    uint32_t (*lot_building_type)(void* user, uint64_t lot);
    bool (*lot_excluded)(void* user, uint64_t lot);
    void (*all_building_types)(void* user, fc4_u32_callback cb, void* ctx);
    fc4_props* (*building_exemplar)(void* user, uint32_t building_type);
    /* Modifiers (§8.1, §8.2). history_max: false = no data in the window. */
    bool (*history_max)(void* user, uint32_t series, uint32_t from_day, uint32_t to_day, float* max);
    float (*ordinance_factor)(void* user, uint32_t prop, uint32_t index);
    float (*nation_economy_modifier)(void* user, uint32_t demand_id);
    /* Cap relief (§8.3). */
    void (*connections)(void* user, fc4_connection_callback cb, void* ctx);
    float (*traffic_arrived)(void* user, uint64_t connection);
    float (*airport_trips)(void* user);
    float (*seaport_trips)(void* user);
    /* Region (§9). Cities are cISC4RegionalCity tokens (borrowed). */
    void (*all_connected_cities)(void* user, fc4_token_callback cb, void* ctx);
    void (*connected_cities)(void* user, fc4_token_callback cb, void* ctx);
    uint64_t (*own_regional_city)(void* user);
    int32_t (*city_population)(void* user, uint64_t city, uint32_t pop_type);
    int32_t (*city_extrapolated_population)(void* user, uint64_t city, uint32_t pop_type);
    int32_t (*city_allowable_extrapolation)(void* user, uint64_t city, uint32_t pop_type);
    float (*city_tax_rate)(void* user, uint64_t city, uint32_t tax_type);
    float (*city_workforce_percentage)(void* user, uint64_t city);
    void (*city_extrapolate_growth)(void* user, uint64_t city, uint32_t pop_type, float amount);
    void (*growth_developers)(void* user, fc4_growth_developer_callback cb, void* ctx);
    uint32_t (*day_of_last_successful_build)(void* user, uint32_t dev_type);
    /* History (§10.4). */
    void (*record_history)(void* user, uint32_t series, int32_t value);
    void (*set_history_queue_length)(void* user, uint32_t series, uint32_t length);
} fc4_demand_host;

/* The simulator (§5, §13). Host tables are copied; NULL if missing or too small. */
fc4_demand_sim* fc4_demand_sim_create(const fc4_demand_host* host, uint64_t token);
/* Releases every node in the map. */
void fc4_demand_sim_destroy(fc4_demand_sim* s);
/* Init / Shutdown, core part (§13): subscriptions and agents are the shim's. */
bool fc4_demand_sim_init(fc4_demand_sim* s);
bool fc4_demand_sim_shutdown(fc4_demand_sim* s);
/* GetDemand (initializes first if needed): the node's token, no reference
 * added; 0 if none. */
uint64_t fc4_demand_sim_get_demand(fc4_demand_sim* s, uint32_t demand_id, uint32_t demand_index);
void fc4_demand_sim_update_occupant_effects(fc4_demand_sim* s, uint32_t building_type, float old_pct, float new_pct);
/* CalculateJobsPerUnitOfDemand: out = the 3 outputs (may be NULL), each the
 * demand.md 4.1 link factor, percent x 0.01f (DQ12: rounded once below 2^31,
 * twice from 2^31 up). The game's record is four floats: the plugin writes
 * them to jobs[1..3] and never touches jobs[0]. */
void fc4_demand_sim_jobs_per_unit(const fc4_demand_sim* s, uint32_t demand_id, uint32_t demand_index, float* out);
/* GetJobsBySensus (DQ10): (jobs[3] + jobs[2]) + jobs[1] times the supply,
 * the low 32 bits of _ftol, no clamp; slot 9 returns it in EAX. */
uint32_t fc4_demand_sim_jobs_by_census(const fc4_demand_sim* s, uint32_t census_type);
float fc4_demand_sim_neutral_tax_rate(const fc4_demand_sim* s);
/* GetLocalPopulationSummary: (key, value) in key order; returns the count. */
size_t fc4_demand_sim_local_population_summary(const fc4_demand_sim* s, fc4_population_visitor visit, void* ctx);
/* DoMessage (§10.2) for everything but kMessageStateChanged. Always true. */
bool fc4_demand_sim_handle_message(fc4_demand_sim* s, uint32_t msg_type, uint32_t data1, uint32_t data2,
                                   uint32_t data3);
/* kMessageStateChanged (§7): lot = the message's IGZUnknown; then data1..data4. */
void fc4_demand_sim_state_changed(fc4_demand_sim* s, uint64_t lot, uint32_t old_state, uint32_t new_state,
                                  uint32_t building_type, uint32_t wealth);
void fc4_demand_sim_set_economy_bonuses(fc4_demand_sim* s, float co, float i);
bool fc4_demand_sim_get_state(const fc4_demand_sim* s, fc4_demand_sim_state* out);
/* Persistence (§12.1): nodes go out as references by token; on read, ours
 * arrive with their core handle (fc4_ref.core_handle = the fc4_demand_node*, no
 * reference: the map takes one with add_ref); anything else is released and
 * skipped. */
bool fc4_demand_sim_write(const fc4_demand_sim* s, const fc4_ostream* out);
bool fc4_demand_sim_read(fc4_demand_sim* s, const fc4_istream* in);

/* A demand node (§3) for the GZCOM object `token`; only post_active_demand of
 * the host table is used. */
fc4_demand_node* fc4_demand_node_create(const fc4_demand_host* host, uint64_t token);
void fc4_demand_node_destroy(fc4_demand_node* n);
bool fc4_demand_node_init(fc4_demand_node* n);
bool fc4_demand_node_shutdown(fc4_demand_node* n);
bool fc4_demand_node_simulation_begin(fc4_demand_node* n);
uint32_t fc4_demand_node_id(fc4_demand_node* n);
bool fc4_demand_node_set_id(fc4_demand_node* n, uint32_t id);
uint32_t fc4_demand_node_region_use(fc4_demand_node* n);
bool fc4_demand_node_set_region_use(fc4_demand_node* n, uint32_t v);
/* EndOfCycle: folds the pending values and recalculates (posts); true if a
 * value was pending. */
bool fc4_demand_node_end_of_cycle(fc4_demand_node* n);
/* An FC4_DEMAND_* value. */
float fc4_demand_node_get(const fc4_demand_node* n, uint32_t which);
/* Setters: SUPPLY, DEMAND (committed), ACTIVE_MAX, ACTIVE_MIN, ECONOMY_MODIFIER,
 * TAX_MODIFIER, DEMAND_CAP; false for the others. */
bool fc4_demand_node_set(fc4_demand_node* n, uint32_t which, float v);
/* AddToSupplyValue (SUPPLY) / AddToDemandValue (DEMAND). */
bool fc4_demand_node_add(fc4_demand_node* n, uint32_t which, float v);
/* Persistence (§12.2): versions 0-2 are read. */
bool fc4_demand_node_write(const fc4_demand_node* n, const fc4_ostream* out);
bool fc4_demand_node_read(fc4_demand_node* n, const fc4_istream* in);

/* Demand shadow (diagnostics): FreeCity4's propagation (§4.4) and monthly
 * modifiers (§8.1), the code its demand simulator runs, over node states read
 * from another demand simulator (the original game's). Nothing is created in
 * the game, stored, posted or written. */
typedef struct fc4_demand_shadow fc4_demand_shadow;

/* One node's state (§3.1). */
typedef struct fc4_demand_node_state {
    uint32_t key, region_use;
    float supply, demand, new_supply, new_demand;
    float active_demand, active_min, active_max;
    float economy_modifier, tax_modifier, demand_cap;
} fc4_demand_node_state;

typedef struct fc4_demand_shadow_info {
    uint32_t struct_size; /* caller sets = sizeof(fc4_demand_shadow_info) */
    uint32_t exemplars, keys, links;
    uint32_t tuning_found; /* 0: the tuning exemplar is missing (code defaults) */
    uint32_t reserved;
} fc4_demand_shadow_info;

typedef struct fc4_demand_shadow_stats {
    uint32_t struct_size; /* caller sets = sizeof(fc4_demand_shadow_stats) */
    uint32_t nodes, passes, settled, transfers, end_of_cycles;
    uint32_t missing_targets; /* link visits whose target is not among the nodes */
    uint32_t reserved;
} fc4_demand_shadow_stats;

/* §8.1 for one total node. */
typedef struct fc4_demand_modifier {
    float tax_modifier, economy_modifier;
    float max_rate, ratio, tax_curve, ordinance_factor, nation;
    uint32_t from_history; /* 0: no history in the window, max_rate = neutral * 10 */
} fc4_demand_modifier;

typedef struct fc4_demand_modifiers {
    uint32_t struct_size; /* caller sets = sizeof(fc4_demand_modifiers) */
    uint32_t today;
    float neutral_raw, neutral;
    fc4_demand_modifier node[12]; /* 0x21010, 0x21020, ... 0x24400 (§8.1 order) */
} fc4_demand_modifiers;

/* The graph (§4.2) and tuning (§11.1) from the exemplars `host` serves
 * (for_each_exemplar_key and exemplar only). NULL on bad arguments. */
fc4_demand_shadow* fc4_demand_shadow_create(const fc4_demand_host* host);
void fc4_demand_shadow_destroy(fc4_demand_shadow* s);
bool fc4_demand_shadow_get_info(const fc4_demand_shadow* s, fc4_demand_shadow_info* out);
/* The graph's keys (exemplar nodes and link targets), ascending; returns their number. */
uint32_t fc4_demand_shadow_keys(const fc4_demand_shadow* s, fc4_u32_callback cb, void* ctx);
/* Whether propagation can change the key's totals other than by folding its
 * own pending values (a transfer target, or a Sums node). */
bool fc4_demand_shadow_is_fed(const fc4_demand_shadow* s, uint32_t key);
/* DoDemandIndexLinks over `count` states (distinct keys) into `output` (same
 * order; may equal `input`); `stats` may be NULL. False on bad arguments. */
bool fc4_demand_shadow_propagate(const fc4_demand_shadow* s, const fc4_demand_node_state* input, uint32_t count,
                                 fc4_demand_node_state* output, fc4_demand_shadow_stats* stats);
/* GetNeutralTaxRate (unclamped) from the supplies r, c, i of 0x1001, 0x3001, 0x4001; the sum is
 * f32(f32(i + c) + r), as the simulator's (demand §5, DQ9). */
float fc4_demand_shadow_neutral_tax_rate(const fc4_demand_shadow* s, float r, float c, float i);
/* §8.1 from those supplies, the saved CO / I economy bonuses, and host's today,
 * history_max, ordinance_factor and nation_economy_modifier (no other callback). */
bool fc4_demand_shadow_modifiers(const fc4_demand_shadow* s, const fc4_demand_host* host, float r, float c, float i,
                                 float co_bonus, float i_bonus, fc4_demand_modifiers* out);
/* ---- the game's grids, shared by the grid simulators ----------------------------
 *
 * The game's own cSC4SimGrid<T> objects (simgrid.md §6.4), driven by the core.
 * A grid is a token holding ONE reference (released through `release`). `kind`
 * is the grid class ID of the element type (0x49B9E602 uint8 ... 0x49B9E60B
 * double); values cross as doubles (exact for every type up to 32 bits). */
typedef struct fc4_grid_api {
    uint32_t struct_size; /* sizeof(fc4_grid_api) */
    void* user;
    /* A new grid through GZCOM: Init, SetTractSize(tract_size),
     * SetInstanceID(instance). 0 = failure. */
    uint64_t (*create)(void* user, uint32_t kind, uint32_t instance, int32_t tract_size);
    uint32_t (*class_of)(void* user, uint64_t grid); /* GetGZCLSID, 0 if unknown */
    void (*geometry)(void* user, uint64_t grid, fc4_grid_geometry* out);
    double (*get)(void* user, uint64_t grid, uint32_t kind, int32_t tx, int32_t tz);          /* GetTractValue */
    void (*set)(void* user, uint64_t grid, uint32_t kind, int32_t tx, int32_t tz, double v);  /* SetTractValue */
    void (*fill)(void* user, uint64_t grid, uint32_t kind, double v);                         /* SetTractValues */
    void (*shutdown)(void* user, uint64_t grid);
    void (*release)(void* user, uint64_t grid);
} fc4_grid_api;

/* 63: a performance hook (devperf). The host's bulk tract reader copies every
 * tract value of `grid`, x outer (out[x * count_z + z]), as native elements of
 * class `kind` into `out` (room for `count`) and returns the number written
 * (0 for a kind it does not support); the core uses the copy only when that is
 * count_x * count_z, and reads tract by tract through `get` otherwise, with
 * the same values. Registered process-wide for the grids whose
 * fc4_grid_api.get is `get`; a NULL `reader` removes it. */
typedef uint32_t (*fc4_grid_bulk_reader)(uint64_t grid, uint32_t kind, void* out, uint32_t count);
void fc4_grid_set_bulk_reader(double (*get)(void* user, uint64_t grid, uint32_t kind, int32_t tx, int32_t tz),
                              fc4_grid_bulk_reader reader);
/* 63: the bulk tract writer (devperf): for each i < n in order, SetTractValue of tract index[i]
 * (x * count_z + z) to values[i], a native element of class `kind`; returns n when it wrote them all
 * and 0 when it wrote none (writes switched off, an unsupported kind), and the core then sets them
 * tract by tract through `set`. Registered process-wide for the grids whose fc4_grid_api.set is
 * `set`; a NULL `writer` removes it. */
typedef uint32_t (*fc4_grid_bulk_writer)(void* user, uint64_t grid, uint32_t kind, const uint32_t* index,
                                         const void* values, uint32_t n);
void fc4_grid_set_bulk_writer(void (*set)(void* user, uint64_t grid, uint32_t kind, int32_t tx, int32_t tz, double v),
                              fc4_grid_bulk_writer writer);

/* An inclusive cell rectangle. */
typedef struct fc4_cell_rect {
    int32_t x1, z1, x2, z2;
} fc4_cell_rect;

/* ---- power (docs/specs/power.md) -------------------------------------------------
 *
 * Tokens: occupants (also poles, lines and plants: the cISC4Occupant
 * identity), lots, deals, connections, cities, departments, line items; 0 is
 * null. A message delivered while another call into the same object is
 * running is queued and handled when that call returns. */
typedef struct fc4_power fc4_power;

typedef struct fc4_power_post {
    uint32_t msg_type;
    uint32_t data[4];
    uint32_t reserved;
    uint64_t unknown; /* the IGZUnknown, 0 = none */
} fc4_power_post;

typedef struct fc4_power_pole {
    uint32_t ignored; /* pole slot 13 */
    int32_t cell_x, cell_z;
} fc4_power_pole;

typedef struct fc4_power_line {
    uint32_t preview;
    int32_t cell_x, cell_z;
} fc4_power_line;

typedef struct fc4_power_deal {
    uint64_t token;
    uint64_t city; /* GetCityOnOtherSideOfConnection */
    int32_t amount;
    uint32_t reserved;
} fc4_power_deal;
typedef void (*fc4_power_deal_callback)(void* ctx, const fc4_power_deal* deal);

typedef struct fc4_power_budget_item {
    uint32_t department;
    uint32_t line;
    int64_t cost;
    uint32_t index;
    float local_funding; /* a fraction */
} fc4_power_budget_item;

typedef struct fc4_power_message {
    uint32_t msg_type;
    uint32_t data1, data2, data3, data4;
    uint32_t reserved;
    uint64_t void1;     /* the IGZUnknown as a token, 0 = null */
    fc4_cell_rect rect; /* the four int32 at void2 (zoning changed) */
} fc4_power_message;

typedef struct fc4_power_found {
    uint32_t has_lowest_funded;
    float lowest_funding;
    uint64_t lowest_funded; /* one reference for the caller */
    uint32_t has_most_worn;
    int32_t wear_percent;
    uint64_t most_worn; /* one reference for the caller */
} fc4_power_found;

typedef struct fc4_power_state {
    uint32_t struct_size;
    uint8_t initialized, running, enabled, recompute;
    uint32_t state;
    int32_t plant_count;
    uint32_t pole_count, consumers, powered, unpowered, requirement;
    uint32_t last_production, last_consumption, last_import, last_export, last_demand;
    float deficit;
    uint32_t reentrant_calls;
    int64_t cost_base;
    uint64_t department;
    uint64_t grid;
} fc4_power_state;

/* What the power simulator needs from the game: one callback per PowerHost
 * method (engine/core/src/power/mod.rs, power.md §11.1). Missing callbacks
 * behave as "nothing there" (0, false, no object). */
typedef struct fc4_power_host {
    uint32_t struct_size; /* sizeof(fc4_power_host) */
    void* user;
    fc4_grid_api grids;
    /* city, tuning, ordinance, simulator */
    void (*cell_counts)(void* user, int32_t* x, int32_t* z);
    void (*cell_widths)(void* user, float* x, float* z);
    fc4_props* (*tuning)(void* user); /* a NEW fc4_props (the core destroys it), NULL if missing */
    float (*cumulative_factor)(void* user, uint32_t id, uint32_t index);
    uint32_t (*sim_time)(void* user);
    void (*acquire_services)(void* user);
    void (*release_services)(void* user);
    void (*add_agent)(void* user, uint32_t level, const char* name, size_t name_len, uint32_t flags);
    void (*remove_agent)(void* user);
    void (*subscribe)(void* user, const uint32_t* ids, size_t count);
    void (*unsubscribe)(void* user, const uint32_t* ids, size_t count);
    void (*post)(void* user, const fc4_power_post* msg);
    void (*record_history)(void* user, uint32_t series, int32_t value);
    void (*set_history_queue_length)(void* user, uint32_t series, uint32_t length);
    /* references */
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    /* A reference read from a save (QI'd to iid, one reference) to the object's
     * canonical token (one reference); 0 = dropped. */
    uint64_t (*resolve_ref)(void* user, uint64_t token, uint32_t iid);
    /* occupants */
    uint32_t (*occupant_type)(void* user, uint64_t occ);
    bool (*occupant_property)(void* user, uint64_t occ, uint32_t id, fc4_props* out);
    void (*set_occupant_property)(void* user, uint64_t occ, uint32_t id, uint16_t mem_type, bool is_array,
                                  const void* data, size_t count);
    void (*occupant_position)(void* user, uint64_t occ, float* xyz);
    void (*occupant_bounding_box)(void* user, uint64_t occ, float* min_xyz, float* max_xyz);
    void (*occupant_bounding_cells)(void* user, uint64_t occ, fc4_cell_rect* out);
    bool (*occupant_in_manager)(void* user, uint64_t occ);
    bool (*building_type)(void* user, uint64_t occ, uint32_t* out);
    bool (*power_pole)(void* user, uint64_t occ, fc4_power_pole* out);
    void (*break_all_connections)(void* user, uint64_t pole, bool flag);
    bool (*pole_is_distressed)(void* user, uint64_t pole);
    void (*set_pole_distressed)(void* user, uint64_t pole, bool on);
    bool (*power_line)(void* user, uint64_t occ, fc4_power_line* out);
    bool (*line_connected_poles)(void* user, uint64_t line, uint64_t* first, uint64_t* second);
    void (*remove_pole_connection)(void* user, uint64_t first, uint64_t second); /* first.RemoveConnection(second, 0) */
    bool (*network_occupant_cell)(void* user, uint64_t occ, int32_t* x, int32_t* z);
    bool (*has_network_flag)(void* user, uint64_t occ, uint32_t flag);
    uint32_t (*network_flags)(void* user, uint64_t occ);
    void (*occupants_in_cells)(void* user, const fc4_cell_rect* rect, fc4_token_callback cb, void* ctx);
    void (*remove_occupant)(void* user, uint64_t occ);
    /* zones, tools */
    uint32_t (*zone_type)(void* user, int32_t x, int32_t z);
    void (*power_line_tool_zone_change)(void* user, const fc4_cell_rect* rect, uint32_t data3);
    /* lots */
    uint64_t (*lot_at)(void* user, int32_t x, int32_t z);
    uint64_t (*occupant_lot)(void* user, uint64_t occ);
    uint64_t (*as_lot)(void* user, uint64_t token);
    void (*lot_bounding_rect)(void* user, uint64_t lot, fc4_cell_rect* out);
    uint32_t (*lot_state)(void* user, uint64_t lot);
    bool (*lot_needs_power)(void* user, uint64_t lot);
    bool (*lot_has_power)(void* user, uint64_t lot);
    void (*set_lot_has_power)(void* user, uint64_t lot, bool on);
    void (*set_lot_condition)(void* user, uint64_t lot, uint32_t condition);
    void (*demolish_lots)(void* user, const uint64_t* lots, size_t count, uint32_t flags);
    void (*add_radiation)(void* user, const float* xyz, float strength, float radius);
    bool (*current_view_input_control)(void* user, uint32_t* id);
    void (*remove_current_view_input_control)(void* user);
    /* neighbours */
    void (*find_deals)(void* user, uint32_t deal_type, fc4_power_deal_callback cb, void* ctx);
    int32_t (*deal_amount_used)(void* user, uint64_t deal);
    void (*set_deal_amount_used)(void* user, uint64_t deal, int32_t used);
    void (*set_deal_flags)(void* user, uint64_t deal, uint32_t flags, bool on);
    bool (*deal_type)(void* user, uint64_t token, uint32_t* out);
    void (*mark_deals_dirty)(void* user);
    void (*connections_of_type)(void* user, uint32_t type, fc4_token_callback cb, void* ctx);
    void (*connections_to_city)(void* user, uint32_t type, uint64_t city, fc4_token_callback cb, void* ctx);
    uint64_t (*find_unvisited_connection)(void* user, uint32_t type, uint64_t city);
    uint64_t (*connection_on_other_side)(void* user, uint64_t conn);
    void (*connection_cell)(void* user, uint64_t conn, int32_t* x, int32_t* z);
    void (*set_connection_visited)(void* user, uint64_t conn, bool on);
    bool (*connection_is_active)(void* user, uint64_t conn);
    void (*set_connection_active)(void* user, uint64_t conn, bool on);
    /* budget */
    uint64_t (*department)(void* user, uint32_t id);
    uint64_t (*create_department)(void* user, uint32_t id, uint32_t group);
    void (*set_department_name)(void* user, uint64_t dept, uint32_t group, uint32_t instance);
    void (*set_department_max_funding)(void* user, uint64_t dept, float fraction);
    float (*department_funding)(void* user, uint64_t dept);
    int64_t (*budget_funding)(void* user, uint64_t dept);
    uint64_t (*create_line_item)(void* user, uint64_t dept, uint32_t line, bool locally_funded);
    uint64_t (*create_line_item_for_building_type)(void* user, uint64_t dept, uint32_t line, bool flag);
    uint64_t (*line_item)(void* user, uint64_t dept, uint32_t line);
    void (*set_line_item_name)(void* user, uint64_t item, uint32_t group, uint32_t instance);
    void (*set_line_item_display_flag)(void* user, uint64_t item, uint32_t flag, bool on);
    int64_t (*line_item_secondary_info)(void* user, uint64_t item);
    void (*set_line_item_secondary_info)(void* user, uint64_t item, int64_t value);
    void (*set_line_item_full_expenses)(void* user, uint64_t item, int64_t value);
    bool (*budget_item_for_purpose)(void* user, uint64_t occ, uint32_t purpose, fc4_power_budget_item* out);
    void (*change_budget_item_line)(void* user, uint64_t occ, uint32_t index, uint32_t line);
    void (*change_budget_item_cost)(void* user, uint64_t occ, uint32_t index, int64_t cost);
    void (*add_locally_funded_object)(void* user, uint64_t dept, uint64_t occ, uint32_t purpose);
    void (*remove_locally_funded_object)(void* user, uint64_t dept, uint64_t occ, uint32_t purpose);
    bool (*is_locally_funded_object)(void* user, uint64_t dept, uint64_t occ, uint32_t purpose);
    bool (*set_local_funding_percent)(void* user, uint64_t dept, uint64_t occ, float fraction, uint32_t purpose);
    void (*set_local_full_funding)(void* user, uint64_t dept, uint64_t occ, int64_t amount, uint32_t purpose);
} fc4_power_host;

/* The host table is copied; `token` is the simulator's own host identity. NULL
 * on a short table. */
fc4_power* fc4_power_create(const fc4_power_host* host, uint64_t token);
void fc4_power_destroy(fc4_power* p);
bool fc4_power_init(fc4_power* p);                                   /* §4.1 */
bool fc4_power_shutdown(fc4_power* p);                               /* §4.2 */
bool fc4_power_do_message(fc4_power* p, const fc4_power_message* m); /* §4.3, §5: always true */
/* The argument-free numeric slots 9, 11-15, 17, 21, 22, 24, 26-29, 39, 41-44. */
int64_t fc4_power_slot_value(const fc4_power* p, uint32_t slot);
void fc4_power_position_to_cell(const fc4_power* p, float x, float z, int32_t* cx, int32_t* cz); /* 5 */
void fc4_power_cell_to_position(const fc4_power* p, int32_t cx, int32_t cz, float* x, float* z); /* 6 */
int32_t fc4_power_annual_consumption_by_type(const fc4_power* p, int32_t type);                  /* 10 */
int32_t fc4_power_consumption_at(const fc4_power* p, int32_t x, int32_t z);                       /* 16 */
int32_t fc4_power_plant_count_of_type(const fc4_power* p, int32_t type);                           /* 23 */
/* 25: every plant with one reference for the callback (released if cb is NULL). */
size_t fc4_power_plants(const fc4_power* p, fc4_token_callback cb, void* ctx);
bool fc4_power_find_plants(const fc4_power* p, fc4_power_found* out);                          /* 30 */
bool fc4_power_is_occupant_powered(const fc4_power* p, uint64_t occ);                           /* 31 */
bool fc4_power_occupant_consumed(const fc4_power* p, uint64_t occ, uint32_t* out);              /* 32 */
bool fc4_power_occupant_produced(const fc4_power* p, uint64_t occ, uint32_t* produced, uint32_t* used); /* 33 */
bool fc4_power_occupant_max_produced(const fc4_power* p, uint64_t occ, uint32_t* out);          /* 34 */
uint64_t fc4_power_powered_grid(const fc4_power* p); /* 20, 35, 36: borrowed token, 0 = none */
bool fc4_power_is_powered(const fc4_power* p, int32_t x, int32_t z);                             /* 19, 37 */
bool fc4_power_is_wired(const fc4_power* p, int32_t x, int32_t z);                               /* 38 */
float fc4_power_chance_of_strike(const fc4_power* p);                                             /* 40 */
float fc4_power_plant_efficiency(const fc4_power* p, uint64_t occ);                               /* 45 */
float fc4_power_plant_condition(const fc4_power* p, uint64_t occ);                                /* 46 */
float fc4_power_strike_efficiency(const fc4_power* p);                                            /* 47 */
uint64_t fc4_power_department(const fc4_power* p); /* 48 forwards to it; borrowed, 0 = none */
bool fc4_power_generation_cost(const fc4_power* p, uint64_t occ, int64_t* out);                 /* 49 */
bool fc4_power_plant_funding(const fc4_power* p, uint64_t occ, float* out);                     /* 50 */
bool fc4_power_set_plant_funding(const fc4_power* p, uint64_t occ, float fraction);             /* 51 */
bool fc4_power_get_state(const fc4_power* p, fc4_power_state* out);
/* Tooling: the recompute report (one line per cycle, then one per plant and per
 * flood). Collected only while switched on; take delivers the last complete
 * cycle's text (UTF-8, lines separated by '\n') once and returns false when
 * there is none. */
void fc4_power_set_report(const fc4_power* p, bool on);
bool fc4_power_take_report(const fc4_power* p, fc4_bytes_sink sink, void* ctx);
bool fc4_power_write(const fc4_power* p, const fc4_ostream* out); /* §9 */
bool fc4_power_read(fc4_power* p, const fc4_istream* in);         /* §9: false only for version > 6 */

/* ---- power shadow (tooling; engine/capi/src/power_shadow.rs) -----------------
 *
 * With the ORIGINAL power simulator running, each of its SimNewMonth
 * deliveries and each SimNewDay / SimIdle delivery that does work (one DoIdle
 * step, §6) is wrapped: before it the plugin captures the original's §9.1
 * record (its own Write into memory: primitives little-endian, references as
 * u32 index + 1 (0 = null) then u32 class ID, each object listed once with its
 * tokens) and its slots 27-29, and fc4_power_shadow_begin copies the live
 * powered grid and runs FreeCity4's step on the record with every input read
 * through a read-only host table (write callbacks NULL; grids: geometry and get
 * only); a month also rebuilds the bookkeeping from scratch (§5.1 for every
 * live occupant, a wire rebuild of the map). After the delivery the plugin
 * captures again and fc4_power_shadow_finish compares the record's fields and
 * bitmaps, the consumption map, the powered grid cell by cell, the counts and
 * the occupant properties, deal usage, connection flags and budget item costs
 * FreeCity4 wrote (kept in the shadow) with the game's. The unsaved values
 * the step needs are mirrored: known from a city start
 * (FC4_POWER_SHADOW_CITY_START), the dirty rectangle merged from the messages
 * the plugin forwards (fc4_power_shadow_message). Not compared (not
 * readable): posted messages, history, budget line items, demolitions, lot
 * conditions, the poles the month's shuffle picks (the count is compared),
 * deal flags. */
typedef struct fc4_power_shadow fc4_power_shadow;

/* fc4_power_capture_ref.flags: the object answers cISC4PowerPoleOccupant
 * (0xA8C86142); the pole is known by `occupant`. */
#define FC4_POWER_CAPTURE_POLE 1u

typedef struct fc4_power_capture_ref {
    uint64_t occupant;   /* cISC4Occupant 0x26BB767E (plants, poles); borrowed; 0 = none */
    uint64_t grid;       /* cISC4SimGrid 0xC9BD939A */
    uint64_t department; /* cISC4DepartmentBudget 0x223E2B67 */
    uint32_t class_id;   /* GetGZCLSID() */
    uint32_t flags;      /* FC4_POWER_CAPTURE_POLE */
} fc4_power_capture_ref;

typedef struct fc4_power_capture {
    uint32_t struct_size;
    uint32_t ok; /* non-zero: Write returned true and the stream stayed good */
    const uint8_t* bytes;
    size_t len;
    const fc4_power_capture_ref* refs;
    size_t ref_count;
} fc4_power_capture;

/* The original's unsaved plant counts: slots 27, 28, 29. */
typedef struct fc4_power_shadow_counts {
    uint32_t struct_size;
    uint32_t ok; /* non-zero: read */
    uint32_t overworked, aged, distressed;
    uint32_t reserved;
} fc4_power_shadow_counts;

/* fc4_power_shadow_options.flags: created at the city's PostCityInit (a load
 * or a new city), when the original was just constructed and initialised: its
 * unsaved pass values and brokenFraction are 0, it has no dirty rectangle and
 * its ordinance factor is the one read at creation (power.md §5.3, §7.6, §8,
 * §9.1). Without it (a shadow started mid-city) they are unknown until the
 * original shows them. (ABI 33; the field was `reserved`.) */
#define FC4_POWER_SHADOW_CITY_START 1u

typedef struct fc4_power_shadow_options {
    uint32_t struct_size;
    uint32_t max_cells; /* differing cells listed per line (0 = 8, at most 64) */
    uint32_t max_lines; /* cycle lines with a difference per month (0 = 32; UINT32_MAX = all) */
    uint32_t flags;     /* FC4_POWER_SHADOW_CITY_START */
    /* cISC4NeighborConnection::GetVisitedThisCycle (slot 11) of a connection
     * token, called with the host table's `user`: the shadow emulates
     * FindUnvisitedConnection with it. NULL: the live FindUnvisitedConnection. */
    bool (*connection_visited)(void* user, uint64_t connection);
} fc4_power_shadow_options;

typedef struct fc4_power_shadow_stats {
    uint32_t struct_size;
    uint32_t deliveries, idle_skipped, steps, cycles, months, clean, incomplete, reruns, not_enabled;
    int32_t last_mismatches; /* -1: not compared */
    uint32_t uncached_reads;
    uint64_t mismatches, not_exact, bookkeeping_mismatches;
} fc4_power_shadow_stats;

/* `options` may be NULL (defaults). NULL when the table is short or the grid
 * table has no geometry / get. */
fc4_power_shadow* fc4_power_shadow_create(const fc4_power_host* host, const fc4_power_shadow_options* options);
void fc4_power_shadow_destroy(fc4_power_shadow* s);
/* Whether to wrap this delivery: every SimNewMonth and SimNewDay; a SimIdle
 * unless the original was idle after the last wrapped delivery (state 4, no
 * recompute) and `event` is false (`event`: one of the original's §5
 * messages arrived since). A delivery passed through is counted. */
bool fc4_power_shadow_wants(fc4_power_shadow* s, uint32_t msg_type, bool event);
/* 0 = ran, -1 = not run (finish reports why), -2 = not a day, idle or month.
 * `counts` may be NULL. */
int32_t fc4_power_shadow_begin(fc4_power_shadow* s, uint32_t msg_type, int32_t julian,
                               const fc4_power_capture* before, const fc4_power_shadow_counts* counts);
/* The delivery's difference count (-1 = not compared); log lines go to `sink`,
 * one call per line (each ends in "mismatches=N"): the cycle line at the end
 * of a recompute cycle (capped per month), the month, bookkeeping and month
 * summary lines after a SimNewMonth, INCOMPLETE lines (at most 3 a month). */
int32_t fc4_power_shadow_finish(fc4_power_shadow* s, const fc4_power_capture* after,
                                const fc4_power_shadow_counts* counts, fc4_bytes_sink sink, void* ctx);
bool fc4_power_shadow_get_stats(fc4_power_shadow* s, fc4_power_shadow_stats* out);
/* One of the original's messages as it is delivered to the original (the
 * plugin's message target, inside or outside a wrapped delivery): a removal
 * (void1 = the occupant's cISC4Occupant token) or a zoning change (rect)
 * merges the rectangle the original merges into its dirty rectangle (§5.3,
 * PW5); a tuning reload re-reads the tuning and the ordinance factor (§8,
 * PW7). Reads go through the host table at once. True for these messages. */
bool fc4_power_shadow_message(fc4_power_shadow* s, const fc4_power_message* m); /* ABI 33 */

/* ---- plumbing (docs/specs/plumbing.md) ---------------------------------------
 *
 * Tokens: occupants (also pipes and water producers: the cISC4Occupant
 * identity), lots, deals, connections, cities, departments, line items; 0 is
 * null. Both grids are the game's own cSC4SimGrid<uint8> (fc4_grid_api).
 * A message delivered while another call into the same object is running is
 * queued and handled when that call returns. */
typedef struct fc4_plumbing fc4_plumbing;

/* GetBudgetItemForPurpose (slot 51) as the budget reports it. */
typedef struct fc4_plumbing_budget_item {
    uint32_t department;
    uint32_t line;
    int64_t cost;
    uint32_t index;
    uint32_t reserved;
} fc4_plumbing_budget_item;

/* A message for fc4_plumbing_do_message (§5.1). `occupant` is GetVoid1() as a
 * cISC4Occupant (Insert / RemoveOccupant); `deal` is GetIGZUnknown() queried
 * for cISC4NeighborDeal (deal status), 0 when it is null or not a deal. */
typedef struct fc4_plumbing_message {
    uint32_t msg_type;
    uint32_t data1, data2, data3;
    uint32_t reserved;
    uint64_t occupant;
    uint64_t deal;
} fc4_plumbing_message;

/* GetWaterProducerInfo outputs (slot 24). */
typedef struct fc4_plumbing_producer_info {
    uint32_t production;
    uint32_t max_capacity;
    float efficiency;
    bool polluted;
    uint32_t used; /* property 0x08D9763D, 0 if absent */
} fc4_plumbing_producer_info;

/* FindWaterBuildings results (slot 27). Each occupant is a BORROWED token (the
 * core's producer list holds the references); the plugin AddRefs one it writes
 * to a non-null output pointer. */
typedef struct fc4_plumbing_found {
    uint32_t has_lowest_funded;
    float lowest_funding;
    uint64_t lowest_funded;
    uint32_t has_worst_condition;
    int32_t condition_percent;
    uint64_t worst_condition;
} fc4_plumbing_found;

/* Diagnostics. */
typedef struct fc4_plumbing_state {
    uint32_t struct_size;
    uint8_t initialized, grids_initialized, enabled, recompute;
    uint32_t state; /* 0..9, 9 = idle */
    int32_t count_x, count_z;
    int32_t pass_counter;
    uint32_t producers, num_pipes, num_distressed, num_burst, num_polluted;
    uint32_t average_water_pollution;
    uint32_t watered_cells, building_cells;
    uint32_t reentrant_calls;
    uint32_t reserved;
    float pipe_condition, factor;
    int64_t demand, total_water_cost;
    int64_t last_produced, last_consumed, last_imported, last_exported;
    uint64_t department;
    uint64_t working_grid;
    uint64_t published_grid;
} fc4_plumbing_state;

/* What the plumbing simulator needs from the game (fc4.h: one callback per
 * PlumbingHost method, engine/core/src/plumbing/mod.rs, plumbing.md §11).
 * Missing callbacks behave as "nothing there" (0, false, no object). */
typedef struct fc4_plumbing_host {
    uint32_t struct_size; /* sizeof(fc4_plumbing_host) */
    void* user;
    fc4_grid_api grids;
    /* city */
    void (*cell_counts)(void* user, int32_t* x, int32_t* z);
    void (*position_to_cell)(void* user, float x, float z, int32_t* cx, int32_t* cz);
    fc4_props* (*tuning)(void* user); /* a NEW fc4_props (the core destroys it), NULL if missing */
    /* The session seed S (foundations §6.3): the generator made at Init is
     * seeded with derive(S, 0x0990C075), the class's GZCLSID (ABI 64; before,
     * this was the wall-clock value itself). */
    uint32_t (*session_seed)(void* user);
    /* simulator, messages, history */
    void (*add_agent)(void* user, uint32_t level, const char* name, size_t name_len, uint32_t flags);
    void (*remove_agent)(void* user);
    void (*subscribe)(void* user, const uint32_t* ids, size_t count);
    void (*unsubscribe)(void* user, const uint32_t* ids, size_t count);
    /* Posts a standard message: WATER_MAP_CHANGED (0x6946DC34) carries
     * void1 = IGZUnknown = the simulator; DEAL_STATUS (0xEA418D8F) carries
     * data1 (§6.7 step 9). */
    void (*post)(void* user, uint32_t msg_type, uint32_t data1);
    void (*record_history)(void* user, uint32_t id, uint32_t value);
    /* false = no ordinance simulator (the core then reads 1.0). */
    bool (*cumulative_factor)(void* user, uint32_t id, uint32_t index, float* out);
    int32_t (*water_value)(void* user, int32_t x, int32_t z);
    /* occupants */
    uint32_t (*occupant_type)(void* user, uint64_t occ);
    bool (*occupant_property)(void* user, uint64_t occ, uint32_t id, fc4_props* out);
    void (*set_occupant_property)(void* user, uint64_t occ, uint32_t id, uint16_t mem_type, bool is_array,
                                  const void* data, size_t count);
    void (*occupant_position)(void* user, uint64_t occ, float* xyz);
    void (*occupant_bounding_box)(void* user, uint64_t occ, float* min_xyz, float* max_xyz);
    void (*occupant_bounding_cells)(void* user, uint64_t occ, fc4_cell_rect* out);
    bool (*building_type)(void* user, uint64_t occ, uint32_t* out);
    void (*buildings_in_cell)(void* user, int32_t x, int32_t z, fc4_token_callback cb, void* ctx);
    /* network occupants (pipes) */
    bool (*is_network_occupant)(void* user, uint64_t occ);
    bool (*has_network_flag)(void* user, uint64_t occ, uint32_t flag);
    uint32_t (*network_flags)(void* user, uint64_t occ);
    void (*set_network_flag)(void* user, uint64_t occ, uint32_t flag);
    void (*clear_network_flag)(void* user, uint64_t occ, uint32_t flag);
    bool (*connects_to_neighbor)(void* user, uint64_t occ, uint32_t dir, uint32_t network_type);
    void (*occupied_cell)(void* user, uint64_t occ, int32_t* x, int32_t* z);
    /* The direct call to cSC4PipeOccupant::SetWateredState (docs/abi/direct-calls.md). */
    void (*set_pipe_watered)(void* user, uint64_t occ, bool watered);
    /* lots */
    uint64_t (*lot_at)(void* user, int32_t x, int32_t z);
    uint64_t (*occupant_lot)(void* user, uint64_t occ);
    void (*lot_rect)(void* user, uint64_t lot, fc4_cell_rect* out);
    bool (*lot_needs_power)(void* user, uint64_t lot);
    bool (*lot_has_power)(void* user, uint64_t lot);
    void (*set_lot_condition)(void* user, uint64_t lot, uint8_t condition);
    /* terrain */
    bool (*terrain_cell_counts)(void* user, int32_t* x, int32_t* z); /* false = no terrain */
    bool (*cell_has_water)(void* user, int32_t x, int32_t z);
    void (*redisplay_terrain)(void* user, int32_t min_x, int32_t min_z, int32_t max_x, int32_t max_z);
    /* disasters: false = the pipe-burst manager is missing (§7.1). */
    bool (*attempt_pipe_burst)(void* user, int32_t x, int32_t z);
    /* neighbours */
    void (*find_deals)(void* user, uint32_t deal_type, fc4_token_callback cb, void* ctx);
    uint32_t (*deal_type)(void* user, uint64_t deal);
    uint64_t (*deal_city)(void* user, uint64_t deal);
    int32_t (*deal_amount)(void* user, uint64_t deal);
    int32_t (*deal_amount_used)(void* user, uint64_t deal);
    void (*set_deal_amount_used)(void* user, uint64_t deal, int32_t used);
    void (*set_deal_flags)(void* user, uint64_t deal, uint32_t flags, bool on);
    void (*connections_of_type)(void* user, uint32_t connection_type, fc4_token_callback cb, void* ctx);
    void (*connections_of_type_to_city)(void* user, uint32_t connection_type, uint64_t city, fc4_token_callback cb,
                                        void* ctx);
    uint64_t (*find_unvisited_connection)(void* user, uint32_t connection_type, uint64_t city);
    uint64_t (*connection_on_other_side)(void* user, uint64_t conn);
    void (*connection_cell)(void* user, uint64_t conn, int32_t* x, int32_t* z);
    void (*set_connection_visited)(void* user, uint64_t conn, bool visited);
    bool (*connection_is_active)(void* user, uint64_t conn);
    void (*set_connection_active)(void* user, uint64_t conn, bool active);
    void (*mark_deals_dirty)(void* user);
    /* budget */
    uint64_t (*create_department)(void* user, uint32_t id, uint32_t group);
    void (*set_department_name)(void* user, uint64_t dept, uint32_t group, uint32_t instance);
    void (*set_department_max_funding)(void* user, uint64_t dept, float fraction);
    uint64_t (*create_line_item)(void* user, uint64_t dept, uint32_t line, bool locally_funded);
    void (*set_line_item_name)(void* user, uint64_t line_item, uint32_t group, uint32_t instance);
    void (*remove_line_item)(void* user, uint64_t dept, uint32_t line);
    float (*department_funding)(void* user, uint64_t dept);
    uint64_t (*department_by_id)(void* user, uint32_t id);
    uint64_t (*line_item)(void* user, uint64_t dept, uint32_t line);
    uint64_t (*create_line_item_for_building_type)(void* user, uint64_t dept, uint32_t line, bool locally_funded);
    void (*set_line_item_display_flag)(void* user, uint64_t line_item, uint32_t flag, bool on);
    void (*add_to_line_item_expenses)(void* user, uint64_t line_item, int64_t delta);
    void (*set_line_item_expenses)(void* user, uint64_t line_item, int64_t value);
    int64_t (*line_item_secondary_info)(void* user, uint64_t line_item);
    void (*set_line_item_secondary_info)(void* user, uint64_t line_item, int64_t value);
    bool (*budget_item_for_purpose)(void* user, uint64_t occ, uint32_t purpose, fc4_plumbing_budget_item* out);
    void (*change_budget_item_line)(void* user, uint64_t occ, uint32_t index, uint32_t line);
    void (*change_budget_item_cost)(void* user, uint64_t occ, uint32_t index, int64_t cost);
    uint64_t (*line_item_from_budget_item)(void* user, const fc4_plumbing_budget_item* item);
    bool (*local_funding)(void* user, uint64_t dept, uint64_t occ, float* out);
    /* references */
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    /* A reference read from a save (QI'd to iid, one reference) to the object's
     * canonical token (one reference); 0 = dropped. */
    uint64_t (*resolve_ref)(void* user, uint64_t token, uint32_t iid);
} fc4_plumbing_host;

fc4_plumbing* fc4_plumbing_create(const fc4_plumbing_host* host, uint64_t token);
void fc4_plumbing_destroy(fc4_plumbing* p);
bool fc4_plumbing_init(fc4_plumbing* p);          /* §4.1 */
bool fc4_plumbing_shutdown(fc4_plumbing* p);      /* §4.2 */
/* §5.1: true except for the day and idle messages, which return DoIdle's result.
 * A message delivered during another call is queued. */
bool fc4_plumbing_do_message(fc4_plumbing* p, const fc4_plumbing_message* m);
/* The argument-free slots that return a number: 10 (with an occupant: use
 * fc4_plumbing_water_production), 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
 * 23. Slots 14-17 and 23 return the low 32 bits of a 64-bit total (§10.4). */
int64_t fc4_plumbing_slot_value(const fc4_plumbing* p, uint32_t slot);
uint8_t fc4_plumbing_supply_value(const fc4_plumbing* p, int32_t x, int32_t z);        /* 5 */
bool fc4_plumbing_are_watered_buildings_in_cell(const fc4_plumbing* p, int32_t x, int32_t z); /* 8 */
bool fc4_plumbing_is_building_watered(const fc4_plumbing* p, uint64_t occ);           /* 9 */
uint32_t fc4_plumbing_water_production(const fc4_plumbing* p, uint64_t occ);          /* 10 */
bool fc4_plumbing_is_cell_watered(const fc4_plumbing* p, int32_t x, int32_t z);       /* 11 */
/* Slot 24: false = the occupant is not a producer (outputs untouched). */
bool fc4_plumbing_get_producer_info(const fc4_plumbing* p, uint64_t occ, fc4_plumbing_producer_info* out);
/* Slot 25: false = the condition or its curve is absent (output untouched). */
bool fc4_plumbing_producer_condition_percent(const fc4_plumbing* p, uint64_t occ, float* out);
/* Slot 26: false = no budget item for the purpose (output untouched). */
bool fc4_plumbing_occupant_monthly_upkeep_cost(const fc4_plumbing* p, uint64_t occ, int64_t* out);
bool fc4_plumbing_find_water_buildings(const fc4_plumbing* p, fc4_plumbing_found* out); /* 27 */
/* Slot 28: every producer occupant in list order, each AddRef'd for the caller.
 * Without a callback the references are given back. */
size_t fc4_plumbing_water_producers(const fc4_plumbing* p, fc4_token_callback cb, void* ctx);
uint64_t fc4_plumbing_watered_grid(const fc4_plumbing* p); /* 6, 7: borrowed token, 0 = none */
bool fc4_plumbing_get_state(const fc4_plumbing* p, fc4_plumbing_state* out);
bool fc4_plumbing_write(const fc4_plumbing* p, const fc4_ostream* out); /* §9 */
bool fc4_plumbing_read(fc4_plumbing* p, const fc4_istream* in);         /* §9: false for version != 4 */

/* ---- plumbing shadow (tooling; engine/capi/src/plumbing_shadow.rs) ----------
 *
 * With the ORIGINAL plumbing simulator running, every SimIdle / SimNewDay
 * (one DoIdle: one pass state, §6) and SimNewMonth delivery to its agent is
 * wrapped by fc4_plumbing_shadow_before / _after. The shadow follows the
 * original's water pass from outside: it sees state 0 run (its passCounter,
 * read through `pass_counter`, moves; PS6; without it, a probe cell of the
 * next working grid changes in state 2), captures the original's record (its
 * own Write, through `capture`) after state 1 and runs FreeCity4's pass on it
 * over private grid copies, reading every input through the host table (read
 * callbacks and the grid table's geometry / get only; nothing is written to
 * the game). Our states 2 to 5 run one per delivery, each in the
 * fc4_plumbing_shadow_before of the delivery in which the original runs the
 * same state, on the original's flag map of that moment (from its record), so
 * they read what the original reads; a pass has no time limit (it is followed
 * until its swap or its restart). At the original's swap (GetWateredGrid
 * changes) it compares the published grid cell by cell, the producers, deals
 * and water connections;
 * after the original's state 8, the totals, the building counts, demand (the
 * original's running sum, read then; PS4, PS8), pipes, sources and the
 * producers' budget items and condition (ageing inferred from the conditions
 * when unknown; PS7). One line per pass. The month (ApplyPipeFunding, §7.1)
 * is compared on its own with the funding of the record's department (PS5):
 * the distressed target exactly when the shadow was made at the city start
 * (fc4_plumbing_shadow_city_start; the original's pipe condition is then 0,
 * PS3), otherwise not exact; the random bursts by range. Not compared (not
 * readable): pipe watered states, redisplay, posts, history, line-item
 * expenses, lot conditions, deal flags, the original's pipe condition and
 * pass state. */
typedef struct fc4_plumbing_shadow fc4_plumbing_shadow;

/* One reference of a captured record, queried for each interface the reader
 * asks for (0 = the object does not answer it); borrowed identities. */
typedef struct fc4_plumbing_capture_ref {
    uint64_t grid;       /* cISC4SimGrid 0xC9BD939A */
    uint64_t occupant;   /* cISC4Occupant 0x26BB767E */
    uint64_t network;    /* the pipe 0x49C05B9F (only a pipe answers it) */
    uint64_t department; /* cISC4DepartmentBudget 0x223E2B67 */
    uint32_t class_id;   /* GetGZCLSID() */
    uint32_t reserved;
} fc4_plumbing_capture_ref;

/* A captured record: primitives little-endian, references as uint32
 * objectID (index into `refs` + 1, 0 = null) then uint32 classID. */
typedef struct fc4_plumbing_capture {
    uint32_t struct_size;
    uint32_t ok; /* Write returned true and the stream stayed good */
    const uint8_t* bytes;
    size_t len;
    const fc4_plumbing_capture_ref* refs;
    size_t ref_count;
} fc4_plumbing_capture;

/* The original simulator, read-only. */
typedef struct fc4_plumbing_original {
    uint32_t struct_size; /* sizeof(fc4_plumbing_original) */
    void* user;
    uint64_t (*watered_grid)(void* user); /* GetWateredGrid (slot 7): borrowed grid token, 0 = none */
    /* Slots 12-23: 14-17 and 23 as their signed 32-bit EAX, the others
     * zero-extended. */
    int64_t (*slot_value)(void* user, uint32_t slot);
    bool (*producer_info)(void* user, uint64_t occupant, fc4_plumbing_producer_info* out); /* slot 24 */
    bool (*connection_visited)(void* user, uint64_t connection); /* GetVisitedThisCycle; may be NULL */
    /* The original's own Write into memory; the view stays valid until the
     * next call. */
    bool (*capture)(void* user, fc4_plumbing_capture* out);
    /* ABI 33: the original's passCounter (record field 7, written only by
     * state 0; plumbing.md §6.1, PS6) through its own Write, references not
     * looked at; false when it failed. May be NULL: a pass start is then seen
     * on a probe cell. */
    bool (*pass_counter)(void* user, int32_t* out);
} fc4_plumbing_original;

typedef struct fc4_plumbing_shadow_stats {
    uint32_t struct_size;
    uint32_t deliveries;
    uint32_t passes_seen;       /* swaps seen */
    uint32_t passes_compared;   /* pass lines */
    uint32_t passes_clean;      /* exact, compared to the end, no difference */
    uint32_t passes_not_exact;  /* the city changed while the pass ran: differences not counted */
    uint32_t passes_incomplete; /* not compared, or the end not seen */
    uint32_t months, months_clean;
    int32_t last_mismatches; /* -1: nothing compared yet */
    uint32_t captures;
    uint32_t reserved;
    uint64_t cells, cell_mismatches, scalar_mismatches;
} fc4_plumbing_shadow_stats;

/* `host`: only its read callbacks and the grid table's geometry / get are
 * called (write callbacks may be NULL). `orig`: watered_grid, slot_value and
 * capture are required. `max_cells`: differing cells listed per line (0 = 8,
 * at most 64). `passes_per_day`: passes replayed per game day (0 = every
 * pass); the others are only followed. NULL for a missing or short table. */
fc4_plumbing_shadow* fc4_plumbing_shadow_create(const fc4_plumbing_host* host, const fc4_plumbing_original* orig,
                                                uint32_t max_cells, uint32_t passes_per_day);
void fc4_plumbing_shadow_destroy(fc4_plumbing_shadow* s);
/* Before the original's SimIdle / SimNewDay / SimNewMonth (`julian` = data1):
 * 0; -2 for any other message type (nothing done). */
int32_t fc4_plumbing_shadow_before(fc4_plumbing_shadow* s, uint32_t msg_type, int32_t julian);
/* After it: log lines go to `sink`, one call per line (a pass line at the end
 * of a replayed pass, INCOMPLETE lines, the month line). Returns the counted
 * differences of a comparison finished in this delivery, -1 if none. */
int32_t fc4_plumbing_shadow_after(fc4_plumbing_shadow* s, fc4_bytes_sink sink, void* ctx);
bool fc4_plumbing_shadow_get_stats(fc4_plumbing_shadow* s, fc4_plumbing_shadow_stats* out);
/* A shadow made at the city start (PostCityInit), before its first delivery:
 * the original's pipe condition and ageing flag are its constructor's (0,
 * false) after every load (plumbing.md §7.1, PS3), so the month's distressed
 * target is compared exactly. False for NULL or after a delivery. (ABI 33) */
bool fc4_plumbing_shadow_city_start(fc4_plumbing_shadow* s);

/* ---- flammability (docs/specs/flammability.md) ------------------------------
 *
 * Tokens: occupants and lots (the cISC4Occupant / cISC4Lot identity); 0 is
 * null. Both grids are the game's own cSC4SimGrid<uint8> (fc4_grid_api): the
 * map view walks the adjusted grid's memory through GetGridData (§13.2).
 * A message delivered while another call into the same object is running is
 * queued and handled when that call returns. */
typedef struct fc4_flammability fc4_flammability;

/* A message for fc4_flammability_do_message (§5). `occupant` is GetVoid1() as
 * a cISC4Occupant (Insert / Remove / ChangeOccupant); `lot` is GetIGZUnknown()
 * queried for cISC4Lot (ConditionChanged), 0 when it is null or not a lot. */
typedef struct fc4_flammability_message {
    uint32_t msg_type;
    uint32_t reserved;
    uint64_t occupant;
    uint64_t lot;
} fc4_flammability_message;

/* Diagnostics. */
typedef struct fc4_flammability_state {
    uint32_t struct_size;
    uint8_t initialized;
    uint8_t pad0[3];
    int32_t count_x, count_z;
    uint8_t average;
    uint8_t pad1[3];
    float global_multiplier;
    int32_t zoned_sum, zoned_count;
    uint32_t candidates, hotspots;
    int32_t tract_size, cycle_period, tracts_per_day;
    uint32_t reserved;
    uint64_t adjusted_grid;
    uint64_t raw_grid;
    uint32_t reentrant_calls;
    uint32_t reserved2;
} fc4_flammability_state;

/* What the flammability simulator needs from the game (fc4.h: one callback per
 * FlammabilityHost method, engine/core/src/flammability/mod.rs, §12). Missing
 * callbacks behave as "nothing there" (0, false, no object). The agent
 * registration and the six notifications of §4.1 steps 2-3 are the plugin's,
 * not callbacks. */
typedef struct fc4_flammability_host {
    uint32_t struct_size; /* sizeof(fc4_flammability_host) */
    void* user;
    fc4_grid_api grids;
    /* city */
    void (*city_cell_counts)(void* user, int32_t* x, int32_t* z);
    void (*position_to_cell)(void* user, float x, float z, int32_t* cx, int32_t* cz);
    bool (*is_tutorial)(void* user);
    fc4_props* (*tuning)(void* user); /* a NEW fc4_props (the core destroys it), NULL if missing */
    /* occupants */
    uint32_t (*occupant_type)(void* user, uint64_t occ);
    bool (*occupant_property)(void* user, uint64_t occ, uint32_t id, fc4_props* out);
    void (*occupant_bounds)(void* user, uint64_t occ, fc4_cell_rect* out);
    void (*occupant_position)(void* user, uint64_t occ, float* xyz);
    void (*occupants_in_cells)(void* user, const fc4_cell_rect* rect, fc4_token_callback cb, void* ctx);
    /* lots */
    uint64_t (*occupant_lot)(void* user, uint64_t occ);
    uint8_t (*lot_condition)(void* user, uint64_t lot);
    uint64_t (*lot_building)(void* user, uint64_t lot);
    uint64_t (*random_lot)(void* user);
    /* other subsystems; the bool/int32 returns encode the core's Option: a
     * missing service reads "not watered" / "unzoned" / factor 1.0 (§4.1 step 5
     * [choice]). */
    bool (*cumulative_factor)(void* user, uint32_t prop, uint32_t index, float* out);
    bool (*current_season)(void* user, int32_t* out);
    /* -1 = no plumbing simulator, 0 = not watered, 1 = watered. */
    int32_t (*is_cell_watered)(void* user, int32_t x, int32_t z);
    bool (*zone_type)(void* user, int32_t x, int32_t z, uint32_t* out);
    void (*record_history)(void* user, uint32_t history_type, int32_t value);
    /* fire (§6.8): false = the fire manager or its interface is missing. */
    bool (*ignite_cell)(void* user, uint32_t x, uint32_t z, bool use_probability, bool teleport, uint32_t fire_iid);
    void (*set_fire_cause)(void* user, uint32_t cause);
    /* messages: MessagePost with void1 = IGZUnknown = the simulator (§9). */
    void (*post)(void* user, uint32_t msg_type);
    /* Plumbing slot 7 GetWateredGrid(): the published watered grid as a
     * token (0 = none). Read by the flammability shadow only: non-NULL lets
     * it poll the watered state (by content, FL7, FL9; the pointer is only
     * counted); NULL = not polled. */
    uint64_t (*watered_grid)(void* user);
} fc4_flammability_host;

/* `session_seed` is the session seed S (foundations §6.3, ABI 64): the
 * generator is seeded with 0 (§7) and its double path's clock reseed takes
 * derive(S, 0xE990C045). */
fc4_flammability* fc4_flammability_create(const fc4_flammability_host* host, uint64_t token, uint32_t session_seed);
void fc4_flammability_destroy(fc4_flammability* p);
bool fc4_flammability_init(fc4_flammability* p);      /* §4.1 */
bool fc4_flammability_shutdown(fc4_flammability* p);  /* §4.2 */
/* §5: always true for a live object. A message delivered during another call
 * is queued and handled when the outermost call returns. */
bool fc4_flammability_do_message(fc4_flammability* p, const fc4_flammability_message* m);
bool fc4_flammability_is_cell_flammable(const fc4_flammability* p, int32_t x, int32_t z); /* slot 6 */
/* Slot 7: false = no hotspot or its tract value is 0, and no output is
 * written (§13.2). */
bool fc4_flammability_hotspot(const fc4_flammability* p, uint8_t* value, int32_t* x, int32_t* z);
uint8_t fc4_flammability_average(const fc4_flammability* p); /* slot 8 */
uint64_t fc4_flammability_grid(const fc4_flammability* p);   /* slot 5: borrowed token, 0 = none */
bool fc4_flammability_get_state(const fc4_flammability* p, fc4_flammability_state* out);
bool fc4_flammability_write(const fc4_flammability* p, const fc4_ostream* out); /* §10 */
bool fc4_flammability_read(fc4_flammability* p, const fc4_istream* in);         /* §10: false for version != 2 */

/* ---- pollution (docs/specs/pollution.md) --------------------------------------
 *
 * Tokens: occupants (the cISC4Occupant identity), deals, visual effects, the
 * department; 0 is null. The eight grids are the game's own
 * cSC4SimGrid<int16> (fc4_grid_api), in save order: air, water, garbage,
 * radiation sources, then the air, water, garbage and radiation values.
 *
 * Threads: the game calls the getters from a second thread while the main
 * thread is inside a day or a month. Every function marked "any thread" reads
 * the core's locked state; the others are main-thread only. A message
 * delivered while another main-thread call into the same object is running is
 * queued and handled when the outermost call returns. */
typedef struct fc4_pollution fc4_pollution;

/* GetBudgetItemForPurpose as the budget reports it (SDK BudgetItem). */
typedef struct fc4_pollution_budget_item {
    uint32_t department;
    uint32_t line;
    uint32_t purpose;
    uint32_t index;
    int64_t cost;
    float local_funding;
    uint32_t reserved;
} fc4_pollution_budget_item;

/* One grid's statistics: tracts, non-zero tracts, sum, minimum and maximum of
 * the raw int16 values. */
typedef struct fc4_pollution_grid_stats {
    uint32_t tracts, nonzero;
    int64_t sum;
    int32_t min, max;
} fc4_pollution_grid_stats;

/* Diagnostics. */
typedef struct fc4_pollution_state {
    uint32_t struct_size;
    uint8_t initialized, loaded, garbage_uncollected, effects_visible;
    uint32_t tracts_per_day;
    uint32_t cycle_resets;    /* tract-iterator resets at the end of a cycle */
    uint32_t reentrant_calls; /* messages queued during another call */
    uint32_t lists[4];        /* water treatment, incinerators, waste-to-energy, recycling */
    int32_t avg_air, avg_water;
    uint32_t avg_garbage, total_uncollected;
    int32_t garbage_supply, garbage_capacity;
    uint32_t active_landfill_cells, developed_tracts;
    float efficiency, recycling_effect;
    uint32_t cell_x, cell_z;
    uint32_t reserved;
    /* produced, imported, exported, recycled (truncated), incinerated,
     * waste-to-energy, landfilled */
    int64_t last_month[7];
    int64_t month[7];
    int64_t last_month_cost;
    uint64_t department;
    fc4_pollution_grid_stats grids[8];
} fc4_pollution_state;

/* What the pollution simulator needs from the game: one callback per
 * PollutionHost method (engine/core/src/pollution/simulator.rs), in the
 * trait's order. Missing callbacks behave as "nothing there" (0, false, no
 * object), except cell_widths (16 m), cumulative_factor and
 * department_funding (1.0). */
typedef struct fc4_pollution_host {
    uint32_t struct_size; /* sizeof(fc4_pollution_host) */
    void* user;
    fc4_grid_api grids;
    /* city */
    void (*cell_counts)(void* user, int32_t* x, int32_t* z);
    void (*cell_widths)(void* user, float* x, float* z);
    void (*position_to_cell)(void* user, float x, float z, int32_t* cx, int32_t* cz);
    fc4_props* (*tuning)(void* user); /* a NEW fc4_props (the core destroys it), NULL if missing */
    float (*altitude)(void* user, float x, float z);
    /* simulator, messages, history */
    void (*add_agent)(void* user, uint32_t level, const char* name, size_t len, uint32_t flags);
    void (*remove_agent)(void* user);
    void (*subscribe)(void* user, const uint32_t* msg_types, size_t n);
    void (*unsubscribe)(void* user, const uint32_t* msg_types, size_t n);
    void (*post)(void* user, uint32_t msg_type, uint32_t data1); /* MessagePost; data2..4 = 0, no void / IGZUnknown (§5.2) */
    void (*record_history)(void* user, uint32_t series, int32_t value);
    void (*set_history_queue_length)(void* user, uint32_t series, uint32_t length);
    void (*add_prop_requester)(void* user);
    void (*remove_prop_requester)(void* user);
    /* other simulators */
    uint32_t (*zone_type)(void* user, int32_t x, int32_t z);
    uint8_t (*air_traffic)(void* user, int32_t x, int32_t z); /* the traffic simulator's air map */
    float (*cumulative_factor)(void* user, uint32_t id, uint32_t index);
    void (*find_deals)(void* user, uint32_t deal_type, fc4_token_callback cb, void* ctx);
    int32_t (*deal_amount)(void* user, uint64_t deal);
    int32_t (*deal_amount_used)(void* user, uint64_t deal);
    void (*set_deal_amount_used)(void* user, uint64_t deal, int32_t used);
    void (*mark_deals_dirty)(void* user);
    bool (*is_occupant_powered)(void* user, uint64_t occ);
    int32_t (*population)(void* user);
    void (*landfill_usage)(void* user, uint32_t* usage, uint32_t* capacity);
    uint32_t (*landfill_tile_count)(void* user);
    void (*set_lot_condition)(void* user, int32_t cell_x, int32_t cell_z, uint32_t condition);
    /* occupants */
    bool (*occupant_property)(void* user, uint64_t occ, uint32_t id, fc4_props* out);
    void (*set_occupant_property)(void* user, uint64_t occ, uint32_t id, uint16_t mem_type, bool is_array,
                                  const void* data, size_t count);
    void (*remove_occupant_property)(void* user, uint64_t occ, uint32_t id);
    void (*occupant_position)(void* user, uint64_t occ, float* xyz);
    void (*occupant_bounding_box)(void* user, uint64_t occ, float* min_xyz, float* max_xyz);
    /* host objects */
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    uint64_t (*resolve_ref)(void* user, uint64_t token, uint32_t iid);
    /* budget */
    uint64_t (*create_department)(void* user, uint32_t id, uint32_t group); /* borrowed; the core AddRefs */
    void (*set_department_name)(void* user, uint64_t dept, uint32_t group, uint32_t instance);
    void (*set_department_max_funding)(void* user, uint64_t dept, float fraction);
    void (*create_line_item)(void* user, uint64_t dept, uint32_t line, bool local_funding, uint32_t group,
                             uint32_t instance);
    float (*department_funding)(void* user, uint64_t dept);
    void (*set_line_item_full_expenses)(void* user, uint64_t dept, uint32_t line, int64_t value);
    bool (*budget_item_for_purpose)(void* user, uint64_t occ, uint32_t purpose, fc4_pollution_budget_item* out);
    void (*add_budget_item_to_budget)(void* user, uint64_t occ, uint32_t index);
    void (*remove_budget_item_from_budget)(void* user, uint64_t occ, uint32_t index);
    void (*change_budget_item_cost)(void* user, uint64_t occ, uint32_t index, int64_t cost);
    void (*add_to_budget_item_line_expenses)(void* user, const fc4_pollution_budget_item* item, int64_t delta);
    /* visual effects (0 = none) */
    uint64_t (*create_visual_effect)(void* user, uint32_t id, uint32_t kind);
    void (*set_effect_position)(void* user, uint64_t effect, const float* xyz);
    void (*set_effect_param)(void* user, uint64_t effect, uint32_t param, float value);
    void (*start_effect)(void* user, uint64_t effect, uint32_t t);
    void (*stop_effect)(void* user, uint64_t effect, uint32_t t);
    bool (*is_effect_started)(void* user, uint64_t effect);
} fc4_pollution_host;

/* `token`: the simulator's own host identity; `session_seed` is the session
 * seed S (foundations §6.3): the visual-effect generator is seeded with
 * derive(S, 0x4C09C4D3) (ABI 64; before, the argument was its seed). It is
 * process-wide (RN3): the first fc4_pollution_create of the process seeds it,
 * later simulators (later city loads) continue its stream. */
fc4_pollution* fc4_pollution_create(const fc4_pollution_host* host, uint64_t token, uint32_t session_seed);
/* A loaded, never-initialised object gives its list and department references back. */
void fc4_pollution_destroy(fc4_pollution* p);
bool fc4_pollution_init(fc4_pollution* p);     /* §4.1 */
bool fc4_pollution_shutdown(fc4_pollution* p); /* §4.1 */
/* §5.3: always true. `occupant`: the occupant of Insert / RemoveOccupant;
 * `view_level`: the uint32 void2 points at for PreViewLevelChange. */
bool fc4_pollution_do_message(fc4_pollution* p, uint32_t msg_type, uint64_t occupant, uint32_t view_level);
/* Any thread. Slots 10, 11, 12, 20-24, 31, 32, 34-45, 67-70 (booleans 0 / 1);
 * 0 for any other slot. */
int64_t fc4_pollution_slot_value(const fc4_pollution* p, uint32_t slot);
/* Any thread. Slots 7 / 8 / 9 (the value), 13-16 and 49 (0 / 1). */
int32_t fc4_pollution_slot_cell(const fc4_pollution* p, uint32_t slot, int32_t x, int32_t z);
uint64_t fc4_pollution_grid(const fc4_pollution* p, uint32_t kind); /* 5: borrowed token; any thread */
uint16_t fc4_pollution_value(const fc4_pollution* p, uint32_t kind, int32_t x, int32_t z); /* 6; any thread */
/* 17-19 (kind 0 / 1 / 2); any thread. */
bool fc4_pollution_worst_tract(const fc4_pollution* p, uint32_t kind, uint32_t* x, uint32_t* z, int32_t* v);
/* Any thread. 25 SetIsRadioactive (v != 0), 26-28 Set{Air,Water,Garbage}Value
 * (v as int16), 50 SetIsActiveLandfill (v != 0); false for any other slot. */
bool fc4_pollution_set_cell(fc4_pollution* p, uint32_t slot, int32_t x, int32_t z, int32_t v);
int32_t fc4_pollution_add_to_garbage_supply(fc4_pollution* p, int32_t n); /* 29 */
int32_t fc4_pollution_set_garbage_capacity(fc4_pollution* p, int32_t n);  /* 30 */
int64_t fc4_pollution_garbage_put_in_landfill(fc4_pollution* p, int32_t n); /* 51: the new total */
float fc4_pollution_recycling_center_effect(const fc4_pollution* p); /* 33: the value it points at */
float fc4_pollution_garbage_scaling_factor(const fc4_pollution* p);  /* 48 */
/* 46: incinerators, waste-to-energy, recycling as BORROWED tokens (the core's
 * lists hold the references); the caller AddRefs each one it hands out. */
size_t fc4_pollution_garbage_buildings(const fc4_pollution* p, fc4_token_callback cb, void* ctx);
uint32_t fc4_pollution_building_garbage_capacity(const fc4_pollution* p, uint64_t occ); /* 47 */
/* 52 GetMaxCapacity(prop), 53 GetCurrentCapacity(prop), 58-64 (prop ignored):
 * `*out` gets the value the slot writes; returns the slot's bool. */
bool fc4_pollution_occupant_u32(const fc4_pollution* p, uint32_t slot, uint64_t occ, uint32_t prop, uint32_t* out);
/* 54-57 (SC4Percentage as float): `*out` is written on success, and for slot
 * 56 also on failure (1.0). */
bool fc4_pollution_occupant_percent(const fc4_pollution* p, uint32_t slot, uint64_t occ, float* out);
bool fc4_pollution_maintenance_cost(const fc4_pollution* p, uint64_t occ, int64_t* out); /* 65: always writes */
void fc4_pollution_add_radiation(fc4_pollution* p, float x, float y, float z, float strength, float radius); /* 66 */
/* cISC4PropRequester: slot 3 GetRequestedShare (inclusive cells), slots 4, 5, 6. */
int32_t fc4_pollution_requested_share(const fc4_pollution* p, int32_t x0, int32_t z0, int32_t x1, int32_t z1);
uint32_t fc4_pollution_prop_requester_slot(const fc4_pollution* p, uint32_t slot);
bool fc4_pollution_get_state(const fc4_pollution* p, fc4_pollution_state* out); /* main thread */
uint64_t fc4_pollution_grid_by_index(const fc4_pollution* p, uint32_t index); /* save order; borrowed; any thread */
bool fc4_pollution_write(const fc4_pollution* p, const fc4_ostream* out); /* §6 */
bool fc4_pollution_read(fc4_pollution* p, const fc4_istream* in);         /* §6: versions 0-4; before Init */

/* ---- pollution shadow (tooling) -------------------------------------------------
 *
 * With the ORIGINAL pollution simulator running, each of its SimNewDay /
 * SimNewMonth deliveries is wrapped: before it, the plugin captures the
 * original's record (its own Write into a capture stream) and calls
 * fc4_pollution_shadow_begin, which copies the live grids the record refers
 * to and runs FreeCity4's day or month on them with every input read through
 * the host table (read callbacks and the grid table's geometry / get only;
 * nothing is written to the game). After the delivery the plugin captures
 * the record again and calls fc4_pollution_shadow_finish, which compares the
 * eight grids cell by cell, the saved fields, and the occupant properties
 * and deal usage with the original's. Not compared (not readable): the
 * radiation sum (not saved), posted messages, history, budget values, visual
 * effects, worst tracts. */
typedef struct fc4_pollution_shadow fc4_pollution_shadow;

/* One reference of a captured record, queried for each interface the reader
 * asks for (0 = the object does not answer it); borrowed identities. */
typedef struct fc4_pollution_capture_ref {
    uint64_t grid;       /* cISC4SimGrid 0xC9BD939A */
    uint64_t occupant;   /* cISC4Occupant 0x26BB767E */
    uint64_t department; /* cISC4DepartmentBudget 0x223E2B67 */
    uint32_t class_id;   /* GetGZCLSID() */
    uint32_t reserved;
} fc4_pollution_capture_ref;

/* A captured record: primitives little-endian, references as uint32
 * objectID (index into `refs` + 1, 0 = null) then uint32 classID. */
typedef struct fc4_pollution_capture {
    uint32_t struct_size;
    uint32_t ok; /* Write returned true and the stream stayed good */
    const uint8_t* bytes;
    size_t len;
    const fc4_pollution_capture_ref* refs;
    size_t ref_count;
} fc4_pollution_capture;

typedef struct fc4_pollution_shadow_stats {
    uint32_t struct_size;
    uint32_t deliveries, clean, incomplete, reruns;
    int32_t last_mismatches; /* -1: not compared */
    uint32_t uncached_reads;
    uint32_t reserved;
    uint64_t cells, cell_mismatches, field_mismatches, prop_mismatches;
} fc4_pollution_shadow_stats;

/* `max_cells`: differing cells listed per grid and line (0 = 4, at most 64). */
fc4_pollution_shadow* fc4_pollution_shadow_create(const fc4_pollution_host* host, uint32_t max_cells);
void fc4_pollution_shadow_destroy(fc4_pollution_shadow* s);
/* 0 = ran, -1 = not run (finish reports why), -2 = not a day or month. */
int32_t fc4_pollution_shadow_begin(fc4_pollution_shadow* s, uint32_t msg_type, int32_t julian,
                                   const fc4_pollution_capture* before);
/* The delivery's difference count (-1 = not compared); log lines go to `sink`,
 * one call per line (a day with a difference, at most 8 a month; after a
 * month, the month line). */
int32_t fc4_pollution_shadow_finish(fc4_pollution_shadow* s, const fc4_pollution_capture* after, fc4_bytes_sink sink,
                                    void* ctx);
bool fc4_pollution_shadow_get_stats(fc4_pollution_shadow* s, fc4_pollution_shadow_stats* out);

/* ---- protection: fire and police (docs/specs/protection.md) ----------------
 *
 * One handle is one simulator: kind 0 = fire (cSC4FireProtectionSimulator,
 * 0x2990C05A), 1 = police (cSC4PoliceSimulator, 0x6990C04F). Tokens:
 * occupants (stations, jails, automata) by their cISC4Occupant identity,
 * departments, lots, grids and the air dispatch source; 0 is null. The core
 * owns one reference on every token it keeps (stations, jails, departments,
 * the air source): create_department and create_air_source hand one over, and
 * a saved reference read back by fc4_protection_read either arrives with one
 * (a host object) or gets one through add_ref (fc4_ref.core_handle set).
 * Both grids are the game's own cSC4SimGrid (fc4_grid_api: uint8 adjusted,
 * int16 unadjusted, §2). A message delivered while another call into the same
 * object is running is queued and handled when that call returns.
 *
 * Threads: the core is single-threaded. The caller serialises every call on
 * one handle (the plugin holds a per-object lock). */
typedef struct fc4_protection fc4_protection;

/* GetBudgetItemForPurpose with the item's department looked up by its ID
 * (§6.1): has_department = GetDepartmentBudget(item.departmentID) != NULL;
 * locally_funded = that department's IsLocallyFundedObjectInDepartment. */
typedef struct fc4_protection_budget_item {
    uint32_t has_department;
    uint32_t locally_funded;
    float local_funding; /* item.localFundingPercentage (a fraction) */
    uint32_t reserved;
    int64_t cost;
    uint64_t department; /* borrowed */
} fc4_protection_budget_item;

/* GetDispatchInfo (§5). Tokens borrowed. `station` is the record's +0x20,
 * the source station whose dispatch effect draws the cone (§7.2, PR2);
 * `call_id` its +0x08 and `active` its +0x10 (PR1). `reserved` (formerly the
 * vehicle automaton) is not read: leave it 0. */
typedef struct fc4_protection_dispatch_info {
    uint64_t station;
    uint64_t reserved;
    uint32_t call_id;
    uint32_t active;
    float x, z;
} fc4_protection_dispatch_info;

/* A message for fc4_protection_do_message (§5). `occupant` is GetVoid1() as
 * a cISC4Occupant (occupant messages); `unknown` is GetIGZUnknown() (the
 * police crime message). */
typedef struct fc4_protection_message {
    uint32_t msg_type;
    uint32_t data1;
    uint32_t data2;
    uint32_t reserved;
    uint64_t occupant;
    uint64_t unknown;
} fc4_protection_message;

/* Diagnostics. covered_tracts / max_coverage / coverage_sum: the adjusted
 * grid's tracts with a value > 0, its largest value and the sum. */
typedef struct fc4_protection_state {
    uint32_t struct_size;
    uint32_t kind;
    uint8_t initialized, loaded, strike_flag, reserved0;
    uint32_t stations;
    uint32_t tracts;
    int32_t count_x, count_z;
    uint32_t covered_tracts, max_coverage, coverage_sum;
    int32_t tracts_per_day;
    int32_t zoned, sum, non_zero;
    float avg, pct;
    uint32_t strike_date, next_call;
    uint32_t jails, funded_jails, jailbreaks;
    int32_t jail_capacity, jail_population, jailed, arrests_last, crimes_last;
    uint32_t reentrant_calls;
    uint32_t reserved1;
    uint64_t department, jail_department, adjusted_grid, unadjusted_grid, air_source;
} fc4_protection_state;

/* What the protection simulators need from the game (one callback per
 * ProtectionHost method, engine/core/src/protection/mod.rs, §14). Missing
 * callbacks behave as "nothing there" (0, false, no object); the read-only
 * shadow table leaves every callback that changes the game NULL. `kind` is
 * 0 = fire, 1 = police. */
typedef struct fc4_protection_host {
    uint32_t struct_size; /* sizeof(fc4_protection_host) */
    void* user;
    fc4_grid_api grids;
    /* lifecycle (§4) */
    void (*acquire_services)(void* user, uint32_t kind);
    void (*release_services)(void* user, uint32_t kind);
    /* AddAgent(target, level, name, flags) once per level, in order. */
    void (*add_agent)(void* user, const char* name, size_t name_len, const uint32_t* levels, size_t count,
                      uint32_t flags);
    void (*remove_agent)(void* user);
    void (*subscribe)(void* user, const uint32_t* msg_types, size_t count);
    void (*unsubscribe)(void* user, const uint32_t* msg_types, size_t count);
    void (*register_dispatch_source)(void* user);
    void (*unregister_dispatch_source)(void* user);
    void (*set_grid_instance)(void* user, uint64_t grid, uint32_t instance);
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    /* resources: a NEW fc4_props (the core destroys it), NULL if missing */
    fc4_props* (*tuning)(void* user, uint32_t kind);
    /* budget (§4.1, §6.1, §6.6) */
    uint64_t (*create_department)(void* user, uint32_t id, uint32_t group, uint32_t name_group,
                                  uint32_t name_instance, float max_funding); /* one reference, 0 = none */
    uint32_t (*department_id)(void* user, uint64_t dept);
    float (*department_funding)(void* user, uint64_t dept); /* *GetFundingPercentage() */
    void (*add_locally_funded)(void* user, uint64_t dept, uint64_t occ, uint32_t purpose);
    void (*remove_locally_funded)(void* user, uint64_t dept, uint64_t occ, uint32_t purpose);
    bool (*set_local_funding)(void* user, uint64_t dept, uint64_t occ, float pct, uint32_t purpose);
    bool (*budget_item)(void* user, uint64_t occ, uint32_t purpose, fc4_protection_budget_item* out);
    /* occupants */
    uint32_t (*occupant_type)(void* user, uint64_t occ);
    bool (*occupant_property)(void* user, uint64_t occ, uint32_t id, fc4_props* out);
    void (*set_occupant_property)(void* user, uint64_t occ, uint32_t id, uint16_t mem_type, bool is_array,
                                  const void* data, size_t count);
    void (*occupant_position)(void* user, uint64_t occ, float* xyz);
    bool (*is_prop_occupant)(void* user, uint64_t occ);
    int32_t (*lot_road_access)(void* user, uint64_t occ); /* -1 = no lot, 0 = no road, 1 = road */
    /* city, simulator, zones, terrain, disasters */
    uint32_t (*sim_day)(void* user);
    bool (*lot_location)(void* user, uint64_t unknown, int32_t* x, int32_t* z); /* false = not a lot */
    void (*cell_center_to_position)(void* user, int32_t cell_x, int32_t cell_z, float* x, float* z);
    uint32_t (*zone_value)(void* user, int32_t cell_x, int32_t cell_z);
    float (*sea_level)(void* user);
    float (*altitude)(void* user, float x, float z);
    bool (*disaster_ongoing)(void* user);
    /* dispatch manager (§5, §8): dispatch_ids writes at most `capacity` IDs
     * (never called with 0) and returns the number written. */
    uint32_t (*dispatch_count)(void* user, uint32_t dispatch_type);
    uint32_t (*dispatch_ids)(void* user, uint32_t dispatch_type, uint32_t* ids, uint32_t capacity);
    bool (*dispatch_info)(void* user, uint32_t id, fc4_protection_dispatch_info* out);
    /* automata controllers (§6.3, §9.5) */
    void (*add_controllers)(void* user, const char* group, size_t group_len, uint64_t occ);
    void (*remove_controllers)(void* user, const char* group, size_t group_len, uint64_t occ);
    /* other simulators, history, messages */
    bool (*police_percent_coverage)(void* user, float* out); /* false = no police simulator */
    void (*record_history)(void* user, uint32_t id, int32_t value);
    void (*post)(void* user, uint32_t msg_type, uint64_t subject); /* §12; subject 0 = none */
    /* air dispatch sources (§8.4): one reference for the core, 0 = none */
    uint64_t (*create_air_source)(void* user, uint32_t kind, uint32_t* class_id);
    void (*air_source_init)(void* user, uint64_t source);
    void (*air_source_shutdown)(void* user, uint64_t source);
} fc4_protection_host;

/* Which sub-object QueryInterface(iid) answers with (§1): 0 = none, 1 = the
 * primary interface (also cIGZUnknown), 2 = cIGZMessageTarget2, 3 = the
 * dispatch source (0xC9C00435), 4 = cIGZSerializable. */
uint32_t fc4_protection_query_interface(uint32_t kind, uint32_t iid);
/* `session_seed` is the session seed S (foundations §6.3, ABI 64): the
 * generator is seeded with derive(S, GZCLSID), fire 0x2990C05A, police
 * 0x6990C04F. */
fc4_protection* fc4_protection_create(const fc4_protection_host* host, uint32_t kind, uint64_t token,
                                      uint32_t session_seed);
void fc4_protection_destroy(fc4_protection* p);
bool fc4_protection_init(fc4_protection* p);     /* §4.1 */
bool fc4_protection_shutdown(fc4_protection* p); /* §4.2 */
bool fc4_protection_do_message(fc4_protection* p, const fc4_protection_message* m); /* §5: always true */
/* Primary interfaces (§15.1 fire slot / §15.2 police slot). List getters call
 * `cb` once per object (borrowed tokens; the caller AddRefs what it keeps). */
uint64_t fc4_protection_grid(const fc4_protection* p);            /* 5: borrowed, 0 = none */
uint64_t fc4_protection_department(const fc4_protection* p);      /* 6: borrowed */
uint64_t fc4_protection_jail_department(const fc4_protection* p); /* police 9: borrowed */
bool fc4_protection_is_station(const fc4_protection* p, uint64_t occ); /* 7 / 22 */
bool fc4_protection_station_funding(const fc4_protection* p, uint64_t occ, int32_t* cost, float* pct); /* 8 / 7 */
bool fc4_protection_set_station_funding(fc4_protection* p, uint64_t occ, float pct);                  /* 9 / 8 */
bool fc4_protection_protection_station_funding(const fc4_protection* p, uint64_t occ, float* out); /* MT 11 */
float fc4_protection_station_efficiency(const fc4_protection* p, uint64_t occ); /* 10 / 25 */
bool fc4_protection_is_on_strike(const fc4_protection* p);                      /* 11 / 10 */
float fc4_protection_chance_of_strike(const fc4_protection* p);                 /* 12 / 11 */
bool fc4_protection_begin_strike(fc4_protection* p, uint64_t occ);              /* 13 / 12 */
bool fc4_protection_end_strike(fc4_protection* p);                              /* 14 / 13 */
uint32_t fc4_protection_strike_start_date(const fc4_protection* p);             /* 15 / 14 */
uint32_t fc4_protection_strike_length(const fc4_protection* p);                 /* 16 / 15 */
/* 17 / 16: appends (no clear); returns the number appended. */
int32_t fc4_protection_striking_stations(const fc4_protection* p, fc4_token_callback cb, void* ctx);
bool fc4_protection_is_station_on_strike(const fc4_protection* p, uint64_t occ); /* 18 / 17 */
float fc4_protection_average_coverage(const fc4_protection* p);                  /* 19 / 18 */
float fc4_protection_percent_coverage(const fc4_protection* p); /* 20 / 19: the value behind the pointer */
/* 21 / 20 (§7.3): fire zeroes both outputs first; police leaves them when it
 * fails. */
bool fc4_protection_occupant_coverage(const fc4_protection* p, uint64_t occ, float pct, float* cover_radius,
                                      float* station_radius);
/* 22 / 21 and dispatch-source 6: the caller clears its list first. */
uint32_t fc4_protection_stations(const fc4_protection* p, fc4_token_callback cb, void* ctx);
uint32_t fc4_protection_station_count(const fc4_protection* p);                     /* 23 / 24 */
float fc4_protection_station_dispatch_radius(const fc4_protection* p, uint64_t occ); /* 24 / 23 */
int32_t fc4_protection_total_dispatch_count(const fc4_protection* p);              /* 25 / 26 */
/* Police jails (§9, §15.2). */
int32_t fc4_protection_reset_unjailed_criminals(fc4_protection* p);              /* 27 */
uint32_t fc4_protection_jail_capacity_of(const fc4_protection* p, uint64_t occ); /* 28 */
uint32_t fc4_protection_jail_capacity(const fc4_protection* p);                  /* 29 */
uint32_t fc4_protection_jail_population(const fc4_protection* p);                /* 30 */
uint32_t fc4_protection_criminal_count(const fc4_protection* p);                 /* 31 */
uint32_t fc4_protection_jails(const fc4_protection* p, fc4_token_callback cb, void* ctx); /* 32: clear first */
bool fc4_protection_is_jail(const fc4_protection* p, uint64_t occ);              /* 33 */
uint32_t fc4_protection_arrest_count(const fc4_protection* p);                   /* 34 */
uint32_t fc4_protection_jail_count(const fc4_protection* p);                     /* 35 */
float fc4_protection_jailed_percentage(const fc4_protection* p);                 /* 36 */
int64_t fc4_protection_jail_upkeep_cost(const fc4_protection* p, uint64_t occ);  /* 37 */
bool fc4_protection_process_monthly_crime_stats(fc4_protection* p);              /* 38 */
bool fc4_protection_is_in_jailbreak(const fc4_protection* p, uint64_t occ);      /* 39 */
bool fc4_protection_begin_jailbreak(fc4_protection* p, uint64_t occ);            /* 40 */
bool fc4_protection_end_jailbreak(fc4_protection* p, uint64_t occ);              /* 41 */
/* 42: 0 and no callback when empty (the caller's list stays as it is); else
 * the caller clears its list first. */
int32_t fc4_protection_jailbreak_buildings(const fc4_protection* p, fc4_token_callback cb, void* ctx);
uint32_t fc4_protection_station_arrest_count(const fc4_protection* p, uint64_t occ); /* 43 */
uint32_t fc4_protection_station_crime_count(const fc4_protection* p, uint64_t occ);  /* 44 */
/* The dispatch source (§8, §15.4 slot). */
bool fc4_protection_can_handle_dispatch_type(const fc4_protection* p, uint32_t t);           /* 3 */
bool fc4_protection_can_dispatch(const fc4_protection* p, float x, float z, bool* all_busy); /* 4 */
int32_t fc4_protection_max_possible_dispatch_count(const fc4_protection* p);                /* 5 */
/* 7: NULL flags / base = not requested; each output written as soon as its
 * property is read (§8.1). */
bool fc4_protection_dispatches_at_station(const fc4_protection* p, uint64_t occ, uint32_t* flags, uint32_t* base,
                                          uint32_t* count);
uint64_t fc4_protection_dispatch_from_nearest_station(fc4_protection* p, float x, float z, uint32_t* id); /* 8 */
bool fc4_protection_dispatch_from_station(fc4_protection* p, uint64_t occ, uint32_t* id);               /* 9 */
bool fc4_protection_revoke_dispatch_from_station(fc4_protection* p, uint64_t occ, uint32_t id);         /* 10 */
bool fc4_protection_dispatch_automata_properties(const fc4_protection* p, uint32_t* b, float* c3, uint32_t* d);
bool fc4_protection_get_state(const fc4_protection* p, fc4_protection_state* out);
bool fc4_protection_write(const fc4_protection* p, const fc4_ostream* out); /* §11 */
bool fc4_protection_read(fc4_protection* p, const fc4_istream* in);         /* §11 */
/* The grid shadow: a handle on a read-only host table (grids unused), never
 * initialised. fc4_protection_shadow_run reads the tuning, takes the
 * original's stations (list order), department and strike flag as BORROWED
 * tokens (valid for this call and later queries on the handle until the next
 * run), rebuilds the unadjusted grid (§5.1) in a core grid of geometry `geom`
 * and derives the adjusted grid (§2, factor 1.0). fc4_protection_shadow_grid
 * copies the adjusted grid (x outer: x * count_z + z) and returns the number
 * of tracts; fc4_protection_shadow_unadjusted_grid likewise the unadjusted
 * int16 grid (PR5: the original keeps its own at primary +0x84). Destroy
 * with fc4_protection_destroy (releases nothing). */
fc4_protection* fc4_protection_shadow_create(const fc4_protection_host* host, uint32_t kind);
bool fc4_protection_shadow_run(fc4_protection* p, const fc4_grid_geometry* geom, const uint64_t* stations,
                               size_t count, uint64_t department, bool strike_flag);
size_t fc4_protection_shadow_grid(const fc4_protection* p, uint8_t* out, size_t n);
size_t fc4_protection_shadow_unadjusted_grid(const fc4_protection* p, int16_t* out, size_t n); /* ABI 27 */

/* ---- crime (docs/specs/crime.md) ---------------------------------------------
 *
 * Tokens: lots, building occupants, occupants and grids (the game objects'
 * identities); 0 is null. Both crime grids are the game's own
 * cSC4SimGrid<uint8> (fc4_grid_api); the population grids are uint16 and the
 * EQ grids float ones.
 *
 * The crime type list and the generator are process-wide (§7, §9): the first
 * fc4_crime_create seeds the generator with derive_seed(session_seed,
 * 0x6C09C3F6) (foundations §6.3); later objects share both. The original's
 * session value is cRZTime, u32 seconds since 1901-01-01 00:00 UTC
 * (time() + 2177452800, §9 / CQ5), the value every clock-seeded generator
 * shares (CQ21; fc4_core::rng::rz_time_from_unix).
 *
 * Threads (foundations §19): the game calls getters from a second thread while
 * the main thread is inside a call. The functions marked "any thread" read a
 * snapshot the main thread publishes at the end of every outermost call and
 * never touch the core; every other function is main-thread only. A message
 * delivered while another call into the same object is running is queued and
 * handled when that call returns. */
typedef struct fc4_crime fc4_crime;

/* A message for fc4_crime_do_message (§5). */
typedef struct fc4_crime_message {
    uint32_t msg_type;
    uint32_t data1;    /* GetData1() (exemplar changed: the group) */
    uint32_t data2;    /* GetData2() (exemplar changed: the instance) */
    uint32_t reserved;
    uint64_t occupant; /* GetVoid1() as a cISC4Occupant (inserted / removed) */
} fc4_crime_message;

/* One element of GetCrimeIncidentArray: the game's 28-byte tCrimeEvent
 * (§11.4) with `has_lot` in a padding byte. has_lot = 0: an incident without a
 * lot; the game's element keeps its previous content. */
typedef struct fc4_crime_event {
    uint32_t cell_x, cell_z;
    uint32_t type_id;
    uint8_t visuals_created;
    uint8_t has_lot;
    uint8_t pad[2];
    uint32_t icon_type, icon_group, icon_instance;
} fc4_crime_event;

/* One occupant of the bounding-box query of §11.3. */
typedef struct fc4_crime_nearby {
    uint64_t token;
    uint32_t occupant_type;
    uint8_t in_crime_group;        /* sims: IsOccupantGroup(0x4102) */
    uint8_t has_incident_property; /* props: HasProperty(0x8AA864D5) */
    uint8_t answers_iid;           /* the kind's QueryInterface succeeded */
    uint8_t pad;
    float position[3];
    uint32_t prop_state; /* props: GetState() */
} fc4_crime_nearby;
typedef void (*fc4_crime_nearby_callback)(void* ctx, const fc4_crime_nearby* o);

#define FC4_CRIME_NEAREST_SIM 0u
#define FC4_CRIME_NEAREST_PROP 1u

/* Diagnostics. */
typedef struct fc4_crime_state {
    uint32_t struct_size;
    uint8_t initialized, loaded_from_save, has_message_server, pad0;
    uint32_t pipeline_state;
    uint32_t monthly_crime_count;
    uint32_t average_bits; /* the stored float32 (NaN 0xFFC00000 when unzoned) */
    int32_t hotspot_x, hotspot_z, hotspot_value;
    float ordinance_factor;
    int32_t criminal_base, crimes_to_commit;
    uint32_t incidents;
    uint32_t types; /* the process-wide type list's length (0 = not loaded) */
    uint32_t tract_size;
    float decay, reoffend;
    uint32_t census[12]; /* pop[wealth][bucket] (§6.1), wealth-major */
    float jobless[3];
    uint32_t incident_epoch, reentrant_calls, reserved;
    uint64_t crime_grid, effect_grid;
} fc4_crime_state;

/* What the crime simulator needs from the game (one callback per CrimeHost
 * method, engine/core/src/crime/mod.rs, §13). Missing callbacks behave as
 * "nothing there" (0, false, no object). */
typedef struct fc4_crime_host {
    uint32_t struct_size; /* sizeof(fc4_crime_host) */
    void* user;
    fc4_grid_api grids;
    /* lifecycle glue (§4) */
    void (*add_agent)(void* user, const uint8_t* name, size_t len, const uint32_t* levels, uint32_t count,
                      uint32_t flags);
    void (*remove_agent)(void* user);
    void (*subscribe)(void* user, const uint32_t* msg_types, uint32_t count);
    void (*unsubscribe)(void* user, const uint32_t* msg_types, uint32_t count);
    void (*acquire_services)(void* user);
    void (*release_services)(void* user);
    void (*add_prop_requester)(void* user);
    /* resources (§7, §8) */
    fc4_props* (*tuning)(void* user); /* a NEW fc4_props (the core destroys it), NULL if missing */
    void (*for_each_type_key)(void* user, fc4_key_visitor visit, void* ctx);
    fc4_props* (*exemplar)(void* user, uint32_t type_id, uint32_t group, uint32_t instance); /* NEW or NULL */
    /* A crime type name, §7 steps 2-3 (CQ8): type_id is always 0x2026960B.
     * Look up (type, group + language ID, instance), then (type, group,
     * instance) if that is missing and the language ID is not 0; copy the text
     * and detokenize it in place (skipped with no detokenizer). true: loaded,
     * the text went to the sink. false and text sent to the sink: the
     * detokenizer returned false. false and nothing sent: no such LTEXT. A
     * failed load discards any text sent: the name is empty unless 0xAA12FCB0
     * exists (CQ16). The core replaces U+2022 itself (step 4). */
    bool (*ltext)(void* user, uint32_t type_id, uint32_t group, uint32_t instance, fc4_bytes_sink sink, void* ctx);
    /* residential and demand (§6.1, §6.2): three grid tokens, one reference each */
    void (*population_grids)(void* user, uint64_t* out3);
    void (*eq_grids)(void* user, uint64_t* out3);
    bool (*demand)(void* user, uint32_t id, float* value, float* max, float* min);
    /* police and ordinances (§6.4, §6.6) */
    uint64_t (*police_grid)(void* user); /* one reference handed over, 0 = none */
    uint32_t (*jail_population)(void* user);
    int32_t (*reset_unjailed_criminals)(void* user);
    void (*process_monthly_crime_stats)(void* user);
    bool (*ordinance_crime_factor)(void* user, float* out);
    /* lots and zones (§6.6-§6.9, §11): lot tokens are borrowed */
    uint64_t (*random_lot)(void* user);
    uint64_t (*lot_at)(void* user, int32_t cell_x, int32_t cell_z);
    void (*lot_bounding_rect)(void* user, uint64_t lot, fc4_cell_rect* out);
    bool (*lot_location)(void* user, uint64_t lot, int32_t* x, int32_t* z);
    uint8_t (*lot_condition)(void* user, uint64_t lot);
    bool (*lot_building)(void* user, uint64_t lot, uint64_t* building, uint32_t* purpose, uint32_t* wealth);
    void (*lot_center_cell)(void* user, uint64_t lot, int32_t* x, int32_t* z);
    uint32_t (*zone_type)(void* user, int32_t cell_x, int32_t cell_z);
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    /* occupants and props (§6.10, §11.3, §11.6) */
    bool (*occupant_has_property)(void* user, uint64_t occ, uint32_t id);
    /* The raw storage of an array property (count x element size bytes, the
     * variant's RefUint8 data whatever its type, no conversion; §6.10, CQ13;
     * a string is its stored bytes, "AB" gives m = 65, r = 66, CQ22); false
     * if absent, a scalar or an object array (CQ22 [choice]). */
    bool (*occupant_property_bytes)(void* user, uint64_t occ, uint32_t id, fc4_bytes_sink sink, void* ctx);
    void (*occupant_position)(void* user, uint64_t occ, float* xyz);
    void (*occupants_in_box)(void* user, const float* min3, const float* max3, uint32_t kind,
                             fc4_crime_nearby_callback cb, void* ctx);
    uint32_t (*prop_crime_incident_id)(void* user, uint32_t prop);
    /* arson (§6.8) */
    bool (*disaster_layer_present)(void* user);
    void (*ignite)(void* user, uint64_t building, uint32_t cause);
    /* messages (§10) */
    void (*post_crime_committed)(void* user, uint32_t type_id, uint64_t lot, const uint8_t* name, size_t len);
    void (*post_crime_map_updated)(void* user);
} fc4_crime_host;

/* The host table is copied; `token` is the simulator's own identity. */
fc4_crime* fc4_crime_create(const fc4_crime_host* host, uint64_t token, uint32_t session_seed);
void fc4_crime_destroy(fc4_crime* p);
bool fc4_crime_init(fc4_crime* p);     /* §4.1 */
bool fc4_crime_shutdown(fc4_crime* p); /* §4.2 */
/* §5: always true for a live object; queued during another call. */
bool fc4_crime_do_message(fc4_crime* p, const fc4_crime_message* m);
/* Any thread (the published snapshot). */
uint64_t fc4_crime_grid(const fc4_crime* p);          /* slot 5: borrowed token, 0 = none */
uint32_t fc4_crime_average(const fc4_crime* p);       /* slot 6 */
uint32_t fc4_crime_monthly_count(const fc4_crime* p); /* slot 7 */
/* Slot 8: false and nothing written when the value is 0 or less. */
bool fc4_crime_hotspot(const fc4_crime* p, uint8_t* value, int32_t* x, int32_t* z);
uint32_t fc4_crime_incident_count(const fc4_crime* p);
/* Slot 11: copies up to `capacity` events, returns the incident count. */
uint32_t fc4_crime_incidents(const fc4_crime* p, fc4_crime_event* out, uint32_t capacity);
bool fc4_crime_set_visuals(const fc4_crime* p, uint32_t index, bool value); /* slot 12 */
bool fc4_crime_get_visuals(const fc4_crime* p, uint32_t index);             /* slot 13 */
/* Slots 9/10 (§11.3): the selection over the occupants the box query
 * returned, in its order: the index of the candidate, or -1. Pure. */
int32_t fc4_crime_nearest_of(uint32_t kind, const float* pos3, float radius, const fc4_crime_nearby* occupants,
                             uint32_t count);
/* Main thread. Prop requester slot 3 (rect x1, z1, x2, z2) and slot 6 (0 = none). */
int32_t fc4_crime_requested_share(const fc4_crime* p, int32_t x1, int32_t z1, int32_t x2, int32_t z2);
uint32_t fc4_crime_select_prop(const fc4_crime* p, const uint32_t* props, uint32_t count, int32_t cell_x,
                               int32_t cell_z);
bool fc4_crime_get_state(const fc4_crime* p, fc4_crime_state* out);
bool fc4_crime_write(const fc4_crime* p, const fc4_ostream* out); /* §12 */
bool fc4_crime_read(fc4_crime* p, const fc4_istream* in);         /* §12: false for version != 6 */

/* The crime shadow (tooling, engine/plugin/FC4CrimeShadow.h): the tuning and
 * the type list read through a host table of which only tuning,
 * for_each_type_key, exemplar and ltext are called, and the replay of
 * pipeline state 4's grid work on grids read from the original simulator. */
typedef struct fc4_crime_shadow fc4_crime_shadow;

typedef struct fc4_crime_shadow_info {
    uint32_t struct_size;
    uint32_t types;
    uint8_t tuning_found;
    uint8_t pad[3];
    uint32_t tract_size;
    float decay, reoffend;
} fc4_crime_shadow_info;

/* One incident to replay: its lot's bounding rect (has_rect = 0: no lot) and type. */
typedef struct fc4_crime_shadow_incident {
    fc4_cell_rect rect;
    uint32_t type_id;
    uint8_t has_rect;
    uint8_t pad[3];
} fc4_crime_shadow_incident;

typedef struct fc4_crime_shadow_month_in {
    uint32_t struct_size;
    fc4_grid_geometry geometry;
    const uint8_t* pre; /* count_x * count_z values, x outer: the grid before the step */
    int32_t hotspot_x, hotspot_z, hotspot_value; /* the hotspot before the step (tracts) */
    const fc4_crime_shadow_incident* incidents;
    uint32_t incident_count;
    const uint32_t* zones; /* count_x * count_z corner-cell zone types, x outer; NULL = no average */
} fc4_crime_shadow_month_in;

typedef struct fc4_crime_shadow_month_out {
    uint32_t struct_size;
    int32_t hotspot_x, hotspot_z, hotspot_value;
    uint32_t count;         /* incidents stamped (severity > 0) */
    uint32_t unknown_types; /* incidents of a type not in the list */
    uint32_t no_rect;       /* counted incidents with no rect (not stamped) */
    uint32_t severity_sum;
    uint32_t average_bits; /* the stored average's float32 bits, 0 when zones was NULL */
    uint8_t average_computed;
    uint8_t pad[3];
} fc4_crime_shadow_month_out;

fc4_crime_shadow* fc4_crime_shadow_create(const fc4_crime_host* host);
void fc4_crime_shadow_destroy(fc4_crime_shadow* s);
bool fc4_crime_shadow_get_info(const fc4_crime_shadow* s, fc4_crime_shadow_info* out);
/* §6.7: 1 = a candidate, 0 = not, -1 = unknown type. */
int32_t fc4_crime_shadow_candidate(const fc4_crime_shadow* s, uint32_t type_id, uint32_t zone_type, bool has_building,
                                   uint32_t purpose, uint32_t wealth, uint8_t condition);
/* Decay, the hotspot re-read, each incident stamped in order, the zoned
 * average; `post` receives count_x * count_z values. */
bool fc4_crime_shadow_month(const fc4_crime_shadow* s, const fc4_crime_shadow_month_in* in, uint8_t* post,
                            fc4_crime_shadow_month_out* out);

/* ---- aura (docs/specs/aura.md) ---------------------------------------------
 *
 * cSC4AuraSimulator (0x0990BDD8): the monthly time-sliced pipeline (§6), the
 * occupant effects (§7), the getters (§8) and the save record (§11). The host
 * table has one callback per AuraHost method (engine/core/src/aura/mod.rs)
 * except the aura's own grids, which are the game's cSC4SimGrid<int8> /
 * <int16> through `grids` (int8 0x49B9E603, int16 0x49B9E605; the map view
 * reads them as raw memory, §14.2). The other simulators' grids are read
 * through `input_grid` (a BORROWED token per FC4_AURA_INPUT_*, valid until the
 * outermost call into the core returns) and `grids.geometry` / `grids.get`
 * with the grid's element class (pollution int16, grade maps float, crime /
 * trip / congestion uint8, population uint16). Occupants are tokens (their
 * cISC4Occupant identity); 0 is null. Missing callbacks read as "nothing
 * there" (0, false, none).
 *
 * Threads: the getters (fc4_aura_ratings, fc4_aura_grid) may be called from
 * any thread, also during a main-thread call: they read a snapshot published
 * after every main-thread call, never the core. Every other function is
 * main-thread. A message, transient effect or strike buster delivered while
 * another main-thread call is running is queued and handled when that call
 * returns (fc4_aura_state.reentrant_calls). */
typedef struct fc4_aura fc4_aura;

/* `which` of input_grid. */
#define FC4_AURA_INPUT_AIR        0u
#define FC4_AURA_INPUT_WATER      1u
#define FC4_AURA_INPUT_GARBAGE    2u
#define FC4_AURA_INPUT_RADIATION  3u
#define FC4_AURA_INPUT_SCHOOL     4u  /* residential slot 39, cISC4SimGrid<float> */
#define FC4_AURA_INPUT_HOSPITAL   5u  /* residential slot 40, cISC4SimGrid<float> */
#define FC4_AURA_INPUT_CRIME      6u
#define FC4_AURA_INPUT_TRIP       7u
#define FC4_AURA_INPUT_CONGESTION 8u
#define FC4_AURA_INPUT_POP1       9u  /* GetPopulationGrids (slot 25): R$, R$$, R$$$ */
#define FC4_AURA_INPUT_POP2       10u
#define FC4_AURA_INPUT_POP3       11u
#define FC4_AURA_INPUT_COUNT      12u

/* `which` of fc4_aura_grid (slots 12-15). */
#define FC4_AURA_GRID_AURA      0u
#define FC4_AURA_GRID_TRANSIENT 1u
#define FC4_AURA_GRID_PARK      2u
#define FC4_AURA_GRID_LANDMARK  3u

/* A lot (cISC4Lot): GetBoundingRect (inclusive cells), GetZoneType,
 * GetOccupantWealth, GetBuilding as a cISC4Occupant token (borrowed; 0 none). */
typedef struct fc4_aura_lot {
    int32_t x1, z1, x2, z2;
    uint32_t zone_type;
    int32_t occupant_wealth;
    uint64_t building;
} fc4_aura_lot;

/* One (developer type, count) pair of GetPopulations (slot 54): every entry
 * of every map, counts of 0 included (aura.md 6.5.1). */
typedef void (*fc4_aura_population_sink)(void* ctx, uint32_t developer_type, int32_t count);

/* What any thread may read: slots 8-10 and the four grid tokens of slots
 * 12-15 (borrowed; 0 = none), as of the last main-thread call. */
typedef struct fc4_aura_ratings {
    uint32_t struct_size;
    int8_t mayor_rating, max_rating, min_rating, pad;
    int32_t max_x, max_z, min_x, min_z; /* cells */
    uint32_t reserved[2];
    uint64_t aura_grid, transient_grid, park_map, landmark_map;
} fc4_aura_ratings;

/* Diagnostics (main thread). */
typedef struct fc4_aura_state {
    uint32_t struct_size;
    uint8_t initialized, re_add_landmarks, has_lot_iterator, pad0;
    int32_t state; /* pipeline state 0..17 (§6) */
    uint32_t cycle_start_day;
    int32_t mayor_rating, unused_rating, global_effect;
    int16_t max_rating, min_rating;
    int32_t max_x, max_z, min_x, min_z;
    int32_t effective_base_aura[3];
    uint32_t hotspots, low_tracts, departments;
    float riot_roll;
    uint32_t slice_step;
    int32_t slice_miss;
    uint32_t aura_tracts; /* over the aura grid: count, sum, min, max */
    int32_t aura_sum, aura_min, aura_max;
    uint32_t base_tracts; /* base tracts with data (!= -128) */
    uint32_t reentrant_calls;
    float long_decay, short_decay;
    uint32_t reserved;
} fc4_aura_state;

typedef struct fc4_aura_host {
    uint32_t struct_size; /* sizeof(fc4_aura_host) */
    void* user;
    fc4_grid_api grids;
    /* city and simulator */
    void (*cell_width)(void* user, float* x, float* z);                             /* city 91 / 92 */
    void (*position_to_cell)(void* user, float x, float z, int32_t* cx, int32_t* cz); /* city 95 */
    uint32_t (*sim_date_number)(void* user);                                        /* simulator 8 */
    fc4_props* (*tuning)(void* user); /* a NEW fc4_props (the core destroys it), NULL if missing */
    void (*publish_base_aura)(void* user, const int32_t* base_aura3); /* §10: R$, R$$, R$$$ */
    /* lots */
    uint32_t (*lot_count)(void* user);
    uint64_t (*lot_iterator)(void* user); /* a fresh iterator (0 = none) */
    /* The iterator's next lot and its population pairs; false at the end. */
    bool (*lot_next)(void* user, uint64_t it, fc4_aura_lot* out, fc4_aura_population_sink sink, void* ctx);
    void (*lot_iterator_release)(void* user, uint64_t it);
    bool (*lot_at)(void* user, int32_t x, int32_t z, fc4_aura_lot* out); /* GetLot(x, z, false) */
    /* GetLandValueType(x, z); negative = no land value simulator (aura.md 6.5.1,
     * AU10: the value/wealth term is skipped); NULL reads as none */
    int32_t (*land_value_type)(void* user, int32_t x, int32_t z);
    /* occupants */
    bool (*occupant_has_property)(void* user, uint64_t occ, uint32_t id);
    bool (*occupant_property)(void* user, uint64_t occ, uint32_t id, fc4_props* out);
    uint32_t (*occupant_type)(void* user, uint64_t occ);
    void (*occupant_position)(void* user, uint64_t occ, float* xyz);
    bool (*budget_item_department)(void* user, uint64_t occ, uint32_t purpose, uint32_t* department);
    /* budget, ordinances */
    bool (*department_funding)(void* user, uint32_t department, float* fraction);
    float (*tax_rate)(void* user, int32_t index);
    int32_t (*cumulative_offset)(void* user, uint32_t prop, uint32_t index);
    /* the other simulators' grids (FC4_AURA_INPUT_*): a borrowed token, 0 = none */
    uint64_t (*input_grid)(void* user, uint32_t which);
    /* effects on the game */
    void (*record_history)(void* user, uint32_t series, int32_t value);
    void (*attempt_disaster_start)(void* user, uint32_t manager, int32_t x, int32_t z); /* cells, result ignored */
    void (*end_strike)(void* user, uint32_t which); /* 0 police, 1 fire, 2 health, 3 school (§8.4) */
    void (*post)(void* user, uint32_t msg_type);    /* queued; void1 = IGZUnknown = the simulator */
    /* CPU-cycle limit timer (simulator §5.5); none = never expires */
    void (*limit_timer_start)(void* user, uint32_t ms);
    bool (*limit_timer_expired)(void* user);
    /* framework plumbing (§4.1, §4.2) */
    void (*subscribe)(void* user, const uint32_t* ids, uint32_t count);
    void (*unsubscribe)(void* user, const uint32_t* ids, uint32_t count);
    void (*add_agent)(void* user, uint32_t level, const char* name, uint32_t flags);
    void (*remove_agent)(void* user);
    bool (*registry_add)(void* user); /* §4.3; false = not published (not an error) */
    void (*registry_remove)(void* user);
} fc4_aura_host;

/* The host table is copied; `token` is the simulator's own identity;
 * `session_seed` is the session seed S (foundations §6.3, ABI 64): its
 * cRZRandom (§9) is seeded with derive(S, 0x0990BDD8). NULL for a short
 * table. */
fc4_aura* fc4_aura_create(const fc4_aura_host* host, uint64_t token, uint32_t session_seed);
void fc4_aura_destroy(fc4_aura* p); /* releases the grids and a held lot iterator */
bool fc4_aura_init(fc4_aura* p);     /* §4.1 */
bool fc4_aura_shutdown(fc4_aura* p); /* §4.2 */
/* §5: true except for a late SimNewDay; void1 = the occupant token of the
 * occupant messages (keep it alive until the outermost call returns when this
 * is called from inside another call: it is queued). */
bool fc4_aura_do_message(fc4_aura* p, uint32_t msg_type, uint32_t data1, uint32_t data2, uint32_t data3,
                         uint64_t void1);
void fc4_aura_add_transient_effect_type(fc4_aura* p, float x, float z, uint32_t type); /* slot 6, §8.1 */
void fc4_aura_add_transient_effect(fc4_aura* p, float x, float z, float radius, float magnitude); /* slot 7 */
void fc4_aura_bust_strike(fc4_aura* p, uint32_t type);                                          /* slot 11 */
/* Any thread: the snapshot (false for NULL), one grid token of it. */
bool fc4_aura_get_ratings(const fc4_aura* p, fc4_aura_ratings* out);
uint64_t fc4_aura_grid(const fc4_aura* p, uint32_t which);
bool fc4_aura_get_state(const fc4_aura* p, fc4_aura_state* out);
bool fc4_aura_write(const fc4_aura* p, const fc4_ostream* out); /* §11.2 */
bool fc4_aura_read(fc4_aura* p, const fc4_istream* in);         /* §11.3; Read runs before Init */

/* ---- aura shadow (tooling; engine/capi/src/aura_shadow.rs) -----------------
 *
 * With the ORIGINAL aura simulator running, FreeCity4's pipeline follows it
 * step by step (aura.md §14.3). The plugin reads the original's state (s32
 * at +0x70) before and after each SimIdle / SimNewDay / SimNewMonth delivery
 * to its agent; when it moved, fc4_aura_shadow_step runs our steps from..to-1
 * at once (a return to 0: to the end of our cycle), reading every input live
 * through a read-only host table (write callbacks NULL; grids: geometry and
 * get only). Occupant inserted / removed reach ours when the original
 * receives them (fc4_aura_shadow_occupant). When the original's state
 * returns to 0 the plugin captures its record (its own Write into memory:
 * primitives little-endian, references as u32 index + 1 (0 = null) then u32
 * class ID, each object listed once with its grid token) and
 * fc4_aura_shadow_cycle compares it tract by tract with ours (base, aura,
 * park map, landmark map, transient; the work grid after each step 5..13;
 * mayor rating, best / worst, hotspots); ours then restarts from the record.
 * The riot roll is not compared. */
typedef struct fc4_aura_shadow fc4_aura_shadow;

typedef struct fc4_aura_capture_ref {
    uint64_t grid;     /* as cISC4SimGrid (IID 0xC9BD939A), borrowed; 0 = not a grid */
    uint32_t class_id; /* GetGZCLSID() */
    uint32_t reserved;
} fc4_aura_capture_ref;

typedef struct fc4_aura_capture {
    uint32_t struct_size;
    uint32_t ok; /* non-zero: Write returned true and the stream stayed good */
    const uint8_t* bytes;
    size_t len;
    const fc4_aura_capture_ref* refs;
    size_t ref_count;
} fc4_aura_capture;

typedef struct fc4_aura_shadow_stats {
    uint32_t struct_size;
    uint32_t cycles, compared, clean, incomplete;
    int32_t last_mismatches; /* fc4_aura_shadow_cycle's result for the last cycle; -1 = not compared */
    uint32_t rating_mismatches;
    uint32_t desynced; /* compared cycles in which our state did not follow the original's */
    uint64_t tracts, base_mismatches, aura_mismatches;
    uint64_t aura_transient; /* aura differences in tracts whose final transient differs */
} fc4_aura_shadow_stats;

/* One move of the original's state, in one delivery to its agent (ABI 30). */
typedef struct fc4_aura_shadow_step_in {
    uint32_t struct_size;
    int32_t from, to;   /* the original's state before / after (0..17) */
    int32_t julian;     /* the current date */
    uint64_t work_grid; /* the original's work grid (+0x44) as cISC4SimGrid<int8>, borrowed; 0 = not compared */
} fc4_aura_shadow_step_in;

/* `cells_x` / `cells_z`: the city's cell counts; `max_cells`: differing tracts
 * listed per grid and line (0 = 4, at most 64). */
fc4_aura_shadow* fc4_aura_shadow_create(const fc4_aura_host* host, int32_t cells_x, int32_t cells_z,
                                        uint32_t max_cells);
void fc4_aura_shadow_destroy(fc4_aura_shadow* s);
/* City load (PostCityInit): ours starts from the record (Read, then Init:
 * state 1, as the original's), with a fresh tax baseline. One line. */
bool fc4_aura_shadow_baseline(fc4_aura_shadow* s, const fc4_aura_capture* capture, fc4_bytes_sink sink, void* ctx);
/* ABI 30. Before a delivery that finds the original in `state`: for 15 and
 * 17 (they read the transient grid; 17 decays it) the original's transient
 * grid (GetTransientAuraGrid, borrowed) is copied into ours. True when copied. */
bool fc4_aura_shadow_before_step(fc4_aura_shadow* s, int32_t state, uint64_t transient_grid);
/* ABI 30. The original's state moved: our steps from..to-1 run now (to 0: to
 * the end of our cycle), and our work grid is compared after a step 5..13.
 * Returns our pipeline calls, -1 when nothing ran. */
int32_t fc4_aura_shadow_step(fc4_aura_shadow* s, const fc4_aura_shadow_step_in* step);
/* ABI 30. The original received occupant inserted (`inserted`) or removed:
 * ours receives the same. False when there is no simulator of ours yet. */
bool fc4_aura_shadow_occupant(fc4_aura_shadow* s, uint64_t occupant, bool inserted);
/* The original's state returned to 0: the cycle followed is compared with
 * its record, one line; ours then restarts from the record (state 0).
 * Returns the differences (base + aura + park map + landmark map + transient
 * tracts, work cells, one each for the rating, best / worst and hotspots),
 * -1 when nothing was compared. */
int32_t fc4_aura_shadow_cycle(fc4_aura_shadow* s, const fc4_aura_capture* capture, int32_t julian,
                              fc4_bytes_sink sink, void* ctx);
bool fc4_aura_shadow_get_stats(fc4_aura_shadow* s, fc4_aura_shadow_stats* out);

/* ---- flora (docs/specs/flora.md) -------------------------------------------
 * Two objects. fc4_flora_registry is the process-wide flora type registry
 * (§3.2): the host creates ONE per process and hands it to every simulator.
 * For each record the game can see it holds the game-layout block the host
 * built (docs/abi/layouts.md "Static flora record"; GetStaticFloraData
 * returns it); the host keeps one reference on every block for the life of
 * the process. fc4_flora is one cISC4FloraSimulator.
 *
 * Threads: every fc4_flora_* call runs under the registry's process-wide
 * re-entrant lock (one thread at a time; the owner may re-enter from a host
 * callback). The fc4_flora_registry_* getters (slots 7-14) never take it:
 * they read a snapshot, refreshed after every call that can change the
 * registry, so a second thread never waits for a month in progress.
 * A message delivered during another call is queued and handled when the
 * outermost call returns. Tokens are opaque host identities, 0 = null. */
typedef struct fc4_flora_registry fc4_flora_registry;
typedef struct fc4_flora fc4_flora;

/* A static flora record's values (§3.3), in the game layout's order. */
typedef struct fc4_flora_record {
    uint32_t type_id, next_type, seed_type;
    uint8_t water, flags, reserved[2];
    float floats[14]; /* property 0x083DC35B, in order */
    float cluster_radius, cluster_height;
    uint8_t table[256]; /* [elevation index][moisture type] */
    uint32_t unnamed, family_count;
    int64_t plop_cost;
    const uint32_t* families; /* family_count IDs, borrowed for the call (NULL when 0) */
} fc4_flora_record;

/* A flora occupant's values (§3.4). */
typedef struct fc4_flora_occupant {
    uint32_t flora_type, birth_date, last_seeding_date;
    int8_t state; /* signed: 0 young .. 4 dying */
    uint8_t orientation, reserved[2];
} fc4_flora_occupant;

typedef struct fc4_flora_param {
    uint32_t as_u32;
    float as_f32;
} fc4_flora_param;

/* A message for fc4_flora_do_message (§5). */
typedef struct fc4_flora_message {
    uint32_t msg_type, data1, data2, data3;
    uint64_t occupant;            /* GetVoid1() as a cISC4Occupant (insert / remove occupant) */
    const uint8_t* text;          /* the cheat's command line (UTF-8, not terminated; NULL = none) */
    size_t text_len;
    const fc4_flora_param* params; /* PlaceFlora's command parameters (NULL = none) */
    size_t param_count;
} fc4_flora_message;

/* Diagnostics and the status line's counters (host calls the core made,
 * by phase: SimNewMonth, SimNewDay, anything else). */
typedef struct fc4_flora_state {
    uint32_t struct_size;
    uint8_t initialized, enabled, propagation, propagation_forced, preserve, registry_loaded, pad[2];
    float size_x, size_z;
    int32_t cells_x, cells_z;
    uint32_t today, period, blocks_per_month, wild_seeds_per_month;
    int32_t iterator_count, iterator_index;
    uint32_t burnt_list;
    float burn_accumulator, burnt_removal_accumulator, burn_rate, burnt_removal_step, elevation_scale;
    /* the registry (process-wide) */
    uint32_t map_len, type_count, family_count, block_count;
    /* counters since creation */
    uint32_t created, create_failed; /* create_flora_occupant gave one / none */
    uint32_t planted;                /* inserts outside the monthly pass (tool, lots, cheats) */
    uint32_t spread;                 /* inserts during it (seeds, wild seeds) */
    uint32_t refused;                /* inserts the occupant manager refused */
    uint32_t aged;                   /* stage changes to 1 or 2 */
    uint32_t reborn;                 /* birth := today, state 0 */
    uint32_t dying;                  /* stage 4 (low score) */
    uint32_t burnt;                  /* KillFloraOccupant */
    uint32_t cleared;                /* removals during SimNewDay (burnt trees) */
    uint32_t removed_month;          /* removals during SimNewMonth */
    uint32_t removed_other;          /* demolition, buildings, networks, terrain */
    uint32_t moved;
    uint32_t blocks_visited, occupants_visited, months, days;
    uint32_t records_without_block;
    uint32_t reentrant_calls, queued_messages, lock_waits, reserved;
} fc4_flora_state;

/* The game-layout record builder. */
typedef struct fc4_flora_registry_host {
    uint32_t struct_size;
    void* user;
    /* Builds a block with the values of `rec` holding `exemplar` (its
     * reference passes to the block; 0 = none) and keeps one reference on it
     * for the life of the process. Returns its address, or 0 (the host then
     * releases `exemplar`). */
    uint64_t (*create_block)(void* user, const fc4_flora_record* rec, uint64_t exemplar);
    void (*set_block_seed)(void* user, uint64_t block, uint32_t seed_type); /* §6.11 */
    void (*release_exemplar)(void* user, uint64_t exemplar);                /* not kept */
} fc4_flora_registry_host;

/* One flora exemplar: NEW props (or NULL; taken) and one exemplar reference
 * (0 = none; taken). */
typedef void (*fc4_flora_exemplar_visitor)(void* ctx, uint32_t type_id, uint32_t group, uint32_t instance,
                                           fc4_props* props, uint64_t exemplar);

/* Everything the flora simulator asks of the game (one callback per core
 * FloraHost method, in its order). Out-pointers are written only when the
 * callback has a value. */
typedef struct fc4_flora_host {
    uint32_t struct_size;
    void* user;
    /* city, clock, resources */
    bool (*city_exists)(void* user);
    void (*city_size)(void* user, float* x, float* z);
    void (*city_cell_counts)(void* user, int32_t* x, int32_t* z);
    bool (*city_established)(void* user);
    uint32_t (*sim_date_number)(void* user);
    fc4_props* (*tuning)(void* user); /* NEW (taken) or NULL */
    /* each exemplar of type 0x6534284A in the resource manager's order */
    void (*flora_exemplars)(void* user, fc4_flora_exemplar_visitor visit, void* ctx);
    uint64_t (*obliterated_segment)(void* user);
    /* framework */
    void (*register_app_shutdown_callback)(void* user);
    void (*subscribe)(void* user, const uint32_t* ids, size_t count);
    void (*unsubscribe)(void* user, const uint32_t* ids, size_t count);
    void (*register_command)(void* user, uint32_t id, const uint8_t* name, size_t len);
    void (*unregister_command)(void* user, uint32_t id);
    void (*add_agent)(void* user, uint32_t level, const uint8_t* name, size_t len, uint32_t flags);
    void (*remove_agent)(void* user);
    void (*acquire_services)(void* user);
    void (*release_services)(void* user);
    void (*register_cheat)(void* user, uint32_t id, const uint8_t* name, size_t len);
    void (*unregister_cheat)(void* user, uint32_t id);
    void (*post)(void* user, uint32_t msg_type, uint32_t data2);
    /* terrain, lots, weather */
    bool (*terrain_initialized)(void* user);
    float (*altitude)(void* user, float x, float z);
    float (*sea_level)(void* user);
    void (*face_normal)(void* user, float x, float z, float* out3); /* out3 zeroed by the caller */
    float (*lot_savvy_altitude)(void* user, float x, float z, bool* on_lot);
    uint32_t (*moisture_type)(void* user, float x, float z);
    /* occupants */
    uint32_t (*occupant_type)(void* user, uint64_t occ);
    bool (*occupant_has_property)(void* user, uint64_t occ, uint32_t id);
    void (*occupant_position)(void* user, uint64_t occ, float* out3);
    void (*set_occupant_position)(void* user, uint64_t occ, const float* pos3);
    void (*occupant_bounding_box)(void* user, uint64_t occ, float* min3, float* max3);
    bool (*occupant_in_manager)(void* user, uint64_t occ);
    void (*set_occupant_in_manager)(void* user, uint64_t occ, bool on);
    bool (*network_flags)(void* user, uint64_t occ, uint32_t* out); /* false: no cISC4NetworkOccupant */
    bool (*flora_occupant)(void* user, uint64_t occ, fc4_flora_occupant* out); /* false: no cISC4FloraOccupant */
    void (*set_flora_type)(void* user, uint64_t occ, uint32_t v);
    void (*set_birth_date)(void* user, uint64_t occ, uint32_t v);
    void (*set_last_seeding_date)(void* user, uint64_t occ, uint32_t v);
    void (*set_flora_state)(void* user, uint64_t occ, int8_t v);
    void (*set_flora_orientation)(void* user, uint64_t occ, uint32_t v);
    void (*set_flora_record)(void* user, uint64_t occ, uint64_t block); /* 0 = null record */
    void (*post_occupant_change)(void* user, uint64_t occ);
    uint64_t (*create_flora_occupant)(void* user); /* one reference for the caller; 0 = none */
    void (*init_occupant)(void* user, uint64_t occ);
    void (*add_ref)(void* user, uint64_t occ);
    void (*release)(void* user, uint64_t occ);
    /* The occupant (as `iid`) of a reference read from a save: `token` is the
     * reference the stream handed over and passes to this call; 0 = none
     * (the host dropped it). */
    uint64_t (*resolve_ref)(void* user, uint64_t token, uint32_t iid);
    /* occupant manager */
    bool (*has_occupant_manager)(void* user);
    bool (*insert_occupant)(void* user, uint64_t occ);
    bool (*remove_occupant)(void* user, uint64_t occ);
    void (*move_occupant)(void* user, uint64_t occ);
    /* occ_type 0 with blocking = the blocking filter (§6.4.1) */
    void (*occupants_in_bbox)(void* user, const float* min3, const float* max3, uint32_t occ_type, bool blocking,
                              fc4_token_callback cb, void* ctx);
    /* rect NULL = null corners (the whole city) */
    void (*occupants_in_cells)(void* user, const fc4_cell_rect* rect, uint32_t occ_type, fc4_token_callback cb,
                               void* ctx);
    void (*occupants_in_manager_cells)(void* user, uint32_t occ_type, fc4_token_callback cb, void* ctx);
    /* obliteration segment (§10.3, ABI 30): QueryInterface for cISC4DBSegment,
     * then OpenOStream(key, &s, true) (`write`) or OpenIStream(key, &s); the
     * stream token, 0 = not open. The core puts / gets each record's u16
     * version itself. */
    uint64_t (*segment_open)(void* user, uint64_t seg, uint32_t type, uint32_t group, uint32_t instance,
                             bool write);
    void (*segment_close)(void* user, uint64_t stream); /* Close*Stream, then both references released */
    bool (*stream_put_u16)(void* user, uint64_t stream, uint16_t v);
    bool (*stream_put_u32)(void* user, uint64_t stream, uint32_t v);
    bool (*stream_get_u16)(void* user, uint64_t stream, uint16_t* out); /* false: failed, *out untouched */
    bool (*stream_get_u32)(void* user, uint64_t stream, uint32_t* out);
    bool (*stream_error)(void* user, uint64_t stream); /* GetError() != 0 */
    /* the occupant's cIGZSerializable::Write / Read on the stream */
    bool (*write_occupant)(void* user, uint64_t occ, uint64_t stream);
    bool (*read_occupant)(void* user, uint64_t occ, uint64_t stream);
} fc4_flora_host;

/* The registry (one per process; the host table is copied). */
fc4_flora_registry* fc4_flora_registry_create(const fc4_flora_registry_host* host);
void fc4_flora_registry_destroy(fc4_flora_registry* r); /* the simulators keep theirs alive */
/* Any thread: */
uint64_t fc4_flora_registry_record(const fc4_flora_registry* r, uint32_t type_id); /* slot 7; 0 = none */
/* Slot 11; 0 is valid after an Init, invalid after an exemplar-change reload until the next Init (FQ14). */
bool fc4_flora_registry_is_type_valid(const fc4_flora_registry* r, uint32_t type_id);
bool fc4_flora_registry_is_family_valid(const fc4_flora_registry* r, uint32_t id);      /* slot 12 */
/* Slot 9: NULL buffer: *count := N; else copies min(*count, N) and stores it. */
uint32_t fc4_flora_registry_types_into(const fc4_flora_registry* r, uint32_t* buffer, uint32_t* count);
/* `which` 0 = type array (registration order), 1 = every map key (ascending,
 * 0 included), 2 = family IDs. `cb` runs after the snapshot is released. */
uint32_t fc4_flora_registry_list(const fc4_flora_registry* r, uint32_t which, fc4_token_callback cb, void* ctx);
uint32_t fc4_flora_registry_family(const fc4_flora_registry* r, uint32_t family, fc4_token_callback cb, void* ctx);
/* Returns "loaded" (the first Init of the process ran). */
bool fc4_flora_registry_sizes(const fc4_flora_registry* r, uint32_t* map_len, uint32_t* types, uint32_t* families,
                              uint32_t* blocks);

/* The simulator. `token` is its own host identity; `session_seed` is the
 * session seed S (foundations §6.3, ABI 64): its cRZRandom (§3.1, §8) is
 * seeded with derive(S, 0xC990C034). */
fc4_flora* fc4_flora_create(const fc4_flora_host* host, const fc4_flora_registry* registry, uint64_t token,
                            uint32_t session_seed);
void fc4_flora_destroy(fc4_flora* f); /* never-initialised: releases the loaded burnt list */
bool fc4_flora_init(fc4_flora* f);
bool fc4_flora_shutdown(fc4_flora* f);
bool fc4_flora_do_message(fc4_flora* f, const fc4_flora_message* m); /* always true */
/* Slot 5: props NEW (taken; NULL = false), exemplar one reference (taken). */
bool fc4_flora_add_exemplar(fc4_flora* f, fc4_props* props, uint64_t exemplar);
/* Slot 6: the caller's block and its values; false = already registered
 * (no reference taken); true: the caller gives the registry one reference. */
bool fc4_flora_add_record(fc4_flora* f, const fc4_flora_record* values, uint64_t block);
bool fc4_flora_location_ok(fc4_flora* f, uint32_t type_id, float x, float z, uint64_t exclude);
uint32_t fc4_flora_best_type(fc4_flora* f, float x, float z, float* score);
/* Slot 17: pos (3 floats) is rewritten; returns the inserted occupant (borrowed) or 0. */
uint64_t fc4_flora_add_new_occupant(fc4_flora* f, uint32_t type_id, float* pos, bool force);
bool fc4_flora_add_occupant(fc4_flora* f, uint64_t occ);
bool fc4_flora_demolish_occupant(fc4_flora* f, uint64_t occ, int32_t flags);
bool fc4_flora_demolish_cells(fc4_flora* f, const fc4_cell_rect* rect, int32_t flags);
bool fc4_flora_demolish_bbox(fc4_flora* f, const float* min3, const float* max3, int32_t flags);
bool fc4_flora_kill(fc4_flora* f, uint64_t occ);
bool fc4_flora_save_obliterated(fc4_flora* f, uint64_t segment);
bool fc4_flora_get_state(const fc4_flora* f, fc4_flora_state* out);
bool fc4_flora_write(const fc4_flora* f, const fc4_ostream* out);
bool fc4_flora_read(fc4_flora* f, const fc4_istream* in);

/* ---- flora census shadow (tooling) -------------------------------------------
 * A differential of the ORIGINAL flora simulator: the registry after load
 * against one built from the same exemplars, and the census of flora
 * occupants around each monthly pass against §6.7.2's deterministic rule
 * (engine/capi/src/flora_shadow.rs says exactly what is compared). */
typedef struct fc4_flora_shadow fc4_flora_shadow;

typedef struct fc4_flora_census_entry {
    uint64_t token;
    uint32_t flora_type, birth_date, last_seeding_date;
    int8_t state;
    uint8_t in_manager, is_flora, reserved;
    float pos[3], bbox_min[3], bbox_max[3];
    uint32_t moisture; /* GetMoistureType(x, z) before the pass */
    float altitude;    /* GetAltitude(x, z) before the pass */
    uint32_t reserved2;
} fc4_flora_census_entry;

typedef struct fc4_flora_shadow_month_in {
    uint32_t struct_size;
    uint32_t julian;   /* data1 */
    uint32_t sim_date; /* GetSimDateNumber() */
    float sea_level;
    const uint8_t* before; /* the original's own Write before / after (NULL = not captured) */
    size_t before_len;
    const uint8_t* after;
    size_t after_len;
    const fc4_flora_census_entry* census_before;
    size_t before_count;
    const fc4_flora_census_entry* census_after;
    size_t after_count;
    const fc4_flora_record* records; /* the original's, from its blocks, by type_id */
    size_t record_count;
} fc4_flora_shadow_month_in;

typedef struct fc4_flora_shadow_stats {
    uint32_t struct_size;
    uint32_t months, incomplete;
    int32_t last_mismatches;
    uint32_t registry_checks, registry_mismatches;
    uint32_t occupants, visited, matched, mismatched, vanished, appeared;
    uint32_t predicted_reborn, predicted_aged, predicted_removed, predicted_seeded;
} fc4_flora_shadow_stats;

fc4_flora_shadow* fc4_flora_shadow_create(void);
void fc4_flora_shadow_destroy(fc4_flora_shadow* s);
/* tuning: NEW (taken) or NULL; returns the elevation scale (16 / 0xC9DBDD02, else 0.08). */
float fc4_flora_shadow_set_tuning(fc4_flora_shadow* s, fc4_props* tuning);
/* `exemplars` enumerates as the host's flora_exemplars (exemplar references
 * must be 0); records[i] is the record of types[i] (type_id 0 = none); key0
 * NULL = none. One line to `sink`; returns the differences (-1 = bad args). */
int32_t fc4_flora_shadow_registry(fc4_flora_shadow* s,
                                  void (*exemplars)(void* user, fc4_flora_exemplar_visitor visit, void* ctx),
                                  void* user, const uint32_t* types, size_t type_count,
                                  const fc4_flora_record* records, const fc4_flora_record* key0, bool key0_valid,
                                  fc4_bytes_sink sink, void* ctx);
/* One line to `sink`; returns the mismatches, -2 = not compared (the line
 * says why), -1 = bad arguments. */
int32_t fc4_flora_shadow_month(fc4_flora_shadow* s, const fc4_flora_shadow_month_in* m, fc4_bytes_sink sink,
                               void* ctx);
bool fc4_flora_shadow_get_stats(fc4_flora_shadow* s, fc4_flora_shadow_stats* out);

/* ---- landfill developer (docs/specs/landfill.md) ----------------------------
 *
 * cSC4LandfillDeveloper (0xEA09D4AE). Tokens: the sanitation department, the
 * zot props and every game occupant by its cISC4Occupant identity; 0 is null.
 * The core owns one reference on the department (get_department_budget hands
 * out a borrowed one; the core add_refs it on PostCityInit) and on every zot
 * (add_lot_prop hands out a borrowed one; the core add_refs it; a zot read
 * back by fc4_landfill_read arrives with one, or gets one through add_ref when
 * fc4_ref.core_handle is set). Shutdown releases them.
 *
 * Landfill tiles (§4.8.1, LG3): with insert_landfill_tile the host builds
 * each tile as a game cSC4LandfillOccupant (tile exemplar attached through
 * cISCExemplarPropertyHolder::SetDefaultExemplar) and inserts it; it is then
 * found through occupant_at, priced from its exemplar (§4.9.1, LG11),
 * removed through occupant_remove and saved by the occupant manager (§6.4,
 * LG8), like the original's. Without the callback, or when it makes none,
 * the tile is kept in this layer instead: the host's occupant callbacks
 * never see it, and only pollution's active-landfill flag shows it. A load
 * rebuilds no tile a cell already has; a flagged mask cell without one (a
 * city saved while tiles were kept here) gets its tile back on PostCityInit
 * [choice].
 *
 * A message delivered while another call into the same object is running is
 * queued and handled when that call returns.
 *
 * Threads: the core is single-threaded. The caller serialises every call on
 * one handle (the plugin holds a per-object lock). */
typedef struct fc4_landfill fc4_landfill;

/* A cell region (SC4CellRegion<long>): the inclusive rectangle and
 * (x1 - x0 + 1) * (z1 - z0 + 1) mask bytes, index (z - z0) * width + (x - x0),
 * non-zero = a mask cell. mask NULL: every cell set. A short mask is padded
 * with cleared cells; a rectangle of more than 2^20 cells is refused. */
typedef struct fc4_landfill_region {
    int32_t x0, z0, x1, z1;
    const uint8_t* mask;
    size_t len;
} fc4_landfill_region;

/* The zone effects init_zone_effects created (§4.10): one reference each for
 * the core, 0 = none. */
typedef struct fc4_landfill_effects {
    uint64_t truck;
    uint64_t seagull;
} fc4_landfill_effects;

/* Diagnostics. `moved`, `supply_delta`, `free_capacity` and
 * `capacity_reported` are the last SimNewMonth's. */
typedef struct fc4_landfill_state {
    uint32_t struct_size;
    uint8_t initialized, loaded, capacity_reported, reserved0;
    uint32_t zones, connected_zones;
    uint32_t cells;           /* mask cells over all zones */
    uint32_t heaps, valid_heaps, active_heaps;
    uint32_t zots;            /* zones with a zot prop */
    uint32_t virtual_tiles;   /* tiles kept in this layer */
    uint32_t active_cells;    /* cells this object flagged (and the rebuild found flagged) */
    uint32_t reentrant_calls;
    uint32_t capacity_total, used_total;
    int32_t free_capacity;    /* the last SetGarbageCapacity value */
    uint32_t rebuilt_tiles;   /* tiles rebuilt at the last PostCityInit after a load */
    uint64_t stored;          /* sum of contained over every heap */
    int64_t moved;            /* sum of GarbagePutInLandfill */
    int64_t supply_delta;     /* sum of AddToGarbageSupply */
    int64_t maintenance;      /* slot 7 */
    uint64_t department;      /* borrowed; 0 = none */
    uint32_t game_tiles;      /* tiles inserted through insert_landfill_tile since creation (ABI 33) */
    uint32_t game_tile_failures; /* insert_landfill_tile calls that made none (kept here instead) */
} fc4_landfill_state;

/* What the landfill developer needs from the game (one callback per
 * LandfillHost method that reaches the game, engine/core/src/landfill/mod.rs,
 * §8, plus two pollution reads). Missing callbacks read as 0 / false / no-op
 * (department funding as 1.0). Returned tokens are borrowed unless stated. */
typedef struct fc4_landfill_host {
    uint32_t struct_size; /* sizeof(fc4_landfill_host) */
    void* user;
    /* lifecycle (§1.1, §1.2) */
    void (*acquire_services)(void* user);
    void (*release_services)(void* user);
    void (*add_agent)(void* user, uint32_t level, const char* name, size_t len, uint32_t flags);
    void (*remove_agent)(void* user);
    void (*subscribe)(void* user, uint32_t msg_type);
    void (*unsubscribe)(void* user, uint32_t msg_type);
    fc4_props* (*tuning)(void* user);                                                     /* NEW or NULL */
    fc4_props* (*exemplar)(void* user, uint32_t type, uint32_t group, uint32_t instance); /* NEW or NULL */
    /* city (slots 93, 94, 97) */
    void (*cell_counts)(void* user, int32_t* x, int32_t* z);
    void (*cell_center_to_position)(void* user, int32_t x, int32_t z, float* px, float* pz);
    /* terrain (slots 13, 19, 29, 49, 68) */
    float (*altitude)(void* user, float px, float pz);
    float (*altitude_at_nearest_grid)(void* user, float px, float pz);
    float (*sea_level)(void* user);
    void (*set_altitude)(void* user, int32_t x1, int32_t z1, int32_t x2, int32_t z2, float altitude);
    void (*redisplay_terrain)(void* user, int32_t x1, int32_t z1, int32_t x2, int32_t z2);
    /* pollution simulator (slots 20, 21, 24, 29, 30, 50, 51; reads 22, 49) */
    uint32_t (*garbage_supply)(void* user);
    uint32_t (*landfill_tile_capacity)(void* user);
    int32_t (*total_garbage_uncollected)(void* user);
    void (*add_to_garbage_supply)(void* user, int32_t delta);
    void (*set_garbage_capacity)(void* user, int32_t available);
    void (*set_is_active_landfill)(void* user, int32_t x, int32_t z, bool on);
    void (*garbage_put_in_landfill)(void* user, int32_t n);
    int32_t (*garbage_capacity)(void* user);
    bool (*is_active_landfill)(void* user, int32_t x, int32_t z);
    /* budget (§4.9) */
    uint64_t (*get_department_budget)(void* user, uint32_t id);
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    float (*department_funding_percentage)(void* user, uint64_t dept);
    bool (*line_item_current_expenses)(void* user, uint64_t dept, uint32_t line, int64_t* out);
    void (*set_line_item_full_expenses)(void* user, uint64_t dept, uint32_t line, int64_t value);
    /* the first value of `property` of the occupant's property holder */
    bool (*occupant_item_cost)(void* user, uint64_t occ, uint32_t property, int64_t* out);
    /* zone manager (slots 6, 11, 12) */
    int32_t (*zone_count)(void* user, int32_t zone_type);
    int32_t (*zone_type)(void* user, int32_t x, int32_t z);
    bool (*place_zone)(void* user, const fc4_landfill_region* region, int32_t zone_type);
    /* occupants (game occupants only) */
    uint64_t (*occupant_at)(void* user, int32_t x, int32_t z, uint32_t occupant_type);
    bool (*occupant_remove)(void* user, uint64_t occ);
    void (*occupant_changed)(void* user, uint64_t occ);
    void (*occupant_position)(void* user, uint64_t occ, float* xyz);
    void (*occupant_set_position)(void* user, uint64_t occ, const float* xyz);
    void (*occupant_set_removal_flags)(void* user, uint64_t occ, uint32_t flags);
    void (*occupant_post_message)(void* user, uint64_t occ, uint32_t message);
    bool (*is_landfill_occupant)(void* user, uint64_t occ);
    bool (*is_empty_landfill_tile)(void* user, uint64_t occ);
    void (*remove_incompatible_occupants)(void* user, int32_t x, int32_t z);
    void (*set_empty_cell_texture)(void* user, int32_t x, int32_t z);
    void (*remove_base_texture)(void* user, int32_t x, int32_t z);
    /* prop manager (slots 26, 29) */
    uint64_t (*add_lot_prop)(void* user, uint32_t prop_id, const float* xyz, int32_t orientation);
    bool (*remove_prop)(void* user, uint64_t occ);
    int32_t (*zot_orientation)(void* user);
    /* traffic */
    uint64_t (*find_network_tile)(void* user, int32_t x, int32_t z, uint32_t flag_mask, bool flag);
    void (*subnetworks_in_region)(void* user, const fc4_landfill_region* region, fc4_token_callback cb, void* ctx);
    void (*occupant_count_for_all_subnetworks)(void* user, uint32_t developer_type, fc4_token_callback cb,
                                               void* ctx);
    /* lot developer (slot 21) */
    bool (*attempt_area_levelling)(void* user, int32_t x1, int32_t z1, int32_t x2, int32_t z2, float f,
                                   bool with_water, bool water_flag, float max_area_slope);
    /* 3D view */
    uint32_t (*current_view_level)(void* user);
    /* zone effects (§4.10, rendering only) */
    void (*init_zone_effects)(void* user, int32_t cx, int32_t cz, int32_t x1, int32_t z1, int32_t x2, int32_t z2,
                              fc4_landfill_effects* out);
    void (*shutdown_zone_effects)(void* user, uint64_t truck, uint64_t seagull, bool seagull_started);
    void (*set_truck_enabled)(void* user, uint64_t truck, bool enabled);
    void (*start_seagull)(void* user, uint64_t seagull, uint32_t flag);
    void (*stop_seagull)(void* user, uint64_t seagull, uint32_t flag);
    /* history (§5): MessageSend of 0x89EFA536 */
    void (*send_history)(void* user, uint32_t series, uint32_t value);
    /* tiles (§4.8.1 step 3, LG3; appended in ABI 33): build a game
     * cSC4LandfillOccupant for cell (x, z) with the tile exemplar
     * (0x6534284A, 0x07BDDF1C, tile_id) attached, the orientation, flag 4,
     * removal flags 2 and the world position xyz; Init it and insert it into
     * the occupant manager. Its cISC4Occupant token (borrowed: the occupant
     * manager holds it), 0 if none was made (the tile is then kept here). The
     * core has already set the cell's active-landfill flag. */
    uint64_t (*insert_landfill_tile)(void* user, int32_t x, int32_t z, uint32_t tile_id, int32_t orientation,
                                     const float* xyz);
} fc4_landfill_host;

/* QueryInterface (§1): whether the class answers `iid`. */
bool fc4_landfill_query_interface(uint32_t iid);
/* GetGZCLSID: 0xEA09D4AE. */
uint32_t fc4_landfill_class_id(void);
/* The table is copied; `token` is the developer's own identity. NULL for a
 * NULL or short table. */
fc4_landfill* fc4_landfill_create(const fc4_landfill_host* host, uint64_t token);
/* Releases what a never-initialised (loaded) object still holds. */
void fc4_landfill_destroy(fc4_landfill* p);
bool fc4_landfill_init(fc4_landfill* p);
/* Each initialised zone removes its zot prop (remove_prop) and releases it
 * (§4.4.9, LG13); the zot of a zone never initialised is released only.
 * Drops the tiles kept here. */
bool fc4_landfill_shutdown(fc4_landfill* p);
/* DoMessage (§3.2): `region` is the zoning message's GetVoid2 (copied), NULL
 * otherwise. Always true. */
bool fc4_landfill_do_message(fc4_landfill* p, uint32_t msg_type, uint32_t data1, uint32_t data2, uint32_t data3,
                             uint32_t data4, const fc4_landfill_region* region);
float fc4_landfill_efficiency(const fc4_landfill* p);          /* slot 5, a fraction */
int64_t fc4_landfill_maintenance_costs(const fc4_landfill* p); /* slot 7 */
int64_t fc4_landfill_current_budget(const fc4_landfill* p);    /* slot 8 */
/* Slot 9 (an int64, high dword 0); 0 for a NULL or empty region. */
int64_t fc4_landfill_zoning_cost(const fc4_landfill* p, const fc4_landfill_region* region);
void fc4_landfill_usage(const fc4_landfill* p, uint32_t* usage, uint32_t* capacity); /* slot 10 */
int32_t fc4_landfill_tile_count(const fc4_landfill* p);                              /* slot 11 */
/* The tile kept here for a cell: false when there is none (a game tile made
 * through insert_landfill_tile is not reported). */
bool fc4_landfill_tile_at(const fc4_landfill* p, int32_t x, int32_t z, uint32_t* tile_id, int32_t* orientation);
bool fc4_landfill_get_state(const fc4_landfill* p, fc4_landfill_state* out);
/* §6.1; the zot references go out with class 0. Read runs before Init. */
bool fc4_landfill_write(const fc4_landfill* p, const fc4_ostream* out);
bool fc4_landfill_read(fc4_landfill* p, const fc4_istream* in);

/* ---- landfill shadow (tooling, no spec behaviour) ----------------------------
 *
 * With the ORIGINAL developer running: before its SimNewMonth, capture its
 * record (its own Write, in the fc4_pollution_capture format with the
 * occupant field of each reference set) and read its slot 7; begin loads the
 * record into a private developer in shadow mode (every write recorded, never
 * sent; no reference taken), restores that maintenance and runs our month.
 * After the original's delivery, capture again and read slot 7 again; finish
 * compares the zones and heaps, the free capacity with garbage_capacity, the
 * garbage moved with the supply's change, the maintenance, and the
 * active-landfill flags with is_active_landfill, and hands one line to
 * `sink`. The shadow only calls the table's read callbacks. */
typedef struct fc4_landfill_shadow fc4_landfill_shadow;

typedef struct fc4_landfill_shadow_stats {
    uint32_t struct_size;
    uint32_t months, clean, incomplete;
    int32_t last_mismatches; /* -1: not compared */
    uint32_t reserved;
    uint64_t zones_compared, heaps_compared, heap_mismatches;
    uint64_t cells_compared, cell_mismatches, field_mismatches;
} fc4_landfill_shadow_stats;

/* max_diffs: the differences listed per month (0 -> 8, at most 64). */
fc4_landfill_shadow* fc4_landfill_shadow_create(const fc4_landfill_host* host, uint32_t max_diffs);
void fc4_landfill_shadow_destroy(fc4_landfill_shadow* s);
/* 0 = ran; -1 = not run (finish reports why). */
int32_t fc4_landfill_shadow_begin(fc4_landfill_shadow* s, int32_t julian, const fc4_pollution_capture* before,
                                  int64_t maintenance);
/* The difference count, -1 when not compared. */
int32_t fc4_landfill_shadow_finish(fc4_landfill_shadow* s, const fc4_pollution_capture* after, int64_t maintenance,
                                   fc4_bytes_sink sink, void* ctx);
bool fc4_landfill_shadow_get_stats(fc4_landfill_shadow* s, fc4_landfill_shadow_stats* out);
/* ---- land value shadow (tooling; engine/capi/src/landvalue_shadow.rs) -------
 *
 * With the ORIGINAL land value simulator running: at the end of each of its
 * passes (LandValueMapChanged 0x87CC75B3, posted at the end of state 8) the
 * plugin reads its grids and scalars through the interface and FreeCity4's
 * pass (states 0-8) runs on the original's live inputs through a read-only
 * host table (write callbacks NULL; our grids are local, the intrinsic grid
 * starts as a copy of the original's). Compared: intrinsic, total and slope
 * grids tract by tract, GetLandValueType at each total tract's first cell,
 * the average, the mode altitude and the record's mean altitude. */
typedef struct fc4_landvalue_shadow fc4_landvalue_shadow;

/* What the plugin read from the original (grids x outer, count_x * count_z). */
typedef struct fc4_landvalue_original {
    uint32_t struct_size;
    uint32_t has_record; /* non-zero: its own Write parsed (version 1) */
    fc4_grid_geometry intrinsic_geometry; /* slot 7 */
    const uint8_t* intrinsic;
    fc4_grid_geometry total_geometry; /* slot 8 */
    const uint8_t* total;
    fc4_grid_geometry slope_geometry; /* slot 9 */
    const float* slope;
    const uint8_t* types; /* slot 5 at (tx*S, tz*S) per total tract; NULL = not read */
    uint8_t average;        /* slot 10 */
    uint8_t record_average; /* the record's average */
    uint8_t record_flag;    /* the record's byte 5: dirty or a pass running */
    uint8_t reserved;
    float mode_altitude; /* slot 11 */
    float mean_altitude; /* the record's mean altitude (no getter) */
} fc4_landvalue_original;

typedef struct fc4_landvalue_shadow_stats {
    uint32_t struct_size;
    uint32_t passes, compared, clean;
    int32_t last_mismatches; /* -1 = not compared */
    uint32_t intrinsic_mismatches, total_mismatches, slope_mismatches, type_mismatches, scalar_mismatches;
    uint32_t desirability_reruns;  /* passes compared with a developer's previous desirability map */
    uint32_t desirability_changes; /* snapshot changes (note_day and after each comparison; LV2) */
} fc4_landvalue_shadow_stats;

/* `host`: only its read callbacks are called (create_grid, the writes,
 * release, the history and post may be NULL). `tuning`: the exemplar (copied;
 * NULL = missing: code defaults). `max_cells`: differing tracts listed per
 * grid (0 = 4, at most 64). */
fc4_landvalue_shadow* fc4_landvalue_shadow_create(const fc4_landvalue_host* host, const fc4_landvalue_tuning* tuning,
                                                  uint32_t max_cells);
void fc4_landvalue_shadow_destroy(fc4_landvalue_shadow* s);
/* SimNewDay: snapshots each listed developer's desirability map (read-only)
 * while its slot 10 (host->developer_lists_valid) is true or unknown, so the
 * snapshot before a change is the map from before the developer's last
 * state-17 copy (landvalue.md §7.4, LV2); returns how many snapshots
 * changed. Our pass applies the developers in ascending pointer order, as
 * the original does (LV1). */
uint32_t fc4_landvalue_shadow_note_day(fc4_landvalue_shadow* s);
/* The end of an original pass: one line to `sink`. Returns the differences,
 * -1 when nothing was compared (INCOMPLETE). */
int32_t fc4_landvalue_shadow_compare(fc4_landvalue_shadow* s, const fc4_landvalue_original* orig, int32_t julian,
                                     fc4_bytes_sink sink, void* ctx);
/* The counters as the last call that finished left them (callable from the
 * sink or a host callback). */
bool fc4_landvalue_shadow_get_stats(const fc4_landvalue_shadow* s, fc4_landvalue_shadow_stats* out);

/* ---- flammability shadow (tooling; engine/capi/src/flammability_shadow.rs) --
 *
 * With the ORIGINAL flammability simulator running: a mirror simulator (ours,
 * on its own grids) gets the original's six messages
 * (fc4_flammability_shadow_message, in the same delivery as the original's) and
 * reads live inputs through a read-only host table; at each of the
 * original's cycle ends (SimCycleComplete 0x08F415FE) the plugin reads the
 * original's record and getters and the shadow compares: the raw grid
 * exactly, the adjusted grid against the values the cycle could have
 * written, the average's range, the multiplier, the hotspots (every rotation
 * of the visiting order), and the getters (slots 6-8) on the resynchronised
 * mirror. The fire roll, the iterator start and the hotspot shuffle (random
 * draws) are not compared. */
typedef struct fc4_flammability_shadow fc4_flammability_shadow;

typedef struct fc4_flammability_shadow_hotspot {
    int32_t x, z, weight;
} fc4_flammability_shadow_hotspot;

typedef struct fc4_flammability_original {
    uint32_t struct_size;
    uint32_t has_record; /* non-zero: its own Write parsed (version 2) */
    uint64_t simulator;  /* the original's identity (a new one restarts the baseline) */
    fc4_grid_geometry geometry;     /* the record's adjusted grid */
    fc4_grid_geometry raw_geometry; /* the record's raw grid */
    const uint8_t* adjusted;        /* x outer */
    const uint8_t* raw;
    const fc4_flammability_shadow_hotspot* hotspots; /* the record's, in list order */
    const uint8_t* flammable;                 /* slot 6 per city cell, x outer; NULL = not read */
    uint32_t hotspot_count;
    float global_multiplier; /* the record's */
    uint8_t average;         /* the record's */
    uint8_t average_getter;  /* slot 8 */
    uint8_t hotspot_found;   /* slot 7's result */
    uint8_t hotspot_value;
    int32_t hotspot_x, hotspot_z;
    int32_t cells_x, cells_z; /* the city's cell counts (flammable) */
} fc4_flammability_original;

typedef struct fc4_flammability_shadow_stats {
    uint32_t struct_size;
    uint32_t cycles, compared, clean;
    int32_t last_mismatches; /* -1 = not compared */
    uint32_t raw_mismatches, adjusted_mismatches, unvisited, scalar_mismatches, hotspot_mismatches,
        getter_mismatches;
    uint32_t events; /* messages mirrored */
} fc4_flammability_shadow_stats;

/* `host`: only its read callbacks are called (the grid API, record_history,
 * ignite_cell, set_fire_cause, random_lot and post may be NULL).
 * `max_cells`: differing tracts listed per grid (0 = 4, at most 64). */
fc4_flammability_shadow* fc4_flammability_shadow_create(const fc4_flammability_host* host, uint32_t max_cells);
void fc4_flammability_shadow_destroy(fc4_flammability_shadow* s);
/* One of the original's six messages; false when not mirrored (no baseline
 * yet, or another type). SimNewDay (0x66956814) means the original's day
 * delivery has just finished: the day is counted and the watered state
 * polled (§6.5, FL7/FL8). Until the first such day, every other call (e.g.
 * the plumbing pass end WaterMapChanged 0x6946DC34) first polls it. A poll
 * reads host->is_cell_watered for every tract and compares by content (the
 * host->watered_grid pointer is only counted; NULL = nothing polled): the
 * tracts whose state changed are marked for the cycle (FL5-FL7). */
bool fc4_flammability_shadow_message(fc4_flammability_shadow* s, const fc4_flammability_message* m);
/* The original's cycle end: the first (or after a new simulator) is the
 * baseline; later ones are compared (after a poll; a tract marked in the
 * cycle may hold its value under either watered state). The marks of the
 * changes seen after the last wrap day (by the day count since the
 * original's Init) are carried into the next cycle (FL8). One line. Returns
 * the differences, -1 when nothing was compared. */
int32_t fc4_flammability_shadow_cycle(fc4_flammability_shadow* s, const fc4_flammability_original* orig,
                                      int32_t julian, fc4_bytes_sink sink, void* ctx);
/* The counters as the last call that finished left them (callable from the
 * sink or a host callback). */
bool fc4_flammability_shadow_get_stats(const fc4_flammability_shadow* s, fc4_flammability_shadow_stats* out);

/* ---- residential (docs/specs/residential.md, residential-education-health.md) ----
 *
 * The residential simulator, education and health included (class 0x4990C013).
 * Tokens: occupants (the cISC4Occupant identity), departments
 * (cISC4DepartmentBudget), line items, visual effects, grids; 0 is null. The
 * residential grids are the game's own cSC4SimGrid<T> through fc4_grid_api
 * (uint8 0x49B9E602, uint16 0x49B9E604, float 0x49B9E60A); the building map
 * and the growth developers' population maps are BORROWED grid tokens read
 * through the same table.
 *
 * Threads: the core is main-thread only. After every outermost main-thread
 * call a snapshot and a detached copy of the state are published; a getter on
 * the main thread with no call running uses the core, anywhere else it runs on
 * a throw-away view of the detached copy (grids borrowed, writes ignored). A
 * state change made anywhere else (a message, a strike end, a funding or
 * auto-budget setter) is queued, its occupant held, and handled when the next
 * outermost main-thread call returns. Init, Shutdown, Read, get_state and
 * set_work_budget are refused off the main thread or during a call. */
typedef struct fc4_residential fc4_residential;

/* `which` of fc4_residential_buildings (slots 47, 49, 61). */
#define FC4_RESIDENTIAL_LIST_SCHOOL           0u
#define FC4_RESIDENTIAL_LIST_HOSPITAL         1u
#define FC4_RESIDENTIAL_LIST_HEALTH_AFFECTING 2u

/* GetBudgetItemForPurpose's item (budget.md §2). */
typedef struct fc4_residential_budget_item {
    uint32_t department;
    uint32_t line; /* 0 = use the building type */
    uint32_t purpose;
    uint32_t index;
    int64_t cost;
    float local_funding; /* a fraction (1.0 = 100 %) */
    uint32_t reserved;
} fc4_residential_budget_item;

/* The query record of slots 14, 15, 26 and 27 (REH §4.11): the game record's
 * fields after its leading cRZString (+0x14 .. +0x38 in order), the two costs
 * widened to 64 bits. */
typedef struct fc4_residential_query_data {
    uint32_t capacity, effective_capacity, capacity_per_staff, effective_per_staff, served;
    float grade, staff_funding;
    uint32_t reserved0;
    int64_t staff_cost;
    float vehicle_funding;
    uint32_t reserved1;
    int64_t vehicle_cost;
} fc4_residential_query_data;

/* Slots 43 / 44. */
typedef struct fc4_residential_min_max {
    uint32_t min_x, min_z, max_x, max_z;
    float min, max;
} fc4_residential_min_max;

/* What any thread may read: published after every outermost main-thread call.
 * Grid tokens are borrowed (0 = none). */
typedef struct fc4_residential_snapshot {
    uint32_t struct_size;
    uint8_t initialized;
    uint8_t school_strike, health_strike;             /* slots 6 / 7 */
    uint8_t school_auto_budget, hospital_auto_budget; /* slots 52 / 54 */
    uint8_t pad[3];
    uint32_t state;                                         /* the daily state machine (§5.1) */
    int32_t population;                                     /* slot 16 */
    int32_t school_building_count, hospital_building_count; /* slots 46 / 48 */
    int32_t population_size, eq_size;
    uint32_t reserved;
    uint64_t proximity[3];                 /* slot 5 (R$, R$$, R$$$) */
    uint64_t population_grids[3];          /* slot 25 */
    uint64_t age;                          /* slot 41 */
    uint64_t school_grade, hospital_grade; /* slots 39 / 40 */
    uint64_t eq[3], hq[3];                 /* slots 23 / 24 */
} fc4_residential_snapshot;

/* Diagnostics (main thread with no call running). */
typedef struct fc4_residential_state {
    uint32_t struct_size;
    uint8_t initialized, city_started, dirty, tuning_loaded;
    uint8_t school_strike, health_strike, school_auto_budget, hospital_auto_budget;
    uint32_t state;
    int32_t population, running_population;
    uint32_t population_by_wealth[3]; /* the sum of each population grid's tracts */
    uint32_t global_le;               /* slot 34 */
    uint64_t le_sum;
    float global_eq, global_hq;               /* slots 32 / 33 */
    int32_t school_covered, hospital_covered; /* the last coverage states' counters (REH §4.4) */
    int32_t school_buildings, hospital_buildings;
    uint32_t pending_school, pending_hospital, health_affecting, other;
    int32_t population_size, eq_size;
    int32_t tracts_x, tracts_z;     /* the population R$ grid */
    int32_t iterator_x, iterator_z; /* state 4's scan position */
    int32_t school_strike_start, health_strike_start;
    uint32_t departments, grids; /* held (of 4 / of 25) */
    uint32_t reentrant_calls, deferred_calls, view_calls;
    uint32_t reserved;
} fc4_residential_state;

/* create_visual_effect: the host cannot show effects (RG1; treated as
 * created). */
#define FC4_EFFECT_NOT_SHOWN UINT64_MAX

/* What the residential simulator needs from the game: one callback per
 * ResidentialHost / EducationHealthHost method of the core
 * (engine/core/src/residential), in the traits' order, plus hand_out. Missing
 * callbacks behave as "nothing there" (0, false, no object); a missing
 * department_funding, or a NaN from it, is a department whose funding cannot
 * be read, and the output keeps the caller's value (REH §4.2, RG10). */
typedef struct fc4_residential_host {
    uint32_t struct_size; /* sizeof(fc4_residential_host) */
    void* user;
    fc4_grid_api grids;
    /* lifecycle (§4.1, §4.2) */
    void (*acquire_services)(void* user);
    void (*acquire_developers)(void* user);
    bool (*acquire_demand_node)(void* user);
    bool (*acquire_message_server)(void* user);
    bool (*acquire_budget_simulator)(void* user);
    bool (*has_simulator)(void* user);
    void (*add_agent)(void* user, uint32_t level, const char* name, uint32_t flags);
    void (*remove_agent)(void* user);
    void (*subscribe)(void* user, uint32_t msg_type);
    void (*unsubscribe)(void* user, uint32_t msg_type);
    void (*release_message_server_and_budget)(void* user);
    void (*release_services)(void* user);
    void (*set_history_queue_length)(void* user, uint32_t series, uint32_t length);
    /* CreateDepartmentBudget, AddRef, name, max and initial funding (and the
     * secondary departments' local line item): a token holding one reference,
     * 0 = none. */
    uint64_t (*create_department)(void* user, uint32_t id, uint32_t name, float max_funding, float initial_funding);
    void (*release_department)(void* user, uint64_t dept);
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    /* AddRef for a reference handed to the game (slots 21, 22, 47, 49, 61);
     * NULL = add_ref. */
    void (*hand_out)(void* user, uint64_t token);
    /* resources (§7): a NEW fc4_props (the core destroys it), NULL if missing */
    fc4_props* (*tuning)(void* user);
    /* the monthly pass's inputs (§5.3, §5.5) */
    uint64_t (*building_map)(void* user); /* BDS slot 41: BORROWED cISC4SimGrid<uint8>, 0 = none */
    uint8_t (*building_map_key)(void* user, uint32_t purpose, uint32_t wealth);
    /* growth developer slot 15 of the developer type: BORROWED cISC4SimGrid<int16>, 0 = none */
    uint64_t (*developer_population_map)(void* user, uint32_t developer_type);
    uint16_t (*pollution_value)(void* user, uint32_t kind, int32_t cell_x, int32_t cell_z);
    uint32_t (*sim_month)(void* user);
    uint32_t (*sim_day_number)(void* user);
    float (*altitude)(void* user, float x, float z);
    /* visual effects (§5.5 step 2, REH §4.7 step 3, RG1): the effect (one
     * reference, released by start_and_release_effect), 0 = the create
     * failed (one draw), FC4_EFFECT_NOT_SHOWN = the host cannot show effects
     * (treated as created: both draws, the flag). NULL = cannot show. */
    uint64_t (*create_visual_effect)(void* user, const char* name);
    void (*set_effect_transform)(void* user, uint64_t effect, const float* translation_xyz);
    void (*start_and_release_effect)(void* user, uint64_t effect);
    /* messages (§5.7, §8, REH §5) */
    void (*record_history)(void* user, uint32_t series, int32_t value);
    void (*post_strike_message)(void* user, uint32_t msg_type, uint64_t occ, uint32_t data2, uint32_t data3);
    /* occupants (§4.3, §6.6) */
    uint64_t (*occupant_as_building)(void* user, uint64_t occ);
    bool (*is_occupant_group)(void* user, uint64_t occ, uint32_t group);
    bool (*occupant_bool_property)(void* user, uint64_t occ, uint32_t id, bool* out);
    bool (*set_occupant_bool_property)(void* user, uint64_t occ, uint32_t id, bool value);
    /* a reference read with `iid` (one reference handed over): the token the
     * core keeps, 0 = refused */
    uint64_t (*resolve_ref)(void* user, uint64_t token, uint32_t iid);
    /* education and health: budget (REH §4.1, §4.2) */
    bool (*budget_item)(void* user, uint64_t occ, uint32_t purpose, fc4_residential_budget_item* out);
    void (*change_budget_item_line)(void* user, uint64_t occ, uint32_t index, uint32_t line);
    uint64_t (*department_budget)(void* user, uint32_t id); /* borrowed, 0 = none */
    float (*department_funding)(void* user, uint64_t dept); /* NaN = cannot be read (RG10) */
    bool (*is_locally_funded)(void* user, uint64_t dept, uint64_t occ, uint32_t purpose);
    void (*add_locally_funded)(void* user, uint64_t dept, uint64_t occ, uint32_t purpose);
    void (*remove_locally_funded)(void* user, uint64_t dept, uint64_t occ, uint32_t purpose);
    void (*set_local_funding)(void* user, uint64_t dept, uint64_t occ, float fraction, uint32_t purpose);
    uint64_t (*line_item)(void* user, uint64_t dept, uint32_t line); /* borrowed, 0 = none */
    uint64_t (*create_line_item_for_building_type)(void* user, uint64_t dept, uint32_t line, bool local_funding);
    void (*remove_line_item)(void* user, uint64_t dept, uint32_t line);
    int64_t (*line_item_count)(void* user, uint64_t line_item);
    void (*set_line_item_count)(void* user, uint64_t line_item, int64_t count);
    void (*set_line_item_display_flag)(void* user, uint64_t line_item, uint32_t flag, bool on);
    /* education and health: occupants (REH §2, §4.1, §4.4-§4.6) */
    bool (*occupant_property)(void* user, uint64_t occ, uint32_t id, fc4_props* out);
    void (*set_occupant_u32)(void* user, uint64_t occ, uint32_t id, uint32_t value);
    void (*set_occupant_f32)(void* user, uint64_t occ, uint32_t id, float value);
    uint32_t (*building_type)(void* user, uint64_t occ);
    uint32_t (*building_slot11)(void* user, uint64_t occ);
    bool (*occupant_lot_center)(void* user, uint64_t occ, float* xz); /* false = no lot */
    void (*occupant_position)(void* user, uint64_t occ, float* xz);
    /* ordinances (REH §4.3) */
    void (*ordinances_on)(void* user, fc4_u32_callback cb, void* ctx);
    fc4_props* (*ordinance_properties)(void* user, uint32_t id); /* a NEW fc4_props, NULL = skip it */
    /* automata (REH §4.9) */
    void (*add_controllers)(void* user, const char* group_name, uint64_t occ);
    void (*remove_controllers)(void* user, const char* group_name, uint64_t occ);
} fc4_residential_host;

/* Constructor values (§3). `token`: the simulator's own identity; the class's
 * cRZRandom is seeded with derive_seed(session_seed, 0x4990C013). The calling
 * thread becomes the main thread. */
fc4_residential* fc4_residential_create(const fc4_residential_host* host, uint64_t token, uint32_t session_seed);
/* Queued calls are dropped (their occupants released); grids still held are released. */
void fc4_residential_destroy(fc4_residential* p);
bool fc4_residential_init(fc4_residential* p);     /* §4.1: false without the demand node, server or simulator */
bool fc4_residential_shutdown(fc4_residential* p); /* §4.2 */
/* §4.3: the step's "finished" value (false only when a time-sliced NewDay
 * state ran out of budget). `void1`: the occupant token of the occupant
 * messages. Queued (true) off the main thread or during a call. */
bool fc4_residential_do_message(fc4_residential* p, uint32_t msg_type, uint32_t data1, uint32_t data2, uint32_t data3,
                                uint64_t void1);
void fc4_residential_set_work_budget(fc4_residential* p, uint32_t units); /* §5.2 [choice]; default unbounded */
bool fc4_residential_get_snapshot(const fc4_residential* p, fc4_residential_snapshot* out); /* any thread */
bool fc4_residential_get_state(const fc4_residential* p, fc4_residential_state* out);
/* §9.2: the version-7 record; grids as fc4_ref {token, grid CLSID}, occupants
 * and departments as {token, 0}. Off the main thread the detached state is
 * written. */
bool fc4_residential_write(const fc4_residential* p, const fc4_ostream* out);
bool fc4_residential_read(fc4_residential* p, const fc4_istream* in); /* §9.2; before Init */
/* Slots 8 / 9: whether a strike was ended. Off the main thread the end is
 * queued and the answer is the published strike flag. */
bool fc4_residential_end_strike(fc4_residential* p, bool school);
float fc4_residential_strike_chance(const fc4_residential* p, bool school); /* 10 / 11 */
float fc4_residential_system_rating(const fc4_residential* p, bool school); /* 12 / 13 */
/* 14 / 15 (REH §4.11, EH12): resets `totals` +0x14..+0x28 (in/out; the rest
 * untouched), adds the query fields of every building that has one of
 * `groups`, whatever its query returned, and returns their number; `groups`
 * NULL = no group. */
uint32_t fc4_residential_system_totals(const fc4_residential* p, bool school, const uint32_t* groups,
                                       uint32_t group_count, fc4_residential_query_data* totals);
bool fc4_residential_set_occupant_funding(fc4_residential* p, uint64_t occ, float staff, float vehicle,
                                          bool school); /* 19: true */
/* 20: `staff` / `vehicle` in/out; true. */
bool fc4_residential_get_occupant_funding(const fc4_residential* p, uint64_t occ, float* staff, float* vehicle,
                                          bool school);
/* 21 / 22: a NEW float grid per call handed out with one reference (0 = none);
 * `min` / `max` get the range (255 / 0 without a grid). */
uint64_t fc4_residential_average_grid(const fc4_residential* p, bool is_eq, float* min, float* max);
/* 26 / 27: the query record (in/out); false at the first missing field, with
 * the fields read before it written (EH10). */
bool fc4_residential_get_query_data(const fc4_residential* p, bool school, uint64_t occ, fc4_residential_query_data* data);
bool fc4_residential_estimate_capacity(const fc4_residential* p, uint64_t occ, uint32_t* capacity,
                                       uint32_t* staff); /* 28: true */
uint8_t fc4_residential_cell_life_expectancy(const fc4_residential* p, int32_t x, int32_t z);     /* 29 */
float fc4_residential_cell_workforce(const fc4_residential* p, int32_t x, int32_t z);           /* 30 */
float fc4_residential_global_workforce(const fc4_residential* p);                               /* 31 */
float fc4_residential_global_quotient(const fc4_residential* p, bool is_eq);                    /* 32 / 33 */
uint8_t fc4_residential_global_le(const fc4_residential* p);                                    /* 34 */
float fc4_residential_cell_quotient(const fc4_residential* p, bool is_eq, int32_t x, int32_t z); /* 35 / 36 */
float fc4_residential_cell_quotient_by_wealth(const fc4_residential* p, bool is_eq, int32_t x, int32_t z,
                                              uint8_t wealth); /* 37 / 38 (wealth 1..3) */
uint8_t fc4_residential_new_age_by_wealth(const fc4_residential* p, uint8_t wealth); /* 42 */
/* 43 / 44: false (all outputs 0) when the population is 0. */
bool fc4_residential_quotient_min_max(const fc4_residential* p, bool is_eq, fc4_residential_min_max* out);
/* 45: false (outputs untouched) for a building that is neither a school nor a hospital. */
bool fc4_residential_occupant_coverage(const fc4_residential* p, uint64_t occ, float level, float* distance,
                                       float* radius);
/* 47 / 49 / 61 (FC4_RESIDENTIAL_LIST_*): each occupant goes to `sink` with one
 * reference handed out for the caller's list. `groups` NULL = every group;
 * slot 61 ignores `groups`. Returns the count. */
uint32_t fc4_residential_buildings(const fc4_residential* p, uint32_t which, const uint32_t* groups,
                                   uint32_t group_count, fc4_token_callback sink, void* ctx);
bool fc4_residential_set_global_auto_budget(fc4_residential* p, bool school, bool on); /* 53 / 55: the value */
bool fc4_residential_auto_budget(const fc4_residential* p, uint64_t occ);              /* 56 */
bool fc4_residential_set_auto_budget(const fc4_residential* p, uint64_t occ, bool on); /* 57 */
bool fc4_residential_ideal_funding(const fc4_residential* p, uint64_t occ, float* out); /* 58 */

/* ---- residential shadow (tooling) ---------------------------------------------
 *
 * With the ORIGINAL residential simulator running, each SimNewDay delivery to
 * its agent is wrapped (one state of the monthly cycle per day, §5.1): before
 * it, the plugin captures the original's record (its own Write into a capture
 * stream) and calls fc4_residential_shadow_begin, which copies the 25 live
 * grids the record refers to, loads the record into a private core over the
 * copies, restores the simulator tuning and ordinance effects from the game
 * and runs FreeCity4's daily state, reading every input through the host
 * table (read callbacks and the grid table's geometry / get only; nothing is
 * written to the game). States 1, 2 and 4 run at the finish instead, where
 * the original's progress is known (a state it stopped part-way runs as far
 * as it got). After the delivery the plugin captures the record again, reads
 * the original's population, global EQ / HQ and LE, and calls
 * fc4_residential_shadow_finish, which compares the 25 grids cell by cell,
 * the saved fields, the occupant properties and local funding, and the four
 * scalars. fc4_residential_shadow_describe gives the one start-up line of
 * what is not readable (hence not compared). */
typedef struct fc4_residential_shadow fc4_residential_shadow;

/* One reference of a captured record, queried for each interface the reader
 * asks for (0 = the object does not answer it); borrowed identities. */
typedef struct fc4_residential_capture_ref {
    uint64_t grid;       /* cISC4SimGrid 0xC9BD939A */
    uint64_t occupant;   /* cISC4Occupant 0x26BB767E */
    uint64_t department; /* cISC4DepartmentBudget 0x223E2B67 */
    uint32_t class_id;   /* GetGZCLSID() */
    uint32_t reserved;
} fc4_residential_capture_ref;

/* A captured record: primitives little-endian, references as uint32
 * objectID (index into `refs` + 1, 0 = null) then uint32 classID. */
typedef struct fc4_residential_capture {
    uint32_t struct_size;
    uint32_t ok; /* Write returned true and the stream stayed good */
    const uint8_t* bytes;
    size_t len;
    const fc4_residential_capture_ref* refs;
    size_t ref_count;
} fc4_residential_capture;

/* The original's getters after its delivery. */
typedef struct fc4_residential_shadow_scalars {
    uint32_t struct_size;
    int32_t population;         /* slot 16 */
    float global_eq, global_hq; /* slots 32 / 33 */
    uint32_t global_le;         /* slot 34 (AL, zero-extended) */
    uint32_t reserved;
} fc4_residential_shadow_scalars;

typedef struct fc4_residential_shadow_stats {
    uint32_t struct_size;
    uint32_t deliveries, compared, clean, incomplete;
    uint32_t idle;            /* state-10 days (not compared) */
    uint32_t cycles;          /* monthly cycles seen to end (state 9 -> 10) */
    int32_t last_mismatches;  /* -1: not compared */
    uint64_t cells, cell_mismatches, field_mismatches;
    uint64_t prop_mismatches; /* occupant properties and local funding */
    uint64_t scalar_mismatches;
} fc4_residential_shadow_stats;

/* `max_cells`: differing cells listed per grid and line (0 = 4, at most 64). */
fc4_residential_shadow* fc4_residential_shadow_create(const fc4_residential_host* host, uint32_t max_cells);
void fc4_residential_shadow_destroy(fc4_residential_shadow* s);
/* Before a SimNewDay delivery (`julian` = its data1): 0 = ready, -1 = not
 * run (finish reports why), -2 = state 10 (the original does nothing; a
 * finish returns -2 and logs nothing). */
int32_t fc4_residential_shadow_begin(fc4_residential_shadow* s, int32_t julian, const fc4_residential_capture* before);
/* The delivery's difference count (-1 = not compared, -2 = nothing to
 * compare); log lines go to `sink`, one call per line: one per delivery
 * ("residential shadow YYYY-MM-DD state N (name): ...; mismatches=N", or
 * INCOMPLETE and why) and, after the delivery that ends a cycle, the cycle
 * line. `scalars` NULL: not compared. */
int32_t fc4_residential_shadow_finish(fc4_residential_shadow* s, const fc4_residential_capture* after,
                                      const fc4_residential_shadow_scalars* scalars, fc4_bytes_sink sink, void* ctx);
bool fc4_residential_shadow_get_stats(fc4_residential_shadow* s, fc4_residential_shadow_stats* out);
/* One line: what is not readable, hence not compared. */
void fc4_residential_shadow_describe(fc4_bytes_sink sink, void* ctx);

/* ---- development (docs/specs/building-development.md, growth-developer.md) ----
 *
 * The building development simulator (class 0x8990BE7A) and the growth
 * developers (class 0x0990C138), replaced as one unit. The host implements
 * fc4_development_host (every DevelopmentHost / GrowthDevelopers method of
 * engine/core/src/development_simulator, in the traits' order) and
 * fc4_growth_host (engine/core/src/growth_developer); the plugin's classes
 * forward their slots to the fc4_development_* / fc4_growth_* functions.
 *
 * The process-wide statics (BDS §2: the building catalogue, the tried-tract
 * marks and the BDS generator; growth §7: the salt generator and the time
 * slice step) are one fc4_development_process and one fc4_growth_statics the
 * host creates once per process. The catalogue is exported from the process
 * (fc4_development_catalogue_*) so the host can keep the game-layout copies
 * slots 11-22, 27-28, 37-40, 42-43 and 45 return.
 *
 * Tokens are opaque host identities: developers (cISC4GrowthDeveloper*),
 * lots, occupants, the construction node (cISC4Demand*) and grids
 * (fc4_grid_api). 0 is the null object. The cores own one reference on every
 * developer, special building, node, source demand and source map token they
 * keep, and on every occupant create_building_occupant or
 * create_foundation_occupant hands over (slot 23 releases its foundation
 * once the occupant manager holds it). A saved
 * reference read back for one of the host's objects arrives with that
 * reference; one naming a FreeCity4 object (fc4_ref::core_handle set) is
 * given one (add_ref). Lot tokens are borrowed.
 *
 * Re-entrancy: no core borrow is held across a host callback; a message
 * delivered while a call on the same handle is running is queued until the
 * outermost call returns, other re-entrant calls run at once (counted in
 * reentrant_calls). Threads: single-threaded cores; the plugin serialises
 * every call on the BDS, its process statics and every growth developer with
 * one process-wide lock. */
typedef struct fc4_development_process fc4_development_process;
typedef struct fc4_development fc4_development;
typedef struct fc4_growth_statics fc4_growth_statics;
typedef struct fc4_growth fc4_growth;

typedef struct fc4_res_key {
    uint32_t type_id, group, instance;
} fc4_res_key;

/* GrowthProfile: the developer token, the tract and its stage (the tract
 * developer reads the low byte). */
typedef struct fc4_growth_profile {
    uint64_t developer;
    int32_t x, z, stage;
    uint32_t reserved;
} fc4_growth_profile;

/* One growth or decline list entry: the game's 12-byte {x, z, stage}. */
typedef struct fc4_growth_item {
    int32_t x, z, max_stage;
} fc4_growth_item;

/* ConstructionProfile in the game's layout (BDS §4.8: 0x24 bytes). */
typedef struct fc4_construction_profile {
    int32_t zone;
    int32_t unknown2;
    uint32_t wealth, purpose;
    uint8_t land_value_min, land_value_max;
    uint8_t reserved[2];
    uint32_t building_type;
    int32_t orientation;
    int32_t x, z;
} fc4_construction_profile;

/* One lot configuration as the maximum density table reads it (§4.7.4). */
typedef struct fc4_lot_config {
    uint32_t size_x, size_z;
    uint32_t zones;       /* bit z set: IsCompatibleWithZoneType(z), z in 0..16 */
    uint32_t has_primary; /* 1: a primary building configuration object */
    uint32_t orientation;
    float x, z;
    uint32_t n_ids;
    const uint32_t* ids; /* the object's building or family IDs */
} fc4_lot_config;

/* The catalogue's building profile (§2; the host builds the game layout). */
typedef struct fc4_building_profile {
    uint32_t purpose, wealth;
    uint16_t capacity;
    uint8_t value_499afa38;
    uint8_t flag_27812836;
    uint32_t category_flags;
    float occupant_size[3];
    uint32_t reserved0;
    int64_t plop_cost;
    uint32_t n_capacities;       /* capacity table entries (handed to the callback) */
    uint32_t capacities_present; /* 1: property 0x27812834 exists, so one capacity map, even empty (DG11) */
} fc4_building_profile;

typedef struct fc4_development_message {
    uint32_t msg_type, data1, data2, data3, data4;
    uint32_t reserved;
    uint64_t occupant; /* GetVoid1() as a cISC4Occupant (occupant messages), 0 otherwise */
    uint64_t lot;      /* GetIGZUnknown() as a cISC4Lot (StateChanged), 0 otherwise */
} fc4_development_message;

/* One PlaceBuilding command parameter. */
typedef struct fc4_command_param {
    uint32_t as_u32;
    float as_f32;
} fc4_command_param;

/* The advice subject §4.10 builds and discards. */
typedef struct fc4_advice_subject {
    float x, z;
    uint32_t data1;
    uint32_t reserved;
    uint64_t lot;
} fc4_advice_subject;

/* One decline plan entry. */
typedef struct fc4_plan_entry {
    uint32_t day;
    int32_t x, z;
    uint32_t stage;
    uint64_t developer;
} fc4_plan_entry;

/* Diagnostics; the counters are cumulative since the handle was created. */
typedef struct fc4_development_state {
    uint32_t struct_size; /* = sizeof(fc4_development_state) */
    uint8_t initialized, loaded;
    uint8_t state;  /* the planning state machine, 0-4 (§4.2) */
    uint8_t cheats; /* bit 0 ignore plumbing, 1 power, 2 caps, 3 debug growth, 4 coffee bar */
    uint32_t developers, special_buildings, highest_stages, decline_plan;
    uint32_t marked_types, marked_cells;
    uint32_t building_types; /* in the process catalogue */
    uint32_t grow_budget;    /* §4.4.1; 0xFFFFFFFF = unlimited */
    int32_t total_resources, free_resources;
    float supply, demand;
    uint32_t grow_calls, built;       /* tract developer Grow calls, those > 0 */
    uint32_t decline_calls, declined; /* tract developer Decline calls, those > 0 */
    uint32_t built_by_wealth[4];      /* by the developer's wealth */
    uint32_t declined_by_wealth[4];
    uint32_t built_by_stage[16]; /* by the profile's stage (larger count as 15) */
    uint32_t reentrant_calls;
    int32_t cell_count_x, cell_count_z;
    uint32_t reserved;
    uint64_t rng_steps; /* of the process-wide BDS generator */
    uint64_t building_map;
    uint64_t construction_node;
} fc4_development_state;

/* One exemplar: NEW props (the callee owns them; may be NULL) and the
 * localized building name (property 0x8A416A99) when has_name. */
typedef void (*fc4_development_exemplar_visitor)(void* ctx, const fc4_res_key* key, fc4_props* props,
                                                 const uint8_t* name, size_t name_len, bool has_name);
typedef void (*fc4_lot_config_visitor)(void* ctx, const fc4_lot_config* config);
/* A growth or decline list, delivered in one call. */
typedef void (*fc4_growth_items_sink)(void* ctx, const fc4_growth_item* items, size_t n);
typedef void (*fc4_type_token_callback)(void* ctx, uint32_t key, uint64_t token);
typedef void (*fc4_type_value_callback)(void* ctx, uint32_t key, uint32_t value);
typedef void (*fc4_plan_entry_callback)(void* ctx, const fc4_plan_entry* e);
/* One maximum density entry: zone, building map key, the raw entry and
 * GetMaxCapacity's value for it (float(trunc(entry))). */
typedef void (*fc4_density_callback)(void* ctx, uint32_t zone, uint32_t key, float entry, float max_capacity);
typedef void (*fc4_key_float_callback)(void* ctx, uint32_t key, float value);

typedef struct fc4_development_host {
    uint32_t struct_size; /* sizeof(fc4_development_host) */
    void* user;
    fc4_grid_api grids;
    /* clock, city, terrain */
    uint32_t (*sim_day)(void* user);
    uint32_t (*difficulty)(void* user);
    void (*cell_widths)(void* user, float* out2);
    void (*terrain_cell_counts)(void* user, int32_t* out2);
    void (*position_to_cell)(void* user, float x, float z, int32_t* out2);
    void (*cell_center)(void* user, int32_t x, int32_t z, float* out2); /* the centre's (x, z) */
    float (*altitude)(void* user, float x, float z);
    /* resources: every exemplar whose ExemplarType is one of `types` (all
     * when n_types is 0), in the resource key list's order */
    void (*exemplars)(void* user, const uint32_t* types, size_t n_types, fc4_development_exemplar_visitor visit,
                      void* ctx);
    void (*lot_configurations)(void* user, fc4_lot_config_visitor visit, void* ctx);
    fc4_props* (*tuning)(void* user); /* NEW props (the core destroys them), NULL if missing */
    /* framework */
    void (*register_static_cleanup)(void* user);
    void (*post_building_types_updated)(void* user, uint32_t count);
    void (*subscribe)(void* user, const uint32_t* types, size_t n);
    void (*unsubscribe)(void* user, const uint32_t* types, size_t n);
    void (*register_command)(void* user, uint32_t id, const char* name, size_t len);
    void (*unregister_command)(void* user, uint32_t id);
    void (*add_agent)(void* user, uint32_t level, const char* name, size_t len, uint32_t flags);
    void (*remove_agent)(void* user);
    void (*register_cheat)(void* user, uint32_t id, const char* name, size_t len);
    void (*unregister_cheat)(void* user, uint32_t id);
    void (*acquire_services)(void* user);
    void (*release_services)(void* user);
    /* demand */
    bool (*census_supply)(void* user, uint32_t id, float* out);
    uint64_t (*create_construction_node)(void* user); /* new cSC4Demand, one reference, not Inited */
    void (*node_init)(void* user, uint64_t node);
    void (*node_shutdown)(void* user, uint64_t node);
    float (*node_supply)(void* user, uint64_t node);
    float (*node_demand)(void* user, uint64_t node);
    void (*node_set_supply)(void* user, uint64_t node, float v);
    void (*node_set_demand)(void* user, uint64_t node, float v);
    void (*node_add_to_demand)(void* user, uint64_t node, float v);
    bool (*node_end_of_cycle)(void* user, uint64_t node);
    float (*demand_active_value)(void* user, uint64_t node); /* a developer's source node */
    /* tract developer: Grow (the host makes and frees the outcome list; flag
     * arrives false) and Decline */
    int32_t (*tract_grow)(void* user, const fc4_growth_profile* profile, bool* flag);
    int32_t (*tract_decline)(void* user, const fc4_growth_profile* profile);
    /* lots, lot manager, lot developer */
    uint32_t (*lot_building_type)(void* user, uint64_t lot);
    void (*lot_bounding_rect)(void* user, uint64_t lot, fc4_cell_rect* out);
    bool (*lot_growth_stage)(void* user, uint64_t lot, uint8_t* out);
    uint64_t (*lot_building)(void* user, uint64_t lot);
    uint32_t (*lot_state)(void* user, uint64_t lot);
    void (*lot_center)(void* user, uint64_t lot, float* out3);
    float (*lot_slope)(void* user, uint64_t lot);
    bool (*lot_demolishable)(void* user, uint64_t lot);
    uint64_t (*lot_at)(void* user, int32_t x, int32_t z); /* GetLot(x, z, true), borrowed */
    /* borrowed tokens kept alive until the outermost BDS call returns */
    void (*lots_in_cell_rect)(void* user, const fc4_cell_rect* rect, fc4_token_callback cb, void* ctx);
    float (*lot_developer_slope)(void* user, uint64_t lot);
    void (*update_lot_elements)(void* user, uint64_t lot, uint32_t kind);
    float (*area_altitude)(void* user, const float* rect4);
    /* zones, land value, power, demolition, budget */
    int8_t (*zone_type)(void* user, int32_t x, int32_t z);
    void (*dezone)(void* user, const fc4_cell_rect* rect, bool flag);
    bool (*powered)(void* user, int32_t x, int32_t z);
    uint32_t (*land_value_type)(void* user, int32_t x, int32_t z);
    uint8_t (*land_value)(void* user, int32_t x, int32_t z);
    int64_t (*demolish_lots)(void* user, const uint64_t* lots, size_t n); /* the cost */
    int64_t (*total_funds)(void* user);
    void (*set_total_funds)(void* user, int64_t v);
    /* occupants */
    bool (*has_occupant_manager)(void* user);
    bool (*insert_occupant)(void* user, uint64_t occ);
    bool (*remove_occupant)(void* user, uint64_t occ);
    bool (*building_occupant_at_cell)(void* user, int32_t x, int32_t z);
    bool (*occupant_has_property)(void* user, uint64_t occ, uint32_t id);
    bool (*occupant_property_i64)(void* user, uint64_t occ, uint32_t id, int64_t* out);
    bool (*occupant_in_group)(void* user, uint64_t occ, uint32_t group);
    void (*occupant_position)(void* user, uint64_t occ, float* out3);
    uint64_t (*create_building_occupant)(void* user); /* one reference for the core */
    void (*set_building_type)(void* user, uint64_t occ, uint32_t type);
    uint32_t (*building_type)(void* user, uint64_t occ); /* building occupant slot 4 (§4.9.1) */
    /* §4.9.1 / §4.9.2: GetResource(key, 0xA52160F5), then SetDefaultExemplar
     * on the occupant's cISCExemplarPropertyHolder (QI 0x0AC2B5F7); false
     * when the resource does not load */
    bool (*set_default_exemplar)(void* user, uint64_t occ, const fc4_res_key* key);
    /* the float32 values of property id through the occupant's own holder:
     * false when missing, else the count into *count and at most n values */
    bool (*occupant_property_f32s)(void* user, uint64_t occ, uint32_t id, float* out, size_t n, size_t* count);
    void (*occupant_bounding_box)(void* user, uint64_t occ, float* out6); /* min then max */
    void (*set_occupant_bounding_box)(void* user, uint64_t occ, const float* box6);
    void (*set_occupant_position)(void* user, uint64_t occ, const float* pos3);
    void (*set_occupant_orientation)(void* user, uint64_t occ, int32_t orientation);
    void (*init_occupant)(void* user, uint64_t occ);
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    /* foundations and retaining walls (§4.9.2, DG6) */
    uint64_t (*create_foundation_occupant)(void* user); /* class 0x49C05C8F, GZCOM's one reference */
    uint32_t (*foundation_type)(void* user, uint64_t occ); /* cISC4FoundationOccupant (0xC8FCE101) slot 4 */
    void (*set_foundation_type)(void* user, uint64_t occ, uint32_t type);          /* slot 5 */
    void (*set_foundation_max_height)(void* user, uint64_t occ, float v);          /* slot 7 */
    void (*set_foundation_size)(void* user, uint64_t occ, const float* size3);     /* slot 8 */
    void (*set_foundation_usage)(void* user, uint64_t occ, int32_t usage);         /* slot 10 */
    void (*set_foundation_orientation)(void* user, uint64_t occ, int32_t orient);  /* slot 12 */
    uint32_t (*retaining_wall_type)(void* user, uint64_t occ); /* the wall's slot 11 */
    /* lot developer slot 17, the int32 overload of GetLowestOrHighestAreaAltitude */
    float (*area_altitude_cells)(void* user, const fc4_cell_rect* rect, bool lowest, bool with_water);
    /* advisors */
    void (*push_advisor_event)(void* user, uint32_t advisor, uint32_t event, const fc4_advice_subject* subject);
    /* growth developers (the game's cISC4GrowthDeveloper slots) */
    uint64_t (*create_developer)(void* user, const fc4_res_key* key); /* class 0x0990C138, one reference */
    bool (*developer_init)(void* user, uint64_t dev, const fc4_res_key* key); /* Init(holder of key) */
    bool (*developer_shutdown)(void* user, uint64_t dev);
    uint32_t (*developer_sub_type)(void* user, uint64_t dev);
    uint32_t (*developer_purpose)(void* user, uint64_t dev);
    bool (*developer_lists_valid)(void* user, uint64_t dev);
    /* which: 0 GetGrowthList, 1 GetDeclineList; delivered in one call */
    void (*developer_list)(void* user, uint64_t dev, uint32_t which, fc4_growth_items_sink sink, void* ctx);
    uint64_t (*developer_source_demand)(void* user, uint64_t dev);
    void (*developer_start_map_update)(void* user, uint64_t dev);
    bool (*developer_map_update_finished)(void* user, uint64_t dev);
} fc4_development_host;

/* The process statics; `session_seed` fixes the clock seeds (tests). */
fc4_development_process* fc4_development_process_create(uint32_t session_seed);
void fc4_development_process_destroy(fc4_development_process* pp);
bool fc4_development_catalogue_loaded(const fc4_development_process* pp); /* §1.3 step 1 has run */
/* which: 0 building types, 1 families, 2 foundation types, 3 wall types,
 * 4 profiled types. Returns the count; copies at most n. */
size_t fc4_development_catalogue_types(const fc4_development_process* pp, uint32_t which, uint32_t* out, size_t n);
size_t fc4_development_catalogue_family(const fc4_development_process* pp, uint32_t family, uint32_t* out, size_t n);
/* which: 0 building, 2 foundation, 3 wall exemplar key of type t */
bool fc4_development_catalogue_key(const fc4_development_process* pp, uint32_t which, uint32_t t, fc4_res_key* out);
/* which: 0 exemplar name (0x20), 1 localized building name; valid building types only */
bool fc4_development_catalogue_name(const fc4_development_process* pp, uint32_t which, uint32_t t,
                                   fc4_bytes_sink sink, void* ctx);
/* The profile of t; the capacity table goes to cb (key, capacity). */
bool fc4_development_catalogue_profile(const fc4_development_process* pp, uint32_t t, fc4_building_profile* out,
                                       fc4_type_value_callback cb, void* ctx);
size_t fc4_development_catalogue_density(const fc4_development_process* pp, fc4_density_callback cb, void* ctx);
float fc4_development_catalogue_max_capacity(const fc4_development_process* pp, uint32_t zone, uint32_t purpose,
                                             uint32_t wealth); /* slot 45 */
uint32_t fc4_development_developer_type(uint32_t purpose, uint32_t wealth);
uint32_t fc4_development_wealth_of(uint32_t dev_type);
uint32_t fc4_development_building_map_key(uint32_t purpose, uint32_t wealth); /* 4p + w + 1 */
/* 0 none, 1 primary, 2 message target, 3 message target 2, 4 serializable */
uint32_t fc4_development_query_interface(uint32_t iid);

fc4_development* fc4_development_create(const fc4_development_host* host, const fc4_development_process* pp,
                                        uint64_t token);
void fc4_development_destroy(fc4_development* p); /* releases every token the core holds */
bool fc4_development_init(fc4_development* p);    /* §1.3 */
bool fc4_development_shutdown(fc4_development* p);
bool fc4_development_do_message(fc4_development* p, const fc4_development_message* m,
                                const fc4_command_param* params, size_t n_params);
void fc4_development_set_grow_budget(fc4_development* p, uint32_t n); /* §4.4.1; 0 = unlimited */
/* The primary interface (§7.1); the slot in the comment. */
bool fc4_development_location_is_ok(const fc4_development* p, uint32_t t, float x, float z); /* 5 */
uint64_t fc4_development_add_building_at_cell(fc4_development* p, uint32_t t, int32_t cx, int32_t cz,
                                              int32_t orientation); /* 6 */
uint64_t fc4_development_add_building(fc4_development* p, uint32_t t, const float* pos3, int32_t orientation); /* 7 */
bool fc4_development_insert_occupant(fc4_development* p, uint64_t occ);    /* 8 / 24 */
int64_t fc4_development_demolition_cost(const fc4_development* p, uint64_t occ); /* 10 */
bool fc4_development_demolish_foundation(fc4_development* p, uint64_t occ); /* 25 */
/* 23 (§4.9.2): pos3[1] is written when it was below 0 */
bool fc4_development_add_foundation(fc4_development* p, uint32_t t, int32_t usage, float* pos3, float size_x,
                                    float size_z, int32_t orientation, float base);
bool fc4_development_load_foundation(fc4_development* p, uint64_t occ, uint32_t t);     /* 26 */
bool fc4_development_load_retaining_wall(fc4_development* p, uint64_t occ, uint32_t t); /* 29 */
uint64_t fc4_development_developer(const fc4_development* p, uint32_t dev_type); /* 32, borrowed */
/* 34 and 44: cb runs while the core's state is borrowed (no copy is made): it must not call back into p */
uint32_t fc4_development_developers(const fc4_development* p, fc4_type_token_callback cb, void* ctx); /* 34 */
uint64_t fc4_development_building_map(const fc4_development* p);                                     /* 41 */
uint32_t fc4_development_special_buildings(const fc4_development* p, fc4_token_callback cb, void* ctx); /* 44 */
/* 46: `reason` written only on failure */
bool fc4_development_is_suitable(const fc4_development* p, const fc4_construction_profile* profile, int32_t* reason);
uint64_t fc4_development_plop_building(fc4_development* p, const fc4_construction_profile* profile); /* 47 */
int32_t fc4_development_free_resources(const fc4_development* p);                                    /* 48 */
int32_t fc4_development_total_resources(const fc4_development* p);                                   /* 49 */
bool fc4_development_claim(fc4_development* p, uint16_t amount);                                      /* 50 */
bool fc4_development_release_resources(fc4_development* p, uint16_t amount);                         /* 51 */
int32_t fc4_development_last_successful_build(const fc4_development* p, uint32_t dev_type);           /* 52 */
float fc4_development_stage_population_multiplier(const fc4_development* p);                         /* 53 */
bool fc4_development_cheat(const fc4_development* p, uint32_t which); /* 54-58: which 0..4 */
uint32_t fc4_development_highest_stages(const fc4_development* p, fc4_type_value_callback cb, void* ctx);
uint32_t fc4_development_decline_plan(const fc4_development* p, fc4_plan_entry_callback cb, void* ctx);
bool fc4_development_get_state(const fc4_development* p, fc4_development_state* out);
bool fc4_development_write(const fc4_development* p, const fc4_ostream* out); /* record v4 (§6) */
bool fc4_development_read(fc4_development* p, const fc4_istream* in);        /* before Init */

/* ---- growth developer (docs/specs/growth-developer.md) ---- */
typedef struct fc4_growth_message {
    uint32_t msg_type, data1, data2, data3, data4;
    int32_t rect[4]; /* ZoningChange: void2's {x1, z1, x2, z2} */
    uint32_t reserved;
    uint64_t unknown; /* StateChanged: GetIGZUnknown() as a lot (borrowed), 0 = none */
} fc4_growth_message;

typedef struct fc4_growth_state {
    uint32_t struct_size; /* = sizeof(fc4_growth_state) */
    uint8_t initialized;
    uint8_t update_state; /* 0 idle, 1..20 (§3.2) */
    uint8_t has_salt, message_server;
    uint32_t developer_type, purpose, wealth;
    int32_t target_zone;
    uint32_t growth_items, decline_items;
    int32_t total_capacity, last_phantom_sum;
    float last_city_size; /* of the last state 20 (§5.5) */
    uint32_t last_salt, start_day, specials, services, tract_size, reentrant_calls, reserved;
    uint64_t draws; /* salts drawn from the shared generator (all developers) */
    uint64_t source_demand;
    uint64_t desirability_map, capacity_map, population_map, max_capacity_map, zoned_map, working_map;
} fc4_growth_state;

/* source_map codes */
#define FC4_GROWTH_SOURCE_INTRINSIC_LAND_VALUE  0u
#define FC4_GROWTH_SOURCE_TOTAL_LAND_VALUE      1u
#define FC4_GROWTH_SOURCE_SLOPE                 2u
#define FC4_GROWTH_SOURCE_RESIDENTIAL_PROXIMITY 3u /* arg: wealth */
#define FC4_GROWTH_SOURCE_COMMERCIAL_PROXIMITY  4u /* arg: type */
#define FC4_GROWTH_SOURCE_PARK                  5u
#define FC4_GROWTH_SOURCE_POLLUTION             6u /* arg: index */
#define FC4_GROWTH_SOURCE_SCHOOL_GRADE          7u
#define FC4_GROWTH_SOURCE_HOSPITAL_GRADE        8u
#define FC4_GROWTH_SOURCE_CRIME                 9u
#define FC4_GROWTH_SOURCE_TRIP_LENGTH           10u
#define FC4_GROWTH_SOURCE_COMMERCIAL_TRAFFIC    11u
#define FC4_GROWTH_SOURCE_LANDMARK              12u
#define FC4_GROWTH_SOURCE_TRANSIENT_AURA        13u

typedef struct fc4_growth_host {
    uint32_t struct_size; /* sizeof(fc4_growth_host) */
    void* user;
    fc4_grid_api grids;
    /* lifecycle; services: 0 city, 1 BDS, 2 occupant manager, 3 zone
     * manager, 4 land value, 5 traffic (false when null) */
    bool (*acquire_service)(void* user, uint32_t service);
    void (*release_service)(void* user, uint32_t service);
    bool (*acquire_message_server)(void* user);
    void (*release_message_server)(void* user);
    void (*subscribe)(void* user, const uint32_t* types, size_t n);
    void (*unsubscribe)(void* user, const uint32_t* types, size_t n);
    bool (*simulator_present)(void* user);
    void (*add_agent)(void* user, uint32_t level, const char* name, size_t len, uint32_t flags);
    void (*remove_agent)(void* user);
    uint32_t (*sim_date_number)(void* user);
    void (*grid_set_tract_size)(void* user, uint64_t grid, int32_t size);
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    /* demand and neighbours */
    uint64_t (*get_demand)(void* user, uint32_t id, uint32_t index); /* borrowed */
    float (*query_supply)(void* user, uint64_t node);
    void (*add_to_demand_value)(void* user, uint64_t node, float v);
    float (*active_demand)(void* user, uint64_t node);
    uint32_t (*connected_cities)(void* user);
    uint32_t (*all_connected_cities)(void* user);
    /* the BDS */
    float (*stage_population_multiplier)(void* user);
    uint64_t (*tract_developer)(void* user); /* borrowed */
    void (*special_buildings)(void* user, fc4_token_callback cb, void* ctx); /* borrowed, list order */
    float (*max_capacity)(void* user, int32_t zone, uint32_t purpose, uint32_t wealth);
    uint32_t (*building_purpose)(void* user, uint32_t type);
    /* the tract developer */
    void (*update_population)(void* user, uint64_t tract_developer, const fc4_growth_profile* profile);
    /* zones, lots, occupants, city */
    int8_t (*zone_type)(void* user, int32_t x, int32_t z);
    uint32_t (*lot_building_type)(void* user, uint64_t lot, bool flag);
    uint16_t (*lot_capacity)(void* user, uint64_t lot, uint32_t dev_type, bool flag);
    void (*lot_size)(void* user, uint64_t lot, uint8_t* out2);
    void (*lot_bounding_rect)(void* user, uint64_t lot, fc4_cell_rect* out);
    void (*lot_location)(void* user, uint64_t lot, int32_t* out2);
    bool (*occupant_property)(void* user, uint64_t occ, uint32_t id, fc4_props* scratch);
    void (*occupant_position)(void* user, uint64_t occ, float* out3);
    void (*position_to_cell)(void* user, float x, float z, int32_t* out2);
    /* source maps: a grid token with one reference for the core; 0 = null */
    uint64_t (*source_map)(void* user, uint32_t code, uint32_t arg);
    /* time slicing and seeds */
    void (*limit_timer_start)(void* user, uint32_t ms);
    bool (*limit_timer_expired)(void* user);
    /* The session seed S (foundations §6.3), read only when a decline key's
     * coordinate seed is 0xFFFFFFFF (growth-developer.md §7 quirk): that key's
     * generator takes derive(S, 0x0990C138 XOR n), n counting such reads by the
     * developer (ABI 64; before, this was the wall-clock value itself). */
    uint32_t (*session_seed)(void* user);
} fc4_growth_host;

fc4_growth_statics* fc4_growth_statics_create(uint32_t session_seed);
void fc4_growth_statics_destroy(fc4_growth_statics* s);
/* 0 none, 1 primary, 2 message target 2, 3 serializable */
uint32_t fc4_growth_query_interface(uint32_t iid);
fc4_growth* fc4_growth_create(const fc4_growth_host* host, const fc4_growth_statics* statics, uint64_t token);
void fc4_growth_destroy(fc4_growth* p);                /* releases every token the core holds */
bool fc4_growth_init(fc4_growth* p, fc4_props* props); /* slot 3; props NEW (consumed), NULL fails */
bool fc4_growth_shutdown(fc4_growth* p);
bool fc4_growth_do_message(fc4_growth* p, const fc4_growth_message* m);
int32_t fc4_growth_target_zone(const fc4_growth* p);
uint32_t fc4_growth_sub_type(const fc4_growth* p);
uint32_t fc4_growth_purpose(const fc4_growth* p);
uint32_t fc4_growth_wealth(const fc4_growth* p);
/* Writes the info fields (zone .. land value range); the rest untouched. */
void fc4_growth_construction_info(const fc4_growth* p, fc4_construction_profile* out);
bool fc4_growth_lists_valid(const fc4_growth* p);
/* which: 0 growth, 1 decline (after the §5.4 sort). Returns the count; copies at most n. */
size_t fc4_growth_list(const fc4_growth* p, uint32_t which, fc4_growth_item* out, size_t n);
/* which: 0 D, 1 capacity, 2 population, 3 max capacity, 4 zoned, 5 working (borrowed) */
uint64_t fc4_growth_grid(const fc4_growth* p, uint32_t which);
int32_t fc4_growth_total_capacity(const fc4_growth* p);
uint64_t fc4_growth_source_demand(const fc4_growth* p);
void fc4_growth_caps(const fc4_growth* p, uint8_t* out4);
int32_t fc4_growth_desirability_threshold(const fc4_growth* p);
float fc4_growth_abandonment_threshold(fc4_growth* p, int32_t key);
uint32_t fc4_growth_monopoly_use(const fc4_growth* p);
size_t fc4_growth_styles(const fc4_growth* p, fc4_key_float_callback cb, void* ctx);
void fc4_growth_start_map_update(fc4_growth* p);
bool fc4_growth_map_update_finished(const fc4_growth* p);
int32_t fc4_growth_estimate_desirability(const fc4_growth* p, int32_t x, int32_t z);
void fc4_growth_subjective_thresholds(const fc4_growth* p, int32_t i, int32_t* min, int32_t* max);
bool fc4_growth_source_active_demand(const fc4_growth* p, float* out);
bool fc4_growth_get_state(const fc4_growth* p, fc4_growth_state* out);
bool fc4_growth_write(const fc4_growth* p, const fc4_ostream* out); /* record v1 */
bool fc4_growth_read(fc4_growth* p, const fc4_istream* in);        /* before Init */
/* ---- zone manager (docs/specs/zones.md §1.1, §2-§6, §10.1, §11.1) -------------
 *
 * The zone manager (cSC4ZoneManager, class 0x298F9B2D, city component 6): the
 * zone grid (a game int8 grid 0x49B9E603 through `grids`, tract size 1,
 * instance 0, saved by reference), the per-type zone counts, the tuning, the
 * dezone under new networks (InsertOccupant 0x99EF1142) and PlaceZone. The
 * zone developer / lot manager stays the game's: the host reaches it for
 * get_lot, lots_in_cell_rect and do_parcellization.
 *
 * Tokens are host identities of any interface of the object (the host queries
 * what it needs); those the host returns are BORROWED unless stated. 0 = none.
 *
 * Threads: single-threaded; the plugin serialises every call on one object and
 * publishes fc4_zones_get_state / fc4_zones_get_tuning for its other threads.
 * Re-entrant calls (a dezone during a commit, GetZoneType from the zone
 * developer) run at once and are counted.
 *
 * Shadow mode (fc4_zones_create_shadow): reads go to the game, writes are
 * swallowed (services, subscriptions, funds, the commit's demolition,
 * parcellization, ZoningChange, lot textures); references read are borrowed
 * (nothing taken, adopted or released). Give the shadow a grid table without
 * set / fill / shutdown / release. */

typedef struct fc4_zones fc4_zones;

/* The "Zone Manager" tuning exemplar (§4). */
#define FC4_ZONES_TUNING_TYPE 0x6534284Au
#define FC4_ZONES_TUNING_GROUP 0xE7E2C2DBu
#define FC4_ZONES_TUNING_INSTANCE 0xE9482490u

/* The services the record saves (§10.1 field 13), in record order. */
#define FC4_ZONES_SERVICE_CITY 0u
#define FC4_ZONES_SERVICE_OCCUPANT_MANAGER 1u
#define FC4_ZONES_SERVICE_TERRAIN 2u
#define FC4_ZONES_SERVICE_POLLUTION 3u
#define FC4_ZONES_SERVICE_BUDGET 4u

/* A cell region (the game's SC4CellRegion<long>, §11.4): an inclusive rect and
 * one byte per cell, index (z - z0) * width + (x - x0), non-zero = set. NULL
 * mask: every cell set. A short mask reads as cleared cells past its end. */
typedef struct fc4_zone_region {
    int32_t x0, z0, x1, z1;
    uint8_t* mask;
    size_t len;
} fc4_zone_region;

/* PlaceZone's outputs (§6). */
typedef struct fc4_zones_place_result {
    uint32_t struct_size;
    uint8_t ok;        /* AL */
    uint8_t committed; /* a change was committed and ZoningChange posted */
    uint8_t reserved[2];
    int32_t error;     /* 0 ok, 1 funds, 2 landfill on water, 3 too small, 4 too large, 5 blocked */
    uint32_t accepted; /* cells of the ZoningChange region (0 unless committed) */
    uint32_t cleared;  /* region bits this call cleared (rejected cells) */
    uint32_t lots;     /* lots to demolish (*outLots) */
    int64_t cost;      /* *outCost */
} fc4_zones_place_result;

/* What a DoMessage did (§5.1). */
typedef struct fc4_zones_dezone {
    uint32_t struct_size;
    uint8_t dezoned; /* PlaceZone(rect, 0, ...) ran */
    uint8_t reserved[3];
    fc4_cell_rect rect; /* the occupant's bounding cells */
    uint32_t changed;   /* cells re-zoned (the ZoningChange region) */
    uint32_t lots;      /* lots handed to DemolishLots(true, ...) */
} fc4_zones_dezone;

/* Diagnostics and the getters' snapshot. */
typedef struct fc4_zones_state {
    uint32_t struct_size;
    uint8_t initialized, loaded, display_alpha, default_alpha;
    uint32_t zone_count[16];                          /* slot 6 */
    uint32_t undeveloped[16];                         /* slot 7 (never updated after Init, §2) */
    uint32_t undeveloped_total, abandoned_tile_count; /* slots 8, 10 */
    uint32_t failure1_entries;                        /* development-failure map 1 entries */
    uint32_t reentrant_calls, place_calls, dezones;
    int32_t grid_count_x, grid_count_z; /* 0 x 0 without a grid or before its record is read */
    uint64_t grid;                      /* slot 5 (borrowed); 0 = none */
} fc4_zones_state;

/* The tuning (§4), indexed by zone type. */
typedef struct fc4_zones_tuning {
    uint32_t struct_size;
    uint8_t display_alpha, default_alpha;
    uint8_t reserved[2];
    int32_t min_size[16], max_size[16];
    int64_t create_cost[16], destroy_cost[16];
    uint32_t texture[16], drag_color[16];
    float max_slope[16];
} fc4_zones_tuning;

/* Everything the zone manager asks of the game. */
typedef struct fc4_zones_host {
    uint32_t struct_size;
    void* user;
    fc4_grid_api grids; /* the zone grid (int8 0x49B9E603) */
    /* lifecycle and services (§1.1, §10.1). acquire_services: Init step 2 (the city, occupant manager,
     * terrain, pollution and budget simulators still null, and the message server). */
    void (*acquire_services)(void* user);
    void (*release_services)(void* user);
    void (*subscribe)(void* user, uint32_t msg);
    void (*unsubscribe)(void* user, uint32_t msg);
    fc4_props* (*tuning)(void* user); /* a NEW fc4_props (the core destroys it); NULL = missing */
    void (*cell_counts)(void* user, int32_t* x, int32_t* z);
    uint64_t (*service_ref)(void* user, uint32_t which); /* Write: the token saved for FC4_ZONES_SERVICE_*; 0 = null */
    /* Read: service `which` as read (its IID's interface; 0 = null). The host takes the token's ONE
     * reference and replaces the service it held. */
    void (*adopt_service)(void* user, uint32_t which, uint64_t token);
    /* ZoningChange 0x07699455 (§5.3): MessagePost of a message owning a copy of the region. */
    void (*post_zoning_change)(void* user, const fc4_zone_region* region, int32_t zone_type, bool check_slope);
    /* the zone developer / lot manager (§13.1) */
    uint64_t (*get_lot)(void* user, int32_t x, int32_t z); /* lot manager slot 6 GetLot(x, z, false) */
    /* lot manager slot 7: cb(ctx, lot) per lot, ONE reference each for the core */
    void (*lots_in_cell_rect)(void* user, const fc4_cell_rect* rect, bool origin_only, fc4_token_callback cb,
                              void* ctx);
    void (*do_parcellization)(void* user, const fc4_zone_region* region, int32_t zone_type, bool flag);
    /* budget, demolition, landfill (§6) */
    int64_t (*total_funds)(void* user);
    bool (*withdraw_funds)(void* user, int64_t amount);
    /* DemolishLots(commit, lots, checkSlope, flags, 0, 0, &cost, 0) -> cost */
    int64_t (*demolish_lots)(void* user, bool commit, const uint64_t* lots, size_t count, bool check_slope,
                             uint32_t flags);
    int64_t (*landfill_zoning_cost)(void* user, const fc4_zone_region* region);
    /* network tiles (§6) */
    uint64_t (*find_network_tile)(void* user, int32_t x, int32_t z); /* FindNetworkTile(x, z, 0x1F4F, 1) */
    bool (*tile_occupant_flag_set)(void* user, uint64_t tile, uint32_t flag);
    bool (*tile_is_of_type)(void* user, uint64_t tile, uint32_t type);
    int64_t (*tile_demolition_cost)(void* user, uint64_t tile);
    /* terrain, lot developer */
    bool (*cell_has_water)(void* user, int32_t x, int32_t z);
    float (*area_slope)(void* user, const fc4_cell_rect* rect, bool with_water); /* lot developer slot 29 */
    void (*set_empty_lot_base_texture)(void* user, uint64_t lot);                 /* lot developer slot 43 */
    /* occupants (§5.1, §6) */
    uint64_t (*landfill_at)(void* user, int32_t x, int32_t z); /* occupant manager slot 33, type 0x6A11BD2B */
    void (*occupant_bounding_cells)(void* user, uint64_t occupant, fc4_cell_rect* out); /* occupant slot 11 */
    bool (*occupant_can_remove)(void* user, uint64_t occupant, uint32_t flags);        /* occupant slot 14 */
    /* QueryInterface(cISC4NetworkOccupant) + slot 9: 1 / 0, -1 = not a network occupant */
    int32_t (*network_has_any_flag)(void* user, uint64_t occupant, uint32_t flags);
    /* references */
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    /* lots and lot configurations */
    uint64_t (*lot_configuration)(void* user, uint64_t lot);
    void (*lot_bounding_rect)(void* user, uint64_t lot, fc4_cell_rect* out);
    int32_t (*lot_zone_type)(void* user, uint64_t lot);
    uint32_t (*lot_state)(void* user, uint64_t lot);
    uint64_t (*lot_building)(void* user, uint64_t lot);
    bool (*cfg_compatible_with_zone)(void* user, uint64_t cfg, int32_t zone_type);
} fc4_zones_host;

bool fc4_zones_query_interface(uint32_t iid);  /* 1, 0x67652433, 0x452294AA, 0xE4FDA3D4 */
uint32_t fc4_zones_class_id(void);             /* 0x298F9B2D */
bool fc4_zones_is_rci_zone(int32_t zone_type); /* slot 13 */
/* NULL when the table is incomplete. The table is copied. */
fc4_zones* fc4_zones_create(const fc4_zones_host* host);
fc4_zones* fc4_zones_create_shadow(const fc4_zones_host* host);
/* A grid still held is shut down and released. */
void fc4_zones_destroy(fc4_zones* z);
bool fc4_zones_init(fc4_zones* z);
bool fc4_zones_shutdown(fc4_zones* z);
/* Always true. `occupant` = void1 (0 = none); `out` may be NULL. */
bool fc4_zones_do_message(fc4_zones* z, uint32_t msg_type, uint64_t occupant, uint32_t data3, uint32_t data4,
                          fc4_zones_dezone* out);
/* Slot 11 (§6). The region is written back (rejected cells cleared; a NULL mask cannot report them).
 * `lots` (may be NULL): every lot to demolish, borrowed, called at the pricing call, before the commit;
 * the caller takes its own references. A NULL region, or one above 1,048,576 cells or 4096 a side, fails
 * with error 0 and calls nothing. */
bool fc4_zones_place_zone(fc4_zones* z, fc4_zone_region* region, int32_t zone_type, bool commit, bool skip_funds,
                          bool check_slope, bool skip_size_check, bool check_lot_inside, fc4_token_callback lots,
                          void* ctx, fc4_zones_place_result* out);
uint64_t fc4_zones_grid(const fc4_zones* z);                                      /* slot 5 */
uint32_t fc4_zones_zone_count(const fc4_zones* z, int32_t zone_type);             /* slot 6 */
uint32_t fc4_zones_undeveloped_tile_count(const fc4_zones* z, int32_t zone_type); /* slot 7 */
uint32_t fc4_zones_undeveloped_total(const fc4_zones* z);                         /* slot 8 */
/* Slot 9: inserts a zero entry into map 1 when absent (§11.1 quirk). */
uint32_t fc4_zones_development_failure_count(fc4_zones* z, int32_t zone, int32_t result);
uint32_t fc4_zones_abandoned_tile_count(const fc4_zones* z);          /* slot 10 */
int32_t fc4_zones_zone_type(const fc4_zones* z, int32_t x, int32_t z_); /* slot 12 */
/* Slot 14: false for type >= 16; key = {type, group, instance} (3 x uint32). */
bool fc4_zones_texture_for_zone(const fc4_zones* z, int32_t zone_type, uint32_t* key);
uint32_t fc4_zones_drag_color(const fc4_zones* z, int32_t zone_type); /* slot 15 */
uint8_t fc4_zones_display_alpha(const fc4_zones* z);                  /* slot 16 */
uint8_t fc4_zones_default_display_alpha(const fc4_zones* z);          /* slot 17 */
void fc4_zones_set_display_alpha(fc4_zones* z, uint8_t alpha);        /* slot 18 */
int32_t fc4_zones_min_size(const fc4_zones* z, int32_t zone_type);    /* slot 19 */
int32_t fc4_zones_max_size(const fc4_zones* z, int32_t zone_type);    /* slot 20 */
bool fc4_zones_get_state(const fc4_zones* z, fc4_zones_state* out);
bool fc4_zones_get_tuning(const fc4_zones* z, fc4_zones_tuning* out);
/* Game writes the last place_zone / do_message call swallowed (shadow mode). */
uint32_t fc4_zones_last_swallowed(const fc4_zones* z);
bool fc4_zones_write(const fc4_zones* z, const fc4_ostream* out);
/* Runs before Init: nothing is sized from the city. */
bool fc4_zones_read(fc4_zones* z, const fc4_istream* in);

/* ---- zone developer / lot manager (docs/specs/zones.md §1.2, §3.2, §5.2, §7-§10.2, §11.2, §11.3;
 *      engine/capi/src/zonedev.rs) ---------------------------------------------------------------------
 *
 * The plugin's object (class 0x498F9B01: cISC4ZoneDeveloper, cISC4LotManager
 * at +4, cIGZMessageTarget2 at +8, cIGZSerializable at +0xC) forwards its
 * slots to fc4_zonedev_*. Tokens are host identities (lots, configurations,
 * occupants, growth developers, the grid); 0 is null; borrowed unless stated.
 * The lot list functions hand each lot out with ONE reference for the caller,
 * as slots 7-10 do. The master lot list is the core's; the host mirrors it
 * (master_insert / master_remove / master_reset) to keep the game's raw list
 * and safe lot iterators (slot 15, lot-developer.md §15.1). The zone
 * developer's generator is process-wide (§9): the first create seeds it with
 * derive_seed(session_seed, 0x8C09C68A). Shadow mode reads the game and
 * never writes it (no subscription, service, reference, lot call, network
 * tool, message or mirror call reaches the host; references read are
 * borrowed). */
typedef struct fc4_zonedev fc4_zonedev;

/* What building_profile reads of BDS slot 39 (§7.2.1 step 6). */
typedef struct fc4_zonedev_profile {
    uint32_t purpose, wealth;
    uint16_t capacity;
    uint8_t reserved[2];
} fc4_zonedev_profile;

/* What one parcellization or message did. */
typedef struct fc4_zonedev_parcel_result {
    uint32_t struct_size;
    uint32_t lots_created;     /* master-list inserts (CreateLot) */
    uint32_t lots_deleted;     /* master-list removals (DeleteLot, real mode) */
    uint32_t lot_failures;     /* CreateLot calls the host built no lot for (new_lot 0) */
    uint32_t streets_placed;   /* street Place calls that returned true / false (§7.10) */
    uint32_t streets_failed;
    uint32_t demolish_regions; /* DemolishRegion calls (§7.9) */
    uint32_t swallowed;        /* game writes swallowed (shadow mode) */
} fc4_zonedev_parcel_result;

/* Diagnostics and the getters' snapshot. */
typedef struct fc4_zonedev_state {
    uint32_t struct_size;
    uint8_t initialized, loaded, real_mode, reserved;
    uint32_t lot_count;                  /* slot 16 */
    int32_t map_x, map_z;                /* the cell -> lot map's rows (x) and row size (z) */
    uint32_t lots_created, lots_deleted; /* since creation */
    uint32_t parcellizations, dezones;   /* fc4_zonedev_do_parcellization calls; of them not RCI */
    uint32_t reentrant_calls;
    uint32_t view_value;        /* the last message 0xA6B79602's value (§5.2) */
    uint32_t options;           /* bit 0 alternateOrientation, 1 placeStreets, 2 customZoneSize */
    uint64_t max_capacity_grid; /* slot 27 token (borrowed); 0 = none */
} fc4_zonedev_state;

/* One lot configuration object record (lot configuration slot 23, §8.4.1): the type (5 water constraint,
 * 6 land constraint, 7 network), the orientation byte, the footprint in lot space (metres) and the record's
 * extra values (values[0..n_values], borrowed for the call). */
typedef struct fc4_zonedev_cfg_object {
    uint8_t type, orientation;
    uint8_t reserved[2];
    float min_x, min_z, max_x, max_z;
    const uint32_t* values;
    size_t n_values;
} fc4_zonedev_cfg_object;
typedef void (*fc4_zonedev_cfg_object_callback)(void* ctx, const fc4_zonedev_cfg_object* obj);

typedef struct fc4_zonedev_host {
    uint32_t struct_size;
    void* user;
    fc4_grid_api grids; /* the maximum-capacity grid (uint16 0x49B9E604, §1.2 step 4) */
    /* lifecycle, services and messages (§1.2, §5) */
    /* Init steps 1-2 without the 3D view: lot developer, BDS, city, occupant
     * manager, terrain, traffic simulator, zone manager, message server.
     * False if any is missing. */
    bool (*acquire_services)(void* user);
    void (*release_services)(void* user);
    void (*subscribe)(void* user, uint32_t msg);
    void (*unsubscribe)(void* user, uint32_t msg);
    void (*cell_counts)(void* user, int32_t* x, int32_t* z); /* city slots 93, 94 */
    void (*acquire_network_tools)(void* user);              /* PostCityInit: street, dirt-road, road tools */
    void (*release_network_tools)(void* user);
    void (*post_defer_occupant_operations)(void* user, bool begin);     /* 0x49D968C4, posted */
    int32_t (*zone_type)(void* user, int32_t x, int32_t z);             /* zone manager slot 12 */
    void (*demolish_region)(void* user, const fc4_zone_region* region); /* demolition slot 6 */
    /* networks (§7.10-§7.13, §8.8) */
    /* The street tool's PlaceNetworkOccupants(a, b, 0xC49, tool, realMode, -1) for kind 3; AL. */
    bool (*street_place)(void* user, uint32_t kind, int32_t ax, int32_t az, int32_t bx, int32_t bz, bool real_mode);
    void (*reset_network_tool)(void* user, uint32_t kind); /* slot 5 Reset of tool kind 3 or 0 */
    bool (*connects_to_neighbor)(void* user, uint64_t occupant, int32_t edge, uint32_t net_type); /* slot 47 */
    /* Tool kind 3 or 11, slot 9 OccupantExists(x, z, flag, mask): true with mask[0..4] written. */
    bool (*network_tool_occupant_exists)(void* user, uint32_t kind, int32_t x, int32_t z, uint32_t flag,
                                         uint8_t* mask);
    void (*street_joining_helper)(void* user, uint64_t occupant, uint32_t data4); /* unused since ABI 55 */
    /* terrain */
    bool (*cell_has_water)(void* user, int32_t x, int32_t z);
    float (*terrain_altitude)(void* user, float x, float z);              /* slot 13 */
    float (*terrain_vertex_altitude)(void* user, int32_t vx, int32_t vz); /* slot 14 */
    /* city and occupants */
    bool (*location_in_bounds)(void* user, const float* pos3);                       /* city slot 98 */
    bool (*position_to_cell)(void* user, const float* pos3, int32_t* x, int32_t* z); /* city slot 95 */
    /* Occupant manager slot 41 (+ 48): the cell's occupants (of `type` when filtered), borrowed. */
    void (*cell_occupants)(void* user, int32_t x, int32_t z, bool filtered, uint32_t type, fc4_token_callback cb,
                           void* ctx);
    uint32_t (*occupant_type)(void* user, uint64_t occupant);
    void (*occupant_position)(void* user, uint64_t occupant, float* pos3);
    int32_t (*network_has_any_flag)(void* user, uint64_t occupant, uint32_t flags); /* 1/0, -1 not a network */
    void (*remove_occupant)(void* user, uint64_t occupant); /* occupant manager slot 21 (occ, 1, 0) */
    /* references */
    void (*add_ref)(void* user, uint64_t object);
    void (*release)(void* user, uint64_t object);
    /* lots (cISC4Lot) */
    /* A new game lot (x, z, w, h, facing) with no reference yet; 0 = cannot construct one (CreateLot fails). */
    uint64_t (*new_lot)(void* user, int32_t x, int32_t z, int32_t w, int32_t h, int32_t facing);
    void (*lot_init)(void* user, uint64_t lot);
    void (*lot_shutdown)(void* user, uint64_t lot);
    void (*lot_post_city_init)(void* user, uint64_t lot);
    uint64_t (*lot_configuration)(void* user, uint64_t lot);
    uint32_t (*lot_flags)(void* user, uint64_t lot);
    void (*lot_bounding_rect)(void* user, uint64_t lot, fc4_cell_rect* out);
    float (*lot_altitude)(void* user, uint64_t lot);
    void (*lot_location)(void* user, uint64_t lot, int32_t* x, int32_t* z);
    int32_t (*lot_facing)(void* user, uint64_t lot);
    bool (*lot_has_business_unit)(void* user, uint64_t lot);
    int32_t (*lot_zone_type)(void* user, uint64_t lot);
    uint32_t (*lot_state)(void* user, uint64_t lot);
    void (*lot_set_state)(void* user, uint64_t lot, uint32_t state, uint32_t a, uint32_t b);
    void (*lot_set_vertices_locked)(void* user, uint64_t lot, bool locked);
    /* lot configurations */
    bool (*cfg_compatible_with_water)(void* user, uint64_t cfg);
    bool (*cfg_compatible_with_zone)(void* user, uint64_t cfg, int32_t zone);
    uint32_t (*cfg_id)(void* user, uint64_t cfg);
    void (*cfg_size)(void* user, uint64_t cfg, uint32_t* x, uint32_t* z);
    uint8_t (*cfg_growth_stage)(void* user, uint64_t cfg);
    bool (*cfg_compatible_with_wealth)(void* user, uint64_t cfg, uint32_t wealth);
    bool (*cfg_compatible_with_purpose)(void* user, uint64_t cfg, uint32_t purpose);
    /* Slot 24's object: its IDs, cb each; false = null object. */
    bool (*cfg_primary_building_ids)(void* user, uint64_t cfg, fc4_u32_callback cb, void* ctx);
    /* Every configuration of the lot configuration manager (slot 9, the unregistered filter), borrowed. */
    void (*lot_configurations)(void* user, fc4_token_callback cb, void* ctx);
    /* lot developer */
    void (*attempt_area_levelling)(void* user, const fc4_cell_rect* rect); /* slot 21 */
    void (*remove_building_zot_prop)(void* user, uint64_t lot);            /* slot 11 */
    uint64_t (*lot_base_texture_occupant)(void* user, uint64_t lot);       /* slot 41: ONE reference */
    void (*remove_lot_from_business_unit)(void* user, uint64_t lot);       /* slot 33 */
    void (*set_empty_lot_base_texture)(void* user, uint64_t lot);          /* slot 43 */
    /* traffic simulator (§8.5 step 4) */
    /* Slot 22: false = a null iterator; else *node (0: the end) and *list. */
    bool (*desired_lot_insertion_point)(void* user, uint64_t* node, bool* list);
    void (*set_lot_insertion_point)(void* user, uint64_t node); /* 0: the list head */
    /* BDS and growth developers (§7.2.1) */
    uint64_t (*bds_developer)(void* user, uint32_t type); /* BDS slot 32, borrowed */
    void (*building_types_by_family)(void* user, uint32_t family, fc4_u32_callback cb, void* ctx);
    bool (*building_profile)(void* user, uint32_t type, fc4_zonedev_profile* out); /* false = none */
    int16_t (*dev_estimate_desirability)(void* user, uint64_t dev, int32_t x, int32_t z);
    bool (*dev_source_demand)(void* user, uint64_t dev, float* out);                         /* false = no demand */
    bool (*dev_growth_list)(void* user, uint64_t dev, fc4_growth_items_sink sink, void* ctx); /* false = invalid */
    int16_t (*dev_desirability_value)(void* user, uint64_t dev, int32_t tx, int32_t tz);
    uint32_t (*dev_desirability_shift)(void* user, uint64_t dev);
    uint32_t (*dev_purpose)(void* user, uint64_t dev);
    uint32_t (*dev_wealth)(void* user, uint64_t dev);
    /* the master lot list mirror (§8.1) */
    void (*master_insert)(void* user, uint64_t lot, uint64_t before); /* before 0 = appended */
    void (*master_remove)(void* user, uint64_t lot);
    void (*master_reset)(void* user, const uint64_t* lots, size_t n); /* Read; empty at Shutdown */
    /* ABI 55 (re-specs11). */
    float (*area_slope)(void* user, const fc4_cell_rect* rect, bool with_water); /* lot developer slot 29 (ST0) */
    float (*cfg_max_slope)(void* user, uint64_t cfg);                            /* lot configuration slot 8 */
    /* Lot configuration slot 23: cb once per object record, in order. */
    void (*cfg_objects)(void* user, uint64_t cfg, fc4_zonedev_cfg_object_callback cb, void* ctx);
    uint32_t (*network_flags)(void* user, uint64_t occupant);                     /* network slot 12 */
    bool (*network_has_all_flags)(void* user, uint64_t occupant, uint32_t mask);  /* network slot 8 */
    uint32_t (*network_edge_mask)(void* user, uint64_t occupant, uint32_t type);  /* network slot 38 */
    int32_t (*network_is_of_type)(void* user, uint64_t occupant, uint32_t type);  /* slot 22: 1/0, -1 not a network */
    bool (*network_is_flag_0x400000)(void* user, uint64_t occupant);              /* network slot 17 */
    void (*occupant_bounding_cells)(void* user, uint64_t occupant, fc4_cell_rect* out); /* occupant slot 11 */
    bool (*network_tool_present)(void* user, uint32_t kind); /* the tool of kind 0, 3 or 11 */
    /* Tool `kind`'s PlaceNetworkOccupants(ax, az, bx, bz, 0, null, 1, -1) (§5.2.1, §5.2.2). */
    void (*network_tool_join)(void* user, uint32_t kind, int32_t ax, int32_t az, int32_t bx, int32_t bz);
    bool (*cell_has_land)(void* user, int32_t x, int32_t z);     /* terrain slot 35 */
    bool (*cell_is_in_bounds)(void* user, int32_t x, int32_t z); /* city slot 99 */
    bool (*is_navigable)(void* user, int32_t x, int32_t z);      /* surface water slot 8 (x, z, 0, null) */
    void (*lot_update_zone_type)(void* user, uint64_t lot);      /* lot slot 32 */
    bool (*force_empty_lot_base_textures)(void* user);           /* lot developer slot 46 */
    /* Zone manager slot 11 PlaceZone(region, 0, 1, 1, 0, 1, 0, null, null, null) (§7.2.2 step 3). */
    void (*zone_manager_dezone)(void* user, const fc4_zone_region* region);
} fc4_zonedev_host;

bool fc4_zonedev_query_interface(uint32_t iid); /* 1, 0xE818ED2B, 0x681905BD, 0x452294AA, 0xE4FDA3D4 */
uint32_t fc4_zonedev_class_id(void);            /* 0x498F9B01 */
fc4_zonedev* fc4_zonedev_create(const fc4_zonedev_host* host, uint32_t session_seed);
fc4_zonedev* fc4_zonedev_create_shadow(const fc4_zonedev_host* host, uint32_t session_seed);
void fc4_zonedev_destroy(fc4_zonedev* d);
bool fc4_zonedev_init(fc4_zonedev* d);     /* slot 3: false when a service is missing */
bool fc4_zonedev_shutdown(fc4_zonedev* d); /* slot 4 */
/* DoMessage (§5.2): occupant = void1, data1 = cheat ID, value = *void2 (0xA6B79602). out may be NULL. */
bool fc4_zonedev_do_message(fc4_zonedev* d, uint32_t msg_type, uint64_t occupant, uint32_t data1, uint32_t data3,
                            uint32_t data4, uint32_t value, fc4_zonedev_parcel_result* out);
/* Slot 6. A NULL or oversized region does nothing. out may be NULL. */
void fc4_zonedev_do_parcellization(fc4_zonedev* d, const fc4_zone_region* region, int32_t zone_type, bool flag,
                                   fc4_zonedev_parcel_result* out);
/* Slot 5. point: NULL or (x, z). With want_out, *out_written / *out_rect tell the region the core wrote. */
void fc4_zonedev_highlight_parcels(fc4_zonedev* d, const fc4_zone_region* region, int32_t zone_type,
                                   const int32_t* point, bool want_out, bool* out_written, fc4_cell_rect* out_rect,
                                   fc4_zonedev_parcel_result* res);
void fc4_zonedev_set_options(fc4_zonedev* d, bool alternate, bool streets, bool custom);             /* slot 7 */
bool fc4_zonedev_exists_network_of_type(const fc4_zonedev* d, int32_t x, int32_t z, uint32_t flags); /* slot 8 */
bool fc4_zonedev_is_cell_water(const fc4_zonedev* d, int32_t x, int32_t z);                         /* slot 9 */
/* Lot manager. */
uint64_t fc4_zonedev_get_lot_at_position(const fc4_zonedev* d, const float* pos3);           /* slot 5 */
uint64_t fc4_zonedev_get_lot(const fc4_zonedev* d, int32_t x, int32_t z, bool origin_only); /* slot 6 */
/* Slot 7: the lots to append (the caller clears its list first), ONE reference each. Returns the count. */
uint32_t fc4_zonedev_lots_in_cell_rect(const fc4_zonedev* d, const fc4_cell_rect* rect, bool origin_only,
                                       fc4_token_callback cb, void* ctx);
/* Slots 8-10: the lots to append to a list already holding existing[0..n], ONE reference each. */
uint32_t fc4_zonedev_lots_surrounding_cell(const fc4_zonedev* d, int32_t x, int32_t z, uint32_t flags,
                                           const uint64_t* existing, size_t n, fc4_token_callback cb, void* ctx);
uint32_t fc4_zonedev_lots_surrounding_lot(const fc4_zonedev* d, uint64_t lot, uint32_t flags,
                                          const uint64_t* existing, size_t n, fc4_token_callback cb, void* ctx);
uint32_t fc4_zonedev_lots_surrounding_rect(const fc4_zonedev* d, const fc4_cell_rect* rect, uint32_t flags,
                                           const uint64_t* existing, size_t n, fc4_token_callback cb, void* ctx);
/* Slot 11: false when no flag bit is set (out untouched). */
bool fc4_zonedev_vertices_at_edge_of_lot(const fc4_zonedev* d, const fc4_cell_rect* rect, uint32_t flags,
                                         fc4_cell_rect* out);
bool fc4_zonedev_neighbor_cells_at_edge_of_lot(const fc4_zonedev* d, const fc4_cell_rect* rect, uint32_t flags,
                                               fc4_cell_rect* out); /* slot 12, AL */
uint64_t fc4_zonedev_random_lot(fc4_zonedev* d);                            /* slot 13 (draws, §9 Z1) */
uint64_t fc4_zonedev_occupant_lot(const fc4_zonedev* d, uint64_t occupant); /* slot 14 */
uint32_t fc4_zonedev_lot_count(const fc4_zonedev* d);                       /* slot 16 */
/* The master list in order (borrowed). Returns the count. */
uint32_t fc4_zonedev_master_lots(const fc4_zonedev* d, fc4_token_callback cb, void* ctx);
float fc4_zonedev_lot_savvy_terrain_vertex_altitude(const fc4_zonedev* d, int32_t vx, int32_t vz, uint64_t exclude,
                                                    bool* ok);                                 /* slot 17 */
float fc4_zonedev_lot_savvy_terrain_altitude(const fc4_zonedev* d, float x, float z, bool* ok); /* slot 18 */
float fc4_zonedev_lot_savvy_cell_altitude(const fc4_zonedev* d, int32_t cx, int32_t cz);        /* slot 19 */
int32_t fc4_zonedev_can_create_lot(const fc4_zonedev* d, int32_t x, int32_t z, int32_t w, int32_t h,
                                   bool zone_not_needed, bool water_allowed, uint64_t cfg); /* slot 20 */
/* Slot 21 (§8.4.1): 0 outside, 1 water, 3 network, 4 too steep, 5 / 6 constraint tile, 7 special zone, 8 free.
 * A NULL rect gives 0. Writes nothing. */
int32_t fc4_zonedev_can_create_plopped_lot(const fc4_zonedev* d, const fc4_cell_rect* rect, int32_t facing,
                                           uint64_t cfg);
/* Slot 22 (cfg ignored): EAX; *out (may be NULL) gets the lot with ONE reference, or 0. */
bool fc4_zonedev_create_lot_if_possible(fc4_zonedev* d, int32_t x, int32_t z, int32_t w, int32_t h, int32_t facing,
                                        bool zone_not_needed, bool water_allowed, uint64_t* out);
/* Slot 23: EAX 0/1; *out (may be NULL) gets the lot with ONE reference. */
bool fc4_zonedev_create_lot(fc4_zonedev* d, int32_t x, int32_t z, int32_t w, int32_t h, int32_t facing,
                            uint64_t* out);
void fc4_zonedev_delete_lot(fc4_zonedev* d, uint64_t lot);                                    /* slot 24 */
void fc4_zonedev_delete_lots(fc4_zonedev* d, int32_t x1, int32_t z1, int32_t x2, int32_t z2); /* slot 25 */
void fc4_zonedev_lot_vertex_altitude_adjustment_range(const fc4_zonedev* d, int32_t vx, int32_t vz, float* a,
                                                      float* b); /* slot 26 */
uint64_t fc4_zonedev_max_capacity_grid(const fc4_zonedev* d);   /* slot 27, no AddRef */
bool fc4_zonedev_get_state(const fc4_zonedev* d, fc4_zonedev_state* out);
bool fc4_zonedev_write(const fc4_zonedev* d, const fc4_ostream* out);
/* Runs before Init: nothing is sized from the city. Lots are asked for with IID 0xE818FCED. */
bool fc4_zonedev_read(fc4_zonedev* d, const fc4_istream* in);

/* ---- neighbours (docs/specs/neighbors.md; engine/capi/src/neighbors.rs) ------
 *
 * The plugin's cISC4NeighborsSimulator forwards its slots to fc4_neighbors_*,
 * its deals (cISC4NeighborDeal) to fc4_neighbor_deal_* and its connections
 * (cISC4NeighborConnection) to fc4_neighbor_connection_*. Tokens are host
 * identities (regional cities, the city, occupants, props, connections,
 * deals); 0 is null. A connection or deal of ours is a core object behind a
 * handle that its GZCOM object owns (the object destroys the handle when it
 * dies); the core asks for the object through `connection_object` /
 * `deal_object`. A foreign connection (an original accepted on load, §15.2)
 * is read and written through the `foreign_connection` callbacks; foreign
 * deals reach the core as data. The core is re-entrant: a message delivered
 * during another call (the status report SetAmount SENDS, §5.3) is handled at
 * once. Main thread only (the plugin serialises other threads). */
typedef struct fc4_neighbors fc4_neighbors;
typedef struct fc4_neighbor_deal fc4_neighbor_deal;
typedef struct fc4_neighbor_connection fc4_neighbor_connection;

/* A deal's state (§5.1; the property holder is not part of it). Dates are
 * Julian days. */
typedef struct fc4_neighbor_deal_data {
    uint32_t deal_type;
    int32_t other_x, other_z; /* regionCellOtherSide */
    int32_t city_x, city_z;   /* regionCellCitySide */
    int32_t amount, inc_amount, inc_amount2, last_amount, last_inc_amount, amount_used;
    uint32_t flags;
    int64_t cost;
    uint32_t start_date, expiration_date, current_date, suspension_date;
} fc4_neighbor_deal_data;

/* A connection's state (§4.1). */
typedef struct fc4_neighbor_connection_state {
    uint32_t conn_type;
    int32_t x, z;             /* the connection cell */
    int32_t other_x, other_z; /* otherRegionCell */
    uint8_t visited, active;
    uint8_t reserved[2];
} fc4_neighbor_connection_state;

/* What UpdateCommodityStats reads (§8.2). A department is missing when its
 * has_* byte is 0. */
typedef struct fc4_neighbors_figures {
    double total_funds, monthly_income;
    double power_production, power_requirement;
    double water_capacity, water_demand;
    double garbage_recycled, waste_to_energy_capacity, incinerator_capacity, garbage_abducted, garbage_produced;
    uint8_t has_power_department, has_water_department, has_sanitation_department;
    uint8_t reserved[5];
    double power_department_expenses, water_department_expenses, sanitation_department_expenses;
    double waste_to_energy_expenses; /* line item 0x1F470000 of the power department */
} fc4_neighbors_figures;

typedef struct fc4_neighbors_message {
    uint32_t msg_type, data1, data2, data3;
    uint64_t occupant; /* GetVoid1() as cISC4Occupant (0xAA7A014D, 0x99EF1143); 0 = none */
} fc4_neighbors_message;

typedef struct fc4_neighbors_state {
    uint32_t struct_size;
    uint8_t initialized, ready, deals_dirty, stats_dirty, ui_disabled, busy;
    uint8_t reserved0[2];
    uint32_t connections, foreign_connections, deals, reserved1;
    int64_t income, expense; /* §10.1 */
} fc4_neighbors_state;

typedef void (*fc4_neighbor_link_callback)(void* ctx, uint32_t conn_type, int32_t rx, int32_t rz);
typedef void (*fc4_neighbor_deal_data_callback)(void* ctx, const fc4_neighbor_deal_data* data);
/* One property in the game's native variant layout (as fc4_props_set_typed). */
typedef void (*fc4_neighbor_property_callback)(void* ctx, uint32_t id, uint16_t mem_type, bool is_array,
                                               const void* data, size_t count);

/* One callback per NeighborsHost method (engine/core/src/neighbors/mod.rs), in
 * the trait's order. */
typedef struct fc4_neighbors_host {
    uint32_t struct_size; /* sizeof(fc4_neighbors_host) */
    void* user;
    /* lifecycle */
    fc4_props* (*tuning)(void* user); /* a NEW fc4_props (the core destroys it), NULL = missing */
    void (*add_agent)(void* user, uint32_t level, const char* name, size_t len, uint32_t flags);
    void (*remove_agents)(void* user);
    void (*subscribe)(void* user, uint32_t msg_type);
    void (*unsubscribe)(void* user, uint32_t msg_type);
    /* city */
    void (*cell_counts)(void* user, int32_t* x, int32_t* z);
    uint32_t (*today)(void* user);
    void (*utility_figures)(void* user, fc4_neighbors_figures* out);
    void (*landfill_usage)(void* user, uint32_t* usage, uint32_t* capacity);
    void (*send_history)(void* user, uint32_t series, int32_t value);
    /* region */
    uint64_t (*current_city)(void* user);
    uint64_t (*current_city_object)(void* user);
    uint64_t (*city_at)(void* user, int32_t rx, int32_t rz);
    void (*city_position)(void* user, uint64_t city, int32_t* x, int32_t* z);
    void (*city_size)(void* user, uint64_t city, int32_t* x, int32_t* z);
    bool (*city_established)(void* user, uint64_t city);
    /* stat: 0 exported, 1 imported, 2 produced, 3 demanded, 4 cost per unit */
    double (*city_stat)(void* user, uint64_t city, uint32_t stat, int32_t commodity);
    void (*set_city_stat)(void* user, uint64_t city, uint32_t stat, int32_t commodity, double value);
    double (*city_budget)(void* user, uint64_t city);
    void (*set_city_budget)(void* user, uint64_t city, double value);
    void (*set_city_income)(void* user, uint64_t city, double value);
    /* city.FindConnection(type, x, z): the connection (borrowed) and its GetIsActive; 0 = none */
    uint64_t (*city_find_connection)(void* user, uint64_t city, uint32_t conn_type, int32_t x, int32_t z,
                                     bool* active);
    void (*city_connections)(void* user, uint64_t city, fc4_neighbor_link_callback cb, void* ctx);
    void (*change_symmetric_connection)(void* user, uint64_t city, uint64_t conn, bool add);
    void (*reload_city)(void* user, uint64_t city);
    void (*update_local_deals)(void* user, uint64_t city);
    void (*local_deals)(void* user, uint64_t city, fc4_neighbor_deal_data_callback cb, void* ctx);
    /* SetLocalDeals with the deal objects deals[0..count]; kept[i] := 0 for
     * every deal the region removed from the list. */
    void (*set_local_deals)(void* user, uint64_t city, const uint64_t* deals, size_t count, uint8_t* kept);
    void (*update_import_export)(void* user, uint64_t city);
    /* messages: a status report (§5.3), deal 0 for the retry report */
    void (*post_status)(void* user, uint64_t deal, uint32_t status);
    void (*send_status)(void* user, uint64_t deal, uint32_t status);
    void (*post_connection_message)(void* user, uint32_t msg_type, uint32_t conn_type, int32_t x, int32_t z);
    /* markers */
    void (*cell_center)(void* user, int32_t x, int32_t z, float* px, float* pz);
    float (*marker_altitude)(void* user, float px, float pz);
    float (*max_surface_level)(void* user, int32_t x, int32_t z);
    /* position: 3 floats; bool_props[0..count] are set true; returns the prop (0 = none) */
    uint64_t (*add_marker_prop)(void* user, const float* position, int32_t orientation, const uint32_t* bool_props,
                                size_t count);
    void (*marker_props)(void* user, int32_t x, int32_t z, fc4_token_callback cb, void* ctx);
    void (*remove_prop)(void* user, uint64_t prop);
    void (*move_prop)(void* user, uint64_t prop, const float* position);
    /* occupants */
    uint32_t (*occupant_type)(void* user, uint64_t occupant);
    void (*occupant_networks)(void* user, uint64_t occupant, fc4_token_callback cb, void* ctx); /* type numbers */
    void (*occupant_cell)(void* user, uint64_t occupant, int32_t* x, int32_t* z);
    bool (*power_pole_flag)(void* user, uint64_t occupant);
    /* objects (§15.2) */
    /* The GZCOM object for a new connection of ours: takes the NEW handle,
     * returns the object's token holding one reference for the list; 0 = none
     * (the core destroys the handle). */
    uint64_t (*connection_object)(void* user, fc4_neighbor_connection* handle);
    /* The GZCOM object for a deal of ours (first handed to the host): takes
     * the NEW handle, returns the token; no reference is held for the core. */
    uint64_t (*deal_object)(void* user, fc4_neighbor_deal* handle);
    bool (*foreign_connection)(void* user, uint64_t conn, fc4_neighbor_connection_state* out);
    /* -1 = leave, 0 = false, 1 = true */
    void (*set_foreign_connection_flags)(void* user, uint64_t conn, int32_t visited, int32_t active);
    void (*repair_foreign_connection)(void* user, uint64_t conn, uint64_t city);
    void (*release_connection)(void* user, uint64_t conn); /* the list's reference */
    /* Foreign deals (neighbors.md §15.4 NG4; ABI 45, appended): a caller's
     * deal object kept as the listed deal (fc4_neighbors_add_foreign_deal).
     * foreign_deal reads its state through its interface (false =
     * unreadable); foreign_deal_call runs FC4_NEIGHBOR_DEAL_CALL_* on it
     * (returns the call's bool, true for the void ones); release_deal
     * releases the reference the core was given. */
    bool (*foreign_deal)(void* user, uint64_t deal, fc4_neighbor_deal_data* out);
    bool (*foreign_deal_call)(void* user, uint64_t deal, uint32_t op, int64_t a, int64_t b);
    void (*release_deal)(void* user, uint64_t deal);
} fc4_neighbors_host;

/* foreign_deal_call ops: 0-11 = the setters numbered as in
 * fc4_neighbor_deal_set_field (a, b; dates are Julian days), then: */
#define FC4_NEIGHBOR_DEAL_CALL_SET_AMOUNT     12u /* SetAmount(a) */
#define FC4_NEIGHBOR_DEAL_CALL_UPDATE_COST    13u
#define FC4_NEIGHBOR_DEAL_CALL_UPDATE_STATE   14u
#define FC4_NEIGHBOR_DEAL_CALL_CANCEL         15u
#define FC4_NEIGHBOR_DEAL_CALL_ACTIVATE       16u /* returns Activate's bool */
#define FC4_NEIGHBOR_DEAL_CALL_SET_FLAGS      17u /* SetFlags(a, b != 0); returns its bool */

/* `token`: the simulator's host identity; `session`: the session seed
 * (foundations §6.3) of the proposal generator. NULL for a short table. */
fc4_neighbors* fc4_neighbors_create(const fc4_neighbors_host* host, uint64_t token, uint32_t session);
/* Also releases the connections a loaded, never initialised object holds. */
void fc4_neighbors_destroy(fc4_neighbors* p);
void fc4_neighbors_set_two_curves(fc4_neighbors* p, bool on); /* §8.3 [choice], off by default */
bool fc4_neighbors_init(fc4_neighbors* p);                    /* §14.1 */
bool fc4_neighbors_shutdown(fc4_neighbors* p);                /* §14.2 */
bool fc4_neighbors_do_message(fc4_neighbors* p, const fc4_neighbors_message* m); /* §11.1 */
bool fc4_neighbors_get_state(const fc4_neighbors* p, fc4_neighbors_state* out);
bool fc4_neighbors_write(const fc4_neighbors* p, const fc4_ostream* out); /* §13 */
bool fc4_neighbors_read(fc4_neighbors* p, const fc4_istream* in);         /* §13: false unless version 2 */
/* 5: 0 = exists, 1 = ask the player (`neighbour`: the established neighbour, 0 = the default name). */
int32_t fc4_neighbors_request_create_connection(fc4_neighbors* p, uint32_t t, int32_t x, int32_t z,
                                                uint64_t* neighbour);
uint64_t fc4_neighbors_create_connection(fc4_neighbors* p, uint32_t t, int32_t x, int32_t z, bool flip); /* 6 */
void fc4_neighbors_delete_connection(fc4_neighbors* p, uint32_t t, int32_t x, int32_t z);                /* 7 */
void fc4_neighbors_delete_occupant_connection(fc4_neighbors* p, uint64_t occupant);                     /* 8 */
void fc4_neighbors_delete_all_connections_of_type(fc4_neighbors* p, uint32_t t);                        /* 9 */
bool fc4_neighbors_has_connection(const fc4_neighbors* p, uint32_t t, int32_t x, int32_t z);            /* 10 */
uint64_t fc4_neighbors_find_connection(const fc4_neighbors* p, uint32_t t, int32_t x, int32_t z);       /* 11 */
uint64_t fc4_neighbors_find_city_on_other_side_of_connection(const fc4_neighbors* p, int32_t x, int32_t z); /* 12 */
/* 13 / 14: the other city's connection (borrowed) and its GetIsActive. */
uint64_t fc4_neighbors_find_connection_on_other_side(const fc4_neighbors* p, uint32_t t, int32_t x, int32_t z,
                                                     bool* active);
/* §2.2 with the current regional city: false without one. */
bool fc4_neighbors_neighbouring_region_cell(const fc4_neighbors* p, int32_t x, int32_t z, int32_t* rx, int32_t* rz);
uint64_t fc4_neighbors_find_unvisited_connection(const fc4_neighbors* p, uint32_t t, uint64_t city); /* 15 */
/* 16-19: which 0 = all, 1 = of type, 2 = of type to city, 3 = to city. Borrowed tokens; returns the count. */
size_t fc4_neighbors_connections(const fc4_neighbors* p, uint32_t which, uint32_t t, uint64_t city,
                                 fc4_token_callback cb, void* ctx);
/* 20-22: which 0 = adjacent, 1 = connected, 2 = all connected. Borrowed tokens. */
size_t fc4_neighbors_cities(const fc4_neighbors* p, uint32_t which, bool include_unestablished,
                            fc4_token_callback cb, void* ctx);
uint64_t fc4_neighbors_find_connection_suitable_for_deal(const fc4_neighbors* p, uint32_t deal_type,
                                                         uint64_t city); /* 23 */
uint64_t fc4_neighbors_propose_deal(fc4_neighbors* p, uint32_t deal_type, uint64_t city); /* 24: 0 = none */
size_t fc4_neighbors_propose_deals(fc4_neighbors* p, bool add, fc4_token_callback cb, void* ctx); /* 25 */
bool fc4_neighbors_add_deal(fc4_neighbors* p, const fc4_neighbor_deal* deal);                      /* 26, ours */
/* 26 for a foreign deal, the old way: a deal of ours with its state is added; its token.
 * Superseded by fc4_neighbors_add_foreign_deal (NG4). */
uint64_t fc4_neighbors_add_deal_data(fc4_neighbors* p, const fc4_neighbor_deal_data* data);
/* 26 for a foreign deal (NG4, ABI 45): the object itself is listed. `deal` holds one
 * reference for the core, released through release_deal; on false it is still the caller's. */
bool fc4_neighbors_add_foreign_deal(fc4_neighbors* p, uint64_t deal);
void fc4_neighbors_set_ui_disable_flag(fc4_neighbors* p, bool on); /* 27 */
/* 28-30: which 0 = all, 1 = of type (slot 30), 2 = of type with city (slot 29). */
size_t fc4_neighbors_find_deals(fc4_neighbors* p, uint32_t which, uint32_t deal_type, uint64_t city,
                                fc4_token_callback cb, void* ctx);
/* The deals' states in list order: returns the count, fills out[0..cap]. */
size_t fc4_neighbors_deal_data(const fc4_neighbors* p, fc4_neighbor_deal_data* out, size_t cap);
bool fc4_neighbors_holds_deal(const fc4_neighbors* p, const fc4_neighbor_deal* deal);
void fc4_neighbors_claim_deal(fc4_neighbors* p, const fc4_neighbor_deal* deal); /* "the current simulator" */
void fc4_neighbors_mark_deals_dirty(fc4_neighbors* p);                          /* 31 */
int64_t fc4_neighbors_total_income(const fc4_neighbors* p);                     /* 32 */
int64_t fc4_neighbors_total_expense(const fc4_neighbors* p);                    /* 33 */
void fc4_neighbors_update_commodity_stats(fc4_neighbors* p, uint64_t city);     /* 34: the city object */
double fc4_neighbors_deal_cost(const fc4_neighbors* p, uint32_t deal_type, double amount, uint64_t city); /* 35 */
double fc4_neighbors_deal_max_amount(const fc4_neighbors* p, uint32_t deal_type, uint64_t city);          /* 36 */

/* Deals (cISC4NeighborDeal). */
fc4_neighbor_deal* fc4_neighbor_deal_create(void); /* Proposed + Silent, no simulator; the caller owns it */
void fc4_neighbor_deal_destroy(fc4_neighbor_deal* h);
void fc4_neighbor_deal_set_token(fc4_neighbor_deal* h, uint64_t token); /* the object that owns the handle */
bool fc4_neighbor_deal_get(const fc4_neighbor_deal* h, fc4_neighbor_deal_data* out);
/* field 0 type, 1 other side (a, b), 2 city side (a, b), 3 lastAmount, 4
 * lastIncAmount, 5 incAmount, 6 incAmount2, 7 amountUsed, 8-11 start,
 * expiration, current, suspension date (a). */
void fc4_neighbor_deal_set_field(fc4_neighbor_deal* h, uint32_t field, int32_t a, int32_t b);
void fc4_neighbor_deal_set_amount(fc4_neighbor_deal* h, int32_t a);
void fc4_neighbor_deal_update_cost(fc4_neighbor_deal* h);
void fc4_neighbor_deal_copy(fc4_neighbor_deal* h, const fc4_neighbor_deal_data* src);
void fc4_neighbor_deal_update_state(fc4_neighbor_deal* h);
void fc4_neighbor_deal_cancel(fc4_neighbor_deal* h);
bool fc4_neighbor_deal_activate(fc4_neighbor_deal* h);
bool fc4_neighbor_deal_set_flags(fc4_neighbor_deal* h, uint32_t f, bool on);
fc4_neighbor_deal* fc4_neighbor_deal_clone(const fc4_neighbor_deal* h, bool silent); /* a NEW handle */
bool fc4_neighbor_deal_write(const fc4_neighbor_deal* h, const fc4_ostream* out); /* §5.6 */
bool fc4_neighbor_deal_read(fc4_neighbor_deal* h, const fc4_istream* in);

/* Connections (cISC4NeighborConnection). */
fc4_neighbor_connection* fc4_neighbor_connection_create(void);
void fc4_neighbor_connection_destroy(fc4_neighbor_connection* h);
bool fc4_neighbor_connection_get(const fc4_neighbor_connection* h, fc4_neighbor_connection_state* out);
/* field 0 type (a), 1 cell (a, b), 2 other region cell (a, b), 3 visited (a != 0), 4 active (a != 0). */
void fc4_neighbor_connection_set_field(fc4_neighbor_connection* h, uint32_t field, int32_t a, int32_t b);
bool fc4_neighbor_connection_inner_cell(const fc4_neighbor_connection* h, int32_t count_x, int32_t count_z,
                                        int32_t* x, int32_t* z);
void fc4_neighbor_connection_repair(fc4_neighbor_connection* h, int32_t count_x, int32_t count_z, int32_t pos_x,
                                    int32_t pos_z, int32_t size_x, int32_t size_z);
void fc4_neighbor_connection_set_props(fc4_neighbor_connection* h, const fc4_props* props); /* a copy */
size_t fc4_neighbor_connection_props(const fc4_neighbor_connection* h, fc4_neighbor_property_callback cb,
                                     void* ctx);
bool fc4_neighbor_connection_write(const fc4_neighbor_connection* h, const fc4_ostream* out); /* §4.3 */
bool fc4_neighbor_connection_read(fc4_neighbor_connection* h, const fc4_istream* in);         /* version 1 only */
uint32_t fc4_neighbor_connection_class_id(void);                                               /* 0x0A134F24 */

/* ---- neighbours shadow (tooling; engine/capi/src/neighbors_shadow.rs) --------
 *
 * Our core's month on a snapshot of the ORIGINAL simulator's inputs, read
 * through the game's interfaces only: an ordinary fc4_neighbors* whose host
 * is the snapshot (every fc4_neighbors_* query works on it, and
 * fc4_neighbors_do_message runs the month). Writes stay in the snapshot;
 * status reports are counted (a SENT one is delivered to the shadow at
 * once); UpdateImportExport is reproduced as a delta on the stored values. */
typedef struct fc4_neighbors_shadow_city {
    uint64_t token;
    int32_t pos_x, pos_z, size_x, size_z;
    uint8_t established;
    uint8_t reserved[7];
    double budget, income;
    double stats[5][3]; /* [stat][commodity - 1]: exported, imported, produced, demanded, cost per unit */
} fc4_neighbors_shadow_city;

typedef struct fc4_neighbors_shadow_conn {
    uint64_t city; /* the regional city holding it (the current city's: the original's list, in order) */
    uint64_t token;
    fc4_neighbor_connection_state state;
} fc4_neighbors_shadow_conn;

typedef struct fc4_neighbors_shadow_input {
    uint32_t struct_size;
    uint32_t today;
    int32_t count_x, count_z;
    uint64_t current_city, city_object;
    uint32_t landfill_usage, landfill_capacity;
    /* The original's state bytes (neighbors.md §15.4 NG7: ready +0x0C, uiDisabled +0x3E,
     * dealsDirty +0x3C, statsDirty +0x3D, busy +0x3F), as the shim read them. */
    uint8_t ready, ui_disabled, two_curves;
    uint8_t deals_dirty, stats_dirty, busy; /* ABI 45; 0 before (the earlier fixed values) */
    uint8_t reserved[2];
    fc4_neighbors_figures figures;
    const fc4_props* tuning; /* borrowed (copied); NULL = missing */
    const fc4_neighbors_shadow_city* cities;
    size_t city_count;
    const fc4_neighbors_shadow_conn* conns;
    size_t conn_count;
    const fc4_neighbor_deal_data* deals; /* the original's FindDealsAll, in order */
    size_t deal_count;
} fc4_neighbors_shadow_input;

typedef struct fc4_neighbors_shadow_counters {
    uint32_t posts, sends, retries, connection_messages, set_local_deals, update_import_export, history, reserved;
} fc4_neighbors_shadow_counters;

/* Everything is copied; destroy with fc4_neighbors_destroy. */
fc4_neighbors* fc4_neighbors_shadow_create(const fc4_neighbors_shadow_input* input);
/* A city of the snapshot as the shadow left it (false: not a shadow / unknown city). */
bool fc4_neighbors_shadow_city_get(const fc4_neighbors* p, uint64_t city, fc4_neighbors_shadow_city* out);
bool fc4_neighbors_shadow_get_counters(const fc4_neighbors* p, fc4_neighbors_shadow_counters* out);

/* ---- traffic (docs/specs/traffic-simulator.md, traffic-network.md, pathfinder.md) ----
 *
 * The traffic simulator (cSC4TrafficSimulator, class 0x6990C1AA, simulator
 * type 0x88027F0D), its network model and the pathfinders it hands out
 * through slot 6, as one replacement unit (traffic-simulator.md §9,
 * pathfinder.md §9). The plugin's cISC4TrafficSimulator forwards its 44 slots
 * to fc4_traffic_*, the cISC4PathFinder objects of slot 6 forward theirs to
 * fc4_traffic_pf_*.
 *
 * Grids are the game's own (fc4_grid_api): u8 0x49B9E602, u32 0x49B9E606,
 * f32 0x49B9E60A, saved by reference. Tokens are host identities (lots,
 * occupants, buildings, connections, holders, departments, grids); those the
 * host returns are BORROWED unless stated. 0 = none. Holder tokens in
 * 0x000FC4F000000000 .. 0x000FC4F00000FFFF are the simulator's own
 * neighbour pools: the host must not hand those out.
 *
 * Threads: single-threaded; the plugin serialises every call on one object
 * and answers other threads from its snapshot (fc4_traffic_get_state).
 * Re-entrancy: a call made from inside a host callback (a lot's own
 * UpdateTrafficVolume asking slots 26 and 27) is answered from a snapshot
 * taken at the start of the outermost call; a message delivered re-entrantly
 * is queued and run when the outermost call ends. Both are counted. */

typedef struct fc4_traffic fc4_traffic;
typedef struct fc4_traffic_pf fc4_traffic_pf;

/* The kind of a traffic source. */
#define FC4_TRAFFIC_SOURCE_LOT 0u
#define FC4_TRAFFIC_SOURCE_CONNECTION 1u

/* The maps of slots 8, 9, 10, 13. */
#define FC4_TRAFFIC_MAP_AIR 0u
#define FC4_TRAFFIC_MAP_COMMERCIAL 1u
#define FC4_TRAFFIC_MAP_CONGESTION 2u
#define FC4_TRAFFIC_MAP_TRIP_LENGTH 3u

/* The size of the slot-34 statistics block (fc4_traffic_stats_block). */
#define FC4_TRAFFIC_STATS_BLOCK 0x23Cu

/* One cSC4TripData (traffic-simulator.md §2). The routes are byte strings in
 * the compressed route format (pathfinder.md §4.15), valid for the call. */
typedef struct fc4_traffic_trip {
    uint32_t count;
    uint8_t trip_type, strategy, flags, status;
    int16_t dest_x, dest_z;
    float trip_length;
    uint32_t travel_types_used;
    const uint8_t* route;
    size_t route_len;
    const uint8_t* return_route;
    size_t return_len;
} fc4_traffic_trip;

typedef void (*fc4_traffic_trip_callback)(void* ctx, const fc4_traffic_trip* trip);

/* An established neighbouring city (traffic-network.md §2.7.4). */
typedef struct fc4_traffic_city {
    int32_t x, z;
    int32_t population[3];
} fc4_traffic_city;

typedef void (*fc4_traffic_city_callback)(void* ctx, const fc4_traffic_city* city);
/* One (developer type, count) entry of map `map` of a lot's population maps. */
typedef void (*fc4_traffic_population_callback)(void* ctx, uint32_t map, uint32_t developer_type, uint32_t count);
/* One path of a path info: its exit byte and its first vertex. */
typedef void (*fc4_traffic_exit_callback)(void* ctx, uint8_t exit, uint64_t vertex);
typedef void (*fc4_traffic_u32_callback)(void* ctx, uint32_t value);
typedef void (*fc4_traffic_bytes_callback)(void* ctx, const uint8_t* bytes, size_t len);

/* A message the simulator posts (§5, MessagePost). */
typedef struct fc4_traffic_post {
    uint32_t msg_type, data1, data2, data3;
} fc4_traffic_post;

/* GetTransitSwitchQueryData's out (slot 35, §4.16). */
typedef struct fc4_traffic_switch_query {
    uint32_t available, max_capacity;
    float load_fraction;
    uint32_t usage, cost;
} fc4_traffic_switch_query;

/* One FerryRouteInfo of slot 31 without its two routes (they come with the
 * callback). */
typedef struct fc4_traffic_ferry_route {
    uint64_t lot_a, lot_b;
    uint32_t ferry_type;
    uint16_t counter1[2], counter2[2];
    uint8_t neighbour, reserved;
    int16_t neighbour_x, neighbour_z;
    uint16_t reserved2;
} fc4_traffic_ferry_route;

typedef void (*fc4_traffic_ferry_callback)(void* ctx, const fc4_traffic_ferry_route* info, const uint8_t* route_ab,
                                           size_t len_ab, const uint8_t* route_ba, size_t len_ba);

/* The state (diagnostics, the log lines, the plugin's snapshot). */
typedef struct fc4_traffic_state {
    uint32_t struct_size;
    uint8_t initialized, loaded;
    uint8_t cycle_state;  /* the work-cycle state (§3.2) */
    uint8_t strike_state; /* 2 = no strike, 0 started, 1 on strike (§4.12) */
    uint32_t day_index, days;
    int32_t total, processed, pass_total, to_reroute, to_search, last_day;
    uint32_t pass_counters[10];
    uint32_t units_per_slice;
    float trip_scale, trip_scale_for_display, last_monthly_income, freight_scaling; /* slots 14, 15, 18, 27 */
    float transit_income, toll_income, avg_car_commute;
    float pct[4][3]; /* slot 26 for wealth 0..3 */
    uint32_t attempted[3], routed[3], job_capacity[3], free_dest[4];
    uint32_t type_counts[9]; /* per travel type, the last cycle (§4.6) */
    uint8_t freight_byte, reserved[3];
    uint32_t accident_cells, accidents_posted, posted, native_updates, trip_reads, trip_writes;
    uint32_t reentrant_calls, queued_messages;
    uint32_t potholes, road_damaged, rail_damaged, edge_records, switches, ferry_terminals, reserved2;
    uint64_t air, commercial, congestion, trip_length; /* slots 8, 9, 10, 13 (borrowed) */
    uint64_t roads, mass_transit;                      /* the two departments */
} fc4_traffic_state;

/* Everything the traffic unit asks of the game. */
typedef struct fc4_traffic_host {
    uint32_t struct_size; /* sizeof(fc4_traffic_host) */
    void* user;
    fc4_grid_api grids;
    /* framework (§1, §5) */
    void (*add_agent)(void* user, uint32_t level, const uint8_t* name, size_t len, uint32_t flags);
    void (*subscribe)(void* user, uint32_t msg_type);
    void (*post_message)(void* user, const fc4_traffic_post* msg); /* queued */
    void (*add_ref)(void* user, uint64_t token);                   /* a switch building the model keeps */
    void (*release)(void* user, uint64_t token);
    /* city, tuning, dates */
    void (*cell_counts)(void* user, int32_t* x, int32_t* z);
    void (*position_to_cell)(void* user, float x, float z, int32_t* cx, int32_t* cz);
    float (*cell_width)(void* user);
    fc4_props* (*tuning)(void* user); /* NEW fc4_props the core destroys, or NULL */
    uint32_t (*sim_day_of_month)(void* user);
    uint32_t (*sim_date_number)(void* user);
    uint64_t (*sim_clock_ms)(void* user); /* game time in ms (§4.13 [choice]) */
    /* departments (budget.md §7-§8) */
    uint64_t (*create_department)(void* user, uint32_t id, float max_funding); /* ONE reference, 0 = none */
    void (*release_department)(void* user, uint64_t department);
    float (*department_funding)(void* user, uint64_t department);
    void (*set_line_item_income)(void* user, uint64_t department, uint32_t item, int64_t income);
    void (*update_network_costs)(void* user, uint64_t department, bool mass_transit, const uint32_t* cells13,
                                 const float* tile_costs13);
    int64_t (*activate_budget_items)(void* user, uint64_t building, uint64_t lot);
    int64_t (*deactivate_budget_items)(void* user, uint64_t holder);
    bool (*budget_item_for_purpose)(void* user, uint64_t building, uint32_t purpose, uint32_t* department,
                                    int64_t* cost);
    uint64_t (*department_budget)(void* user, uint32_t department_id);
    void (*record_history)(void* user, uint32_t id, uint32_t value);
    /* lots */
    void (*lots)(void* user, fc4_token_callback cb, void* ctx);
    uint64_t (*as_lot)(void* user, uint64_t object);
    uint32_t (*lot_zone)(void* user, uint64_t lot);
    bool (*lot_is_livable)(void* user, uint64_t lot);
    bool (*lot_has_road_access)(void* user, uint64_t lot);
    uint64_t (*lot_link)(void* user, uint64_t lot);
    uint32_t (*lot_travel_population)(void* user, uint64_t lot, uint32_t developer_type);
    uint8_t (*lot_strategy_byte)(void* user, uint64_t lot);
    void (*clear_lot_flag)(void* user, uint64_t lot, uint32_t flag);
    uint64_t (*lot_at)(void* user, int32_t x, int32_t z, bool anchor_only);
    void (*lot_bounding_rect)(void* user, uint64_t lot, fc4_cell_rect* out);
    void (*lot_location)(void* user, uint64_t lot, int32_t* x, int32_t* z);
    uint32_t (*lot_facing)(void* user, uint64_t lot);
    uint64_t (*lot_building)(void* user, uint64_t lot);
    void (*lot_populations)(void* user, uint64_t lot, fc4_traffic_population_callback cb, void* ctx);
    uint64_t (*lot_holder)(void* user, uint64_t lot);
    /* traffic sources (cISC4TrafficSource) */
    bool (*update_traffic_volume)(void* user, uint32_t kind, uint64_t source); /* true: the source ran it */
    bool (*trip_list)(void* user, uint32_t kind, uint64_t source, fc4_traffic_trip_callback cb, void* ctx);
    void (*set_trip_list)(void* user, uint32_t kind, uint64_t source, const fc4_traffic_trip* trips, size_t count);
    /* property holders */
    bool (*holder_property)(void* user, uint64_t holder, uint32_t id, fc4_props* out);
    /* AddProperty: mem type, array flag, `count` elements in the native layout;
     * in place for an existing property of the same type and length. */
    void (*set_holder_property)(void* user, uint64_t holder, uint32_t id, uint16_t mem_type, bool is_array,
                                const void* data, size_t count);
    bool (*remove_holder_property)(void* user, uint64_t holder, uint32_t id);
    /* neighbour connections */
    uint64_t (*connection_holder)(void* user, uint64_t connection);
    bool (*take_connection_volume_dirty)(void* user, uint64_t connection);
    uint64_t (*find_connection)(void* user, uint32_t connection_type, int32_t x, int32_t z);
    uint32_t (*connection_type)(void* user, uint64_t connection);
    uint64_t (*connection_on_other_side)(void* user, uint64_t connection);
    bool (*established_city_position)(void* user, uint64_t connection, int32_t* x, int32_t* z);
    void (*connected_cities)(void* user, fc4_traffic_city_callback cb, void* ctx);
    bool (*regional_demand)(void* user, uint32_t w, float* out);
    void (*connections_of_type)(void* user, uint32_t connection_type, fc4_token_callback cb, void* ctx);
    void (*all_connections)(void* user, fc4_token_callback cb, void* ctx);
    void (*connection_cell)(void* user, uint64_t connection, int32_t* x, int32_t* z);
    /* other simulators */
    float (*workforce_percent)(void* user, int32_t x, int32_t z);
    int16_t (*developer_population)(void* user, uint32_t developer_type, int32_t x, int32_t z);
    bool (*is_ordinance_on)(void* user, uint32_t id);
    void (*ordinances_on)(void* user, fc4_traffic_u32_callback cb, void* ctx);
    bool (*ordinance_property)(void* user, uint32_t id, uint32_t prop, fc4_props* out);
    /* occupants */
    uint64_t (*as_network_occupant)(void* user, uint64_t object);
    void (*network_occupant_cell)(void* user, uint64_t occupant, int32_t* x, int32_t* z);
    void (*occupants_in_cells)(void* user, const fc4_cell_rect* rect, fc4_token_callback cb, void* ctx);
    bool (*occupant_filter_includes)(void* user, uint64_t filter, uint64_t occupant);
    bool (*cell_has_network_occupant)(void* user, int32_t x, int32_t z);
    void (*network_occupants_in_rect)(void* user, const fc4_cell_rect* rect, fc4_token_callback cb, void* ctx);
    void (*subway_occupants_in_rect)(void* user, const fc4_cell_rect* rect, fc4_token_callback cb, void* ctx);
    void (*prop_occupants_in_rect)(void* user, const fc4_cell_rect* rect, fc4_token_callback cb, void* ctx);
    void (*occupant_position)(void* user, uint64_t occupant, float* xyz);
    void (*occupant_bounding_cells)(void* user, uint64_t occupant, fc4_cell_rect* out);
    uint32_t (*prop_type)(void* user, uint64_t occupant);
    bool (*occupant_property)(void* user, uint64_t occupant, uint32_t id, fc4_props* out);
    bool (*has_network_flag)(void* user, uint64_t occupant, uint32_t flag);
    bool (*has_any_network_flag)(void* user, uint64_t occupant, uint32_t mask);
    bool (*is_of_type)(void* user, uint64_t occupant, uint32_t network_type);
    bool (*path_connection)(void* user, uint64_t occupant, uint32_t path_type, uint32_t entry, uint32_t exit);
    uint64_t (*path_info)(void* user, uint64_t occupant);
    bool (*path_info_turn_around)(void* user, uint64_t path_info, uint32_t path_type, uint32_t entry);
    void (*path_info_exits_from_entry)(void* user, uint64_t path_info, uint32_t path_type, uint32_t entry,
                                       fc4_traffic_exit_callback cb, void* ctx);
    void (*path_info_entries_near)(void* user, uint64_t path_info, uint32_t path_type, uint64_t vertex,
                                   uint32_t exit, float radius, fc4_traffic_u32_callback cb, void* ctx);
    bool (*is_tunnel)(void* user, uint64_t occupant);
    uint64_t (*tunnel_paired_portal)(void* user, uint64_t occupant);
    /* terrain and water */
    bool (*cell_under_water)(void* user, int32_t x, int32_t z);
    bool (*is_navigable)(void* user, int32_t x, int32_t z);
    float (*water_depth)(void* user, int32_t x, int32_t z);
    bool (*body_of_water)(void* user, int32_t x, int32_t z, uint16_t* out);
    /* language, view, accidents */
    bool (*drives_on_right)(void* user);
    bool (*view_cell_rect)(void* user, fc4_cell_rect* out);
    void (*create_accident)(void* user, int32_t x, int32_t z, int64_t duration_ms);
    /* ABI 61 (traffic-simulator §9.1 TG7): CreateLineItem(line, false), SetName(0x6A3FF01C, name)
     * and, when income, SetType(Income); with only_if_missing nothing when GetLineItem(line) is
     * not null. create_department makes the department only (CreateDepartmentBudget(id,
     * 0xAA369059), SetDepartmentName, SetMaxAllowedFundingPercentage); the core asks for its
     * line items through this. */
    void (*create_line_item)(void* user, uint64_t department, uint32_t line, uint32_t name, bool income,
                             bool only_if_missing);
} fc4_traffic_host;

/* QueryInterface(iid) (§1): 1, 0x68027F37, 0x452294AA, 0xE4FDA3D4. */
bool fc4_traffic_query_interface(uint32_t iid);
uint32_t fc4_traffic_class_id(void);       /* 0x6990C1AA */
uint32_t fc4_traffic_simulator_type(void); /* slot 5: 0x88027F0D */
uint32_t fc4_traffic_default_units_per_slice(void); /* §3.4 [choice] */
/* NULL when the table is incomplete. The table is copied. */
fc4_traffic* fc4_traffic_create(const fc4_traffic_host* host, uint32_t session_seed);
/* Releases what is still held (grids, departments, switch buildings). */
void fc4_traffic_destroy(fc4_traffic* p);
/* Init (slot 3); units_per_slice 0 keeps the default. */
bool fc4_traffic_init(fc4_traffic* p, uint32_t units_per_slice);
bool fc4_traffic_shutdown(fc4_traffic* p); /* slot 4 */
/* DoMessage (§3.1); `object` = void1 / the IGZUnknown. Re-entrantly: queued, true. */
bool fc4_traffic_do_message(fc4_traffic* p, uint32_t msg_type, uint32_t data1, uint32_t data2, uint32_t data3,
                            uint64_t object);
bool fc4_traffic_write(const fc4_traffic* p, const fc4_ostream* out);
/* Runs before Init: nothing is sized from the city. A failed Read changes nothing. */
bool fc4_traffic_read(fc4_traffic* p, const fc4_istream* in);
bool fc4_traffic_get_state(const fc4_traffic* p, fc4_traffic_state* out);
uint64_t fc4_traffic_map(const fc4_traffic* p, uint32_t which); /* slots 8, 9, 10, 13: borrowed */
float fc4_traffic_map_average(const fc4_traffic* p, uint32_t which);
float fc4_traffic_background_traffic(const fc4_traffic* p, int32_t x, int32_t z); /* slot 11 */
/* Slot 12: the 33 u16 (last[16], usage, current[16]); false (zeros) for the shared zero record. */
bool fc4_traffic_edge_density(const fc4_traffic* p, uint8_t x, uint8_t z, uint32_t travel_type, bool evening,
                              uint16_t* out33);
/* Slots 14, 15, 18, 27 for which = 0..3. */
float fc4_traffic_scalar(const fc4_traffic* p, uint32_t which);
bool fc4_traffic_is_road_damaged(const fc4_traffic* p, int32_t x, int32_t z);   /* slot 16 */
bool fc4_traffic_check_rail_accident(fc4_traffic* p, int32_t x, int32_t z);     /* slot 17 */
void fc4_traffic_set_max_trip_capacity(fc4_traffic* p, uint64_t holder, uint32_t k, uint32_t value); /* 19 */
uint32_t fc4_traffic_max_trip_capacity(const fc4_traffic* p, uint64_t holder, uint32_t k);           /* 20 */
uint32_t fc4_traffic_traffic_arrived(const fc4_traffic* p, uint64_t holder, uint32_t k);             /* 21 */
bool fc4_traffic_desired_lot_insertion_point(const fc4_traffic* p);                                  /* 22 */
float fc4_traffic_capacity(const fc4_traffic* p, uint32_t n, int32_t x, int32_t z);                  /* 23 */
float fc4_traffic_travel_time_ratio(const fc4_traffic* p, int32_t x, int32_t z, uint32_t t);         /* 24 */
uint32_t fc4_traffic_connection_count(const fc4_traffic* p, uint32_t n, int32_t x, int32_t z);       /* 25 */
void fc4_traffic_travel_strategy_percentages(const fc4_traffic* p, uint32_t wealth, float* out3);    /* 26 */
/* Slot 28: the switch buildings in table order (borrowed); returns the count. */
size_t fc4_traffic_transit_switches(const fc4_traffic* p, fc4_token_callback cb, void* ctx);
bool fc4_traffic_ferry_route_between_tiles(const fc4_traffic* p, int32_t x1, int32_t z1, int32_t x2, int32_t z2,
                                           fc4_traffic_bytes_callback cb, void* ctx); /* 29 */
size_t fc4_traffic_all_ferry_routes(const fc4_traffic* p, fc4_traffic_bytes_callback cb, void* ctx); /* 30 */
size_t fc4_traffic_ferry_routes_in_use(const fc4_traffic* p, fc4_traffic_ferry_callback cb, void* ctx); /* 31 */
uint32_t fc4_traffic_ferry_terminal_count(const fc4_traffic* p, uint32_t ferry_type); /* 32 */
/* Slot 33: `cb` always called (empty when not found). */
bool fc4_traffic_water_route(fc4_traffic* p, int32_t x1, int32_t z1, int32_t x2, int32_t z2,
                             fc4_traffic_bytes_callback cb, void* ctx);
/* Slot 34: the block (FC4_TRAFFIC_STATS_BLOCK bytes) into out; returns its size, 0 re-entrantly. */
size_t fc4_traffic_stats_block(const fc4_traffic* p, uint8_t* out, size_t len);
bool fc4_traffic_transit_switch_query_data(fc4_traffic* p, int32_t x, int32_t z,
                                           fc4_traffic_switch_query* out); /* 35 */
bool fc4_traffic_are_lots_connected(const fc4_traffic* p, uint64_t a, uint64_t b);              /* 36 */
uint32_t fc4_traffic_connected_occupant_count(const fc4_traffic* p, uint64_t lot, uint32_t dev); /* 37 */
uint32_t fc4_traffic_connected_destination_count(const fc4_traffic* p, uint64_t lot, int32_t k); /* 38 */
size_t fc4_traffic_subnetworks_for_lot(const fc4_traffic* p, uint64_t lot, fc4_traffic_u32_callback cb, void* ctx,
                                       bool* ok); /* 39 */
size_t fc4_traffic_subnetworks_in_rectangle(const fc4_traffic* p, const fc4_cell_rect* rect,
                                            fc4_traffic_u32_callback cb, void* ctx); /* 40 */
size_t fc4_traffic_subnetworks_in_region(const fc4_traffic* p, const fc4_zone_region* region,
                                         fc4_traffic_u32_callback cb, void* ctx); /* 41 */
size_t fc4_traffic_occupant_count_for_all_subnetworks(const fc4_traffic* p, uint32_t dev,
                                                      fc4_traffic_u32_callback cb, void* ctx); /* 42 */
/* Slot 7: false for lot 0 or a lot without trips. */
bool fc4_traffic_setup_path_finder_for_lot(fc4_traffic* p, fc4_traffic_pf* pf, uint64_t lot);

/* ---- the pathfinders of slot 6 (pathfinder.md). Each belongs to the
 * simulator that created it; calls that search name both. */

/* The configuration a pathfinder holds (diagnostics, tests). */
typedef struct fc4_traffic_pf_state {
    uint32_t struct_size;
    uint8_t initialised, suspended, has_route, destination_flags;
    uint32_t flags, filter;
    uint16_t allowed, starting;
    uint32_t density, max_work_time;
    float trip_length;
    fc4_cell_rect start, end;
    int32_t city_x, city_z;
} fc4_traffic_pf_state;

/* CreatePathFinder (slot 6): a new, initialised pathfinder. */
fc4_traffic_pf* fc4_traffic_pf_create(fc4_traffic* p);
void fc4_traffic_pf_destroy(fc4_traffic_pf* pf);
bool fc4_traffic_pf_init(fc4_traffic* p, fc4_traffic_pf* pf); /* slot 3: false when initialised */
bool fc4_traffic_pf_shutdown(fc4_traffic_pf* pf);             /* slot 4 */
bool fc4_traffic_pf_is_initialised(const fc4_traffic_pf* pf);
/* Slots 5-7, 9-15, 18, 21, 22 by number (a, b: the arguments); false for others. */
bool fc4_traffic_pf_set(fc4_traffic_pf* pf, uint32_t slot, uint32_t a, uint32_t b);
/* Slot 8: on = keep a route of the pathfinder's own (fc4_traffic_pf_route). */
void fc4_traffic_pf_set_route_info(fc4_traffic_pf* pf, bool on);
void fc4_traffic_pf_set_starting_cost_array(fc4_traffic_pf* pf, const float* costs9); /* 16; NULL = default */
void fc4_traffic_pf_set_max_trip_length(fc4_traffic_pf* pf, float v);                 /* 17 */
void fc4_traffic_pf_set_rect(fc4_traffic_pf* pf, bool end, const fc4_cell_rect* rect); /* 19, 20 */
uint32_t fc4_traffic_pf_find_path(fc4_traffic* p, fc4_traffic_pf* pf); /* 23: 0, 1, 2 */
uint32_t fc4_traffic_pf_traffic_arrived(const fc4_traffic_pf* pf);     /* 24 */
float fc4_traffic_pf_trip_length(const fc4_traffic_pf* pf);            /* 25 */
float fc4_traffic_pf_trip_length_for_display(const fc4_traffic* p, const fc4_traffic_pf* pf); /* 26 */
void fc4_traffic_pf_end_cell(const fc4_traffic_pf* pf, int32_t* x, int32_t* z);              /* 27 */
uint8_t fc4_traffic_pf_last_travel_type(const fc4_traffic_pf* pf);                          /* 28 */
void fc4_traffic_pf_apply_traffic(fc4_traffic* p, fc4_traffic_pf* pf, uint32_t style);       /* 29 */
bool fc4_traffic_pf_was_trip_reversible(const fc4_traffic* p, const fc4_traffic_pf* pf);     /* 30 */
void fc4_traffic_pf_release_path_nodes(fc4_traffic_pf* pf);                                  /* 31 */
/* Slot 32: the route bytes into cb (always called); returns the mask. */
uint32_t fc4_traffic_pf_compressed_route(const fc4_traffic* p, const fc4_traffic_pf* pf,
                                         fc4_traffic_bytes_callback cb, void* ctx);
/* The route of the last FindPath (root -> result), one {x, z, travelType}
 * (3 bytes) per call; returns the count. */
size_t fc4_traffic_pf_route(const fc4_traffic_pf* pf, fc4_traffic_bytes_callback cb, void* ctx);
bool fc4_traffic_pf_get_state(const fc4_traffic_pf* pf, fc4_traffic_pf_state* out);

/* ---- traffic shadow (tooling, no spec behaviour; engine/capi/src/traffic_shadow.rs) ----
 * A same-process differential of the ORIGINAL traffic simulator: FreeCity4's
 * simulator runs as a private copy on the original's live inputs and is
 * compared bitwise after every delivery the original's agents get (day,
 * month, one-shot). Around each delivery the plugin captures the original's
 * section-6 record through its own Write (fc4_pollution_capture format):
 *   fc4_traffic_shadow_before   the record before: the copy starts on it (first
 *                               delivery) or resyncs on it (when it differs from
 *                               the copy's); the inputs the delivery may change
 *                               (trip lists, lot and connection holder arrays)
 *                               are snapshotted;
 *   (the original's delivery, unchanged)
 *   fc4_traffic_shadow_after    the record after: the copy steps one unit at a
 *                               time to the cycle state and phase progress the
 *                               original stopped at, then its record, every
 *                               trip list, the holder properties and the grids
 *                               are compared; lines go to the sink.
 * The host table's read callbacks only are called (its write, post, budget,
 * agent and reference callbacks may be NULL; the copy keeps those calls to
 * itself) and the grid table's geometry and get only. Lines:
 *   traffic shadow YYYY-MM-DD day|month|begin|message (state a -> b, progress p,
 *     units u): mismatches=N; ...          a delivery with a difference
 *   traffic shadow YYYY-MM-DD kind: INCOMPLETE (why); not compared
 *   traffic shadow month to YYYY-MM-DD: clean|mismatches=N|INCOMPLETE; ... */
typedef struct fc4_traffic_shadow fc4_traffic_shadow;

typedef struct fc4_traffic_shadow_stats {
    uint32_t struct_size;
    uint32_t deliveries;  /* finished (compared or not) */
    uint32_t compared;
    uint32_t clean;       /* compared without a difference */
    uint32_t incomplete;  /* not compared */
    uint32_t resyncs;     /* records before that differed from the copy's */
    int32_t last_mismatches; /* -1: the last delivery was not compared */
    uint32_t reserved;
    uint64_t record_fields;  /* record fields that differed */
    uint64_t trip_lists, trip_mismatches;
    uint64_t props, prop_mismatches;
    uint64_t grid_cells, grid_mismatches;
    uint64_t units;          /* slices the copy ran */
} fc4_traffic_shadow_stats;

/* NULL when the table is incomplete. max_diffs: differences named per line (0
 * -> 8, at most 64); detail_lines: delivery lines per month (0 -> 8). The
 * shadow calls the table's read callbacks, and deactivate_budget_items as a
 * LOOKUP: the table given here must answer it with the cost of the holder's
 * 0xCA550301 budget item without removing anything (NULL reads 0). */
fc4_traffic_shadow* fc4_traffic_shadow_create(const fc4_traffic_host* host, uint32_t session_seed,
                                              uint32_t max_diffs, uint32_t detail_lines);
/* The copy's hypotheses: alternative readings of unclear spec points, to test
 * them against the original (0, the default: the spec as written); set before
 * the first delivery; named in every month line. False for NULL. */
#define FC4_TRAFFIC_SHADOW_HYP_MERGE_ABSORBED 0x1u /* the ConfirmTrips merge sets status 0 only on a trip that absorbed one */
#define FC4_TRAFFIC_SHADOW_HYP_TYPE_COUNTS_SWAP 0x2u /* row 19 pairs (published, building); FullTrips' end swaps them */
#define FC4_TRAFFIC_SHADOW_HYP_REROUTE_MERGE_ALL 0x4u /* the ReRouteTrips merge absorbs routed trips too */
bool fc4_traffic_shadow_set_hypotheses(fc4_traffic_shadow* s, uint32_t mask);
void fc4_traffic_shadow_destroy(fc4_traffic_shadow* s);
/* before: NULL or not ok -> the delivery is reported INCOMPLETE. 0; -1 for NULL s. */
int32_t fc4_traffic_shadow_before(fc4_traffic_shadow* s, uint32_t msg_type, uint32_t data1, uint32_t data2,
                                  uint32_t data3, int32_t julian, const fc4_pollution_capture* before);
/* The difference count; -1 when nothing was compared. */
int32_t fc4_traffic_shadow_after(fc4_traffic_shadow* s, const fc4_pollution_capture* after,
                                 fc4_bytes_sink sink, void* ctx);
/* A message the original receives outside its deliveries (its object borrowed
 * for the call); false for NULL s or before the copy started. */
bool fc4_traffic_shadow_message(fc4_traffic_shadow* s, uint32_t msg_type, uint32_t data1, uint32_t data2,
                                uint32_t data3, uint64_t object);
bool fc4_traffic_shadow_get_stats(fc4_traffic_shadow* s, fc4_traffic_shadow_stats* out);

/* ---- lot developer (docs/specs/lot-developer.md; engine/capi/src/lotdev.rs) ----
 * cSC4LotDeveloper (class 0xA990BFE0, city component 16). The host implements
 * fc4_lotdev_host (one callback per LotDeveloperHost method of the core, in
 * its order); the plugin's cISC4LotDeveloper forwards its slots to the
 * fc4_lotdev_* functions. The §17 geometry slots (16, 18-21, 39, 40, 42,
 * 43, 48-61) are here; slots 38, 41, 45, 62-64 stay the host's.
 * Tokens: lots (cISC4Lot*), configurations (cISC4LotConfiguration*),
 * occupants (cISC4Occupant*), buildings (cISC4BuildingOccupant*), units
 * (cISC4BusinessUnit*), growth developers, the safe lot iterator; 0 = null.
 * Every token the core receives is borrowed, except the night deque's one
 * reference per building (building_add_ref / building_release, and the
 * references fc4_lotdev_read's stream hands over). A unit from unit_create
 * is the host's to keep alive until the outermost call returns.
 * Re-entrancy: a message delivered during a call is queued and handled when
 * the outermost call returns (the host keeps its tokens alive until then);
 * any other call made while the core is busy is refused (the slot's neutral
 * value, counted in refused_calls). The terrain, cost and geometry slots
 * (15-22, 25-32, 39-40, 42-43, 48-61) run on a snapshot refreshed when each
 * outermost call returns, never need the core and always answer.
 * Threads: single-threaded; the host serialises every call on one object. */
typedef void (*fc4_u32s_sink)(void* ctx, const uint32_t* values, size_t n);

typedef struct fc4_lotdev_statics fc4_lotdev_statics;
typedef struct fc4_lotdev fc4_lotdev;

/* The fields of a 0x34-byte cSC4LotConfigurationObject (§17.1). */
typedef struct fc4_lotdev_cfg_object {
    uint32_t object_type;         /* +0x00: 1 prop, 2 base texture, 4 flora, 7 water, ... */
    uint8_t flags;                /* +0x01 */
    uint8_t orientation;          /* +0x02 */
    uint8_t byte3;                /* +0x03 */
    float offset[3];              /* +0x04 .. +0x0C: lot-space x, y, z */
    float footprint_max[2];       /* +0x10, +0x14: max x, max z */
    float footprint_min[2];       /* +0x18, +0x1C: min x, min z */
    uint32_t foundation;          /* +0x20: the foundation type */
    uint32_t tag;                 /* +0x24 */
    uint32_t n_ids;
    const uint32_t* ids; /* families not expanded */
} fc4_lotdev_cfg_object;
typedef void (*fc4_lotdev_cfg_object_callback)(void* ctx, const fc4_lotdev_cfg_object* object);

/* One base-texture record (§6.6, §17.7; lot-records.md §6). */
typedef struct fc4_lotdev_texture {
    uint32_t texture;
    int32_t x, z;
    uint8_t orientation; /* orientation (normal) or rotation (empty lot) */
    uint8_t byte3;       /* object byte 3 (normal) or the flip (empty lot) */
    uint8_t extra[5];    /* 0xFF x 5 (normal); colour >> 24, >> 16, >> 8, alpha, 3 (empty lot) */
    uint8_t frame;       /* 0xFF */
} fc4_lotdev_texture;

typedef struct fc4_lotdev_burnt {
    uint64_t occupant;
    float min[3], max[3];
} fc4_lotdev_burnt;

/* One network connection node of AddNetworkConnectionNodes (§6.5, LD17): the arguments of network manager slot 20
 * CreateConnectionNode(&n, node_type, &cell, b, c, d, orientation). b and c are rotl32(ids[1 / 2], 8 * orientation),
 * d is ids[3] or 0. */
typedef struct fc4_lotdev_conn_node {
    uint32_t node_type; /* ids[0] */
    int32_t x, z;       /* the object's cell */
    uint32_t b, c, d;
    uint8_t orientation;
    uint8_t reserved[3];
} fc4_lotdev_conn_node;

typedef struct fc4_lotdev_profile {
    uint32_t purpose, wealth;
    uint16_t capacity; /* for GetDeveloperType(purpose, wealth) */
    uint16_t reserved;
} fc4_lotdev_profile;

typedef struct fc4_lotdev_message {
    uint32_t msg_type, data1, data2, data3, data4, reserved;
    uint64_t occupant; /* GetVoid1() as a cISC4Occupant (0x99EF1142 / 0x99EF1143), 0 otherwise */
    uint64_t lot;      /* GetIGZUnknown() as a cISC4Lot (0xE8BDA2E7 / 0xE99B3D4D), 0 otherwise */
} fc4_lotdev_message;

typedef struct fc4_lotdev_tuning {
    uint16_t stay_new_time;
    uint8_t attempt_lot_levelling, attempt_internal_lot_levelling;
    float thresholds[3];
    float max_cell_altitude_delta_for_flatness, max_slope_for_flatness;
    uint32_t default_lot_foundation_type, default_building_foundation_type;
    float auto_level_internal_altitude_delta, max_area_slope_for_levelling;
    uint32_t cost_curve_points; /* pairs in cost_curve (at most 16) */
    float cost_curve[32];
} fc4_lotdev_tuning;

typedef struct fc4_lotdev_state {
    uint32_t struct_size; /* = sizeof(fc4_lotdev_state) */
    uint8_t initialized, night_mode, lit_flag, force_empty;
    uint32_t daily_state;
    int32_t visited;
    uint32_t night_len, night_cursor, night_acc, night_batch, no_zot_until_day;
    uint8_t can_check_commute, zots_allowed, net_changed, unit_changed;
    int32_t verts_x;
    float extent_x, extent_z;
    int32_t cell_count_x, cell_count_z;
    uint32_t daily_work_budget;
    /* cumulative counters */
    uint32_t lots_visited, abandoned, restored, condition_changes, zot_changes;
    uint32_t constructions_started, constructions_finished, demolitions, night_batches;
    uint32_t refused_calls, queued_messages, reserved;
} fc4_lotdev_state;

typedef struct fc4_lotdev_host {
    uint32_t struct_size; /* = sizeof(fc4_lotdev_host) */
    void* user;
    /* lifecycle */
    bool (*acquire_services)(void* user);
    void (*release_services)(void* user);
    fc4_props* (*tuning)(void* user); /* NEW props (the core destroys them); NULL = no exemplar */
    bool (*has_simulator)(void* user);
    void (*add_agent)(void* user, uint32_t level, uint32_t message);
    void (*remove_agent)(void* user);
    bool (*acquire_message_server)(void* user);
    void (*release_message_server)(void* user);
    void (*subscribe)(void* user, uint32_t message);
    void (*unsubscribe)(void* user, uint32_t message);
    void (*register_animation_tick)(void* user);
    void (*unregister_animation_tick)(void* user);
    void (*post_message)(void* user, uint32_t msg_type, uint32_t data1, uint64_t lot);
    /* city, simulator, terrain */
    void (*city_cell_counts)(void* user, int32_t* out2);
    void (*city_cell_widths)(void* user, float* out2);
    void (*position_to_cell)(void* user, float x, float z, int32_t* out2); /* city slot 95, no result (LD18) */
    void (*cell_corner_to_position)(void* user, int32_t x, int32_t z, float* out2); /* city slot 96 */
    void (*cell_center_to_position)(void* user, int32_t x, int32_t z, float* out2); /* city slot 97 */
    uint32_t (*sim_date)(void* user);
    void (*terrain_cell_counts)(void* user, int32_t* out2);
    float (*terrain_cell_width)(void* user);
    float (*terrain_altitude)(void* user, int32_t index);
    float (*terrain_water_level)(void* user, int32_t index);
    float (*terrain_altitude_at)(void* user, float x, float z);    /* terrain slot 13, metres */
    float (*terrain_water_level_at)(void* user, float x, float z); /* terrain slot 30 */
    bool (*terrain_is_water_vertex)(void* user, int32_t x, int32_t z); /* terrain slot 37 */
    bool (*terrain_cell_has_water)(void* user, int32_t x, int32_t z);  /* terrain slot 34 */
    uint32_t (*zone_drag_color)(void* user, int32_t zone); /* zone manager slot 15 */
    uint8_t (*zone_display_alpha)(void* user);             /* zone manager slot 16 */
    int32_t (*view_rotation)(void* user);
    uint32_t (*visual_time_mode)(void* user);
    bool (*resource_exists)(void* user, uint32_t type, uint32_t group, uint32_t instance);
    /* lot manager */
    int32_t (*lot_count)(void* user);
    uint64_t (*safe_lot_iterator)(void* user);
    void (*safe_lot_iterator_rewind)(void* user, uint64_t it);
    uint64_t (*safe_lot_iterator_next)(void* user, uint64_t it); /* 0 at the end */
    void (*safe_lot_iterator_free)(void* user, uint64_t it);
    uint64_t (*get_lot)(void* user, int32_t x, int32_t z, bool origin_only);
    void (*lots_in_cell_rect)(void* user, const fc4_cell_rect* rect, bool origin_only, fc4_token_callback cb, void* ctx);
    /* GetLotsSurroundingLot / Rect on a fresh list (the core drops lots it already has) */
    void (*lots_surrounding_lot)(void* user, uint64_t lot, uint32_t flags, fc4_token_callback cb, void* ctx);
    void (*lots_surrounding_rect)(void* user, const fc4_cell_rect* rect, uint32_t flags, fc4_token_callback cb, void* ctx);
    bool (*delete_lot)(void* user, uint64_t lot);
    /* lots */
    bool (*lot_is_initialized)(void* user, uint64_t lot);
    void (*lot_location)(void* user, uint64_t lot, int32_t* out2);
    void (*lot_size)(void* user, uint64_t lot, uint32_t* out2);
    void (*lot_bounding_rect)(void* user, uint64_t lot, fc4_cell_rect* out);
    void (*lot_center)(void* user, uint64_t lot, float* out3);
    int32_t (*lot_zone_type)(void* user, uint64_t lot);
    float (*lot_altitude)(void* user, uint64_t lot);                /* lot slot 16 */
    void (*lot_set_altitude)(void* user, uint64_t lot, float altitude); /* lot slot 17 */
    int32_t (*lot_facing)(void* user, uint64_t lot);                /* lot slot 23 */
    uint32_t (*lot_flags)(void* user, uint64_t lot);
    void (*lot_set_flags)(void* user, uint64_t lot, uint32_t flags);
    void (*lot_set_vertices_locked)(void* user, uint64_t lot, bool locked);
    void (*lot_update_slope)(void* user, uint64_t lot);
    uint32_t (*lot_state)(void* user, uint64_t lot);
    bool (*lot_set_state)(void* user, uint64_t lot, uint32_t state, uint32_t wealth, uint32_t reason);
    uint32_t (*lot_condition)(void* user, uint64_t lot);
    void (*lot_set_condition)(void* user, uint64_t lot, uint32_t condition);
    bool (*lot_is_livable)(void* user, uint64_t lot);
    uint32_t (*lot_days_since_state_change)(void* user, uint64_t lot);
    uint32_t (*lot_building_type)(void* user, uint64_t lot, bool flag);
    void (*lot_set_building_type)(void* user, uint64_t lot, uint32_t building);
    uint64_t (*lot_building)(void* user, uint64_t lot);
    uint64_t (*lot_configuration)(void* user, uint64_t lot);
    void (*lot_set_configuration)(void* user, uint64_t lot, uint64_t cfg);
    uint32_t (*lot_occupant_wealth)(void* user, uint64_t lot);
    float (*lot_occupancy)(void* user, uint64_t lot);
    bool (*lot_needs_power)(void* user, uint64_t lot);
    bool (*lot_has_power)(void* user, uint64_t lot);
    void (*lot_set_has_power)(void* user, uint64_t lot, bool powered);
    bool (*lot_has_job_access)(void* user, uint64_t lot);
    bool (*lot_has_road_access)(void* user, uint64_t lot);
    bool (*lot_has_organic_road_access)(void* user, uint64_t lot, uint32_t purpose);
    void (*lot_update_road_access)(void* user, uint64_t lot);
    float (*lot_travel_jobs)(void* user, uint64_t lot, uint32_t wealth);
    uint64_t (*lot_travel_designate)(void* user, uint64_t lot);
    void (*lot_set_travel_designate)(void* user, uint64_t lot, uint64_t designate);
    uint64_t (*lot_unit)(void* user, uint64_t lot);
    void (*lot_reset_travel_population)(void* user, uint64_t lot);
    void (*lot_add_travel_population)(void* user, uint64_t lot, uint64_t from);
    bool (*lot_is_historical)(void* user, uint64_t lot);
    void (*lot_set_historical)(void* user, uint64_t lot, bool historical);
    uint16_t (*lot_available_capacity)(void* user, uint64_t lot, uint32_t dev_type, bool flag);
    bool (*lot_set_population)(void* user, uint64_t lot, uint32_t dev_type, uint16_t population);
    bool (*lot_property)(void* user, uint64_t lot, uint32_t id, uint32_t* out); /* false = absent */
    void (*lot_set_property)(void* user, uint64_t lot, uint32_t id, uint32_t value);
    /* lot configurations and their manager */
    uint32_t (*cfg_id)(void* user, uint64_t cfg);
    void (*cfg_size)(void* user, uint64_t cfg, uint32_t* out2);
    float (*cfg_max_slope_before_foundation)(void* user, uint64_t cfg);
    uint8_t (*cfg_growth_stage)(void* user, uint64_t cfg);
    bool (*cfg_is_compatible_with_water)(void* user, uint64_t cfg);
    void (*cfg_objects)(void* user, uint64_t cfg, fc4_lotdev_cfg_object_callback cb, void* ctx);
    /* cfg slots 24 / 26: the primary building / foundation object to cb (once); false = none */
    bool (*cfg_primary_building_object)(void* user, uint64_t cfg, fc4_lotdev_cfg_object_callback cb, void* ctx);
    bool (*cfg_primary_foundation_object)(void* user, uint64_t cfg, fc4_lotdev_cfg_object_callback cb, void* ctx);
    bool (*cfg_has_objects_of_type)(void* user, uint64_t cfg, uint32_t object_type);
    uint64_t (*get_lot_configuration)(void* user, uint32_t id);
    void (*object_id_array)(void* user, uint64_t cfg, uint32_t index, fc4_u32s_sink sink, void* ctx);
    /* slots and helpers §17 does not cover */
    uint64_t (*first_building)(void* user, uint64_t lot); /* slot 38, borrowed */
    bool (*update_lot_retaining_walls)(void* user, uint64_t lot); /* slot 62 */
    bool (*update_lot_base_texture_occupant)(void* user, uint64_t lot, bool had_foundation); /* slot 45 */
    uint64_t (*base_texture_occupant)(void* user, uint64_t lot, bool create); /* slot 41; 0 = false */
    void (*base_texture_set)(void* user, uint64_t occ, const fc4_lotdev_texture* records, size_t n);
    void (*add_retaining_wall_occupant)(void* user, uint64_t lot, uint32_t flag); /* 0x005E3070 */
    void (*update_surrounding_retaining_walls)(void* user, uint64_t lot);     /* 0x005E3960 */
    /* AddNetworkConnectionNodes steps 2-3 (§6.5, LD17): network manager slot 20 for each of the n nodes, every
     * result in one list, then slot 21 AutoconnectLotNodes(lot, &list) (also with n = 0) */
    void (*add_network_connection_nodes)(void* user, uint64_t lot, const fc4_lotdev_conn_node* nodes, size_t n);
    /* RemoveNetworkConnectionNodes(lot) (§6.5, LD17): flag-0x200000 network occupants to network manager slot 6 */
    void (*remove_network_connection_nodes)(void* user, uint64_t lot);
    void (*start_construction_occupant)(void* user, uint64_t lot); /* §6.3 hand-off */
    /* buildings and other occupants */
    uint64_t (*occupant_as_building)(void* user, uint64_t occ); /* borrowed; 0 = not a building */
    void (*building_add_ref)(void* user, uint64_t building);
    void (*building_release)(void* user, uint64_t building);
    void (*building_set_lit)(void* user, uint64_t building, bool lit);
    uint32_t (*occupant_type)(void* user, uint64_t occ);               /* occupant slot 7 */
    void (*occupant_position)(void* user, uint64_t occ, float* out3); /* occupant slot 8 */
    void (*occupant_bounding_box)(void* user, uint64_t occ, float* out6); /* min, max */
    void (*occupant_city_cells)(void* user, uint64_t occ, fc4_cell_rect* out);
    bool (*occupant_in_group)(void* user, uint64_t occ, uint32_t group);
    void (*occupant_post_message)(void* user, uint64_t occ, uint32_t message);
    void (*occupant_set_removal_flags)(void* user, uint64_t occ, uint32_t flags);
    void (*occupant_set_lot_tag)(void* user, uint64_t occ, uint32_t tag);
    bool (*occupant_property)(void* user, uint64_t occ, uint32_t id, uint32_t* out);
    void (*occupant_set_property)(void* user, uint64_t occ, uint32_t id, uint32_t value);
    void (*occupant_init)(void* user, uint64_t occ);
    void (*occupant_set_flag)(void* user, uint64_t occ, uint32_t flag);
    bool (*occupant_is_flag_set)(void* user, uint64_t occ, uint32_t flag); /* occupant slot 32 */
    void (*occupant_shutdown)(void* user, uint64_t occ);                   /* occupant slot 4 */
    void (*occupant_set_orientation)(void* user, uint64_t occ, uint32_t orientation);
    void (*occupant_set_position)(void* user, uint64_t occ, const float* pos3);
    void (*occupant_insert)(void* user, uint64_t occ);
    void (*occupant_remove)(void* user, uint64_t occ, uint32_t a, uint32_t flags);
    void (*occupant_changed)(void* user, uint64_t occ);
    void (*occupants_in_bbox)(void* user, const float* min3, const float* max3, fc4_token_callback cb, void* ctx);
    void (*occupants_in_cells)(void* user, const fc4_cell_rect* rect, uint32_t occupant_type, fc4_token_callback cb,
                               void* ctx);
    /* occupant manager slot 37: the occupants of a type whose box meets the closed box, in list order */
    void (*occupants_in_box)(void* user, const float* min3, const float* max3, uint32_t occupant_type,
                             fc4_token_callback cb, void* ctx);
    int32_t (*building_orientation)(void* user, uint64_t building); /* building slot 14 */
    /* foundations (cISC4FoundationOccupant, §17.5) */
    bool (*foundation_kind)(void* user, uint64_t occ, int32_t* out); /* false = not a foundation */
    uint32_t (*foundation_type)(void* user, uint64_t occ);
    /* BDS slot 23 AddNewFoundationOccupant; pos3 in/out */
    bool (*add_new_foundation_occupant)(void* user, uint32_t type, int32_t kind, float* pos3, float dx, float dz,
                                        int32_t orient, float bottom);
    /* prop manager */
    uint64_t (*prop_create)(void* user, uint32_t prop_type);
    void (*lot_props)(void* user, uint64_t lot, fc4_token_callback cb, void* ctx);
    bool (*prop_add_to_lot)(void* user, uint64_t lot, uint64_t occ);
    uint64_t (*prop_add)(void* user, uint32_t prop_type, const float* pos3, int32_t orientation);
    void (*remove_lot_props)(void* user, uint64_t lot);
    bool (*prop_remove)(void* user, uint64_t occ);
    void (*remove_building_props)(void* user, uint64_t building);
    bool (*make_burnt_building)(void* user, uint64_t building, fc4_lotdev_burnt* out);
    void (*add_rubble_decal)(void* user, uint64_t lot, const fc4_lotdev_burnt* burnt, uint32_t instance, float size);
    /* BDS */
    void (*building_profile)(void* user, uint32_t building, fc4_lotdev_profile* out);
    uint32_t (*developer_type)(void* user, uint32_t purpose, uint32_t wealth);
    uint64_t (*developer_of_type)(void* user, uint32_t dev_type);
    uint32_t (*wealth_of_type)(void* user, uint32_t dev_type);
    uint64_t (*developer_for)(void* user, uint32_t purpose, uint32_t wealth);
    bool (*is_building_type)(void* user, uint32_t id);
    void (*building_types_by_family)(void* user, uint32_t family, fc4_u32s_sink sink, void* ctx);
    uint64_t (*add_new_building_occupant)(void* user, uint32_t building, const float* pos3, int32_t orientation);
    bool (*claim_construction_resources)(void* user, uint16_t amount);
    bool (*cheat)(void* user, uint32_t which); /* 0 ignore plumbing, 1 ignore power, 2 coffee bar */
    /* growth developers */
    uint32_t (*dev_purpose)(void* user, uint64_t dev);
    int16_t (*dev_desirability)(void* user, uint64_t dev, int32_t x, int32_t z);
    uint8_t (*dev_water_supply_cap)(void* user, uint64_t dev);
    /* power, plumbing, traffic, demand */
    bool (*is_powered)(void* user, int32_t x, int32_t z);
    bool (*is_cell_watered)(void* user, int32_t x, int32_t z);
    uint8_t (*trip_length)(void* user, int32_t x, int32_t z);
    bool (*jobs_demand)(void* user, float* out); /* false = no node */
    /* flora */
    bool (*flora_type_valid)(void* user, uint32_t id);
    bool (*flora_types_by_family)(void* user, uint32_t id, fc4_u32s_sink sink, void* ctx);
    uint64_t (*flora_add)(void* user, uint32_t flora_type, const float* pos3);
    void (*flora_demolish_in_cell_area)(void* user, const fc4_cell_rect* rect);
    /* demolition */
    bool (*demolish_occupant)(void* user, uint64_t occ);
    bool (*demolish_lot)(void* user, uint64_t lot);
    /* demolition slot 9 ModifyTerrainHeight(1, per_vertex, x0, z0, x1, z1, h, exempt, 5, NULL): the count */
    int32_t (*modify_terrain_height)(void* user, bool per_vertex, int32_t x0, int32_t z0, int32_t x1, int32_t z1,
                                     float h, uint64_t exempt);
    /* business units */
    void (*unit_members)(void* user, uint64_t unit, fc4_token_callback cb, void* ctx);
    uint64_t (*unit_create)(void* user);
    bool (*unit_add)(void* user, uint64_t unit, uint64_t lot);
    bool (*unit_remove)(void* user, uint64_t unit, uint64_t lot);
    /* UpdateLotObjectAlitudes (§6.5, LD17; ABI 60) */
    void (*lot_occupants)(void* user, uint64_t lot, fc4_token_callback cb, void* ctx); /* lot slot 49, list order */
    /* lot slot 11 GetLotConfigurationObject(occ) to cb (once); false = none */
    bool (*lot_configuration_object)(void* user, uint64_t lot, uint64_t occ, fc4_lotdev_cfg_object_callback cb,
                                     void* ctx);
    void (*occupant_moved)(void* user, uint64_t occ); /* occupant manager slot 24 (occ, 1) */
} fc4_lotdev_host;

/* The class statics (§10.1): one per process; the generator is seeded with
 * derive_seed(session_seed, 0x6C09C0F4), rand() starts at state 1. */
fc4_lotdev_statics* fc4_lotdev_statics_create(uint32_t session_seed);
void fc4_lotdev_statics_destroy(fc4_lotdev_statics* s);
/* §1.1: 1 the primary interface (and cIGZUnknown), 2 the message target
 * (+4), 3 the serializable (+8), 0 none. */
uint32_t fc4_lotdev_query_interface(uint32_t iid);
/* NULL when the table is short or `s` is NULL. */
fc4_lotdev* fc4_lotdev_create(const fc4_lotdev_host* host, const fc4_lotdev_statics* s);
void fc4_lotdev_destroy(fc4_lotdev* p);
bool fc4_lotdev_init(fc4_lotdev* p);
bool fc4_lotdev_shutdown(fc4_lotdev* p);
bool fc4_lotdev_do_message(fc4_lotdev* p, const fc4_lotdev_message* m);
void fc4_lotdev_set_daily_work_budget(fc4_lotdev* p, uint32_t n); /* 0 = unlimited */
/* slots 6-14 */
bool fc4_lotdev_start_construction(fc4_lotdev* p, uint64_t lot, uint64_t cfg, uint32_t building, bool animate);
bool fc4_lotdev_construct_default_lot(fc4_lotdev* p, uint64_t lot, uint64_t cfg, uint32_t building, bool animate);
bool fc4_lotdev_abandon_building(fc4_lotdev* p, uint64_t lot, uint32_t reason);
bool fc4_lotdev_reoccupy_building(fc4_lotdev* p, uint64_t lot, uint32_t dev_type);
bool fc4_lotdev_change_building(fc4_lotdev* p, uint64_t lot, uint32_t building);
bool fc4_lotdev_remove_building_zot_prop(fc4_lotdev* p, uint64_t lot);
bool fc4_lotdev_start_demolish_lot(fc4_lotdev* p, uint64_t lot);
bool fc4_lotdev_end_demolish_lot(fc4_lotdev* p, uint64_t lot);
bool fc4_lotdev_explode_lot(fc4_lotdev* p, uint64_t lot, bool burnt);
/* slots 15-22, 25-32: the terrain snapshot (never refused) */
float fc4_lotdev_terrain_altitude_by_vertex(const fc4_lotdev* p, int32_t x, int32_t z, bool with_water);
/* slot 16: SC4Rect<float> in metres, rect4 = {x1, z1, x2, z2} */
float fc4_lotdev_area_altitude_f(const fc4_lotdev* p, const float* rect4, bool lowest, bool with_water);
/* slot 17: SC4Rect<int32> */
float fc4_lotdev_area_altitude(const fc4_lotdev* p, const fc4_cell_rect* rect, bool lowest, bool with_water);
/* slot 18: SC4Rect<int32> (the SDK's float rectangle is wrong) */
float fc4_lotdev_average_area_altitude(const fc4_lotdev* p, const fc4_cell_rect* rect, bool with_water);
/* slots 19-21 return EAX counts, not bools (§17.4) */
int32_t fc4_lotdev_level_area_internal_vertices(fc4_lotdev* p, const fc4_cell_rect* rect, float target,
                                                bool skip_water, bool force, uint64_t lot);
int32_t fc4_lotdev_level_bordering_vertices(fc4_lotdev* p, const fc4_cell_rect* rect, float h, uint64_t lot);
int32_t fc4_lotdev_attempt_area_levelling(fc4_lotdev* p, const fc4_cell_rect* rect, bool with_water,
                                          bool per_vertex, float max_area_slope, uint64_t lot);
void fc4_lotdev_occupant_cell_coverage(const fc4_lotdev* p, uint64_t occ, fc4_cell_rect* out); /* no result (LD18, LD27) */
bool fc4_lotdev_is_area_flat(const fc4_lotdev* p, const fc4_cell_rect* rect, bool with_water);
bool fc4_lotdev_is_lot_flat(const fc4_lotdev* p, uint64_t lot, bool with_water);
float fc4_lotdev_area_altitude_range(const fc4_lotdev* p, const fc4_cell_rect* rect, bool with_water);
float fc4_lotdev_lot_altitude_range(const fc4_lotdev* p, uint64_t lot, bool with_water);
float fc4_lotdev_area_slope(const fc4_lotdev* p, const fc4_cell_rect* rect, bool with_water);
float fc4_lotdev_lot_slope(const fc4_lotdev* p, uint64_t lot, bool with_water);
float fc4_lotdev_occupant_slope(const fc4_lotdev* p, uint64_t occ, bool with_water);
int64_t fc4_lotdev_lot_development_cost(const fc4_lotdev* p, const fc4_cell_rect* rect);
/* slots 33-37, 39-61 */
bool fc4_lotdev_remove_lot_from_its_business_unit(fc4_lotdev* p, uint64_t lot);
void fc4_lotdev_update_travel_designates(fc4_lotdev* p, uint64_t* unit); /* in/out */
void fc4_lotdev_update_travel_populations(fc4_lotdev* p, uint64_t unit);
bool fc4_lotdev_needs_anchor_building(fc4_lotdev* p, const fc4_cell_rect* rect, int32_t purpose);
bool fc4_lotdev_update_lot_elements(fc4_lotdev* p, uint64_t lot, uint32_t kind);
void fc4_lotdev_set_normal_lot_base_texture(fc4_lotdev* p, uint64_t lot, uint64_t cfg, bool flag);
bool fc4_lotdev_get_force_empty(const fc4_lotdev* p);
void fc4_lotdev_set_force_empty(fc4_lotdev* p, bool force);
/* slot 39: position into pos3, the orientation returned (the slot's EAX is &orient) */
int32_t fc4_lotdev_object_position(const fc4_lotdev* p, uint64_t lot, uint64_t cfg, const fc4_lotdev_cfg_object* obj,
                                   float* pos3);
/* slot 40: false = no configuration or primary building object (nothing written) */
bool fc4_lotdev_position_for_building(const fc4_lotdev* p, uint64_t lot, float* pos3, int32_t* orient);
/* slot 42: false = empty boundary map (nothing written) */
bool fc4_lotdev_boundary_lot_texture(const fc4_lotdev* p, const int32_t* sides4, uint32_t* texture,
                                     uint8_t* rotation, bool* flip);
void fc4_lotdev_set_empty_lot_base_texture(fc4_lotdev* p, uint64_t lot); /* slot 43 */
/* slots 48-61 (§17.5); lookups return borrowed foundation occupants, 0 = none */
uint64_t fc4_lotdev_foundation_by_occupant(const fc4_lotdev* p, uint64_t occ);
uint64_t fc4_lotdev_foundation_by_cell(const fc4_lotdev* p, int32_t kind, int32_t x, int32_t z);
uint64_t fc4_lotdev_foundation_by_position(const fc4_lotdev* p, int32_t kind, float x, float z);
uint64_t fc4_lotdev_lot_foundation(const fc4_lotdev* p, uint64_t lot);
bool fc4_lotdev_remove_foundation(fc4_lotdev* p, int32_t kind, float x, float z);
bool fc4_lotdev_remove_lot_foundation(fc4_lotdev* p, int32_t x, int32_t z);
bool fc4_lotdev_add_lot_foundation(fc4_lotdev* p, uint64_t lot, uint64_t cfg, bool use_lot_altitude, bool level,
                                   bool walls);
bool fc4_lotdev_add_lot_foundation_type(fc4_lotdev* p, uint64_t lot, uint32_t type, bool use_lot_altitude,
                                        bool level, bool walls);
bool fc4_lotdev_is_occupant_foundation_required(const fc4_lotdev* p, uint64_t occ);
bool fc4_lotdev_is_box_foundation_required(const fc4_lotdev* p, const float* min3, const float* max3);
bool fc4_lotdev_add_building_foundation(fc4_lotdev* p, uint64_t building, uint64_t cfg);
bool fc4_lotdev_add_building_foundation_type(fc4_lotdev* p, uint64_t building, uint32_t type);
bool fc4_lotdev_remove_building_foundation(fc4_lotdev* p, uint64_t occ);
bool fc4_lotdev_update_lot_foundation(fc4_lotdev* p, uint64_t lot);
/* UpdateLot (§4.2), for tests and tools */
bool fc4_lotdev_update_lot(fc4_lotdev* p, uint64_t lot);
/* §13 */
bool fc4_lotdev_write(const fc4_lotdev* p, const fc4_ostream* out);
bool fc4_lotdev_read(fc4_lotdev* p, const fc4_istream* in);
bool fc4_lotdev_get_state(const fc4_lotdev* p, fc4_lotdev_state* out);
bool fc4_lotdev_get_tuning(const fc4_lotdev* p, fc4_lotdev_tuning* out);

/* ---- history warehouse (docs/specs/history.md; engine/capi/src/history.rs) ----
 * cSC4HistoryWarehouse's core: the series, the yearly thinning pass, the
 * queries and the record. The shim owns the GZCOM object (QueryInterface,
 * reference count, the +4 message target, the agents) and turns cIGZDate
 * arguments into day numbers (GetDayNumber). Dates are Julian day numbers. */
typedef struct fc4_history fc4_history;

typedef struct fc4_history_host {
    uint32_t struct_size; /* sizeof(fc4_history_host) */
    void* user;
    /* cISC4Simulator slot 8: the simulation date number. */
    uint32_t (*sim_date)(void* user);
    /* A millisecond timer for the yearly pass (§4.3); only differences are used. */
    uint32_t (*now_ms)(void* user);
    /* Subscribes / unsubscribes the +4 message target to one message type. */
    void (*subscribe)(void* user, uint32_t msg_type);
    void (*unsubscribe)(void* user, uint32_t msg_type);
    /* AddAgent(+4, level, name, flags) / RemoveAgent(+4) (both levels) on the simulator. */
    void (*add_agent)(void* user, uint32_t level, const char* name, size_t name_len, uint32_t flags);
    void (*remove_agent)(void* user);
    /* MessageSend (synchronous) of a standard message: type, data1, data2. The
     * shim may deliver it back to fc4_history_do_message from inside the call. */
    void (*send)(void* user, uint32_t msg_type, uint32_t data1, uint32_t data2);
} fc4_history_host;

typedef struct fc4_history_point {
    int32_t value;
    uint32_t date;
} fc4_history_point;

typedef struct fc4_history_state {
    uint32_t struct_size;
    uint8_t initialized, has_cursor, pad[2];
    uint32_t pass_state, cursor, pass_counter, series_count, total_points;
} fc4_history_state;

/* The host table is copied. NULL if the table is missing or too small. */
fc4_history* fc4_history_create(const fc4_history_host* host);
void fc4_history_destroy(fc4_history* h);
/* QueryInterface (§1): true for 1, 0x69F4A803 and 0xE4FDA3D4 only (not
 * cIGZMessageTarget2, [quirk]). */
bool fc4_history_answers_iid(uint32_t iid);
/* Init (slot 3) and Shutdown (slot 4), §3. */
bool fc4_history_init(fc4_history* h);
bool fc4_history_shutdown(fc4_history* h);
/* DoMessage (§3): 0x89EFA536 record (data1 = key, data2 = value),
 * 0x89EFA537 queue length, SimNewDay (the pass step; false = call again this
 * interval), SimNewYear. True otherwise (and for a re-entrant call). */
bool fc4_history_do_message(fc4_history* h, uint32_t msg_type, uint32_t data1, uint32_t data2);
/* RecordHistoryData (slot 12, §4.1): sends the record message through the
 * host's send without touching the warehouse. Returns false (HW12). */
bool fc4_history_record(const fc4_history* h, uint32_t key, int32_t value);
/* SetHistoryDataQueueLength (slot 13, §4.2). */
bool fc4_history_set_queue_length(fc4_history* h, uint32_t key, uint32_t n);
/* Coalesce (slot 9, §4.4; granularity 0 month, 1 half-year, 2 year, 3 five
 * years), Dispose (slot 10) and Dispose(key, from, to) (slot 11), §4.5. */
bool fc4_history_coalesce(fc4_history* h, uint32_t key, uint32_t from, uint32_t to, int32_t granularity);
bool fc4_history_dispose(fc4_history* h, uint32_t key);
bool fc4_history_dispose_range(fc4_history* h, uint32_t key, uint32_t from, uint32_t to);
/* Queries (slots 5-8, §4.6-§4.8). precision_bits = the caller's x87
 * precision: 24 (the simulators) or 53 (Lua; any value other than 24).
 * Outputs (may be NULL) are untouched when the result is false. */
bool fc4_history_get_count(const fc4_history* h, uint32_t key, uint32_t from, uint32_t to, float* out);
bool fc4_history_get_trend(const fc4_history* h, uint32_t key, uint32_t from, uint32_t to, uint32_t precision_bits,
                           float* slope, float* intercept);
bool fc4_history_get_point(const fc4_history* h, uint32_t key, uint32_t date, uint32_t precision_bits, float* out);
bool fc4_history_get_min_max_average(const fc4_history* h, uint32_t key, uint32_t from, uint32_t to,
                                     uint32_t precision_bits, float* min, float* max, float* avg);
/* Diagnostics. fc4_history_keys copies at most `capacity` keys in ascending
 * order (out may be NULL) and returns the key count. fc4_history_get_series
 * copies at most `capacity` points (out may be NULL), sets *count to the
 * point count; false for a missing key. */
bool fc4_history_get_state(const fc4_history* h, fc4_history_state* out);
size_t fc4_history_keys(const fc4_history* h, uint32_t* out, size_t capacity);
bool fc4_history_get_series(const fc4_history* h, uint32_t key, uint32_t* max_count, fc4_history_point* out,
                            size_t capacity, size_t* count);
/* Persistence (§7): Write writes version 2; Read takes versions 0-2 and adds
 * to the series already there. */
bool fc4_history_write(const fc4_history* h, const fc4_ostream* out);
bool fc4_history_read(fc4_history* h, const fc4_istream* in);

/* ---- 24-hour clock (docs/specs/clock24.md; engine/capi/src/clock24.rs) ------
 * cSC424HourClock's core: the time of day, the visual time, the pause
 * counters, the cheats and the record. The shim owns the GZCOM object
 * (QueryInterface, reference count, the system service, the message target)
 * and reads the "Main Simulator" exemplar. Messages are posted (queued), so
 * no callback re-enters the clock; call it from the main thread only. */
typedef struct fc4_clock24 fc4_clock24;

typedef struct fc4_clock24_host {
    uint32_t struct_size; /* sizeof(fc4_clock24_host) */
    void* user;
    /* A monotonic clock in microseconds (the timer's unit 3, §4.3). */
    uint64_t (*now_micros)(void* user);
    /* MessagePost of a standard message (§3.3): void1 and the IGZUnknown
     * field = the clock, data2, data3. */
    void (*post)(void* user, uint32_t msg_type, uint32_t data2, uint32_t data3);
    /* Init / Shutdown (§4.2): the framework's system services and on-idle list. */
    void (*add_system_service)(void* user);
    void (*remove_system_service)(void* user);
    void (*add_to_on_idle)(void* user);
    void (*remove_from_on_idle)(void* user);
    /* The cheat code manager (§3.2): whether it exists, RegisterCheatCode,
     * the message-target notification, UnregisterCheatCode. */
    bool (*has_cheat_manager)(void* user);
    void (*register_cheat)(void* user, uint32_t id, const char* name, size_t name_len);
    void (*add_cheat_notification)(void* user, uint32_t id);
    void (*remove_cheat_notification)(void* user, uint32_t id);
    void (*unregister_cheat)(void* user, uint32_t id);
} fc4_clock24_host;

/* The "Main Simulator" exemplar's clock properties (§6): 0x897E709F
 * magnification, 0x09B73421 night start, 0x09B73422 night end. has_* = 0:
 * the property is missing. */
typedef struct fc4_clock24_tuning {
    uint32_t struct_size; /* sizeof(fc4_clock24_tuning) */
    uint8_t has_magnification, has_night_start, has_night_end, pad;
    float magnification;
    uint32_t night_start, night_end;
} fc4_clock24_tuning;

typedef struct fc4_clock24_state {
    uint32_t struct_size;
    uint8_t initialized, day_night_notifications, timer_running, pad;
    int32_t pause_count, hidden_pause_count;
    float magnification, min_magnification, max_magnification;
    uint32_t seconds, visual_seconds, night_start, night_end;
    int32_t visual_mode;
    uint64_t timer_elapsed_us;
} fc4_clock24_state;

/* The host table is copied. NULL if the table is missing or too small. */
fc4_clock24* fc4_clock24_create(const fc4_clock24_host* host);
void fc4_clock24_destroy(fc4_clock24* c);
/* QueryInterface (§1): 1, 0x681BD53A, 0x452294AA, 0xE4FDA3D4 and the
 * system service's 0x287FB697 [inferred]. */
bool fc4_clock24_answers_iid(uint32_t iid);
/* Init (slot 3, §4.2): tuning NULL = no exemplar (nothing changes); false for
 * a too-small tuning struct. Shutdown (slot 4). */
bool fc4_clock24_init(fc4_clock24* c, const fc4_clock24_tuning* tuning);
bool fc4_clock24_shutdown(fc4_clock24* c);
/* OnIdle (system-service slot 11, §4.3); the argument is not used. True. */
bool fc4_clock24_on_idle(fc4_clock24* c, uint32_t idle_count);
/* DoMessage (§4.8): the cheat message 0x230E27AC with data1 = the cheat ID and
 * the typed text (text may be NULL). True. */
bool fc4_clock24_do_message(fc4_clock24* c, uint32_t msg_type, uint32_t data1, const char* text, size_t text_len);
/* cISC424HourClock slots 5-29 (§8; the game's forms, not the SDK's). */
void fc4_clock24_set_visual_time_mode(fc4_clock24* c, int32_t mode);                   /* 5 */
int32_t fc4_clock24_get_visual_time_mode(const fc4_clock24* c);                        /* 6 */
void fc4_clock24_get_time_percentage(const fc4_clock24* c, float* out);                /* 7 */
void fc4_clock24_set_time_percentage(fc4_clock24* c, float p);                         /* 8 */
void fc4_clock24_get_visual_time_percentage(const fc4_clock24* c, float* out);         /* 9 */
uint32_t fc4_clock24_get_hour(const fc4_clock24* c);                                   /* 10 */
void fc4_clock24_set_hour(fc4_clock24* c, uint32_t hour);                              /* 11 */
uint32_t fc4_clock24_get_visual_hour(const fc4_clock24* c);                            /* 12 */
uint32_t fc4_clock24_get_minute(const fc4_clock24* c);                                 /* 13 */
uint32_t fc4_clock24_set_minute(fc4_clock24* c, uint32_t minute);                      /* 14: the new seconds */
uint32_t fc4_clock24_get_visual_minute(const fc4_clock24* c);                          /* 15 */
uint32_t fc4_clock24_get_seconds(const fc4_clock24* c);                                /* 16 */
uint32_t fc4_clock24_set_seconds(fc4_clock24* c, uint32_t seconds);                    /* 17: GetMinute */
uint32_t fc4_clock24_get_visual_seconds(const fc4_clock24* c);                         /* 18 */
uint32_t fc4_clock24_get_event_hour(const fc4_clock24* c, uint32_t index);             /* 19 */
uint32_t fc4_clock24_get_time_magnification(const fc4_clock24* c, float* out);         /* 20: the bits */
bool fc4_clock24_set_time_magnification(fc4_clock24* c, float m);                      /* 21 */
uint32_t fc4_clock24_get_time_magnification_min_max(const fc4_clock24* c, float* min,
                                                    float* max);                       /* 22: max's bits */
bool fc4_clock24_set_time_magnification_min_max(fc4_clock24* c, float min, float max); /* 23 */
bool fc4_clock24_pause(fc4_clock24* c, bool on);                                       /* 24 */
bool fc4_clock24_is_paused(const fc4_clock24* c);                                      /* 25 */
bool fc4_clock24_hidden_pause(fc4_clock24* c, bool on);                                /* 26 */
bool fc4_clock24_is_hidden_paused(const fc4_clock24* c);                               /* 27 */
bool fc4_clock24_is_night(const fc4_clock24* c);                                       /* 28 */
void fc4_clock24_enable_day_night_notifications(fc4_clock24* c, bool on);              /* 29 */
bool fc4_clock24_get_state(const fc4_clock24* c, fc4_clock24_state* out);
/* Persistence (§7): Write writes version 2 (26 bytes); Read has no version
 * check and skips version 1's timer record. */
bool fc4_clock24_write(const fc4_clock24* c, const fc4_ostream* out);
bool fc4_clock24_read(fc4_clock24* c, const fc4_istream* in);
/* ---- tract developer (docs/specs/tract-developer.md §1-§4, §6, §7) -------------
 *
 * The tract developer (cSC4TractDeveloper, class 0x2990C142, city component 17):
 * Grow (§4.3), Decline (§4.19), UpdatePopulation (§4.18), the style rotation
 * (§4.20, the only saved state, §6) and the tuning (§4.21). It owns no grid. The
 * BDS, the growth developers, the lot developer, the lot manager and the lot
 * configuration manager are reached through the host, whoever implements them.
 *
 * Tokens: growth developers, lots, lot configurations, configuration filters and
 * network tiles; 0 = none. Every token the host hands out is BORROWED and must
 * stay valid until the top-level call that received it returns (the core never
 * AddRefs or releases): a host keeps the references its lists gave it until
 * then. The core orders lots and configurations by token (the original orders
 * them by address, §9.5 item 1): hand out the object addresses.
 *
 * Statics (§9.1): the first fc4_tract_create of the process seeds the shared
 * file-local generator with derive_seed(session_seed, 0x4C09C113) (foundations
 * §6.3, §4.22 [choice]); later objects share it. The C rand() of R1 / R8 is the
 * core's own (state 1, foundations §12.6).
 *
 * Threads: single-threaded; the plugin serialises the calls on one object. A
 * call while the object is already inside the core (a host callback coming
 * back) gets the fallback and is counted (fc4_tract_state.reentrant).
 *
 * Shadow mode (fc4_tract_create_shadow): reads go to the game; writes are
 * swallowed and counted (services, agents, the message server, construction,
 * abandonment, reoccupation, lots created / deleted / turned / state changes,
 * demolition, terrain, construction resources, filters); SetPopulation calls
 * are recorded for fc4_tract_shadow_update_population. Own statics. */

typedef struct fc4_tract fc4_tract;

/* The "Building Development" tuning exemplar (§4.21). */
#define FC4_TRACT_TUNING_TYPE 0x6534284Au
#define FC4_TRACT_TUNING_GROUP 0xE7E2C2DBu
#define FC4_TRACT_TUNING_INSTANCE 0xE8DA7677u

/* dev_cap's `which` (growth developer slots 19-22) and cheat's (BDS slots 54-57). */
#define FC4_TRACT_CAP_POWER 0u
#define FC4_TRACT_CAP_WATER 1u
#define FC4_TRACT_CAP_FIRE 2u
#define FC4_TRACT_CAP_PARK 3u
#define FC4_TRACT_CHEAT_IGNORE_PLUMBING 0u
#define FC4_TRACT_CHEAT_IGNORE_POWER 1u
#define FC4_TRACT_CHEAT_IGNORE_CAPS 2u
#define FC4_TRACT_CHEAT_DEBUG_GROWTH 3u
/* The most bits cfg_required_roads may report (the core reads four). */
#define FC4_TRACT_MAX_ROAD_BITS 32u

/* GrowthProfile as the slots read it (§7.1); only the low byte of stage is used (Grow). */
typedef struct fc4_tract_profile {
    uint64_t developer;
    int32_t x, z;
    int32_t stage;
    int32_t reserved;
} fc4_tract_profile;

/* GetConstructionInfo's fields the tract developer reads (growth-developer §6.2). */
typedef struct fc4_tract_construction {
    int32_t zone;
    int32_t unknown2;
    uint32_t wealth;
    uint32_t purpose;
    uint8_t land_value_min, land_value_max;
    uint8_t reserved[2];
    uint32_t building_type;
    int32_t x, z;
} fc4_tract_construction;

/* BuildingProfile purpose (+0x00), wealth (+0x04), capacity (+0x08) (docs/abi
 * layouts.md); an unknown type reads as zeros. */
typedef struct fc4_tract_building_profile {
    uint32_t purpose;
    uint32_t wealth;
    uint16_t capacity;
    uint16_t reserved;
} fc4_tract_building_profile;

typedef void (*fc4_tract_style_callback)(void* ctx, uint32_t group, float fraction);
typedef void (*fc4_tract_population_callback)(void* ctx, uint64_t lot, uint32_t dev_type, uint16_t population);

/* Missing callbacks answer neutrally (0, false, empty, none). */
typedef struct fc4_tract_host {
    uint32_t struct_size;
    void* user;
    /* lifecycle (§1.3, §1.4): false = a service (or the simulator, the message server) is missing */
    bool (*acquire_services)(void* user);
    bool (*add_agent)(void* user, uint32_t level, uint32_t message);
    bool (*acquire_message_server)(void* user);
    void (*release_message_server)(void* user);
    void (*remove_agent)(void* user);
    void (*release_services)(void* user);
    /* A NEW fc4_props with the tuning exemplar's properties (§4.21), or NULL. */
    fc4_props* (*tuning)(void* user);
    /* growth developer (growth-developer.md): slots 5, 6, 7, 8, 9; the desirability
     * map's (slot 13) tract size, shift and tract value; the capacity map's (slot 14)
     * tract value; GetSourceDemand (18) then cISC4Demand slot 12 (false: no node);
     * the caps (19-22); GetMonopolyUse (25); GetStylePercentages (26) in key order. */
    int32_t (*dev_target_zone)(void* user, uint64_t dev);
    uint32_t (*dev_subtype)(void* user, uint64_t dev);
    uint32_t (*dev_purpose)(void* user, uint64_t dev);
    uint32_t (*dev_wealth)(void* user, uint64_t dev);
    void (*dev_construction_info)(void* user, uint64_t dev, fc4_tract_construction* out);
    int32_t (*dev_desirability_tract_size)(void* user, uint64_t dev);
    uint32_t (*dev_desirability_shift)(void* user, uint64_t dev);
    int16_t (*dev_desirability_value)(void* user, uint64_t dev, int32_t tx, int32_t tz);
    uint32_t (*dev_capacity_value)(void* user, uint64_t dev, int32_t tx, int32_t tz);
    bool (*dev_source_demand)(void* user, uint64_t dev, float* out);
    uint8_t (*dev_cap)(void* user, uint64_t dev, uint32_t which);
    int32_t (*dev_monopoly_use)(void* user, uint64_t dev);
    void (*dev_style_percentages)(void* user, uint64_t dev, fc4_tract_style_callback cb, void* ctx);
    /* BDS (building-development.md): slot 39; slot 19 (the types in an order whose
     * insertion into a 193-bucket STLport hash_set reproduces the set); slot 14 +
     * IsOccupantGroup; the bool property 0x2781283F; the uint32 array property
     * 0x29B55F73 (false: absent); slot 33 (no reference); slots 50, 51; 54-57. */
    void (*building_profile)(void* user, uint32_t building, fc4_tract_building_profile* out);
    void (*building_types_by_family)(void* user, uint32_t family, fc4_u32_callback cb, void* ctx);
    bool (*building_in_occupant_group)(void* user, uint32_t building, uint32_t group);
    bool (*building_is_monopoly)(void* user, uint32_t building);
    bool (*building_field_ids)(void* user, uint32_t building, fc4_u32_callback cb, void* ctx);
    uint64_t (*developer_for)(void* user, uint32_t purpose, uint32_t wealth);
    bool (*claim_construction_resources)(void* user, uint16_t amount);
    void (*release_construction_resources)(void* user, uint16_t amount);
    bool (*cheat)(void* user, uint32_t which);
    /* lot developer (lot-developer.md): slots 36, 6, 8, 9, 29 */
    bool (*needs_anchor_building)(void* user, const fc4_cell_rect* rect, uint32_t purpose);
    bool (*start_construction)(void* user, uint64_t lot, uint64_t cfg, uint32_t building, bool flag);
    bool (*abandon_building)(void* user, uint64_t lot, uint32_t reason);
    bool (*reoccupy_building)(void* user, uint64_t lot, uint32_t dev_type);
    float (*area_slope)(void* user, const fc4_cell_rect* rect);
    /* lot manager (zones.md): slots 6, 7, 9, 10, 20, 22, 24 */
    uint64_t (*get_lot)(void* user, int32_t x, int32_t z);
    void (*lots_in_cell_rect)(void* user, const fc4_cell_rect* rect, fc4_token_callback cb, void* ctx);
    void (*lots_surrounding_lot)(void* user, uint64_t lot, uint32_t flags, fc4_token_callback cb, void* ctx);
    void (*lots_surrounding_rect)(void* user, const fc4_cell_rect* rect, uint32_t flags, fc4_token_callback cb,
                                  void* ctx);
    int32_t (*can_create_lot)(void* user, int32_t x, int32_t z, int32_t w, int32_t h);
    uint64_t (*create_lot_if_possible)(void* user, int32_t x, int32_t z, int32_t w, int32_t h, int32_t facing);
    bool (*delete_lot)(void* user, uint64_t lot);
    /* lots (cISC4Lot) */
    void (*lot_bounding_rect)(void* user, uint64_t lot, fc4_cell_rect* out);
    float (*lot_slope)(void* user, uint64_t lot);
    void (*lot_location)(void* user, uint64_t lot, int32_t* x, int32_t* z);
    void (*lot_size)(void* user, uint64_t lot, uint32_t* w, uint32_t* h);
    int32_t (*lot_facing)(void* user, uint64_t lot);
    void (*lot_set_facing)(void* user, uint64_t lot, int32_t facing);
    bool (*lot_has_road_access)(void* user, uint64_t lot);
    bool (*lot_has_organic_road_access)(void* user, uint64_t lot, uint32_t purpose);
    int32_t (*lot_zone_type)(void* user, uint64_t lot);
    uint32_t (*lot_state)(void* user, uint64_t lot);
    void (*lot_set_state)(void* user, uint64_t lot, uint32_t state, uint32_t wealth, uint32_t reason);
    bool (*lot_is_livable)(void* user, uint64_t lot);
    bool (*lot_has_capacity)(void* user, uint64_t lot);
    uint32_t (*lot_days_since_state_change)(void* user, uint64_t lot);
    uint32_t (*lot_occupant_wealth)(void* user, uint64_t lot);
    bool (*lot_is_historical)(void* user, uint64_t lot);
    uint32_t (*lot_building_type)(void* user, uint64_t lot);
    uint16_t (*lot_capacity)(void* user, uint64_t lot, uint32_t dev_type, bool flag);
    uint16_t (*lot_available_capacity)(void* user, uint64_t lot, uint32_t dev_type, bool flag);
    void (*lot_set_population)(void* user, uint64_t lot, uint32_t dev_type, uint16_t population);
    /* The growth stage of the lot's configuration; -1 without one. */
    int32_t (*lot_configuration_stage)(void* user, uint64_t lot);
    /* lot configurations (cISC4LotConfiguration slots 7, 9, 8, 11, 12, 13, 15-18, 21, 24) */
    void (*cfg_size)(void* user, uint64_t cfg, uint32_t* w, uint32_t* h);
    float (*cfg_min_slope)(void* user, uint64_t cfg);
    float (*cfg_max_slope)(void* user, uint64_t cfg);
    uint8_t (*cfg_growth_stage)(void* user, uint64_t cfg);
    uint16_t (*cfg_min_capacity)(void* user, uint64_t cfg);
    uint16_t (*cfg_max_capacity)(void* user, uint64_t cfg);
    bool (*cfg_compatible_zone)(void* user, uint64_t cfg, int32_t zone);
    bool (*cfg_compatible_wealth)(void* user, uint64_t cfg, uint32_t wealth);
    bool (*cfg_compatible_purpose)(void* user, uint64_t cfg, uint32_t purpose);
    bool (*cfg_compatible_style)(void* user, uint64_t cfg, uint32_t style);
    /* Writes up to `cap` bytes (0 / 1) and returns the bit count. */
    size_t (*cfg_required_roads)(void* user, uint64_t cfg, uint8_t* bits, size_t cap);
    /* false: no primary building object; else its object-ID list. */
    bool (*cfg_primary_building_ids)(void* user, uint64_t cfg, fc4_u32_callback cb, void* ctx);
    /* lot configuration manager: slot 8; a filter {wealth, purpose} allocated and
     * registered (slot 18); unregistered (slot 19) and freed; slot 9 */
    uint64_t (*get_lot_configuration)(void* user, uint32_t id);
    uint64_t (*register_configuration_filter)(void* user, uint32_t wealth, uint32_t purpose);
    void (*unregister_configuration_filter)(void* user, uint64_t filter);
    void (*configurations_by_filter)(void* user, uint64_t filter, fc4_token_callback cb, void* ctx);
    /* city grids and services: zone manager slot 12; land value slot 6; the powered
     * grid's cell value; plumbing slot 11; the fire grid (uint8); the park map
     * (int16); the trip length map (§4.8 row 7); city slots 93 / 94; FindNetworkTile
     * (x, z, 0xC4B, 1); network occupant IsOfType; DemolishLots(true, [lot], 0, 0x800,
     * false, 0, 0, 0); RedisplayTerrain(0, 0, rect, 0). */
    int32_t (*zone_at)(void* user, int32_t x, int32_t z);
    uint8_t (*land_value)(void* user, int32_t x, int32_t z);
    int32_t (*powered_value)(void* user, int32_t x, int32_t z);
    bool (*is_cell_watered)(void* user, int32_t x, int32_t z);
    uint8_t (*fire_coverage)(void* user, int32_t x, int32_t z);
    int16_t (*park_value)(void* user, int32_t x, int32_t z);
    int32_t (*road_access_value)(void* user, int32_t x, int32_t z);
    void (*city_cell_counts)(void* user, int32_t* x, int32_t* z);
    uint64_t (*find_network_tile)(void* user, int32_t x, int32_t z);
    bool (*network_is_of_type)(void* user, uint64_t tile, uint32_t network_type);
    bool (*demolish_lot)(void* user, uint64_t lot);
    void (*redisplay_terrain)(void* user, const fc4_cell_rect* rect);
} fc4_tract_host;

typedef struct fc4_tract_grow_result {
    uint32_t struct_size;
    int32_t built;            /* the return value (may be negative) */
    uint8_t out_flag;
    uint8_t reserved[3];
    uint32_t outcome_count;   /* codes appended to the outcome list */
    uint32_t outcomes[16];    /* the first 16 of them */
    uint32_t scored;          /* candidates scored (§4.14) */
    int32_t desired, desired_max;
    uint32_t reserved1;
    uint64_t chosen_cfg;      /* PickBuilding's last choice (0: none) */
    uint32_t chosen_building;
    uint32_t reserved2;
} fc4_tract_grow_result;

/* The style state, flags and counters (counters from creation). */
typedef struct fc4_tract_state {
    uint32_t struct_size;
    uint8_t initialized, configured, all_styles, reserved;
    uint32_t current_style;
    int32_t current_style_index;
    int32_t years_since_style_change;
    int32_t years_between_style_changes;
    uint32_t active_style_count;
    uint32_t active_styles[16];
    uint32_t filters;
    uint32_t grow_calls, decline_calls, update_population_calls;
    uint32_t reserved1;
    int64_t built_total, declined_total;
    uint32_t outcomes[28];    /* outcome codes appended, by code (§4.1) */
    uint32_t scored;
    uint32_t constructions, reoccupations, abandonments, lots_created, lots_deleted, demolitions, populations_set,
        resource_claims;
    uint32_t reentrant;       /* calls refused: the object was inside the core */
    uint32_t swallowed;       /* shadow: game writes swallowed */
    uint32_t reserved2;
} fc4_tract_state;

typedef struct fc4_tract_tuning {
    uint32_t struct_size;
    float overbuild;
    float reoccupy[2];
    float redevelop_size;
    float demolition_costs[2];
    float new_lot_costs[2];
    float occupancy_variation;
    float min_vacancy[2];
    uint8_t kick_out;
    uint8_t reserved[3];
    uint32_t available_style_count;
    uint32_t available_styles[16];
} fc4_tract_tuning;

/* QueryInterface (§1.1): 1, 0xA818ED1B, 0x452294AA, 0xE4FDA3D4. */
bool fc4_tract_query_interface(uint32_t iid);
uint32_t fc4_tract_class_id(void); /* 0x2990C142 */
/* The configuration filter's IsLotConfigurationIncluded (§4.6, §7.4) through
 * `host`'s configuration reads; needs no object. */
bool fc4_tract_filter_includes(const fc4_tract_host* host, uint64_t cfg, uint32_t wealth, uint32_t purpose);

/* The host table is copied; NULL when it is incomplete. */
fc4_tract* fc4_tract_create(const fc4_tract_host* host, uint32_t session_seed);
fc4_tract* fc4_tract_create_shadow(const fc4_tract_host* host);
void fc4_tract_destroy(fc4_tract* p);
bool fc4_tract_init(fc4_tract* p);                       /* §1.3 */
bool fc4_tract_shutdown(fc4_tract* p);                   /* §1.4: true; the style state is kept */
bool fc4_tract_do_message(fc4_tract* p, uint32_t msg);   /* §3: Year runs NewYear; true */
/* Slot 5 (§4.3): returns built; each appended code goes to `outcome` (may be NULL), in order. */
int32_t fc4_tract_grow(fc4_tract* p, const fc4_tract_profile* profile, fc4_u32_callback outcome, void* ctx,
                       bool* out_flag, fc4_tract_grow_result* out);
int32_t fc4_tract_decline(fc4_tract* p, const fc4_tract_profile* profile);           /* slot 6 (§4.19) */
void fc4_tract_update_population(fc4_tract* p, const fc4_tract_profile* profile);    /* slot 7 (§4.18) */
uint32_t fc4_tract_current_style(const fc4_tract* p);                                /* slot 8 */
/* Slot 9: copies up to `cap` styles to `out` (may be NULL); returns the count. */
size_t fc4_tract_active_styles(const fc4_tract* p, uint32_t* out, size_t cap);
void fc4_tract_set_active_styles(fc4_tract* p, const uint32_t* v, size_t n);         /* slot 10 */
bool fc4_tract_is_using_all_styles(const fc4_tract* p);                              /* slot 11 */
void fc4_tract_use_all_styles(fc4_tract* p, bool all);                               /* slot 12 */
int32_t fc4_tract_years_between_style_changes(const fc4_tract* p);                   /* slot 13 */
void fc4_tract_set_years_between_style_changes(fc4_tract* p, int32_t years);         /* slot 14 */
/* The §4.1 debug histogram (off by default; reset by each Grow): 13 x 28, row-major. */
void fc4_tract_set_histogram(fc4_tract* p, bool on);
bool fc4_tract_histogram(const fc4_tract* p, uint32_t* out, size_t cap);
bool fc4_tract_get_state(const fc4_tract* p, fc4_tract_state* out);
bool fc4_tract_get_tuning(const fc4_tract* p, fc4_tract_tuning* out);
/* ReadTuning and its style fix-ups (§4.21) without the rest of Init. */
void fc4_tract_read_tuning(fc4_tract* p);
bool fc4_tract_write(const fc4_tract* p, const fc4_ostream* out);   /* §6 */
bool fc4_tract_read(fc4_tract* p, const fc4_istream* in);           /* §6; before Init */
/* Shadow cores only (-1 / false otherwise): UpdatePopulation with each SetPopulation
 * handed to `cb` instead of the game (returns the count); Grow's setup for the
 * probes; the probes (§4.7, §4.8 with redevelop false, §4.17), not counted. */
int32_t fc4_tract_shadow_update_population(fc4_tract* p, const fc4_tract_profile* profile,
                                           fc4_tract_population_callback cb, void* ctx);
bool fc4_tract_probe_setup(fc4_tract* p, const fc4_tract_profile* profile, bool relaxed);
int32_t fc4_tract_probe_configuration_suitable(const fc4_tract* p, uint64_t cfg);
int32_t fc4_tract_probe_lot_suitable(const fc4_tract* p, uint64_t lot);
int32_t fc4_tract_probe_building_compatible(const fc4_tract* p, uint32_t building, int32_t lo, int32_t hi);

/* ---- commercial and industrial (docs/specs/commercial-industrial.md) ---------
 * The commercial simulator (cSC4CommercialSimulator, 0x8990C080) and the
 * industrial stub (0xA990C022). The shim owns the GZCOM objects (the
 * message-target and serializable sub-objects, the agent) and forwards
 * their slots here. Tokens are opaque host identities: occupants, the two
 * demand nodes and grids. The CO$$ / CO$$$ proximity grids are the game's
 * own cSC4SimGrid<uint8> objects made through `grids` (the core holds one
 * reference on each); fc4_commercial_proximity_map returns the grid's
 * token, which the shim turns back into the game's interface pointer. The
 * building map is read through the same table without a reference. A
 * missing callback reads as 0 / false / none. */
typedef struct fc4_commercial fc4_commercial;
typedef struct fc4_industrial fc4_industrial;

typedef struct fc4_commercial_host {
    uint32_t struct_size; /* sizeof(fc4_commercial_host) */
    void* user;
    /* AddAgent(messageTarget, level, name, flags) / RemoveAgent on the city's simulator (§4.1, §4.2). */
    void (*add_agent)(void* user, uint32_t level, const char* name, uint32_t flags);
    void (*remove_agent)(void* user);
    /* Hold the cached message server; false = none. Called only while none is held. */
    bool (*acquire_message_server)(void* user);
    /* Message server slots 5 / 6 with the message-target sub-object. */
    void (*subscribe)(void* user, uint32_t msg_type);
    void (*unsubscribe)(void* user, uint32_t msg_type);
    void (*release_message_server)(void* user);
    /* Hold the cached city / building development simulator; false = none (§4.1 step 4). */
    bool (*acquire_city)(void* user);
    void (*release_city)(void* user);
    bool (*acquire_bds)(void* user);
    void (*release_bds)(void* user);
    /* city.GetDemandSimulator() (slot 56) is not NULL. */
    bool (*has_demand_simulator)(void* user);
    /* GetDemand(demand_id, 0), AddRef'ed for the core (0 = none); released with release_demand_node. */
    uint64_t (*demand_node)(void* user, uint32_t demand_id);
    void (*release_demand_node)(void* user, uint64_t node);
    /* BDS slot 41 GetBuildingMap(): a grid token readable through `grids`, not held (0 = none). */
    uint64_t (*building_map)(void* user);
    /* BDS slot 42 GetBuildingMapKey(purpose, wealth); only the low byte matters. */
    uint8_t (*building_map_key)(void* user, uint32_t purpose, uint32_t wealth);
    /* occupant QueryInterface(0x87DDA3A9), then the building profile's first u32
     * (the purpose); false when not a building occupant. The host releases the
     * interface. */
    bool (*building_purpose)(void* user, uint64_t occupant, uint32_t* purpose);
    /* The game's grids (required: a full-size table). */
    fc4_grid_api grids;
} fc4_commercial_host;

typedef struct fc4_commercial_state {
    uint32_t struct_size;
    uint8_t initialized, dirty, city_held, bds_held, message_server_held, destroyed;
    uint8_t flag; /* the saved flag byte (§8.1): dirty || state != 3 */
    uint8_t reserved0;
    int32_t state; /* the DoIdle state machine (§4.3) */
    uint32_t ref_count, reserved1;
    uint64_t demand_nodes[2];
    uint64_t co2_grid, co3_grid; /* the CO$$ / CO$$$ grid tokens (0 = none) */
} fc4_commercial_state;

/* Constructed, not initialised, reference count 0. The host table is copied;
 * NULL for a NULL or short table, or an incomplete `grids`. */
fc4_commercial* fc4_commercial_create(const fc4_commercial_host* host);
/* Frees the handle (and the grids' references if they are still held). */
void fc4_commercial_destroy(fc4_commercial* c);
/* QueryInterface (§1): the IIDs the object answers. */
bool fc4_commercial_query_interface(uint32_t iid);
uint32_t fc4_commercial_add_ref(fc4_commercial* c);
/* Release (§2.3): 0 when the object was destroyed (Shutdown ran); the handle
 * stays valid until fc4_commercial_destroy. */
uint32_t fc4_commercial_release(fc4_commercial* c);
bool fc4_commercial_init(fc4_commercial* c);
bool fc4_commercial_shutdown(fc4_commercial* c);
/* DoMessage (§3): the idle and day messages run one DoIdle step; an occupant
 * insert / remove (data3 = occupant type, void1 = the occupant) may mark the
 * maps dirty. Always true. */
bool fc4_commercial_do_message(fc4_commercial* c, uint32_t msg_type, uint32_t data1, uint32_t data2,
                               uint32_t data3, uint64_t void1);
/* One DoIdle step (§4.3). */
void fc4_commercial_do_idle(fc4_commercial* c);
/* Tooling (the commercial shadow; ABI 53): sets the DoIdle machine (§2.1,
 * §4.3) to `dirty` and `state`, as read from the original before one of its
 * steps. False for a NULL object or a state outside 0..3 (nothing changes). */
bool fc4_commercial_set_machine(fc4_commercial* c, bool dirty, int32_t state);
/* Slot 5 GetProximityMap(developerType) (§4.6): the grid token, 0 for any other
 * type or no grid. No reference is added. */
uint64_t fc4_commercial_proximity_map(const fc4_commercial* c, uint32_t developer_type);
bool fc4_commercial_get_state(const fc4_commercial* c, fc4_commercial_state* out);
/* Persistence (§8.1): 19 bytes when both grids exist. Read: false unless the
 * version is 1; the grid references are read with IID 0xC9BD939A and each one
 * the stream hands over becomes the core's reference. */
bool fc4_commercial_write(const fc4_commercial* c, const fc4_ostream* out);
bool fc4_commercial_read(fc4_commercial* c, const fc4_istream* in);

/* The industrial stub (§7): constructed, reference count 0. */
fc4_industrial* fc4_industrial_create(void);
void fc4_industrial_destroy(fc4_industrial* p);
bool fc4_industrial_query_interface(uint32_t iid);
uint32_t fc4_industrial_add_ref(fc4_industrial* p);
uint32_t fc4_industrial_release(fc4_industrial* p);
bool fc4_industrial_init(fc4_industrial* p);
bool fc4_industrial_shutdown(fc4_industrial* p);
bool fc4_industrial_do_message(fc4_industrial* p, uint32_t msg_type);
bool fc4_industrial_is_initialized(const fc4_industrial* p);
/* Persistence (§8.2): `01 00`; Read is false unless the version is 1. */
bool fc4_industrial_write(const fc4_industrial* p, const fc4_ostream* out);
bool fc4_industrial_read(fc4_industrial* p, const fc4_istream* in);

/* ---- civic building simulator (docs/specs/civic-buildings.md) ----------------
 * cSC4CivicBuildingSimulator (0x0A413481). The shim owns the GZCOM objects and
 * forwards their slots here. Tokens are opaque host identities: occupants (a
 * building and its occupant are one token), departments, line items and
 * lots. The core holds one reference on the Landmarks department and on
 * every list entry. Statuses (§4.7): the original's slot 6 returns a pointer
 * callers write through; the core keeps the statuses, fc4_civic_get_status
 * copies one out and fc4_civic_update_status writes one back, so a shim that
 * hands out a stable ConditionalBuildingStatus* keeps a mirror and writes it
 * back. A missing callback reads as 0 / false / none. */
typedef struct fc4_civic fc4_civic;

typedef struct fc4_civic_host {
    uint32_t struct_size; /* sizeof(fc4_civic_host) */
    void* user;
    /* AddAgent(+4, level, name, flags) / RemoveAgent(+4) on the city's simulator. */
    void (*add_agent)(void* user, uint32_t level, const char* name, uint32_t flags);
    void (*remove_agent)(void* user);
    /* Message server slots 5 / 6 with the message-target sub-object. */
    void (*subscribe)(void* user, uint32_t msg_type);
    void (*unsubscribe)(void* user, uint32_t msg_type);
    /* Init step 6: the Lua table `civic` with get_landmark_count() (=
     * fc4_civic_landmark_count); Shutdown unregisters it. */
    void (*register_lua)(void* user);
    void (*unregister_lua)(void* user);
    /* AddRef / Release on a host object. */
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    /* The token of a save reference (a token the host's fc4_istream handed over)
     * read with `iid`; the reference is transferred to the core. 0 = none (the
     * host drops it). */
    uint64_t (*resolve_ref)(void* user, uint64_t token, uint32_t iid);
    /* budget.GetDepartmentBudget(id) (not held; 0 = none) and
     * CreateDepartmentBudget(id, group) (the core AddRefs the one it holds). */
    uint64_t (*department_budget)(void* user, uint32_t id);
    uint64_t (*create_department_budget)(void* user, uint32_t id, uint32_t group);
    void (*set_department_name)(void* user, uint64_t dept, uint32_t group, uint32_t instance);
    void (*set_max_funding)(void* user, uint64_t dept, float max);
    /* *dept.GetFundingPercentage() (a fraction); false when unreadable. */
    bool (*department_funding)(void* user, uint64_t dept, float* out);
    /* budget.GetBudgetItemForPurpose(holder, purpose, &item); false when it fails. */
    bool (*budget_item)(void* user, uint64_t occupant, uint32_t purpose, fc4_budget_item* out);
    void (*change_budget_item_line)(void* user, uint64_t occupant, int32_t index, uint32_t line);
    void (*remove_budget_item)(void* user, uint64_t occupant, int32_t index);
    /* dept.GetLineItem(line) / CreateLineItemForBuildingType(line, isLocal) (0 = none) / RemoveLineItem(line). */
    uint64_t (*line_item)(void* user, uint64_t dept, uint32_t line);
    uint64_t (*create_line_item)(void* user, uint64_t dept, uint32_t line, bool local);
    void (*remove_line_item)(void* user, uint64_t dept, uint32_t line);
    /* Line item AddToFullExpenses / AddToIncome / GetIncome / SetIncome (clamps at 0) / SetType. */
    void (*add_to_full_expenses)(void* user, uint64_t line, int64_t amount);
    void (*add_to_income)(void* user, uint64_t line, int64_t amount);
    int64_t (*line_income)(void* user, uint64_t line);
    void (*set_line_income)(void* user, uint64_t line, int64_t income);
    void (*set_line_type)(void* user, uint64_t line, uint32_t line_type);
    /* occ.GetType(); a bool / u32 property of occ.AsPropertyHolder() (false = absent). */
    uint32_t (*occupant_type)(void* user, uint64_t occupant);
    bool (*occupant_bool_property)(void* user, uint64_t occupant, uint32_t id, bool* out);
    bool (*occupant_u32_property)(void* user, uint64_t occupant, uint32_t id, uint32_t* out);
    /* occ.QueryInterface(0x87DDA3A9) succeeds (the host releases the interface). */
    bool (*is_building_occupant)(void* user, uint64_t occupant);
    uint32_t (*building_type)(void* user, uint64_t building);
    /* BDS.GetBuildingKeyFromType(b.GetBuildingType()) (slot 14): the key's instance; false when it fails. */
    bool (*building_key_instance)(void* user, uint64_t building, uint32_t* instance);
    bool (*is_occupant_group)(void* user, uint64_t occupant, uint32_t group);
    /* The lot-state message's IGZUnknown QueryInterface(0xE818FCED): the lot, with a
     * reference the core releases (0 = not a lot). */
    uint64_t (*message_lot)(void* user, uint64_t unknown);
    /* lot.GetBuilding() (slot 48) as the building's occupant token (0 = none). */
    uint64_t (*lot_building)(void* user, uint64_t lot);
    /* lotManager.GetOccupantLot(occ) (slot 14) (0 = none). */
    uint64_t (*occupant_lot)(void* user, uint64_t occupant);
    uint32_t (*lot_condition)(void* user, uint64_t lot);
    void (*set_lot_condition)(void* user, uint64_t lot, uint32_t condition);
    /* city.GetAutomataControllerManager(), if any: AddControllersFromOccupantGroup(name, occ, 0). */
    void (*add_controllers)(void* user, const char* name, uint64_t occupant);
} fc4_civic_host;

typedef struct fc4_civic_state {
    uint32_t struct_size;
    uint8_t initialized, destroyed;
    uint16_t reserved0;
    uint32_t ref_count, exclusions, statuses, disease_centers;
    uint32_t maintained;  /* GetLandmarkCount() */
    uint32_t temporaries; /* temporary generators made so far (§5) */
    uint64_t landmarks;   /* the Landmarks department (0 = none) */
} fc4_civic_state;

/* Constructed, reference count 0. The host table is copied (NULL for a NULL or
 * short table). `session_seed` is foundations §6.3's S (the ape escape's
 * temporary generators, §5). */
fc4_civic* fc4_civic_create(const fc4_civic_host* host, uint32_t session_seed);
void fc4_civic_destroy(fc4_civic* c);
bool fc4_civic_query_interface(uint32_t iid);
uint32_t fc4_civic_add_ref(fc4_civic* c);
/* 0 when the object was destroyed (Shutdown ran); the handle stays valid until fc4_civic_destroy. */
uint32_t fc4_civic_release(fc4_civic* c);
bool fc4_civic_init(fc4_civic* c);
bool fc4_civic_shutdown(fc4_civic* c);
/* DoMessage (§3): SimNewMonth, occupant inserted / removed (void1 = the
 * occupant), lot state changed (data2 = the new state, `unknown` = the lot
 * message's IGZUnknown). Always true. */
bool fc4_civic_do_message(fc4_civic* c, uint32_t msg_type, uint32_t data1, uint32_t data2, uint32_t data3,
                          uint64_t void1, uint64_t unknown);
/* Slot 5 IsBuildingExcluded(id). */
bool fc4_civic_is_building_excluded(const fc4_civic* c, uint32_t id);
/* Slot 6 GetConditionalBuildingStatus(key), copied out: false when there is no
 * entry. The tooltip is copied NUL-terminated into `text` (truncated to
 * capacity - 1) and its full byte length stored in `text_len`. Every out
 * pointer may be NULL. */
bool fc4_civic_get_status(const fc4_civic* c, uint32_t key, uint8_t* status, uint16_t* building_count,
                          char* text, size_t capacity, size_t* text_len);
/* Slot 7 UpdateConditionalBuildingStatus(key, status): find or insert `key`,
 * then copy the status, count and text_len bytes of `text` (NULL = empty).
 * Returns true. */
bool fc4_civic_update_status(fc4_civic* c, uint32_t key, uint8_t status, uint16_t building_count,
                             const char* text, size_t text_len);
/* The status keys / exclusion IDs in hash order (the record's order). Return
 * the count; copy at most `capacity` (`out` may be NULL). */
size_t fc4_civic_status_keys(const fc4_civic* c, uint32_t* out, size_t capacity);
size_t fc4_civic_exclusions(const fc4_civic* c, uint32_t* out, size_t capacity);
/* Slot 8 GetLandmarkCount(). */
uint32_t fc4_civic_landmark_count(const fc4_civic* c);
/* `which` 0 = disease centers, 1 = maintained buildings, as tokens (0 = a null
 * entry). As fc4_civic_status_keys. */
size_t fc4_civic_list(const fc4_civic* c, uint32_t which, uint64_t* out, size_t capacity);
bool fc4_civic_get_state(const fc4_civic* c, fc4_civic_state* out);
/* Persistence (§7). Read: false unless the version is 6; references come back
 * through resolve_ref (the department with IID 0x223E2B67, list entries with
 * 0x26BB767E). */
bool fc4_civic_write(const fc4_civic* c, const fc4_ostream* out);
bool fc4_civic_read(fc4_civic* c, const fc4_istream* in);

/* ---- park manager (docs/specs/parks.md) ---------------------------------------
 * cSC4ParkManager (0xAA3FE6BC). The shim owns the GZCOM objects and forwards
 * their slots here. Tokens are opaque host identities: occupants (a building
 * and its occupant are one token), the department, line items and lots. The
 * core holds one reference on the department and on every list entry; the
 * city, budget simulator, message server and lot manager are held through
 * the acquire_* / release_* pairs. A missing callback reads as 0 / false /
 * none. */
typedef struct fc4_park fc4_park;

typedef struct fc4_park_host {
    uint32_t struct_size; /* sizeof(fc4_park_host) */
    void* user;
    /* AddAgent(+4, level, name, flags) / RemoveAgent(+4) on the city's simulator. */
    void (*add_agent)(void* user, uint32_t level, const char* name, uint32_t flags);
    void (*remove_agent)(void* user);
    /* Hold the message server; false = none. Called only while none is held. */
    bool (*acquire_message_server)(void* user);
    void (*subscribe)(void* user, uint32_t msg_type);
    void (*unsubscribe)(void* user, uint32_t msg_type);
    void (*release_message_server)(void* user);
    /* Hold the city / the budget simulator / the lot manager; false = none. */
    bool (*acquire_city)(void* user);
    void (*release_city)(void* user);
    bool (*acquire_budget)(void* user);
    void (*release_budget)(void* user);
    bool (*acquire_lot_manager)(void* user);
    void (*release_lot_manager)(void* user);
    /* AddRef / Release on a host object; resolve_ref as fc4_civic_host. */
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    uint64_t (*resolve_ref)(void* user, uint64_t token, uint32_t iid);
    /* The department, as fc4_civic_host. */
    uint64_t (*department_budget)(void* user, uint32_t id);
    uint64_t (*create_department_budget)(void* user, uint32_t id, uint32_t group);
    void (*set_department_name)(void* user, uint64_t dept, uint32_t group, uint32_t instance);
    void (*set_max_funding)(void* user, uint64_t dept, float max);
    bool (*department_funding)(void* user, uint64_t dept, float* out);
    /* budget.GetBudgetItemForPurpose / ChangeBudgetItemLine / GetLineItemFromBudgetItem. */
    bool (*budget_item)(void* user, uint64_t occupant, uint32_t purpose, fc4_budget_item* out);
    void (*change_budget_item_line)(void* user, uint64_t occupant, int32_t index, uint32_t line);
    uint64_t (*line_item_from_budget_item)(void* user, const fc4_budget_item* item);
    /* dept.GetLineItem / CreateLineItemForBuildingType (0 = none). */
    uint64_t (*line_item)(void* user, uint64_t dept, uint32_t line);
    uint64_t (*create_line_item)(void* user, uint64_t dept, uint32_t line, bool local);
    /* Line item SetDisplayFlag / AddToFullExpenses / Get- and SetSecondaryInfoField (64-bit). */
    void (*set_line_display_flag)(void* user, uint64_t line, uint32_t flag, bool on);
    void (*add_to_full_expenses)(void* user, uint64_t line, int64_t amount);
    int64_t (*line_secondary)(void* user, uint64_t line);
    void (*set_line_secondary)(void* user, uint64_t line, int64_t value);
    /* occ.GetType(); occ.AsPropertyHolder() has property `id` (any value); IsOccupantGroup. */
    uint32_t (*occupant_type)(void* user, uint64_t occupant);
    bool (*occupant_has_property)(void* user, uint64_t occupant, uint32_t id);
    bool (*is_occupant_group)(void* user, uint64_t occupant, uint32_t group);
    /* occ.QueryInterface(0x87DDA3A9) succeeds (the host releases the interface). */
    bool (*is_building_occupant)(void* user, uint64_t occupant);
    uint32_t (*building_type)(void* user, uint64_t building);
    /* The lot manager's GetLot(cellX, cellZ, false) at the cell city.PositionToCell
     * gives for the occupant's position (0 = none); lot.SetCondition(c). */
    uint64_t (*occupant_position_lot)(void* user, uint64_t occupant);
    void (*set_lot_condition)(void* user, uint64_t lot, uint32_t condition);
    /* Posts 0xAA667CE2: a new standard message with void1 = the manager
     * (`manager`, the token given at creation) and IGZUnknown = the zoo,
     * MessagePost(msg, false). */
    void (*post_zoo_escape)(void* user, uint64_t manager, uint64_t zoo);
    /* city.GetAutomataControllerManager(), if any: AddControllersFromOccupantGroup(name, occ, 0). */
    void (*add_controllers)(void* user, const char* name, uint64_t occupant);
} fc4_park_host;

typedef struct fc4_park_state {
    uint32_t struct_size;
    uint8_t initialized, destroyed, tuning_read, underfunded;
    uint8_t city_held, budget_held, message_server_held, lot_manager_held;
    uint32_t ref_count, parks, zoos;
    float distress_threshold;
    uint32_t curve_points; /* kept points of the escape curve */
    uint32_t reserved;
    uint64_t department; /* Parks and Recreation (0 = none) */
} fc4_park_state;

/* Constructed, reference count 0. The host table is copied (NULL for a NULL or
 * short table). `session_seed` seeds the member generator (§5:
 * derive(0xAA3FE6BC)); `manager` is the manager's own token (void1 of ZooEscape). */
fc4_park* fc4_park_create(const fc4_park_host* host, uint32_t session_seed, uint64_t manager);
void fc4_park_destroy(fc4_park* p);
bool fc4_park_query_interface(uint32_t iid);
uint32_t fc4_park_add_ref(fc4_park* p);
/* Release (§8): 0 when the object was destroyed (Shutdown ran); the handle stays valid until fc4_park_destroy. */
uint32_t fc4_park_release(fc4_park* p);
/* Init (§4.1). `tuning` = the "Parks Manager" exemplar's properties (copied;
 * NULL = none); read at the object's first Init only. */
bool fc4_park_init(fc4_park* p, const fc4_props* tuning);
bool fc4_park_shutdown(fc4_park* p);
/* DoMessage (§3): SimNewMonth, occupant inserted / removed (void1 = the occupant). Always true. */
bool fc4_park_do_message(fc4_park* p, uint32_t msg_type, uint32_t data1, uint32_t data2, uint32_t data3,
                         uint64_t void1);
/* §4.4 step 5's escape probability for a funding fraction, with the object's curve. */
double fc4_park_escape_probability(const fc4_park* p, float funding);
/* `which` 0 = parks, 1 = zoos, as tokens (0 = a null entry). Returns the
 * length; copies at most `capacity` (`out` may be NULL). */
size_t fc4_park_list(const fc4_park* p, uint32_t which, uint64_t* out, size_t capacity);
bool fc4_park_get_state(const fc4_park* p, fc4_park_state* out);
/* Persistence (§7). Read: false unless the version is 2; list entries come
 * back through resolve_ref (IID 0x26BB767E). */
bool fc4_park_write(const fc4_park* p, const fc4_ostream* out);
bool fc4_park_read(fc4_park* p, const fc4_istream* in);

/* ---- list sinks (engine/capi/src/sinks.rs) -----------------------------------
 * A host callback that returns a list calls the sink it is given one or more
 * times with (ctx, items, len); each call appends. The items are copied during
 * the call. Used by the city detail manager, the port developers and the prop
 * developer. */
typedef void (*fc4_u64s_sink)(void* ctx, const uint64_t* items, size_t len);
/* fc4_u32s_sink is declared with the lot developer above. */

/* One lot configuration object (SDK cSC4LotConfigurationObject, a 0x34-byte
 * record of cfg slot 23): the fields the cores read. */
typedef struct fc4_lot_cfg_object {
    uint32_t object_type; /* 1 prop, 2 base texture, 4 flora, ... (the type byte) */
    uint32_t tag;         /* the object ID (record +0x24), the lot tag of its occupant */
    uint8_t flags;        /* the flag byte (bit 0: optional) */
    uint8_t reserved[3];
    uint32_t id_count;    /* the ID list (record +0x28: prop IDs or family IDs) */
    const uint32_t* ids;
} fc4_lot_cfg_object;
typedef void (*fc4_lot_cfg_objects_sink)(void* ctx, const fc4_lot_cfg_object* items, size_t len);

/* ---- city detail manager (docs/specs/city-detail.md) ---------------------------
 * cSC4CityDetailManager. The plugin's object (with its serializable
 * sub-object) forwards its slots here. Tokens are opaque host identities:
 * lots, occupants and network lots. The increase pass's temporary generator
 * is seeded per session (§3.5 [choice]): derive_seed(S, 0xEAD4BA36 XOR n), n
 * counted per thread across every manager created with the same session
 * seed. A missing callback reads as 0 / false / none / empty. */
typedef struct fc4_city_detail fc4_city_detail;

/* A position and orientation (lot developer slot 39). */
typedef struct fc4_object_placement {
    float x, y, z;
    uint32_t orientation;
} fc4_object_placement;

/* One network lot of IterateAllNetworkLots. */
typedef void (*fc4_network_lot_sink)(void* ctx, int32_t x, int32_t z, uint64_t network_lot);

typedef struct fc4_city_detail_host {
    uint32_t struct_size; /* sizeof(fc4_city_detail_host) */
    void* user;
    /* The city detail setting: the preferences byte, 1 low, 2 medium, 3 high
       (city-detail.md §2; the original's default is 3). */
    uint32_t (*detail_setting)(void* user);
    /* Every lot of the lot manager. */
    void (*lots)(void* user, fc4_u64s_sink sink, void* ctx);
    /* The lot configuration's objects as the game built them (already filtered by the
     * detail preference, §3.1 CD1); false without a configuration. */
    bool (*lot_config_objects)(void* user, uint64_t lot, fc4_lot_cfg_objects_sink sink, void* ctx);
    /* The lot's occupants / the occupants in the lot's cell rect. */
    void (*lot_occupants)(void* user, uint64_t lot, fc4_u64s_sink sink, void* ctx);
    void (*occupants_in_lot_rect)(void* user, uint64_t lot, fc4_u64s_sink sink, void* ctx);
    uint32_t (*occupant_type)(void* user, uint64_t occupant);
    /* The occupant's lot tag; false without one. */
    bool (*occupant_lot_tag)(void* user, uint64_t occupant, uint32_t* tag);
    void (*demolish_flora)(void* user, uint64_t occupant);
    void (*remove_prop)(void* user, uint64_t occupant);
    /* Lot developer slot 39 for configuration object `index` of the lot. */
    void (*object_placement)(void* user, uint64_t lot, uint32_t index, fc4_object_placement* out);
    /* Object `index`'s ID array. */
    void (*object_id_array)(void* user, uint64_t lot, uint32_t index, fc4_u32s_sink sink, void* ctx);
    /* Creates a lot prop / a flora occupant of `id` at the placement; its token (0 = none). */
    uint64_t (*add_lot_prop)(void* user, uint32_t id, const fc4_object_placement* placement);
    uint64_t (*add_flora)(void* user, uint32_t id, const fc4_object_placement* placement);
    void (*set_lot_tag)(void* user, uint64_t occupant, uint32_t tag);
    void (*network_lots)(void* user, fc4_network_lot_sink sink, void* ctx);
    void (*redevelop_network_lot)(void* user, int32_t x, int32_t z, uint64_t network_lot);
} fc4_city_detail_host;

typedef struct fc4_city_detail_state {
    uint32_t struct_size;
    uint32_t level; /* the stored level (0xFFFFFFFF = none) */
    uint32_t ref_count;
    uint8_t destroyed;
    uint8_t reserved[3];
    /* The last Init: 0 nothing, 1 reduce, 2 increase; the occupants removed
     * and created, the draws, the network lots redeveloped. */
    uint32_t last_pass, last_removed, last_created, last_draws, last_network_lots;
    uint32_t session_passes; /* increase passes by this manager object so far (n of §3.5,
                              * 0 at creation; CD5) */
} fc4_city_detail_state;

/* Constructed (level none, reference count 0). The host table is copied (NULL
 * for a NULL or short table). `session_seed` is S of §3.5; `token` is the
 * manager's own host identity. The increase-pass count n of §3.5 belongs to
 * the object and starts at 0 (CD5): the game makes one per city load. */
fc4_city_detail* fc4_city_detail_create(const fc4_city_detail_host* host, uint32_t session_seed, uint64_t token);
void fc4_city_detail_destroy(fc4_city_detail* c);
bool fc4_city_detail_query_interface(uint32_t iid);
uint32_t fc4_city_detail_add_ref(fc4_city_detail* c);
/* Release: 0 when the object was destroyed; the handle stays valid until fc4_city_detail_destroy. */
uint32_t fc4_city_detail_release(fc4_city_detail* c);
/* Init (§3.1): reduce or increase, then redevelop the network lots. Always true. */
bool fc4_city_detail_init(fc4_city_detail* c);
bool fc4_city_detail_shutdown(fc4_city_detail* c);
/* Slot 5 ([inferred] GetCityDetail): the stored level. */
uint32_t fc4_city_detail_get_level(const fc4_city_detail* c);
bool fc4_city_detail_get_state(const fc4_city_detail* c, fc4_city_detail_state* out);
/* Persistence (§7). */
bool fc4_city_detail_write(const fc4_city_detail* c, const fc4_ostream* out);
bool fc4_city_detail_read(fc4_city_detail* c, const fc4_istream* in);

/* ---- airport and seaport developers (docs/specs/port-developers.md) ------------
 * cSC4AirportDeveloper / cSC4SeaportDeveloper. `kind` 0 = airport, 1 =
 * seaport. The plugin's object (with the port, message-target and
 * serializable sub-objects) forwards its slots here. Tokens are opaque host
 * identities: lots, occupants, lot configurations, the budget department. A
 * missing callback reads as 0 / false / none / empty. */
typedef struct fc4_port_developer fc4_port_developer;

typedef struct fc4_port_host {
    uint32_t struct_size; /* sizeof(fc4_port_host) */
    void* user;
    /* A NEW fc4_props with exemplar (0x6534284A, 0x67CD5FA1, instance); NULL if missing. */
    fc4_props* (*exemplar)(void* user, uint32_t instance);
    void (*add_agent)(void* user, uint32_t level, const char* name, uint32_t flags);
    void (*remove_agent)(void* user, const char* name);
    /* The budget department (0 = none) / CreateDepartment(id, group, name group, name instance, max funding, line). */
    uint64_t (*department)(void* user, uint32_t id);
    uint64_t (*create_department)(void* user, uint32_t id, uint32_t group, uint32_t name_group,
                                  uint32_t name_instance, float max_funding, uint32_t line);
    /* The line's full and current expenses; false without the line. */
    bool (*line_expenses)(void* user, uint64_t department, uint32_t line, int64_t* full, int64_t* current);
    void (*set_full_expenses)(void* user, uint64_t department, uint32_t line, int64_t amount);
    bool (*withdraw_funds)(void* user, int64_t amount);
    bool (*occupant_is_preview)(void* user, uint64_t occupant);
    uint32_t (*occupant_type)(void* user, uint64_t occupant);
    bool (*occupant_in_group)(void* user, uint64_t occupant, uint32_t group);
    uint64_t (*occupant_lot)(void* user, uint64_t occupant);
    /* The lot configuration's ID and growth stage; false without one. */
    bool (*lot_config)(void* user, uint64_t lot, uint32_t* id, uint32_t* stage);
    void (*lot_location)(void* user, uint64_t lot, int16_t* x, int16_t* z);
    uint64_t (*lot_at)(void* user, int16_t x, int16_t z);
    uint32_t (*lot_state)(void* user, uint64_t lot);
    uint32_t (*lot_condition)(void* user, uint64_t lot);
    void (*set_lot_condition)(void* user, uint64_t lot, uint32_t condition);
    uint32_t (*lot_days_since_state_change)(void* user, uint64_t lot);
    void (*lot_rect)(void* user, uint64_t lot, fc4_cell_rect* out);
    uint32_t (*lot_facing)(void* user, uint64_t lot);
    bool (*lot_zone_compatible)(void* user, uint64_t lot, uint16_t zone);
    /* The lot configuration with this ID (0 = none: the swap still goes on, PD5). */
    uint64_t (*lot_configuration)(void* user, uint32_t id);
    void (*start_demolish_lot)(void* user, uint64_t lot);
    void (*end_demolish_lot)(void* user, uint64_t lot);
    /* CreateLot(x, z, width, depth, facing); the lot (0 = none). */
    uint64_t (*create_lot)(void* user, int32_t x, int32_t z, int32_t width, int32_t depth, uint32_t facing);
    /* SetLotConfiguration + UpdateZoneType, ConstructDefaultLot(lot, cfg, 0, 1); `configuration` may be 0 (null, PD5). */
    void (*set_lot_configuration)(void* user, uint64_t lot, uint64_t configuration);
    bool (*construct_default_lot)(void* user, uint64_t lot, uint64_t configuration);
    void (*dezone)(void* user, const fc4_cell_rect* rect);
    void (*update_occupant_effects)(void* user, uint64_t lot, float old_efficiency, float new_efficiency);
    void (*record_history)(void* user, uint32_t key, int32_t value);
    void (*push_event)(void* user, uint32_t event);
    /* The office counts of trip type `trip_type`, one per subnetwork. */
    void (*office_counts)(void* user, uint32_t trip_type, fc4_u32s_sink sink, void* ctx);
    void (*lot_subnetworks)(void* user, uint64_t lot, fc4_u32s_sink sink, void* ctx);
    uint32_t (*traffic_arrived)(void* user, uint64_t lot, uint32_t trip_type);
    void (*set_max_trip_capacity)(void* user, uint64_t lot, uint32_t trip_type, uint32_t capacity);
    void (*terrain_size)(void* user, uint32_t* width, uint32_t* depth);
    bool (*cell_has_water)(void* user, uint32_t x, uint32_t z);
    uint32_t (*set_access_props_state)(void* user, uint64_t lot, uint32_t property, uint32_t state);
    /* cISTETerrain slot 51 SetAltitude(x, z, h): vertex indices. The seaport passes the truncated metre
     * coordinates of a cell centre, as the original does (§4.8 step 4, PD6); ones outside the terrain's
     * vertices are skipped. */
    void (*set_altitude)(void* user, int32_t x, int32_t z, float altitude);
} fc4_port_host;

/* One port lot (fc4_port_developer_get_port). */
typedef struct fc4_port_lot {
    uint32_t struct_size;
    uint32_t stage, type_index;
    int16_t x, z;
    float efficiency, usage;
    uint32_t trips; /* airport only */
    uint8_t needs_lookup, swapping, reserved[2];
    uint64_t lot; /* 0 = none (or not looked up yet) */
} fc4_port_lot;

typedef struct fc4_port_state {
    uint32_t struct_size;
    uint32_t kind, tuning_instance, type_count, types_loaded, port_count, ref_count;
    uint16_t zone_type;
    uint8_t tuning_loaded, initialized, destroyed, recheck_access, city_has_sea, reserved;
    /* The last month: ran (with ports), history value, advisor event, ports skipped. */
    uint32_t month_ran;
    int32_t month_history;
    uint32_t month_event, month_skipped, reserved2;
    uint64_t department;
} fc4_port_state;

/* `kind` 0 airport, 1 seaport (NULL for another value or a NULL / short host
 * table, which is copied). `token` is the developer's own host identity. */
fc4_port_developer* fc4_port_developer_create(const fc4_port_host* host, uint32_t kind, uint64_t token);
void fc4_port_developer_destroy(fc4_port_developer* p);
/* QueryInterface (§1): the sub-object offset (0, 4, 8 or 0xC); false for an IID the kind does not answer. */
bool fc4_port_developer_query_interface(uint32_t kind, uint32_t iid, uint32_t* offset);
uint32_t fc4_port_developer_add_ref(fc4_port_developer* p);
/* Release: the last release calls Shutdown and returns 0; the handle stays valid until fc4_port_developer_destroy. */
uint32_t fc4_port_developer_release(fc4_port_developer* p);
/* Init (guarded: a second call returns true at once): tuning, agent, department; the seaport's map.
 * Shutdown (nothing without Init): deletes every port and port type, removes the agent; keeps the
 * tuning-loaded flag (PD10). */
bool fc4_port_developer_init(fc4_port_developer* p);
bool fc4_port_developer_shutdown(fc4_port_developer* p);
/* DoMessage (§3): `data` is the occupant (0x99EF1142) or the lot (0xE8BDA2E7,
 * with the old and new states); the month 0x66956816, PostCityInit, the
 * terrain message and the growth messages take no data. Always true. */
bool fc4_port_developer_do_message(fc4_port_developer* p, uint32_t msg, uint64_t data, uint32_t old_state,
                                   uint32_t new_state);
/* GetAvgEfficiency (1.0 with no ports). */
float fc4_port_developer_avg_efficiency(const fc4_port_developer* p);
/* GetLeastEfficientPort: false with no ports. [choice] With two or more the minimum (first on ties). */
bool fc4_port_developer_least_efficient(const fc4_port_developer* p, float* efficiency, int16_t* x, int16_t* z);
/* GetAllPortLots: up to `capacity` lot tokens; returns the count (`out` may be NULL). */
size_t fc4_port_developer_all_port_lots(const fc4_port_developer* p, uint64_t* out, size_t capacity);
/* GetNumPorts(sizeFlags, stageFlags) (size bit 1 << sizeClass). */
uint32_t fc4_port_developer_num_ports(const fc4_port_developer* p, uint32_t size_flags, uint32_t stage_flags);
uint32_t fc4_port_developer_num_airports(const fc4_port_developer* p, uint32_t size);
uint32_t fc4_port_developer_num_seaports(const fc4_port_developer* p);
/* Get...UsageValues(lot): usage = min(trips, capacity), cap = capacity, then with funding the funded
 * cap and the usage clamped to it (§4.7; the month uses the plain capacity, PD8); false (0, 0) if not
 * a port. */
bool fc4_port_developer_usage_values(const fc4_port_developer* p, uint64_t lot, uint32_t* usage, uint32_t* cap);
uint32_t fc4_port_developer_num_trips(const fc4_port_developer* p);
/* RequiredBudget() (§4.2): the summed monthly cost of the current stages (line 1's full expenses after UpdateBudget). */
int64_t fc4_port_developer_required_budget(const fc4_port_developer* p);
/* The funding ratio fr (§4.2): line 1's current / full, 1.0 without the line or with full 0 (reads the line through the host). */
float fc4_port_developer_funding_ratio(const fc4_port_developer* p);
/* GetMaintenanceCost(lot) (0 if not a port). */
int64_t fc4_port_developer_maintenance_cost(const fc4_port_developer* p, uint64_t lot);
/* Seaport: CityHasSea / GetCellSeaAccess(x, z) (the map byte, 0 outside the map). */
bool fc4_port_developer_city_has_sea(const fc4_port_developer* p);
uint8_t fc4_port_developer_cell_sea_access(const fc4_port_developer* p, int32_t x, int32_t z);
/* Port lot `index` (list order); false past the end. */
bool fc4_port_developer_get_port(const fc4_port_developer* p, uint32_t index, fc4_port_lot* out);
bool fc4_port_developer_get_state(const fc4_port_developer* p, fc4_port_state* out);
/* Persistence (§7). Read: false for another version or a type index out of
 * range (the port list is then unchanged); appends the ports. */
bool fc4_port_developer_write(const fc4_port_developer* p, const fc4_ostream* out);
bool fc4_port_developer_read(fc4_port_developer* p, const fc4_istream* in);

/* ---- prop developer (docs/specs/prop-developer.md) -----------------------------
 * cSC4PropDeveloper (0x89C48F47) and its wealth requester (0x89C49986). The
 * plugin's developer (with the message-target and serializable sub-objects)
 * forwards its slots here; its wealth requester objects answer share 10,
 * satisfaction 0x49A1E995 and custom selector true themselves, and forward
 * SelectProp to fc4_prop_developer_wealth_select. The crime and pollution
 * requesters stay in the host (requester_* callbacks). Tokens are opaque host
 * identities: lots, network lots, prop occupants, requesters, managers.
 * Generators (§5): the member generator is seeded with
 * derive_seed(S, 0x89C48F47) x 28; the wealth file generator
 * (derive_seed(S, 0x4C09C51F)) is shared by every developer created on the
 * thread with the same session seed. A missing callback reads as 0 / false /
 * none / empty. */
typedef struct fc4_prop_developer fc4_prop_developer;

/* A held host object: token (0 = null) and class (for the record). */
typedef struct fc4_prop_ref {
    uint64_t token;
    uint32_t class_id;
    uint32_t reserved;
} fc4_prop_ref;

typedef struct fc4_prop_host {
    uint32_t struct_size; /* sizeof(fc4_prop_host) */
    void* user;
    /* City, simulator, messages. */
    void (*city_cells)(void* user, int32_t* x, int32_t* z);
    void (*cell_width)(void* user, float* x, float* z);
    /* Fills three refs: lot manager, prop manager, network lot manager. */
    void (*managers)(void* user, fc4_prop_ref* out3);
    /* Creates the host object of a new wealth requester; its token. */
    uint64_t (*create_wealth_requester)(void* user);
    void (*add_agent)(void* user, uint32_t level, const char* name, uint32_t flags);
    /* Removes the agent from every level. */
    void (*remove_agents)(void* user, const char* name);
    void (*subscribe)(void* user, uint32_t msg_type);
    void (*unsubscribe_all)(void* user);
    void (*register_animation_tick)(void* user, uint32_t priority);
    void (*unregister_animation_tick)(void* user);
    /* Posts 0xCA5E5027 (data1 x, data2 z, data3 = entered state 3). */
    void (*post_lot_state_props)(void* user, int32_t x, int32_t z, bool entered_3);
    /* AddRef / Release of a held requester or prop. */
    void (*hold)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    /* Host requesters (crime, pollution): slots 3-6. */
    int32_t (*requester_share)(void* user, uint64_t requester, const fc4_cell_rect* rect);
    uint32_t (*requester_satisfaction)(void* user, uint64_t requester);
    bool (*requester_custom)(void* user, uint64_t requester);
    uint32_t (*requester_select)(void* user, uint64_t requester, const uint32_t* ids, size_t count, int32_t x,
                                 int32_t z);
    /* Lots and network lots. lot_at = lotManager.GetLot(x, z, originOnly) (0 = none). */
    uint64_t (*lot_at)(void* user, int32_t x, int32_t z, bool origin_only);
    bool (*lot_livable)(void* user, uint64_t lot);
    uint32_t (*lot_building_type)(void* user, uint64_t lot);
    float (*lot_occupancy)(void* user, uint64_t lot);
    /* The configuration's objects; false without a configuration. */
    bool (*lot_config_objects)(void* user, uint64_t lot, fc4_lot_cfg_objects_sink sink, void* ctx);
    uint32_t (*lot_wealth)(void* user, uint64_t lot);
    void (*lot_location)(void* user, uint64_t lot, int32_t* x, int32_t* z);
    void (*lot_size)(void* user, uint64_t lot, int32_t* x, int32_t* z);
    uint32_t (*lot_state)(void* user, uint64_t lot);
    uint32_t (*zone_type)(void* user, int32_t x, int32_t z);
    uint64_t (*network_lot_at)(void* user, int32_t x, int32_t z);
    bool (*network_lot_objects)(void* user, uint64_t network_lot, fc4_lot_cfg_objects_sink sink, void* ctx);
    uint32_t (*network_wealth)(void* user, int32_t x, int32_t z);
    /* Prop manager. */
    void (*lot_props)(void* user, uint64_t lot, fc4_u64s_sink sink, void* ctx);
    void (*street_props)(void* user, int32_t x, int32_t z, fc4_u64s_sink sink, void* ctx);
    void (*street_props_in_rect)(void* user, const fc4_cell_rect* rect, fc4_u64s_sink sink, void* ctx);
    /* RemovePropA. */
    void (*remove_prop)(void* user, uint64_t prop);
    bool (*prop_type_valid)(void* user, uint32_t id);
    void (*props_by_family)(void* user, uint32_t family, fc4_u32s_sink sink, void* ctx);
    /* The prop profile's requester satisfaction (0x49A1E05A) and wealth byte (0xE9A316EB), default 0. */
    uint32_t (*prop_satisfaction)(void* user, uint32_t id);
    uint8_t (*prop_wealth)(void* user, uint32_t id);
    /* Lot developer CreateLotProp(lot, index, id) / network lot manager
     * CreateProp(netLot, index, id); the occupant (0 = none). */
    uint64_t (*create_lot_prop)(void* user, uint64_t lot, uint32_t index, uint32_t id);
    uint64_t (*create_network_prop)(void* user, uint64_t network_lot, uint32_t index, uint32_t id);
    /* Prop occupants. */
    uint32_t (*prop_class)(void* user, uint64_t prop);
    /* A NEW fc4_props with the occupant's properties; NULL if none. */
    fc4_props* (*prop_properties)(void* user, uint64_t prop);
    /* The lot tag; false without one. */
    bool (*prop_lot_tag)(void* user, uint64_t prop, uint32_t* tag);
    /* Sets the u32 property 0x89A1C16C. */
    void (*set_prop_requester)(void* user, uint64_t prop, uint32_t requester);
    uint32_t (*prop_type)(void* user, uint64_t prop);
    uint32_t (*prop_state)(void* user, uint64_t prop);
    void (*set_prop_state)(void* user, uint64_t prop, uint32_t state);
    /* SetPropTimerState(mask, on). */
    void (*set_prop_timer)(void* user, uint64_t prop, uint32_t mask, bool on);
    void (*set_random_chance)(void* user, uint64_t prop, uint8_t chance);
    void (*set_time_of_day_range)(void* user, uint64_t prop, float start, float end);
    void (*time_of_day_range)(void* user, uint64_t prop, float* start, float* end);
    /* SetSimulatorDateRange(date, duration, flag, 0). */
    void (*set_date_range)(void* user, uint64_t prop, uint32_t date, uint32_t duration, uint8_t flag);
    void (*tick_date)(void* user, uint64_t prop, uint32_t date);
    /* Three floats (x, y, z). set_prop_position also notifies the occupant manager (flag 1). */
    void (*prop_position)(void* user, uint64_t prop, float* out3);
    void (*set_prop_position)(void* user, uint64_t prop, const float* position3);
    /* SetFlag / ClearFlag. */
    void (*set_prop_flag)(void* user, uint64_t prop, uint32_t flag, bool on);
    /* The first cell of a network occupant's bounding cells. */
    void (*network_first_cell)(void* user, uint64_t occupant, int32_t* x, int32_t* z);
    /* cISC4City::PositionToCell. */
    void (*position_to_cell)(void* user, float x, float z, int32_t* cell_x, int32_t* cell_z);
    /* Others: power simulator IsPowered, the 3D view (available, trigger state),
     * the sim date, the 24-hour clock (false without it; visual time mode 0
     * normal, 1 day, 2 night), the lighting manager's IsNight, and the terrain
     * altitude and water level at a position. */
    bool (*is_powered)(void* user, int32_t x, int32_t z);
    bool (*view_available)(void* user);
    bool (*view_trigger_state)(void* user, uint32_t trigger);
    uint32_t (*sim_date)(void* user);
    bool (*clock_time)(void* user, uint32_t* hour, uint32_t* minute);
    uint32_t (*visual_time_mode)(void* user);
    bool (*lighting_is_night)(void* user);
    void (*terrain_heights)(void* user, float x, float z, float* altitude, float* water);
} fc4_prop_host;

typedef struct fc4_prop_message {
    uint32_t msg_type;
    uint32_t data1, data2, data3, data4;
    uint32_t reserved;
    uint64_t object; /* the message's occupant or lot (0 = none) */
} fc4_prop_message;

typedef struct fc4_prop_state {
    uint32_t struct_size;
    int32_t scan_flag;      /* 0 never set up, 1 set up, 2 scanning */
    uint32_t scanner_state; /* 0 fresh, 2 done, any other byte running (1, or a byte above 2 as read, PV13) */
    int32_t scanner_x, scanner_z, scanner_step;
    int32_t clip_x0, clip_z0, clip_x1, clip_z1;
    uint32_t night_cursor, night_mode, night_batch, night_acc;
    float tod_threshold, tod_last;
    uint32_t ref_count;
    uint32_t requester_count, night_count, trigger_count, time_of_day_count, dated_count, power_count, sign_count;
    uint8_t wealth_added, is_night, signs_visible, initialized, destroyed, reserved[3];
} fc4_prop_state;

/* What the last block update did (§4.3). */
typedef struct fc4_prop_block {
    uint32_t struct_size;
    int32_t bx, bz;
    uint32_t lots;
    float occ_sum;
    int32_t slots, kept_all;
    uint32_t target, removed, created, member_draws, requester_count;
    uint8_t stopped; /* no lots (step 3) */
    uint8_t reserved[3];
} fc4_prop_block;

/* The lists of fc4_prop_developer_list. */
#define FC4_PROP_LIST_REQUESTERS  0u
#define FC4_PROP_LIST_NIGHT       1u
#define FC4_PROP_LIST_TIME_OF_DAY 2u
#define FC4_PROP_LIST_DATED       3u
#define FC4_PROP_LIST_POWER       4u
#define FC4_PROP_LIST_SIGNS       5u

/* Constructed (reference count 0). The host table is copied (NULL for a NULL
 * or short table). `session_seed` is S of §5; `token` the developer's own
 * host identity. */
fc4_prop_developer* fc4_prop_developer_create(const fc4_prop_host* host, uint32_t session_seed, uint64_t token);
void fc4_prop_developer_destroy(fc4_prop_developer* p);
/* QueryInterface (§1): the developer's sub-object offset (0, 4 or 8), the
 * wealth requester's (0 or 4); false for another IID. */
bool fc4_prop_developer_query_interface(uint32_t iid, uint32_t* offset);
bool fc4_wealth_requester_query_interface(uint32_t iid, uint32_t* offset);
uint32_t fc4_prop_developer_add_ref(fc4_prop_developer* p);
/* Release: 0 when the object was destroyed; the handle stays valid until fc4_prop_developer_destroy. */
uint32_t fc4_prop_developer_release(fc4_prop_developer* p);
/* Init (§3.1) / Shutdown (§3.2). Always true. */
bool fc4_prop_developer_init(fc4_prop_developer* p);
bool fc4_prop_developer_shutdown(fc4_prop_developer* p);
/* DoMessage (§3.3): false only for an animation tick in night mode 0 (and a NULL message). */
bool fc4_prop_developer_do_message(fc4_prop_developer* p, const fc4_prop_message* msg);
/* AddPropRequester (slot 5, no duplicate check): class 0x89C49986 is a wealth
 * requester (answered by the core); another class is a host requester (held). */
void fc4_prop_developer_add_prop_requester(fc4_prop_developer* p, uint64_t token, uint32_t class_id);
/* RemovePropRequester (slot 6): the first requester with this token; false if absent. */
bool fc4_prop_developer_remove_prop_requester(fc4_prop_developer* p, uint64_t token);
/* Slots 7-10: UpdateAllOptionalProps, ShowSignsAndLabels, ToggleSignsAndLabels, AreSignsAndLabelsVisible. */
void fc4_prop_developer_update_all(fc4_prop_developer* p);
void fc4_prop_developer_show_signs(fc4_prop_developer* p, bool show);
void fc4_prop_developer_toggle_signs(fc4_prop_developer* p);
bool fc4_prop_developer_signs_visible(const fc4_prop_developer* p);
/* The wealth requester's SelectProp(ids, x, z) (§4.1), drawing from the
 * session's wealth file generator; 0 when no ID matches. */
uint32_t fc4_prop_developer_wealth_select(fc4_prop_developer* p, const uint32_t* ids, size_t count, int32_t x,
                                          int32_t z);
bool fc4_prop_developer_get_state(const fc4_prop_developer* p, fc4_prop_state* out);
/* The last block update; `want` and `kept` (each `capacity` entries, may be
 * NULL) receive the per-requester targets and kept counts. */
bool fc4_prop_developer_last_block(const fc4_prop_developer* p, fc4_prop_block* out, uint32_t* want, uint32_t* kept,
                                   size_t capacity);
/* A list (FC4_PROP_LIST_*): returns the length (0 for an unknown list);
 * copies at most `capacity` refs (`out` may be NULL). */
size_t fc4_prop_developer_list(const fc4_prop_developer* p, uint32_t which, fc4_prop_ref* out, size_t capacity);
/* Trigger map entry `index` (iteration order): the trigger and up to
 * `capacity` props; returns the entry's length, SIZE_MAX past the end. */
size_t fc4_prop_developer_trigger_entry(const fc4_prop_developer* p, uint32_t index, uint32_t* trigger,
                                        fc4_prop_ref* out, size_t capacity);
/* Persistence (§7): Write always writes version 5; Read is false below
 * version 4 or for a short record (nothing changes). A requester reference
 * of class 0x89C49986 becomes a wealth requester with that token. */
bool fc4_prop_developer_write(const fc4_prop_developer* p, const fc4_ostream* out);
bool fc4_prop_developer_read(fc4_prop_developer* p, const fc4_istream* in);

/* ---- demolition utility (docs/specs/demolition.md) ----------------------------
 * cSC4DemolitionUtility (0x0990C38A). The shim owns the GZCOM objects and
 * forwards their slots here. Tokens are opaque host identities: occupants,
 * lots, filters, the caller's occupant list and set (0 = none). Tokens handed
 * to the core are borrowed; the city, occupant manager, lot developer and
 * terrain are held through the acquire_* / release_* pairs. The two tuning
 * exemplars are fetched each time the core reads them. A missing callback
 * reads as 0 / false / none. */
typedef struct fc4_demolition fc4_demolition;

typedef struct fc4_demolition_host {
    uint32_t struct_size; /* sizeof(fc4_demolition_host) */
    void* user;
    /* Lifecycle (§4.1): message subscriptions, the city (and its cell counts),
     * the occupant manager, the lot developer and the terrain; false = none. */
    void (*subscribe)(void* user, uint32_t msg_type);
    void (*unsubscribe)(void* user, uint32_t msg_type);
    bool (*acquire_city)(void* user);
    void (*release_city)(void* user);
    void (*city_cell_counts)(void* user, int32_t* x, int32_t* z);
    bool (*acquire_occupant_manager)(void* user);
    void (*release_occupant_manager)(void* user);
    bool (*acquire_lot_developer)(void* user);
    void (*release_lot_developer)(void* user);
    bool (*acquire_terrain)(void* user);
    void (*release_terrain)(void* user);
    /* The tuning (§6): "Demolition Thresholds" and "Building Development"
     * as a new fc4_props the core takes over (NULL = none). */
    fc4_props* (*thresholds_exemplar)(void* user);
    fc4_props* (*building_development_exemplar)(void* user);
    /* Release on a host object (the deferred lot of 0xA9DED179). */
    void (*release)(void* user, uint64_t token);
    /* The view's occupant filter (0 = none) and the view level (false = no view). */
    uint64_t (*acquire_view_filter)(void* user);
    bool (*view_level)(void* user, uint32_t* out);
    /* filter.IsOccupantIncluded / IsOccupantTypeIncluded; the default and
     * realistic occupant filters. */
    bool (*filter_includes)(void* user, uint64_t filter, uint64_t occupant);
    bool (*filter_includes_type)(void* user, uint64_t filter, uint32_t occupant_type);
    bool (*default_filter_includes)(void* user, uint64_t occupant);
    bool (*realistic_filter_includes)(void* user, uint64_t occupant);
    /* Occupants: type, position (x, y, z into out[3]), CanRemove(level),
     * flags, groups, a bool property (false = absent), the bounds (x0, z0,
     * x1, z1 into out[4]), the foundation's slot 9 (false = no foundation
     * interface), the network's flags / types (false = no network interface),
     * Shutdown, the occupant manager's RemoveOccupant and the queries. */
    uint32_t (*occupant_type)(void* user, uint64_t occupant);
    void (*occupant_position)(void* user, uint64_t occupant, float* out);
    bool (*can_remove)(void* user, uint64_t occupant, uint32_t level);
    bool (*occupant_flag_set)(void* user, uint64_t occupant, uint32_t flag);
    uint32_t (*occupant_flags)(void* user, uint64_t occupant);
    bool (*is_occupant_group)(void* user, uint64_t occupant, uint32_t group);
    bool (*occupant_bool_property)(void* user, uint64_t occupant, uint32_t id, bool* out);
    void (*occupant_bounds)(void* user, uint64_t occupant, float* out);
    bool (*foundation_slot9)(void* user, uint64_t occupant, uint32_t* out);
    bool (*network_has_any_flag)(void* user, uint64_t occupant, uint32_t mask, bool* out);
    bool (*network_has_type)(void* user, uint64_t occupant, uint32_t type);
    void (*occupant_shutdown)(void* user, uint64_t occupant);
    bool (*remove_occupant)(void* user, uint64_t occupant, bool b);
    void (*occupants_in_box)(void* user, float x0, float z0, float x1, float z1, fc4_token_callback cb, void* ctx);
    void (*occupants_in_cell_rect)(void* user, const fc4_cell_rect* rect, fc4_token_callback cb, void* ctx);
    /* Prices and removals: the building development / flora simulators'
     * prices (false = no simulator), networks, power poles (their
     * connections, 0xAA7A014D), the prop manager. */
    bool (*building_demolition_cost)(void* user, uint64_t building, int64_t* out);
    bool (*flora_demolition_cost)(void* user, uint64_t occupant, int64_t* out);
    bool (*is_network_occupant)(void* user, uint64_t occupant);
    int64_t (*network_demolition_cost)(void* user, uint64_t occupant);
    bool (*is_power_pole)(void* user, uint64_t occupant);
    int64_t (*pole_demolition_cost)(void* user, uint64_t pole);
    void (*pole_connections)(void* user, uint64_t pole, fc4_token_callback cb, void* ctx);
    uint32_t (*pole_connection_count)(void* user, uint64_t pole);
    void (*pole_break_all_connections)(void* user, uint64_t pole);
    void (*send_pre_demolish)(void* user, uint64_t occupant);
    bool (*has_prop_manager)(void* user);
    bool (*remove_prop)(void* user, uint64_t occupant);
    /* The caller's occupant list (push back) and set (insert). */
    void (*list_append)(void* user, uint64_t list, uint64_t occupant);
    void (*set_insert)(void* user, uint64_t set, uint64_t occupant);
    /* The network manager: QueueRemovals(on), DeleteOccupantConnection,
     * DestroyNetworkOccupants(list of `n`). */
    void (*queue_removals)(void* user, bool on);
    void (*delete_occupant_connection)(void* user, uint64_t occupant);
    void (*destroy_network_occupants)(void* user, const uint64_t* list, size_t n);
    /* City geometry: a cell corner's position; a position's cell (false = outside). */
    void (*cell_corner_to_position)(void* user, int32_t x, int32_t z, float* px, float* pz);
    bool (*position_to_cell)(void* user, float x, float z, int32_t* cx, int32_t* cz);
    /* Lots (0 = none) and the lot developer. */
    uint64_t (*lot_at)(void* user, int32_t x, int32_t z);
    void (*lots_in_cell_rect)(void* user, const fc4_cell_rect* rect, fc4_token_callback cb, void* ctx);
    uint64_t (*lot_building)(void* user, uint64_t lot);
    uint32_t (*lot_state)(void* user, uint64_t lot);
    bool (*lot_demolishable)(void* user, uint64_t lot);
    void (*lot_occupants)(void* user, uint64_t lot, fc4_token_callback cb, void* ctx);
    void (*lot_size)(void* user, uint64_t lot, uint8_t* x, uint8_t* z);
    void (*lot_bounding_rect)(void* user, uint64_t lot, fc4_cell_rect* out);
    uint32_t (*lot_facing)(void* user, uint64_t lot);
    uint32_t (*lot_zone_type)(void* user, uint64_t lot);
    bool (*lot_water_compatible)(void* user, uint64_t lot, bool* out); /* false = no configuration */
    bool (*lot_initialized)(void* user, uint64_t lot);
    bool (*lot_vertices_locked)(void* user, uint64_t lot);
    uint32_t (*lot_condition)(void* user, uint64_t lot);
    void (*explode_lot)(void* user, uint64_t lot, bool b);
    void (*start_demolish_lot)(void* user, uint64_t lot);
    void (*end_demolish_lot)(void* user, uint64_t lot);
    void (*delete_lot)(void* user, uint64_t lot);
    bool (*create_lot_if_possible)(void* user, int32_t x, int32_t z, uint32_t facing);
    uint32_t (*zone_at)(void* user, int32_t x, int32_t z);
    void (*dezone)(void* user, const fc4_cell_rect* rect);
    /* The terrain (§4.7). */
    float (*altitude_at_vertex)(void* user, int32_t x, int32_t z);
    bool (*is_water_vertex)(void* user, int32_t x, int32_t z);
    void (*set_altitude_rect)(void* user, const fc4_cell_rect* rect, float h, uint32_t reason);
    void (*set_altitude_vertex)(void* user, int32_t x, int32_t z, float h);
    void (*redisplay_terrain)(void* user, const fc4_cell_rect* rect, uint32_t reason);
    /* Effects (§4.8): the model extent (false = none), the occupant's sound,
     * the occupant effect (candidate keys; the ray when has_ray), the group effect. */
    bool (*effect_extent)(void* user, uint64_t occupant, float* out);
    void (*play_occupant_sound)(void* user, uint64_t occupant);
    void (*start_occupant_effect)(void* user, uint64_t occupant, const uint32_t* keys, size_t n, bool has_ray,
                                  int32_t rx, int32_t ry, bool grey);
    void (*start_group_effect)(void* user, uint32_t instance, int32_t x, int32_t z);
} fc4_demolition_host;

typedef struct fc4_demolition_state {
    uint32_t struct_size;
    uint8_t initialized, destroyed, filter_created, city_held;
    uint8_t occupant_manager_held, lot_developer_held, terrain_held, reserved;
    uint32_t ref_count;
    int32_t cells_x, cells_z;
    /* The tuning (§6). */
    uint32_t small_medium_model, medium_large_model, small_medium_group, medium_large_group;
    uint32_t destroyed_tile_cost;
} fc4_demolition_state;

/* Constructed, reference count 0. The host table is copied (NULL for a NULL or
 * short table). `token` is the utility's own token. */
fc4_demolition* fc4_demolition_create(const fc4_demolition_host* host, uint64_t token);
void fc4_demolition_destroy(fc4_demolition* d);
/* QueryInterface (§1): true and the sub-object offset (0, 4 or 8; `offset` may be NULL). */
bool fc4_demolition_query_interface(uint32_t iid, uint32_t* offset);
uint32_t fc4_demolition_add_ref(fc4_demolition* d);
/* Release: 0 when the object was destroyed (Shutdown ran); the handle stays valid until fc4_demolition_destroy. */
uint32_t fc4_demolition_release(fc4_demolition* d);
bool fc4_demolition_init(fc4_demolition* d);
bool fc4_demolition_shutdown(fc4_demolition* d);
/* DoMessage (§3): 0xC53D10AA re-reads the tuning; 0xA9DED179 (`lot` holding
 * one reference, 0 = none; `dezone`) finishes a deferred lot demolition. Always true. */
bool fc4_demolition_do_message(fc4_demolition* d, uint32_t msg_type, uint32_t data1, uint32_t data2, uint64_t lot,
                               bool dezone);
/* DemolishOccupant (§4.3), DemolishRegion (§4.4; false for a NULL or
 * oversized region), DemolishLots (§4.5; 0 = a null entry). `*cost` is
 * written only when the method writes it (`cost` may be NULL). */
bool fc4_demolition_demolish_occupant(fc4_demolition* d, bool do_it, uint64_t occupant, uint32_t privilege,
                                      uint32_t flags, bool skip_network_update, uint64_t orphans, int64_t* cost);
bool fc4_demolition_demolish_region(fc4_demolition* d, bool do_it, const fc4_zone_region* region, uint32_t privilege,
                                    uint32_t flags, bool dezone, uint64_t caller_filter, uint64_t demolished,
                                    uint64_t clicked, int32_t sx, int32_t sy, int64_t* cost);
bool fc4_demolition_demolish_lots(fc4_demolition* d, bool do_it, const uint64_t* lots, size_t count,
                                  uint32_t privilege, uint32_t flags, bool dezone, uint64_t caller_filter,
                                  uint64_t demolished, int64_t* cost);
/* ModifyTerrainHeight (§4.7): the vertices changed (or that would change); `exempt` = a lot (0 = none). */
int32_t fc4_demolition_modify_terrain_height(fc4_demolition* d, bool do_it, bool per_vertex, int32_t x0, int32_t z0,
                                             int32_t x1, int32_t z1, float altitude, uint64_t exempt,
                                             uint32_t reason);
/* UpdateNetworks(set) (§4.6). */
void fc4_demolition_update_networks(fc4_demolition* d, const uint64_t* set, size_t count);
bool fc4_demolition_get_state(const fc4_demolition* d, fc4_demolition_state* out);
/* Persistence (§7): u16 1. Read: false unless the version is 1. */
bool fc4_demolition_write(const fc4_demolition* d, const fc4_ostream* out);
bool fc4_demolition_read(fc4_demolition* d, const fc4_istream* in);

/* ---- dispatch manager (docs/specs/dispatch.md) --------------------------------
 * cSC4DispatchManager (0x4990BD93). Tokens are opaque host identities:
 * dispatch sources, stations, vehicle automata, pylons and the vehicle view.
 * The core holds one reference on every record's station, automaton and
 * pylon, on the active pylon, the available pylons and the registered
 * sources; a token from create_pylon, create_vehicle_view, a created
 * begin_vehicle or resolve_ref comes with one reference for the core. Tokens
 * from queries are borrowed. A missing callback reads as 0 / false / none. */
typedef struct fc4_dispatch fc4_dispatch;

/* DispatchInfo (§2); tokens 0 = none. */
typedef struct fc4_dispatch_info {
    uint32_t dispatch_type, id, call_id, view_handle;
    uint8_t active;
    uint8_t reserved[3];
    float target_x, target_y, target_z;
    uint64_t station, automaton, pylon;
} fc4_dispatch_info;

/* One station of a source: bit i of `mask` set = unit i free; `base` = unit 0's call number. */
typedef struct fc4_station_units {
    uint64_t station;
    uint32_t mask, base, count, reserved;
} fc4_station_units;

typedef struct fc4_dispatch_host {
    uint32_t struct_size; /* sizeof(fc4_dispatch_host) */
    void* user;
    /* Messages (§3, §4.1): the message server; MessagePost (queued) with
     * IGZUnknown = the manager, data1 = the dispatch type, data2 = the ID. */
    bool (*acquire_message_server)(void* user);
    void (*subscribe)(void* user, uint32_t msg_type);
    void (*unsubscribe)(void* user, uint32_t msg_type);
    void (*release_message_server)(void* user);
    void (*post)(void* user, uint32_t msg_type, uint32_t dispatch_type, uint32_t id);
    /* The city; the vehicle view (created and registered, layer 5,
     * priority 1000; destroyed; refreshed) and its tracking. */
    bool (*acquire_city)(void* user);
    void (*release_city)(void* user);
    uint64_t (*create_vehicle_view)(void* user);
    void (*destroy_vehicle_view)(void* user, uint64_t view);
    void (*refresh_vehicle_view)(void* user, uint64_t view);
    uint32_t (*track_vehicle)(void* user, uint64_t automaton, uint32_t kind);
    void (*untrack_vehicle)(void* user, uint32_t handle);
    /* AddRef / Release on a host object; the terrain altitude. */
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    float (*altitude)(void* user, float x, float z);
    /* Dispatch sources (protection.md §8.2): CanHandle, CanDispatch (the
     * result; *all_busy), DispatchFromNearestStation (the station, 0 = none;
     * *call_id), DispatchFromStation (station 0 = none; *call_id in and out),
     * RevokeDispatchFromStation, GetStations (returns the count; copies at
     * most `capacity`, `out` may be NULL). */
    bool (*source_can_handle)(void* user, uint64_t source, uint32_t dispatch_type);
    bool (*source_can_dispatch)(void* user, uint64_t source, float x, float z, bool* all_busy);
    uint64_t (*source_dispatch_from_nearest_station)(void* user, uint64_t source, float x, float z,
                                                     uint32_t* call_id);
    bool (*source_dispatch_from_station)(void* user, uint64_t source, uint64_t station, uint32_t* call_id);
    void (*source_revoke_dispatch_from_station)(void* user, uint64_t source, uint64_t station, uint32_t call_id);
    size_t (*source_stations)(void* user, uint64_t source, fc4_station_units* out, size_t capacity);
    /* Vehicles (§4.2 step 5): create the vehicle or re-target info->automaton
     * (0 = none; a created one with one reference for the core); despawn; go home. */
    uint64_t (*begin_vehicle)(void* user, uint64_t source, const fc4_dispatch_info* info);
    void (*automaton_despawn)(void* user, uint64_t automaton);
    void (*automaton_go_home)(void* user, uint64_t automaton);
    /* Pylons: a new initialised pylon (one reference for the core); its
     * source occupant (borrowed), call number, dispatch type and state, UI
     * state; placement along the station's lot (§4.5) and back (§4.4:
     * source slot 7 for the station's unit state; true = placed, the core
     * then sets dispatch state 0 and UI state 1). */
    uint64_t (*create_pylon)(void* user);
    uint64_t (*pylon_source)(void* user, uint64_t pylon);
    void (*pylon_set_source)(void* user, uint64_t pylon, uint64_t station);
    uint32_t (*pylon_call_number)(void* user, uint64_t pylon);
    void (*pylon_set_call_number)(void* user, uint64_t pylon, uint32_t n);
    uint32_t (*pylon_dispatch_type)(void* user, uint64_t pylon);
    void (*pylon_set_dispatch_type)(void* user, uint64_t pylon, uint32_t type);
    void (*pylon_set_dispatch_state)(void* user, uint64_t pylon, uint32_t state);
    void (*pylon_set_ui_state)(void* user, uint64_t pylon, uint32_t state);
    void (*pylon_move_to)(void* user, uint64_t pylon, float x, float y, float z);
    void (*pylon_place_at_unit)(void* user, uint64_t pylon, uint64_t station, uint32_t index, uint32_t count);
    bool (*pylon_return_to_station)(void* user, uint64_t source, uint64_t pylon, uint64_t station);
    void (*pylon_shutdown)(void* user, uint64_t pylon);
    /* A reference read from a record (§7): the object with one reference for the core (0 = none). */
    uint64_t (*resolve_ref)(void* user, uint64_t token, uint32_t iid);
    /* The vehicle's occupant is in that occupant group (§4.2 step 5: a new vehicle is tracked as
     * kind 0 in group 0x4202 or 0x4303, else kind 1). */
    bool (*vehicle_in_group)(void* user, uint64_t automaton, uint32_t group);
} fc4_dispatch_host;

typedef struct fc4_dispatch_state {
    uint32_t struct_size;
    uint8_t initialized, destroyed, loaded, message_server_held;
    uint8_t city_held;
    uint8_t reserved[3];
    uint32_t ref_count, records, next_id;
    uint32_t tool_type; /* the open tool's type; 8 = none */
    uint32_t available_pylons, sources;
    uint64_t vehicle_view, active_pylon;
} fc4_dispatch_state;

/* Constructed, reference count 0. The host table is copied (NULL for a NULL or
 * short table). `token` is the manager's own token. */
fc4_dispatch* fc4_dispatch_create(const fc4_dispatch_host* host, uint64_t token);
void fc4_dispatch_destroy(fc4_dispatch* d);
/* QueryInterface (§1): true and the sub-object offset (0, 4 or 8; `offset` may be NULL). */
bool fc4_dispatch_query_interface(uint32_t iid, uint32_t* offset);
uint32_t fc4_dispatch_add_ref(fc4_dispatch* d);
/* Release: 0 when the object was destroyed; the handle stays valid until fc4_dispatch_destroy. */
uint32_t fc4_dispatch_release(fc4_dispatch* d);
bool fc4_dispatch_init(fc4_dispatch* d);
bool fc4_dispatch_shutdown(fc4_dispatch* d);
/* Slots 15-17: the sources (held; the first that handles `type`, borrowed, 0 = none). */
void fc4_dispatch_register_source(fc4_dispatch* d, uint64_t source);
void fc4_dispatch_unregister_source(fc4_dispatch* d, uint64_t source);
uint64_t fc4_dispatch_get_source(const fc4_dispatch* d, uint32_t dispatch_type);
/* Slots 5-9: CanDispatch, Dispatch (the ID, 0 on failure), UpdateDispatchState,
 * RevokeDispatchByID, RevokeAllDispatches; and RevokeDispatchByPylon. */
bool fc4_dispatch_can_dispatch(const fc4_dispatch* d, uint32_t dispatch_type, float x, float z);
uint32_t fc4_dispatch_dispatch(fc4_dispatch* d, uint32_t dispatch_type, float x, float z);
void fc4_dispatch_update_state(fc4_dispatch* d, uint32_t id, uint32_t state);
bool fc4_dispatch_revoke_by_id(fc4_dispatch* d, uint32_t id);
bool fc4_dispatch_revoke_by_pylon(fc4_dispatch* d, uint64_t pylon);
bool fc4_dispatch_revoke_all(fc4_dispatch* d, uint32_t dispatch_type);
/* Slots 10-12: a record by ID (false = none), the count of a type, the IDs
 * of a type (`n` 0 = no limit; returns the count, copies at most `capacity`). */
bool fc4_dispatch_get_info(const fc4_dispatch* d, uint32_t id, fc4_dispatch_info* out);
uint32_t fc4_dispatch_get_count(const fc4_dispatch* d, uint32_t dispatch_type);
size_t fc4_dispatch_get_id_array(const fc4_dispatch* d, uint32_t dispatch_type, uint32_t n, uint32_t* out,
                                 size_t capacity);
/* Slots 13, 14, 18-22: the dispatch tool, the active pylon (0 = clear /
 * none), a pylon's vehicle, the free units, the vehicle view (borrowed). */
bool fc4_dispatch_begin_tool(fc4_dispatch* d, uint32_t dispatch_type);
bool fc4_dispatch_end_tool(fc4_dispatch* d, uint32_t dispatch_type);
void fc4_dispatch_set_active_pylon(fc4_dispatch* d, uint64_t pylon);
uint64_t fc4_dispatch_get_active_pylon(const fc4_dispatch* d);
uint64_t fc4_dispatch_get_automaton_from_pylon(const fc4_dispatch* d, uint64_t pylon);
uint32_t fc4_dispatch_get_available_pylon_count(const fc4_dispatch* d, uint32_t dispatch_type);
uint64_t fc4_dispatch_get_vehicle_view(const fc4_dispatch* d);
/* DoMessage (§3): 0x99EF1143 (object = the removed occupant), 0xAA1B2002,
 * 0xA6B79602, 0x09D92FC3 (object = the automaton, data2 = the code, data3 = the state). */
bool fc4_dispatch_do_message(fc4_dispatch* d, uint32_t msg_type, uint32_t data2, uint32_t data3, uint64_t object);
/* The records in list order and the available pylons (0 = a null entry):
 * return the count; copy at most `capacity` (`out` may be NULL). */
size_t fc4_dispatch_records(const fc4_dispatch* d, fc4_dispatch_info* out, size_t capacity);
size_t fc4_dispatch_available_pylons(const fc4_dispatch* d, uint64_t* out, size_t capacity);
bool fc4_dispatch_get_state(const fc4_dispatch* d, fc4_dispatch_state* out);
/* Persistence (§7). Read: false unless the version is 2 (`loaded` is set
 * first); references come back through resolve_ref. */
bool fc4_dispatch_write(const fc4_dispatch* d, const fc4_ostream* out);
bool fc4_dispatch_read(fc4_dispatch* d, const fc4_istream* in);

/* ---- city situation manager (docs/specs/city-situation.md) --------------------
 * cSC4CitySituationManager (0x0BB14381), the U-Drive-It missions. Tokens are
 * opaque host identities: advices, automata, targets, automata managers and
 * windows. The core holds one reference on the active advice, the held window
 * and the automata managers (acquire_automata_managers hands them over held);
 * everything else is borrowed. The mission gameplay (targets,
 * CheckForSuccess, pursuit, coverage, player drive, windows, effects) is the
 * host's. A missing callback reads as 0 / false / none, except
 * check_for_success, which reads as 3 (running). */
typedef struct fc4_citysit fc4_citysit;

/* An advice's Lua table as the core needs it (§6). */
typedef struct fc4_citysit_advice_info {
    uint32_t guid;
    uint32_t flags;        /* 8: remember a window; 0x40: a reward */
    uint32_t mission_type; /* 2: no target; 5: coverage; others: target missions */
    float end_delay;       /* seconds */
    int32_t success_money, failure_money;
    float success_radius, success_magnitude, failure_radius, failure_magnitude;
} fc4_citysit_advice_info;

#define FC4_CITYSIT_SCRIPT_START   0u
#define FC4_CITYSIT_SCRIPT_SUCCESS 1u
#define FC4_CITYSIT_SCRIPT_FAILURE 2u
/* The success popup also shows "could not start". */
#define FC4_CITYSIT_POPUP_BRIEFING 0u
#define FC4_CITYSIT_POPUP_SUCCESS  1u
#define FC4_CITYSIT_POPUP_FAILURE  2u

typedef struct fc4_citysit_host {
    uint32_t struct_size; /* sizeof(fc4_citysit_host) */
    void* user;
    /* Lifecycle (§4.1): the debug flag, the "csitrigger" cheat, the
     * subscriptions, the indicator view, the animation tick, the automata
     * managers (each with one reference for the core), the lot manager,
     * MySim, Lua (its table) and the mission resources; false = none. */
    bool (*debug_enabled)(void* user);
    bool (*register_cheat)(void* user);
    void (*unregister_cheat)(void* user);
    void (*subscribe)(void* user, uint32_t msg_type);
    void (*unsubscribe)(void* user, uint32_t msg_type);
    bool (*acquire_indicator_view)(void* user);
    void (*release_indicator_view)(void* user);
    void (*register_animation_tick)(void* user);
    void (*unregister_animation_tick)(void* user);
    void (*acquire_automata_managers)(void* user, fc4_token_callback cb, void* ctx);
    bool (*acquire_lot_manager)(void* user);
    void (*release_lot_manager)(void* user);
    bool (*acquire_mysim)(void* user);
    void (*release_mysim)(void* user);
    bool (*acquire_lua)(void* user);
    void (*release_lua)(void* user);
    void (*create_lua_table)(void* user);
    void (*remove_lua_table)(void* user);
    void (*release_mission_resources)(void* user);
    /* ShowAllIndicators(kind, visible); the preferences' reward list
     * (returns its length, copies at most `capacity`, `out` may be NULL),
     * one entry, saving the preferences. */
    void (*show_all_indicators)(void* user, uint32_t kind, bool visible);
    size_t (*preferences_rewards)(void* user, uint32_t* out, size_t capacity);
    void (*set_preferences_reward)(void* user, uint32_t index, uint32_t guid);
    void (*save_preferences)(void* user);
    /* AddRef / Release on a host object. */
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    /* Advices: QueryInterface(0x6BEB1E8A), advisor 29's advices, by GUID (0 =
     * none), the info (the core zeroes *out first), RetireAdvice, a script
     * (FC4_CITYSIT_SCRIPT_*), the time limit (seconds, 0 = none), binding the
     * mission script to the vehicle (0 = none), coverage cells (false = they
     * could not be built), the first target (0 = none), the target count, a
     * target's name (UTF-8, through the sink). */
    bool (*is_situation_advice)(void* user, uint64_t object);
    void (*advisor_advices)(void* user, fc4_token_callback cb, void* ctx);
    uint64_t (*find_advice)(void* user, uint32_t guid);
    void (*advice_info)(void* user, uint64_t advice, fc4_citysit_advice_info* out);
    void (*retire_advice)(void* user, uint64_t advice);
    void (*run_script)(void* user, uint64_t advice, uint32_t script);
    float (*time_limit)(void* user, uint64_t advice);
    void (*bind_mission_script)(void* user, uint64_t advice, uint64_t vehicle);
    bool (*build_coverage_cells)(void* user, uint64_t advice);
    uint64_t (*first_target)(void* user, uint64_t advice);
    uint32_t (*target_count)(void* user, uint64_t advice);
    void (*target_name)(void* user, uint64_t advice, uint64_t target, fc4_bytes_sink sink, void* ctx);
    /* Advice slot 53 (+0xD4): the advice accepts the automaton (slot 22). */
    bool (*advice_accepts_automaton)(void* user, uint64_t advice, uint64_t automaton);
    /* Automata: QueryInterface(0xA9B40F05), a property's presence, the
     * advice's candidate vehicles, flags, type, the indicator (TrackOccupant
     * kind 4: the handle), the listener, freezing a manager, a sound, the
     * position (x, z). */
    bool (*is_automaton)(void* user, uint64_t object);
    bool (*occupant_has_property)(void* user, uint64_t object, uint32_t id);
    void (*candidate_vehicles)(void* user, uint64_t advice, fc4_token_callback cb, void* ctx);
    uint32_t (*automaton_flags)(void* user, uint64_t automaton);
    uint32_t (*automaton_type)(void* user, uint64_t automaton);
    uint32_t (*track_indicator)(void* user, uint64_t automaton);
    void (*untrack_indicator)(void* user, uint32_t handle);
    void (*add_listener)(void* user, uint64_t automaton);
    void (*remove_listener)(void* user, uint64_t automaton);
    void (*freeze_automata)(void* user, uint64_t manager, bool on);
    void (*play_sound)(void* user, uint64_t automaton, uint32_t sound);
    void (*position)(void* user, uint64_t object, float* x, float* z);
    /* The mission (out of the spec's scope): steps 2-4 of CheckForSuccess(t)
     * (§4.4) from the current status (the new status; the core runs the
     * mission clock), pursuit, the countdown effect, the outcome effect at
     * (x, z), the player-drive mode (vehicle 0 = none), the joyride. */
    uint32_t (*check_for_success)(void* user, uint64_t advice, float t_ms, uint32_t status);
    void (*run_pursuit)(void* user, uint64_t advice, float t_ms);
    void (*start_countdown_effect)(void* user);
    void (*end_countdown_effect)(void* user);
    void (*outcome_effect)(void* user, uint64_t advice, bool success, float x, float z);
    void (*player_drive_mode)(void* user, uint64_t vehicle, bool on);
    void (*start_joyride)(void* user, uint64_t vehicle);
    /* aura.AddTransientEffect (aura.md §8.2); budget.DepositFunds (budget.md §6). */
    void (*add_transient_effect)(void* user, float x, float z, float radius, float magnitude);
    void (*deposit_funds)(void* user, int64_t amount);
    /* Messages: MessageSend 0xABBBCA64 (data1 = active; advice 0 = none, a
     * joyride); MessagePost of 0xEBB67EF7 to itself (advice, vehicle; 0 =
     * none), 0x6BB350E4, 0xAC1045EF. */
    void (*send_activation)(void* user, uint64_t advice, bool active);
    void (*post_show_popup)(void* user, uint64_t advice, uint64_t vehicle);
    void (*post_popup_closed)(void* user, uint64_t advice);
    void (*post_initiate)(void* user, uint32_t guid, bool show_briefing);
    /* The popup (§4.3): HideMissionStatusPanel, end an open MySim dialog, show
     * the popup (FC4_CITYSIT_POPUP_*) modally: the button that closed it (0 = none). */
    void (*hide_mission_status_panel)(void* user);
    void (*end_mysim_dialog)(void* user);
    uint32_t (*show_popup)(void* user, uint32_t popup, uint64_t advice, uint64_t vehicle);
    /* Appended with re-answers15 (CS4-CS11): the next target of a disabled
     * target (§4.7; 0 = none left), advice slot 54 (accepts the group, §8
     * slot 23), advice slot 12 (the groups: returns the count, copies at most
     * `capacity`, `out` may be NULL), the track-list priority (cISC4Advice
     * slot 7, §4.2), a group's automata (borrowed),
     * CreateAutomatonForPlayerDrive (advice 0 = none; held for the core, 0 =
     * none), automaton Remove(0), SetFlag(flag, 1), the group of slot 25,
     * EnterPlayerDriveView (target 0 = none, §8 slot 24), DeactivateSituation
     * steps 1-2 (§4.3.1), EndSituation step 1 (the +0xA9C effect), a MySim
     * window for advice flag 0x20 (held for the core, 0 = none). */
    uint64_t (*next_target)(void* user, uint64_t advice);
    bool (*advice_accepts_group)(void* user, uint64_t advice, uint32_t group);
    size_t (*advice_groups)(void* user, uint64_t advice, uint32_t* out, size_t capacity);
    uint32_t (*advice_priority)(void* user, uint64_t advice);
    void (*group_automata)(void* user, uint32_t group, fc4_token_callback cb, void* ctx);
    uint64_t (*create_automaton_for_player_drive)(void* user, uint64_t advice, uint32_t group);
    void (*remove_automaton)(void* user, uint64_t automaton);
    void (*set_automaton_flag)(void* user, uint64_t automaton, uint32_t flag);
    void (*set_automaton_group)(void* user, uint64_t automaton, uint32_t group);
    void (*enter_player_drive_view)(void* user, uint64_t automaton, uint64_t target);
    void (*clear_mission_state)(void* user, uint64_t advice);
    void (*release_situation_effect)(void* user);
    uint64_t (*acquire_mysim_window)(void* user);
} fc4_citysit_host;

typedef struct fc4_citysit_track_entry {
    uint64_t advice;
    uint64_t automaton; /* 0 = none */
    uint32_t handle;    /* the indicator (0 = none) */
    uint32_t reserved;
} fc4_citysit_track_entry;

typedef struct fc4_citysit_success_count {
    uint32_t guid;
    uint16_t count;
    uint16_t reserved;
} fc4_citysit_success_count;

typedef struct fc4_citysit_state {
    uint32_t struct_size;
    uint8_t initialized, destroyed, indicators_visible, loaded_from_save;
    uint8_t just_activated, warning_given, max_csi, cheat_registered;
    uint8_t indicator_view_held, lot_manager_held, mysim_held, lua_held;
    uint32_t ref_count, status, num_targets, target_index;
    float end_delay, elapsed, countdown, time_limit;
    uint32_t track_entries, tracked, success_counts, automata_managers;
    uint64_t active_situation, active_automaton, current_target, window;
} fc4_citysit_state;

/* Constructed, reference count 0. The host table is copied (NULL for a NULL or
 * short table). `session_seed` seeds the member generator (§5:
 * derive(0x0BB14381)); `token` is the manager's own token. */
fc4_citysit* fc4_citysit_create(const fc4_citysit_host* host, uint32_t session_seed, uint64_t token);
void fc4_citysit_destroy(fc4_citysit* m);
/* QueryInterface (§1): true and the sub-object offset (`offset` may be NULL). */
bool fc4_citysit_query_interface(uint32_t iid, uint32_t* offset);
uint32_t fc4_citysit_add_ref(fc4_citysit* m);
/* Release: 0 when the object was destroyed (Shutdown ran); the handle stays valid until fc4_citysit_destroy. */
uint32_t fc4_citysit_release(fc4_citysit* m);
bool fc4_citysit_init(fc4_citysit* m);
bool fc4_citysit_shutdown(fc4_citysit* m);
/* DoMessage (§3): object = IGZUnknown (an advice, a window) or the occupant /
 * automaton; void1 = the vehicle of 0xEBB67EF7; text / text_len = the cheat's
 * text of 0x230E27AC (UTF-8, NULL = none); the animation tick 0x86AD10EE
 * carries the milliseconds as float bits in data3. Always true. */
bool fc4_citysit_do_message(fc4_citysit* m, uint32_t msg_type, uint32_t data1, uint32_t data2, uint32_t data3,
                            uint64_t object, uint64_t void1, const uint8_t* text, size_t text_len);
/* The animation tick (§4.4). */
void fc4_citysit_tick(fc4_citysit* m, float t_ms);
/* Situations triggered / untriggered (§4.2). */
void fc4_citysit_situation_triggered(fc4_citysit* m, uint64_t advice);
void fc4_citysit_situation_untriggered(fc4_citysit* m, uint64_t advice);
/* Slots 36, 21-25, 37-39 (§4.3, §8): initiate (advice / vehicle 0 = none;
 * false when nothing started), by GUID (group 0 = the advice's groups), for
 * an occupant (false unless an automaton; `advice` 0 = the first track-list
 * entry whose vehicle it is or whose advice accepts it), by automata group,
 * StartJoyride (target 0 = none), TreatAutomatonAsGroup (automaton 0 does
 * nothing), activate, deactivate, end. check_for_success: CheckForSuccess(t)
 * (§4.4; 4 without a mission or an active automaton). */
bool fc4_citysit_initiate_situation(fc4_citysit* m, uint64_t advice, uint64_t vehicle, bool show_briefing);
bool fc4_citysit_initiate_situation_by_guid(fc4_citysit* m, uint32_t guid, bool show_briefing, uint32_t group);
bool fc4_citysit_initiate_situation_for_occupant(fc4_citysit* m, uint64_t occupant, uint64_t advice);
bool fc4_citysit_initiate_situation_by_automata_group(fc4_citysit* m, uint32_t group, bool show_briefing, bool search);
void fc4_citysit_start_joyride(fc4_citysit* m, uint64_t automaton, bool b, uint64_t target);
void fc4_citysit_treat_automaton_as_group(fc4_citysit* m, uint64_t automaton, uint32_t group);
uint32_t fc4_citysit_check_for_success(fc4_citysit* m, float t_ms);
void fc4_citysit_activate_situation(fc4_citysit* m);
void fc4_citysit_deactivate_situation(fc4_citysit* m);
void fc4_citysit_end_situation(fc4_citysit* m);
/* The popup (§4.3) and the outcome (§4.5). */
void fc4_citysit_show_situation_popup(fc4_citysit* m, uint64_t advice, uint64_t vehicle);
void fc4_citysit_success(fc4_citysit* m, uint64_t advice);
void fc4_citysit_failure(fc4_citysit* m, uint64_t advice);
/* Slot 5 / Lua get_success_count; Lua activate_situation. */
uint16_t fc4_citysit_get_success_count(const fc4_citysit* m, uint32_t guid);
bool fc4_citysit_lua_activate_situation(fc4_citysit* m, uint32_t guid, bool show_briefing);
/* Slots 6-12 (tokens borrowed, 0 = none). */
uint64_t fc4_citysit_active_situation(const fc4_citysit* m);
uint32_t fc4_citysit_current_status(const fc4_citysit* m);
uint64_t fc4_citysit_active_auto(const fc4_citysit* m);
uint64_t fc4_citysit_current_target(const fc4_citysit* m);
uint32_t fc4_citysit_current_target_index(const fc4_citysit* m);
uint32_t fc4_citysit_num_targets(const fc4_citysit* m);
uint32_t fc4_citysit_num_targets_remaining(const fc4_citysit* m);
/* Slots 13-15 return float (the shim returns them in ST0, §12 item 8):
 * the time limit, the elapsed time, the remaining time, in seconds. */
float fc4_citysit_time_limit_seconds(const fc4_citysit* m);
float fc4_citysit_situation_time(const fc4_citysit* m);
float fc4_citysit_time_remaining_seconds(const fc4_citysit* m);
/* Slot 16: the current target's name (UTF-8) through the sink. */
bool fc4_citysit_current_target_name(const fc4_citysit* m, fc4_bytes_sink sink, void* ctx);
/* Slots 17, 18: the active advice's success aura / money (false without a mission). */
bool fc4_citysit_situation_aura(const fc4_citysit* m, float* radius, float* magnitude);
bool fc4_citysit_situation_money(const fc4_citysit* m, int64_t* money);
/* Slots 19, 20, 26-33, 35. */
bool fc4_citysit_is_situation_available(const fc4_citysit* m, uint64_t automaton);
void fc4_citysit_set_max_csi(fc4_citysit* m, uint8_t n);
uint8_t fc4_citysit_max_csi(const fc4_citysit* m);
uint32_t fc4_citysit_tracked_automata_count(const fc4_citysit* m);
void fc4_citysit_set_csi_visible(fc4_citysit* m, bool on);
bool fc4_citysit_csi_visible(const fc4_citysit* m);
bool fc4_citysit_handle_tuning_parameters_changed(fc4_citysit* m);
bool fc4_citysit_countdown_in_progress(const fc4_citysit* m);
void fc4_citysit_reset_tutorials(fc4_citysit* m);
void fc4_citysit_initiate_player_drive_mode(fc4_citysit* m, bool on);
/* The track list and the success counts (ascending GUID): return the count;
 * copy at most `capacity` (`out` may be NULL). */
size_t fc4_citysit_track_list(const fc4_citysit* m, fc4_citysit_track_entry* out, size_t capacity);
size_t fc4_citysit_success_counts(const fc4_citysit* m, fc4_citysit_success_count* out, size_t capacity);
bool fc4_citysit_get_state(const fc4_citysit* m, fc4_citysit_state* out);
/* Persistence (§7): version 2 (version 1 is read). */
bool fc4_citysit_write(const fc4_citysit* m, const fc4_ostream* out);
bool fc4_citysit_read(fc4_citysit* m, const fc4_istream* in);
/* ---- disaster layer (docs/specs/disasters.md) ----------------------------------
 * cSC4DisasterLayer (0x61F6ABF5), its seven managers (fire, riot, toxic
 * spill, earthquake, pipe burst, volcano, effect) and their instances. The
 * shim owns the GZCOM objects and forwards their slots here. Managers are
 * named by their type (GetDisasterType: fire 0x621CDA33, riot 0xCA380B06,
 * toxic 0xEBFB0404, earthquake 0x42963812, pipe burst 0x4A1332E8, volcano
 * 0xC4C90997, effect 0x6A7B4397); instances by the core's IDs (from 1, never
 * reused). The fire runs on the wall clock (now_ms); fc4_disasters_tick
 * passes the animation tick of an instance the core registered through
 * register_tick. Tokens are opaque host identities (occupants, lots, fire and
 * toxic occupants, riot leaders, pipes, effects; 0 = none). The core holds a
 * reference (add_ref) on every occupant it keeps in a location or the fire's
 * toast list. A missing callback reads as 0 / false / none (cell_width: 16).
 * A call made from inside a callback is deferred until the outer call
 * returns when it is a message (fc4_disasters_do_message) and refused (the
 * fallback value) otherwise. Nothing is saved (§14). */
typedef struct fc4_disasters fc4_disasters;

/* Message targets (subscribe / unsubscribe / fc4_disasters_do_message). */
#define FC4_DISASTER_TARGET_LAYER 0u
#define FC4_DISASTER_TARGET_MANAGER 1u  /* id = the manager type */
#define FC4_DISASTER_TARGET_INSTANCE 2u /* id = the instance */

/* A message the core posts (§12). */
typedef struct fc4_disaster_post {
    uint32_t msg_type;
    uint32_t data2;
    uint64_t data1; /* may carry a token */
    uint32_t data3;
    uint32_t data4;
    uint32_t void1; /* the instance of 0xCA1B1E64 (0 = none) */
    uint32_t reserved;
} fc4_disaster_post;

/* A message to the layer, a manager or an instance. */
typedef struct fc4_disaster_message {
    uint32_t msg_type;
    uint32_t reserved;
    int64_t data1, data2, data3;
    uint64_t void1; /* GetVoid1() as a token (the removed occupant; the effect) */
    float x, z;     /* world units (the effect moved) */
} fc4_disaster_message;

typedef struct fc4_world_box {
    float x0, z0, x1, z1;
} fc4_world_box;

typedef struct fc4_disasters_host {
    uint32_t struct_size; /* sizeof(fc4_disasters_host) */
    void* user;
    /* The wall clock (ms), the emergency pause and the save switch (nested by the core). */
    uint64_t (*now_ms)(void* user);
    void (*emergency_pause)(void* user);
    void (*emergency_resume)(void* user);
    void (*disable_save)(void* user);
    void (*enable_save)(void* user);
    bool (*sim_paused)(void* user);           /* any pause */
    bool (*sim_emergency_paused)(void* user); /* the emergency pause */
    bool (*city_established)(void* user);
    void (*city_cells)(void* user, uint32_t* x, uint32_t* z);
    float (*cell_width)(void* user);
    /* Messages, ticks (priority 0xC0, the toxic spill 0xFF), history, references. */
    void (*post)(void* user, const fc4_disaster_post* post);
    void (*subscribe)(void* user, uint32_t target_kind, uint32_t id, uint32_t msg_type);
    void (*unsubscribe)(void* user, uint32_t target_kind, uint32_t id, uint32_t msg_type);
    void (*register_tick)(void* user, uint32_t instance, uint32_t priority);
    void (*unregister_tick)(void* user, uint32_t instance);
    void (*record_history)(void* user, uint32_t series, int32_t value);
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    /* The camera; fire protection's (slot 25) and police's (slot 26) GetTotalDispatchCount, the
     * dispatch effectiveness divisor (§2.5). */
    bool (*follow_disasters)(void* user);
    void (*go_to_location)(void* user, int32_t x, int32_t z, uint32_t zoom);
    uint32_t (*fire_dispatch_count)(void* user);
    uint32_t (*police_dispatch_count)(void* user);
    /* Occupants. occupant_cell: false once it has left the occupant manager. A fire occupant's
     * cell rectangle and box are its own (§5.7, §5.10). */
    uint32_t (*building_type_flags)(void* user, uint64_t occ);
    uint32_t (*occupant_type)(void* user, uint64_t occ);
    bool (*occupant_in_manager)(void* user, uint64_t occ);
    bool (*occupant_cell)(void* user, uint64_t occ, int32_t* x, int32_t* z);
    void (*occupant_cell_rect)(void* user, uint64_t occ, fc4_cell_rect* out);
    void (*occupant_position)(void* user, uint64_t occ, float* x, float* z);
    void (*occupant_bbox)(void* user, uint64_t occ, fc4_world_box* out);
    void (*occupant_bbox_areas)(void* user, uint64_t occ, float* top, float* side);
    /* Occupant properties; false = absent. */
    bool (*occupant_prop_u32)(void* user, uint64_t occ, uint32_t id, uint32_t* out);
    bool (*occupant_prop_i64)(void* user, uint64_t occ, uint32_t id, int64_t* out);
    bool (*occupant_prop_f32)(void* user, uint64_t occ, uint32_t id, float* out);
    bool (*occupant_prop_bool)(void* user, uint64_t occ, uint32_t id, bool* out);
    void (*occupants_in_rect)(void* user, const fc4_cell_rect* rect, fc4_token_callback cb, void* ctx);
    void (*remove_occupant)(void* user, uint64_t occ);
    uint64_t (*occupant_lot)(void* user, uint64_t occ);
    void (*set_lot_condition)(void* user, uint64_t lot, uint32_t condition);
    /* Fire (§5). */
    uint8_t (*fire_coverage)(void* user, uint64_t occ);
    uint8_t (*flammability_at)(void* user, int32_t x, int32_t z);
    uint64_t (*create_fire_occupant)(void* user, uint32_t instance, uint64_t target, uint32_t fire_id,
                                     uint32_t top_flames, uint32_t side_flames);
    void (*remove_fire_occupant)(void* user, uint64_t fire, uint64_t target);
    void (*add_fire_aura)(void* user, uint64_t occ);
    bool (*tutorial_no_burn_down)(void* user);
    void (*wind)(void* user, float* x, float* z); /* weather slot 22: the wind vector */
    void (*play_effect)(void* user, const char* name, uint64_t occ);
    void (*play_effect_at)(void* user, const char* name, float x, float z);
    void (*burn_flora)(void* user, uint64_t occ);
    void (*demolish_lot)(void* user, uint64_t occ, uint64_t lot, uint32_t flags);
    void (*spew_radiation)(void* user, uint64_t occ, float amount, float radius);
    /* Riot (§6). */
    uint8_t (*police_coverage)(void* user, int32_t x, int32_t z);
    bool (*is_road_cell)(void* user, int32_t x, int32_t z);
    void (*cell_corner_position)(void* user, int32_t x, int32_t z, float* wx, float* wz);
    uint64_t (*create_riot_leader)(void* user, float x, float z);
    /* The riot's removal lock (network-manager.md §10): the network occupant at a cell
     * (0 = none) and its removal flags (2 = the player's bulldozer). */
    uint64_t (*network_occupant_at)(void* user, int32_t x, int32_t z);
    void (*unset_removal_flags)(void* user, uint64_t occ, uint32_t flags);
    void (*set_removal_flags)(void* user, uint64_t occ, uint32_t flags);
    bool (*automaton_cell)(void* user, uint64_t automaton, int32_t* x, int32_t* z);
    void (*set_riot_effect)(void* user, uint64_t automaton, int32_t stage);
    void (*send_riot_message)(void* user, uint64_t automaton, int32_t stage);
    void (*end_riot_leader)(void* user, uint64_t automaton);
    void (*riot_leader_tick)(void* user, uint64_t automaton);
    /* Toxic spill (§9). */
    uint64_t (*create_toxic_occupant)(void* user, uint32_t instance, int32_t x, int32_t z, int32_t amount);
    void (*set_toxic_amount)(void* user, uint64_t occ, int32_t amount);
    void (*remove_toxic_occupant)(void* user, uint64_t occ);
    /* Terrain (earthquake, §7). */
    void (*apply_terrain_brush)(void* user, uint32_t brush, float x, float z, float strength, float width);
    void (*redraw_terrain)(void* user, const fc4_cell_rect* rect);
    void (*end_long_terrain_operation)(void* user);
    void (*stop_terrain_texture_updates)(void* user);
    void (*place_repulsor)(void* user, float x, float z);
    /* Pipe burst (§8). connected_pipe_cells writes up to `capacity` (x, z) pairs and
     * returns how many there are (the core asks again with room for all). */
    uint64_t (*find_watered_pipe)(void* user, int32_t x, int32_t z);
    void (*pipe_cell)(void* user, uint64_t pipe, int32_t* x, int32_t* z);
    uint32_t (*view_level)(void* user);
    bool (*create_leak_effect)(void* user, uint64_t pipe, uint32_t view_level);
    size_t (*connected_pipe_cells)(void* user, uint64_t pipe, int32_t* xz, size_t capacity);
    void (*demolish_pipe)(void* user, uint64_t pipe, uint32_t flags);
    /* Volcano (§10): the "volcano_control" effect (its completion is delivered as 0xCA493C1D with
     * data1 = the instance) and the lava terrain class are the host's. */
    void (*volcano_start_deformation)(void* user, uint32_t instance, float x, float z);
    void (*volcano_start_lava)(void* user, uint32_t instance, float x, float z, float radius);
    void (*volcano_lava_rates)(void* user, uint32_t instance, float production, float solidification);
    void (*volcano_end_lava)(void* user, uint32_t instance);
} fc4_disasters_host;

typedef struct fc4_disasters_state {
    uint32_t struct_size;
    uint8_t initialised, enabled, assessing, reserved;
    uint32_t disable_counter;
    uint32_t ongoing;
    uint32_t location_count;
    uint32_t fire_instance; /* 0 = none yet */
    uint32_t fire_phase;
    uint32_t fire_count;
    uint32_t toxic_instance; /* 0 = none */
    uint32_t toxic_spills;
    uint32_t reentrant_refused;
    uint32_t fire_timers, police_timers;
    float fire_effectiveness, police_effectiveness;
    uint32_t damaged_buildings;
    uint32_t riot_locked; /* network occupants the riots hold locked */
    int64_t disaster_cost;
} fc4_disasters_state;

typedef struct fc4_disaster_fire {
    uint64_t occupant; /* the fire occupant */
    uint64_t target;
    int32_t counter;
    uint32_t stage, max_stage, fire_id, hosed, top_flames, side_flames;
    uint32_t extinguishing;  /* 1: extinguish_at is valid */
    uint64_t extinguish_at;  /* ms on the fire instance's timer */
} fc4_disaster_fire;

typedef struct fc4_disaster_instance {
    uint32_t manager_type, class_id, phase;
    uint8_t tick_registered, has_location;
    uint16_t reserved;
    int32_t x, z; /* GetLocation (has_location) */
} fc4_disaster_instance;

typedef struct fc4_riot_leader {
    uint64_t automaton;
    int32_t counter, stage, max_stage, x, z;
    uint32_t reserved;
} fc4_riot_leader;

/* The host table is copied; NULL if it is NULL or too small. */
fc4_disasters* fc4_disasters_create(const fc4_disasters_host* host, uint32_t session_seed);
void fc4_disasters_destroy(fc4_disasters* d);
/* The tuning exemplar of `instance` (group 0x8A5971C5: 1 volcano, 2 earthquake, 3 pipe burst,
 * 0xC9B6B73D flame, 0x4A38206D riot); copied, NULL removes it. Read once, at first use. */
bool fc4_disasters_set_tuning(fc4_disasters* d, uint32_t instance, const fc4_props* tuning);
bool fc4_disasters_init(fc4_disasters* d);
bool fc4_disasters_shutdown(fc4_disasters* d);
/* EnableDisasters (nested; returns the state after the call) / DisastersEnabled. */
bool fc4_disasters_enable(fc4_disasters* d, bool enable);
bool fc4_disasters_enabled(const fc4_disasters* d);
bool fc4_disasters_has_manager(const fc4_disasters* d, uint32_t manager_type);
void fc4_disasters_has_begun(fc4_disasters* d, uint32_t instance);
void fc4_disasters_has_ended(fc4_disasters* d);
bool fc4_disasters_is_ongoing(const fc4_disasters* d);
/* Locations (§2.6); occupant 0 = a cell location; instance 0 = any. */
void fc4_disasters_register_location(fc4_disasters* d, uint32_t instance, int32_t x, int32_t z, uint64_t occupant,
                                     uint32_t zoom, bool focus);
void fc4_disasters_unregister_locations(fc4_disasters* d, uint32_t instance);
void fc4_disasters_unregister_location_by_occupant(fc4_disasters* d, uint32_t instance, uint64_t occupant);
bool fc4_disasters_most_recent_location(const fc4_disasters* d, uint32_t instance, int32_t* x, int32_t* z);
size_t fc4_disasters_location_count(const fc4_disasters* d);
void fc4_disasters_go_to_next(fc4_disasters* d);
int64_t fc4_disasters_cost(const fc4_disasters* d);
void fc4_disasters_reset_cost(fc4_disasters* d);
/* DoMessage on a target (FC4_DISASTER_TARGET_*); false for an unknown kind or NULL. */
bool fc4_disasters_do_message(fc4_disasters* d, uint32_t target_kind, uint32_t id, const fc4_disaster_message* m);
void fc4_disasters_tick(fc4_disasters* d, uint32_t instance, uint32_t us);
/* AttemptDisasterStart(x, z) / AttemptDisasterStart(): the instance, 0 = none (always 0 for the
 * fire and the riot, §5.5, §6.2; a new riot is the last of fc4_disasters_manager_instances). */
uint32_t fc4_disasters_attempt_start(fc4_disasters* d, uint32_t manager_type, int32_t x, int32_t z);
uint32_t fc4_disasters_attempt_start_any(fc4_disasters* d, uint32_t manager_type);
uint32_t fc4_disasters_active_count(const fc4_disasters* d, uint32_t manager_type);
/* The instance's Shutdown (§3.1): the fire, riot, pipe burst, toxic spill and volcano leave their
 * manager; the earthquake and the effect stay in its list. */
void fc4_disasters_instance_shutdown(fc4_disasters* d, uint32_t instance);
/* Fire (§5). fire_instance creates it at first use. */
uint32_t fc4_disasters_fire_instance(const fc4_disasters* d);
bool fc4_disasters_ignite_occupant(fc4_disasters* d, uint64_t occupant);
bool fc4_disasters_ignite(fc4_disasters* d, uint64_t occupant, bool force, uint32_t fire_id, bool focus);
bool fc4_disasters_ignite_cell(fc4_disasters* d, int32_t x, int32_t z, bool use_probability, bool teleport,
                               uint32_t fire_id);
bool fc4_disasters_manager_ignite_cell(fc4_disasters* d, int32_t x, int32_t z, bool use_probability, bool teleport,
                                       uint32_t fire_id);
void fc4_disasters_extinguish_cell(fc4_disasters* d, int32_t x, int32_t z, int32_t amount);
void fc4_disasters_quick_burn_cell(fc4_disasters* d, int32_t x, int32_t z);
void fc4_disasters_ignite_cell_radius(fc4_disasters* d, int32_t x, int32_t z, int32_t r0, int32_t r1);
void fc4_disasters_extinguish(fc4_disasters* d, uint64_t occupant);
void fc4_disasters_burn_down(fc4_disasters* d, uint64_t occupant);
/* SetCause: only while the cause is 0 (the first of an episode wins); returns the cause. */
uint32_t fc4_disasters_set_cause(fc4_disasters* d, uint32_t cause);
uint32_t fc4_disasters_cause(const fc4_disasters* d);
uint32_t fc4_disasters_fire_count(const fc4_disasters* d);
uint32_t fc4_disasters_fire_count_in_rect(const fc4_disasters* d, const fc4_cell_rect* rect);
void fc4_disasters_fire_update(fc4_disasters* d);
bool fc4_disasters_fire_set_hosed(fc4_disasters* d, uint64_t fire_occupant, int32_t delta);
/* Toxic spill (§9), volcano (§10), effect disasters (§11). */
uint32_t fc4_disasters_spill_toxic_waste(fc4_disasters* d, int32_t x, int32_t z, int32_t amount);
bool fc4_disasters_toxic_set_hosed(fc4_disasters* d, uint64_t toxic_occupant, int32_t delta);
bool fc4_disasters_volcano_request_end(fc4_disasters* d, uint32_t instance);
uint32_t fc4_disasters_start_effect(fc4_disasters* d, uint64_t effect, float x, float z);
/* Diagnostics. */
bool fc4_disasters_get_state(const fc4_disasters* d, fc4_disasters_state* out);
bool fc4_disasters_get_fire(const fc4_disasters* d, size_t index, fc4_disaster_fire* out);
bool fc4_disasters_get_instance(const fc4_disasters* d, uint32_t instance, fc4_disaster_instance* out);
/* Returns the number of leaders (0 for no such riot); copies at most `capacity`. */
size_t fc4_disasters_riot_leaders(const fc4_disasters* d, uint32_t instance, fc4_riot_leader* out, size_t capacity);
/* Mirror tooling (67; the plugin's disasters_shadow, not spec behaviour). set_mirror_random: the
 * fire, riot, earthquake and pipe-burst generators become deterministic (new fire IDs count from
 * 0x80000001; uniform_u32 0; uniform 1.0; range(a, b) b, so a fire never spreads by itself;
 * gaussian_fast(a, b) a): a core that mirrors the original makes no random decision of its own. */
bool fc4_disasters_set_mirror_random(fc4_disasters* d);
typedef struct fc4_disaster_location {
    uint32_t instance, zoom;
    int32_t x, z;      /* an occupant entry's resolved as GetDisasterLocationByIndex reads it */
    uint64_t occupant; /* 0 = a cell entry */
} fc4_disaster_location;
bool fc4_disasters_get_location(const fc4_disasters* d, size_t index, fc4_disaster_location* out);
/* The riots' locked network occupants (locking order) / the fire-ID set (ascending, the fire
 * instance not created). Return the count; copy at most `capacity` (`out` may be NULL). */
size_t fc4_disasters_riot_locked(const fc4_disasters* d, uint64_t* out, size_t capacity);
size_t fc4_disasters_fire_ids(const fc4_disasters* d, uint32_t* out, size_t capacity);
/* A manager's instance list (GetInstances, slot 11), in order (69). Returns the count; copies at
 * most `capacity` (`out` may be NULL). */
size_t fc4_disasters_manager_instances(const fc4_disasters* d, uint32_t manager_type, uint32_t* out,
                                       size_t capacity);

/* ---- prop manager (docs/specs/prop-lotconfig.md Part A) ----------------------
 * cSC4PropManager (0xA990BD10, city component 5). The shim owns the GZCOM
 * objects and forwards their slots here. Props, buildings, lots, the occupant
 * manager, the lot manager, the resource manager, the view filter and text
 * decals are host objects named by tokens (0 = none). The catalogue (§A2) is
 * process-wide: the host creates one fc4_prop_catalogue per process and hands
 * it to every manager it creates; the first Init that finds it unbuilt builds
 * it (StaticInit). Lists (§A7.2): the query calls take the caller's list
 * (in, n_in), may filter what was there before (quirk 7), and hand the
 * resulting list to a token callback element by element; a token handed back
 * that was not in `in` carries one reference for the caller, a token of `in`
 * that was dropped was released. A missing callback reads as 0 / false /
 * none. */
typedef struct fc4_prop_catalogue fc4_prop_catalogue;
typedef struct fc4_prop_manager fc4_prop_manager;

/* fc4_prop_manager_host.acquire_service services. */
#define FC4_PROP_SERVICE_OCCUPANT_MANAGER 0u
#define FC4_PROP_SERVICE_RESOURCE_MANAGER 1u
#define FC4_PROP_SERVICE_LOT_MANAGER      2u
/* fc4_prop_manager_host.create_builtin_prop kinds (§A6.1 step 3). */
#define FC4_PROP_BUILTIN_PLAIN         0u /* 0x68 bytes, any class but the two below */
#define FC4_PROP_BUILTIN_TERRAIN_DECAL 1u /* 0x2977AA48, 0xA0 bytes */
#define FC4_PROP_BUILTIN_VISUAL_EFFECT 2u /* 0x2977AA49, 0x94 bytes */
/* fc4_prop_manager_host.building_props results (§A7.1 step 3). */
#define FC4_PROP_BUILDING_NO_HOLDER 0u /* the building has no property holder */
#define FC4_PROP_BUILDING_ABSENT    1u /* no property 0xA8322B7F */
#define FC4_PROP_BUILDING_PRESENT   2u /* the vector went to the callback */

typedef struct fc4_prop_manager_host {
    uint32_t struct_size; /* sizeof(fc4_prop_manager_host) */
    void* user;
    /* ---- the catalogue (§A3) ----
     * The resource manager's keys of type 0x6534284A (slot 18) in its order;
     * false when there is no resource manager. */
    bool (*for_each_exemplar_key)(void* user, fc4_key_visitor visit, void* ctx);
    /* An exemplar through the global resource manager (slot 3, IID 0xA52160F5):
     * a NEW fc4_props the core destroys, NULL when it does not load. */
    fc4_props* (*load_exemplar)(void* user, uint32_t type_id, uint32_t group, uint32_t instance);
    /* Application slot 69: the catalogue's shutdown callback (call
     * fc4_prop_catalogue_app_shutdown from it). */
    void (*register_shutdown_callback)(void* user);
    /* Posts kSC4MessagePropExemplarDataUpdated (0x896FDC70) when the message
     * server exists: void1 and the IGZUnknown = the manager, data2 = data2. */
    void (*post_exemplar_data_updated)(void* user, uint64_t manager, uint32_t data2);
    /* ---- lifecycle (§A4) ---- 0x00B43CB0 slots 6 / 9; message server. */
    void (*register_command)(void* user, uint32_t id);
    void (*unregister_command)(void* user, uint32_t id);
    void (*subscribe)(void* user, uint32_t msg_type);
    void (*unsubscribe)(void* user, uint32_t msg_type);
    /* The service's global (FC4_PROP_SERVICE_*) with a new reference; 0 = null. */
    uint64_t (*acquire_service)(void* user, uint32_t service);
    /* The 3D view's view filter (slot 18) with a reference; 0 = null. */
    uint64_t (*view_filter)(void* user);
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    /* ---- creating props (§A6) ----
     * GZCOM: an instance of class_id with IID 0xE9793A65, one reference; 0 when
     * no such class exists. */
    uint64_t (*create_prop_instance)(void* user, uint32_t class_id);
    /* One of the built-in classes (FC4_PROP_BUILTIN_*), one reference. */
    uint64_t (*create_builtin_prop)(void* user, uint32_t kind);
    /* Prop slot 8 SetPropType. */
    void (*set_prop_type)(void* user, uint64_t prop, uint32_t id);
    /* §A6.1 step 5: through the prop's property holder ask for 0x0AC2B5F7,
     * load the exemplar at the key (the manager's resource manager, or the
     * global one when `global`) and hand it over (slot 3). Returns the
     * exemplar's properties as a NEW fc4_props (the core destroys it), NULL
     * when a piece is missing. */
    fc4_props* (*attach_exemplar)(void* user, uint64_t prop, uint32_t type_id, uint32_t group, uint32_t instance,
                                  bool global);
    /* Prop slot 4 SetBoundingBox(min3, max3). */
    void (*set_bounding_box)(void* user, uint64_t prop, const float* min3, const float* max3);
    /* ---- occupants (§A7) ---- IsFlagSet, slot 3 Init, slot 4 Shutdown,
     * QueryInterface(0xE9793A65) + SetPropUsage, slot 9 SetPosition, prop slot
     * 10 SetOrientation, slot 8 GetPosition, and the flag-2 part of removing a
     * prop (placeable object slot 11, clear flag 2). */
    bool (*occupant_flag)(void* user, uint64_t occ, uint32_t flag);
    bool (*occupant_init)(void* user, uint64_t occ);
    void (*occupant_shutdown)(void* user, uint64_t occ);
    void (*set_prop_usage)(void* user, uint64_t occ, uint32_t usage);
    void (*set_position)(void* user, uint64_t occ, const float* pos3);
    void (*set_orientation)(void* user, uint64_t occ, int32_t orientation);
    void (*position)(void* user, uint64_t occ, float* pos3);
    void (*hide)(void* user, uint64_t occ);
    /* ---- the occupant manager ---- slot 20 InsertOccupant(occ, false) (false
     * without an occupant manager); slot 21 RemoveOccupant(occ, 0, 0). */
    bool (*insert_occupant)(void* user, uint64_t occ);
    void (*remove_occupant)(void* user, uint64_t occ);
    /* Slot 36 (box) / slot 40 (cell rectangle) with the filter at `usage`, mask
     * 0xFFFFFFFF: each prop found to `found`, one reference each for the core;
     * false when the call returns false. */
    bool (*occupants_in_box)(void* user, const float* min3, const float* max3, uint32_t usage,
                             fc4_token_callback found, void* ctx);
    bool (*occupants_in_cell_rect)(void* user, const fc4_cell_rect* rect, uint32_t usage,
                                   fc4_token_callback found, void* ctx);
    /* Slot 33 with type 0x278128A0: the building at the cell, with a reference; 0 = none. */
    uint64_t (*building_at_cell)(void* user, int32_t x, int32_t z);
    /* ---- lots and the city ---- lot manager slot 6 GetLot(x, z, false) (no
     * reference; 0 = none); lot slot 14; city slots 96 (cell corner to world)
     * and 95 (world to cell). */
    uint64_t (*lot_at_cell)(void* user, int32_t x, int32_t z);
    void (*lot_cell_rect)(void* user, uint64_t lot, fc4_cell_rect* out);
    void (*cell_to_world)(void* user, int32_t x, int32_t z, float* wx, float* wz);
    void (*world_to_cell)(void* user, float x, float z, int32_t* cx, int32_t* cz);
    /* ---- the building-prop property 0xA8322B7F (§A7.1 step 3) ----
     * FC4_PROP_BUILDING_*; with PRESENT the vector's props go to `visit` in order. */
    uint32_t (*building_props)(void* user, uint64_t building, fc4_token_callback visit, void* ctx);
    /* Replaces the vector of the existing property object. */
    void (*set_building_props)(void* user, uint64_t building, const uint64_t* props, size_t n);
    /* Creates the property object (0x14 bytes) holding `props` and adds it
     * (holder slot 11 AddProperty(0xA8322B7F, obj, false)). */
    void (*add_building_props_property)(void* user, uint64_t building, const uint64_t* props, size_t n);
    /* Holder slot 17 RemoveProperty(0xA8322B7F, false). */
    void (*remove_building_props_property)(void* user, uint64_t building);
    /* ---- the CreateTextDecal command (§A4.3) ---- a text decal occupant (one
     * reference; 0 = none); cISTETerrain slot 13; decal slots 4 (the text, not
     * NUL-terminated), 8 (three floats), 10 and 6 (PL1). */
    uint64_t (*create_text_decal)(void* user);
    float (*terrain_altitude)(void* user, float x, float z);
    void (*decal_set_text)(void* user, uint64_t decal, const char* text, size_t len);
    void (*decal_slot8)(void* user, uint64_t decal, const float* values3);
    void (*decal_slot10)(void* user, uint64_t decal, float value);
    void (*decal_slot6)(void* user, uint64_t decal, float value);
    /* ---- save references (§A8) ---- the token of a save reference read with
     * `iid` (0xC6BB8334, 0x681905BD); the reference is transferred to the core.
     * 0 = none. */
    uint64_t (*resolve_ref)(void* user, uint64_t token, uint32_t iid);
} fc4_prop_manager_host;

/* The 0x1C-byte prop profile (§A3.2) in the original's layout, copied byte for
 * byte (OccupantSize values past the third land in the following fields,
 * quirk 8). */
typedef struct fc4_prop_profile {
    uint32_t class_id;      /* +0x00 */
    float size[3];          /* +0x04 .. +0x0C */
    uint32_t satisfaction;  /* +0x10 */
    uint8_t wealth;         /* +0x14 */
    uint8_t wealth_pad[3];  /* +0x15 .. +0x17: not written by the profile reader */
    uint32_t crime_effect;  /* +0x18 */
} fc4_prop_profile;

/* One argument of the CreateTextDecal command: its text (not NUL-terminated)
 * and its value as a float. */
typedef struct fc4_prop_command_arg {
    const char* text;
    uint32_t text_len;
    float value;
} fc4_prop_command_arg;

/* Diagnostics: every instance field of §A2 (tokens 0 = none). */
typedef struct fc4_prop_manager_state {
    uint32_t struct_size;
    uint8_t initialized, destroyed;
    uint8_t filter; /* the filter object exists */
    uint8_t reserved0;
    uint32_t ref_count;
    uint32_t filter_usage;
    uint64_t occupant_manager, resource_manager, lot_manager, view_filter;
} fc4_prop_manager_state;

typedef struct fc4_prop_catalogue_state {
    uint32_t struct_size;
    uint8_t built, callback_registered;
    uint16_t reserved0;
    uint32_t data2; /* data2 of the catalogue message */
    uint32_t types, families, profiles; /* map sizes */
} fc4_prop_catalogue_state;

/* The process catalogue: create one per process. Destroying the handle leaves
 * the managers created with it their catalogue. app_shutdown is the
 * application shutdown callback (0x004A2A30): the catalogue is torn down and
 * the next Init rebuilds it. */
fc4_prop_catalogue* fc4_prop_catalogue_create(void);
void fc4_prop_catalogue_destroy(fc4_prop_catalogue* c);
void fc4_prop_catalogue_app_shutdown(fc4_prop_catalogue* c);
bool fc4_prop_catalogue_get_state(const fc4_prop_catalogue* c, fc4_prop_catalogue_state* out);

/* Constructed, reference count 0, sharing `catalogue` (NULL: a private one).
 * The host table is copied (NULL for a NULL or short table). `token` is the
 * manager's own host identity (void1 of the catalogue message). */
fc4_prop_manager* fc4_prop_manager_create(const fc4_prop_manager_host* host, const fc4_prop_catalogue* catalogue,
                                          uint64_t token);
void fc4_prop_manager_destroy(fc4_prop_manager* m);
bool fc4_prop_manager_query_interface(uint32_t iid);
uint32_t fc4_prop_manager_add_ref(fc4_prop_manager* m);
/* The last one runs Shutdown and returns 0; the handle stays valid until destroy. */
uint32_t fc4_prop_manager_release(fc4_prop_manager* m);
bool fc4_prop_manager_init(fc4_prop_manager* m);
bool fc4_prop_manager_shutdown(fc4_prop_manager* m);
/* DoMessage (§A4.3): `view_filter` is *void2 of PostViewFilterChange (0 =
 * null), `args` the CreateTextDecal command's arguments. Always true. */
bool fc4_prop_manager_do_message(fc4_prop_manager* m, uint32_t msg_type, uint64_t view_filter,
                                 const fc4_prop_command_arg* args, size_t n_args);
/* Slots 5 / 6: every type / family in bucket order. Return the count; copy at
 * most `capacity` (`out` may be NULL). */
size_t fc4_prop_manager_all_prop_types(const fc4_prop_manager* m, uint32_t* out, size_t capacity);
size_t fc4_prop_manager_all_prop_family_types(const fc4_prop_manager* m, uint32_t* out, size_t capacity);
/* Slot 7: false for an unknown type. */
bool fc4_prop_manager_prop_key_from_type(const fc4_prop_manager* m, uint32_t id, fc4_res_key* out);
bool fc4_prop_manager_is_prop_type_valid(const fc4_prop_manager* m, uint32_t id);
/* Slot 9 (quirk 1): the caller's vector (in, n_in) after the call goes to
 * `out` once. A family assigns its members and returns their count, a type
 * assigns [id] and returns 0, anything else leaves the vector and returns 0. */
uint32_t fc4_prop_manager_prop_types_by_family(const fc4_prop_manager* m, uint32_t id, const uint32_t* in,
                                               size_t n_in, fc4_u32s_sink out, void* ctx);
bool fc4_prop_manager_is_prop_in_family(const fc4_prop_manager* m, uint32_t type, uint32_t family);
bool fc4_prop_manager_is_prop_family_type_valid(const fc4_prop_manager* m, uint32_t id);
/* Slot 12 (quirk 3): inserts an empty name and returns it, copied
 * NUL-terminated (truncated to capacity - 1); returns its byte length. */
size_t fc4_prop_manager_prop_name(fc4_prop_manager* m, uint32_t id, char* out, size_t capacity);
/* Slot 13 (quirk 2): inserts an all-zero profile for an unknown ID. */
bool fc4_prop_manager_prop_profile(fc4_prop_manager* m, uint32_t id, fc4_prop_profile* out);
/* Slot 14: false (nothing written) for an unknown ID. */
bool fc4_prop_manager_prop_profile_ptr(const fc4_prop_manager* m, uint32_t id, fc4_prop_profile* out);
/* Slot 15 CreateProp(id): one reference (the caller's); 0 when id is not a prop type. */
uint64_t fc4_prop_manager_create_prop(fc4_prop_manager* m, uint32_t id);
/* Slot 16 CreateGenericProp(key): 0 when the exemplar does not load. */
uint64_t fc4_prop_manager_create_generic_prop(fc4_prop_manager* m, uint32_t type_id, uint32_t group,
                                              uint32_t instance);
/* Slots 19 AddCityProp(occ) / 25 AddLotProp(lot, occ) (the lot is ignored, quirk 6). */
bool fc4_prop_manager_add_city_prop(fc4_prop_manager* m, uint64_t occ);
bool fc4_prop_manager_add_lot_prop(fc4_prop_manager* m, uint64_t lot, uint64_t occ);
/* Slots 20 / 26 / 32 Add{City,Lot,Street}Prop(id, pos, orientation) (`usage` 1,
 * 2 or 4): the occupant (no reference for the caller), 0 when not added. */
uint64_t fc4_prop_manager_add_prop_new(fc4_prop_manager* m, uint32_t usage, uint32_t id, const float* pos3,
                                       int32_t orientation);
/* Slot 36 AddBuildingProp(building, prop). */
bool fc4_prop_manager_add_building_prop(fc4_prop_manager* m, uint64_t building, uint64_t prop);
/* Slot 17 GetCityProps(list, SC4Rect<float>). */
bool fc4_prop_manager_get_city_props_world(fc4_prop_manager* m, float x1, float z1, float x2, float z2,
                                           const uint64_t* in, size_t n_in, fc4_token_callback out, void* ctx);
/* Slots 18 / 30 Get{City,Street}Props(list, SC4Rect<int32>) (`usage` 1 or 4). */
bool fc4_prop_manager_get_props_in_cells(fc4_prop_manager* m, uint32_t usage, const fc4_cell_rect* rect,
                                         const uint64_t* in, size_t n_in, fc4_token_callback out, void* ctx);
/* Slots 23 / 34 / 31 Get{Lot,Building,Street}Props(list, x, z) (`usage` 2, 3 or
 * 4; the street query is always false, quirk 4). */
bool fc4_prop_manager_get_props_at(fc4_prop_manager* m, uint32_t usage, int32_t x, int32_t z, const uint64_t* in,
                                   size_t n_in, fc4_token_callback out, void* ctx);
/* Slots 24 GetLotProps(list, lot) / 35 GetBuildingProps(list, building)
 * (`usage` 2 or 3; `object` 0 = null). */
bool fc4_prop_manager_get_props_of(fc4_prop_manager* m, uint32_t usage, uint64_t object, const uint64_t* in,
                                   size_t n_in, fc4_token_callback out, void* ctx);
/* Slots 21 / 22 RemoveCityProps (world / cell rectangle). True. */
bool fc4_prop_manager_remove_city_props_world(fc4_prop_manager* m, float x1, float z1, float x2, float z2);
bool fc4_prop_manager_remove_city_props_cells(fc4_prop_manager* m, const fc4_cell_rect* rect);
/* Slots 27 / 37 / 33 Remove{Lot,Building}Props(x, z) and
 * RemoveStreetPropsFromTile (`usage` 2, 3 or 4). */
bool fc4_prop_manager_remove_props_at(fc4_prop_manager* m, uint32_t usage, int32_t x, int32_t z);
/* Slots 28 RemoveLotProps(lot) / 38 RemoveBuildingProps(building) (`usage` 2 or 3). */
bool fc4_prop_manager_remove_props_of(fc4_prop_manager* m, uint32_t usage, uint64_t object);
/* Slot 29 RemoveProp(occ). True. */
bool fc4_prop_manager_remove_prop(fc4_prop_manager* m, uint64_t occ);
/* Slot 39 RemoveBuildingProp(building, prop): whether it was found. */
bool fc4_prop_manager_remove_building_prop(fc4_prop_manager* m, uint64_t building, uint64_t prop);
bool fc4_prop_manager_get_state(const fc4_prop_manager* m, fc4_prop_manager_state* out);
/* The record (§A8): version 1, the occupant manager and the lot manager. Read
 * is false for another version and then changes nothing. */
bool fc4_prop_manager_write(const fc4_prop_manager* m, const fc4_ostream* out);
bool fc4_prop_manager_read(fc4_prop_manager* m, const fc4_istream* in);

/* ---- lot configuration manager (docs/specs/prop-lotconfig.md Part B) ---------
 * cSC4LotConfigurationManager (0x299062C0, city component 9). Lot
 * configurations, filters, the building development simulator, the flora
 * simulator and the prop manager are host objects named by tokens (0 = none).
 * Configuration objects cross as fc4_lotdev_cfg_object: object_type is the
 * type byte, byte3 the mirror flag, footprint_max / footprint_min the (x, z)
 * corners. The catalogue (§B2) is process-wide as the prop manager's.
 * fc4_lot_config_object_array is the configuration class's object reader
 * (§B3.3), in the core so that every host builds the same arrays. A missing
 * callback reads as 0 / false / none. */
typedef struct fc4_lot_config_catalogue fc4_lot_config_catalogue;
/* The lot configuration MANAGER handle. Named `fc4_lot_config_manager`, not
 * `fc4_lot_config`, so that it cannot be confused with the development glue's
 * fc4_lot_config struct (§4.7.4, above): one name for two types is legal C
 * but unusable in a C or C++ host that wants both (the glue15 lane). The C
 * ABI entry points keep their fc4_lot_config_* names. */
typedef struct fc4_lot_config_manager fc4_lot_config_manager;

/* fc4_lot_config_host.acquire_service services. */
#define FC4_LCM_SERVICE_BUILDING_DEVELOPMENT 0u
#define FC4_LCM_SERVICE_FLORA                1u
#define FC4_LCM_SERVICE_PROP_MANAGER         2u

/* What 0x0048AD40 stores in a configuration (§B4.3). */
typedef struct fc4_lot_config_building_info {
    uint16_t min_capacity; /* +0x3E: 0 when no type was found */
    uint16_t max_capacity; /* +0x40 */
    uint8_t non_monopoly;  /* +0x8C */
    uint8_t reserved[3];
    uint32_t n_groups;     /* +0x80: the occupant groups, ascending */
    const uint32_t* groups;
} fc4_lot_config_building_info;

typedef struct fc4_lot_config_host {
    uint32_t struct_size; /* sizeof(fc4_lot_config_host) */
    void* user;
    /* ---- the catalogue (§B3) ----
     * The city detail preference (1 low, 2 medium, 3 high). */
    uint32_t (*detail_level)(void* user);
    /* The resource manager's keys of type 0x6534284A; false without one. */
    bool (*for_each_exemplar_key)(void* user, fc4_key_visitor visit, void* ctx);
    /* Application slot 69: the unload callback (call fc4_lot_config_unload). */
    void (*register_shutdown_callback)(void* user);
    void (*unregister_shutdown_callback)(void* user);
    /* A new lot configuration (0xC0 bytes), one reference. */
    uint64_t (*create_configuration)(void* user);
    /* Configuration slot 31 Load(key, detail) (fc4_lot_config_object_array
     * builds its object array). */
    bool (*load_configuration)(void* user, uint64_t cfg, uint32_t type_id, uint32_t group, uint32_t instance,
                               uint32_t detail);
    /* ---- configuration slots ---- 3 GetID, 5 BelongsToFamily, 6 the families,
     * 7 GetSize, 23 the object array (each object to `visit`, its ids valid
     * during the call) and its replacement (FixLotConfiguration), 24 the
     * primary building (false = none), the building information, 28 the
     * retaining walls and their replacement. */
    uint32_t (*config_id)(void* user, uint64_t cfg);
    bool (*config_belongs_to_family)(void* user, uint64_t cfg, uint32_t family);
    void (*config_families)(void* user, uint64_t cfg, fc4_u32s_sink out, void* ctx);
    void (*config_size)(void* user, uint64_t cfg, uint32_t* x, uint32_t* z);
    void (*config_objects)(void* user, uint64_t cfg, fc4_lotdev_cfg_object_callback visit, void* ctx);
    void (*set_config_objects)(void* user, uint64_t cfg, const fc4_lotdev_cfg_object* objects, size_t n);
    bool (*config_primary_building)(void* user, uint64_t cfg, fc4_lotdev_cfg_object_callback visit, void* ctx);
    void (*set_config_building_info)(void* user, uint64_t cfg, const fc4_lot_config_building_info* info);
    void (*config_retaining_walls)(void* user, uint64_t cfg, fc4_u32s_sink out, void* ctx);
    void (*set_config_retaining_walls)(void* user, uint64_t cfg, const uint32_t* ids, size_t n);
    /* Whether a retaining wall ID is valid (FixLotConfiguration, PL10). */
    bool (*is_retaining_wall_valid)(void* user, uint32_t id);
    /* A configuration not in the map, of size_x x size_z cells with `objects`,
     * for building type `type` (CreateGenericLotConfiguration); one reference
     * for the caller, 0 = none. */
    uint64_t (*create_generic_configuration)(void* user, uint32_t type, uint32_t size_x, uint32_t size_z,
                                             const fc4_lotdev_cfg_object* objects, size_t n);
    /* ---- lifecycle and references ---- the service's global
     * (FC4_LCM_SERVICE_*) with a new reference (0 = null). */
    uint64_t (*acquire_service)(void* user, uint32_t service);
    void (*subscribe)(void* user, uint32_t msg_type);
    void (*unsubscribe)(void* user, uint32_t msg_type);
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    /* Filter slot 3 on a configuration. */
    bool (*filter_matches)(void* user, uint64_t filter, uint64_t cfg);
    /* ---- the building development simulator ----
     * Slot 14 + the load: the type's exemplar as a NEW fc4_props (the core
     * destroys it), NULL when it does not load. */
    fc4_props* (*building_exemplar)(void* user, uint32_t type);
    /* Slots 15 IsBuildingTypeValid, 16 IsBuildingFamilyTypeValid. */
    bool (*is_building_type_valid)(void* user, uint32_t id);
    bool (*is_building_family_type_valid)(void* user, uint32_t id);
    /* Slot 18 GetBuildingTypesByFamily(vector&, id): call `out` once with the
     * assigned vector (a family's members, or [id] for a type); do not call it
     * to leave the vector untouched. Returns the slot's result. */
    uint32_t (*building_types_by_family)(void* user, uint32_t id, fc4_u32s_sink out, void* ctx);
    /* Slot 19 GetBuildingTypesByFamily(hash_set&, id): the types it inserts. */
    uint32_t (*building_types_into_set)(void* user, uint32_t id, fc4_u32s_sink out, void* ctx);
    /* Slot 20 IsBuildingInFamily(type, family). */
    bool (*is_building_in_family)(void* user, uint32_t type, uint32_t family);
    /* Slot 39: the profile's u16 at +8; slot 40: +0x20 and +0x28 (false = no profile). */
    uint16_t (*building_capacity)(void* user, uint32_t type);
    bool (*building_footprint)(void* user, uint32_t type, float* w, float* d);
    /* ---- the flora simulator (global) ---- slot 7 (+0x3C; false = unknown),
     * slot 11, slot 14 (the types it inserts). */
    bool (*flora_radius)(void* user, uint32_t id, float* radius);
    bool (*is_flora_type_valid)(void* user, uint32_t id);
    uint32_t (*flora_types_into_set)(void* user, uint32_t id, fc4_u32s_sink out, void* ctx);
    /* ---- the prop manager (global) ---- slots 8, 9 (as building_types_by_family),
     * 11 and 14 (size x +0x04 and z +0x0C; false = no profile). */
    bool (*is_prop_type_valid)(void* user, uint32_t id);
    uint32_t (*prop_types_by_family)(void* user, uint32_t id, fc4_u32s_sink out, void* ctx);
    bool (*is_prop_family_type_valid)(void* user, uint32_t id);
    bool (*prop_size)(void* user, uint32_t id, float* x, float* z);
    /* ---- save references (§B6) ---- IIDs 0x07813EAE, 0x06C66177, 0x28326F3E;
     * the reference is transferred to the core. 0 = none. */
    uint64_t (*resolve_ref)(void* user, uint64_t token, uint32_t iid);
} fc4_lot_config_host;

/* Diagnostics: the instance fields of §B2 (tokens 0 = none). */
typedef struct fc4_lot_config_state {
    uint32_t struct_size;
    uint8_t initialized, destroyed;
    uint16_t reserved0;
    uint32_t ref_count;
    uint32_t filters; /* registered filters */
    uint64_t building_development, flora, prop_manager;
} fc4_lot_config_state;

typedef struct fc4_lot_config_catalogue_state {
    uint32_t struct_size;
    uint8_t loaded, callback_registered;
    uint16_t reserved0;
    uint32_t detail; /* the detail level it was built for */
    uint32_t configurations, families;
} fc4_lot_config_catalogue_state;

/* ConvertPropertiesToObjectArray (§B3.3): the object array of a lot
 * configuration exemplar at `detail`, each object to `visit` in order.
 * `next_serial` (may be NULL) is the generated tag of a configuration below
 * version 2, advanced once per kept object (the process-wide serial of
 * lot-configuration.md §3.5; seed it when 0). Returns the count. */
size_t fc4_lot_config_object_array(const fc4_props* exemplar, uint32_t detail, uint32_t* next_serial,
                                   fc4_lotdev_cfg_object_callback visit, void* ctx);

fc4_lot_config_catalogue* fc4_lot_config_catalogue_create(void);
void fc4_lot_config_catalogue_destroy(fc4_lot_config_catalogue* c);
bool fc4_lot_config_catalogue_get_state(const fc4_lot_config_catalogue* c, fc4_lot_config_catalogue_state* out);

/* Constructed, reference count 0, sharing `catalogue` (NULL: a private one).
 * The host table is copied (NULL for a NULL or short table). */
fc4_lot_config_manager* fc4_lot_config_create(const fc4_lot_config_host* host, const fc4_lot_config_catalogue* catalogue,
                                              uint64_t token);
void fc4_lot_config_destroy(fc4_lot_config_manager* m);
bool fc4_lot_config_query_interface(uint32_t iid);
uint32_t fc4_lot_config_add_ref(fc4_lot_config_manager* m);
/* The last one runs Shutdown and returns 0; the handle stays valid until destroy. */
uint32_t fc4_lot_config_release(fc4_lot_config_manager* m);
/* Init builds the catalogue, or rebuilds it when the detail changed. */
bool fc4_lot_config_init(fc4_lot_config_manager* m);
bool fc4_lot_config_shutdown(fc4_lot_config_manager* m);
/* The process unload callback (0x0048A240); the family set stays (quirk 10). */
void fc4_lot_config_unload(fc4_lot_config_manager* m);
/* DoMessage (§B4.3): PostCityInit fills every configuration's building
 * information. Always true. */
bool fc4_lot_config_do_message(fc4_lot_config_manager* m, uint32_t msg_type);
void fc4_lot_config_fill_building_information(fc4_lot_config_manager* m, uint64_t cfg);
/* Slots 5 AddLotConfiguration(key), 6 AddLotConfiguration(cfg, add) (always
 * true), 7 RemoveLotConfiguration(id), 8 GetLotConfiguration(id) (no
 * reference; 0 = none). */
bool fc4_lot_config_add_by_key(fc4_lot_config_manager* m, uint32_t type_id, uint32_t group, uint32_t instance);
bool fc4_lot_config_add_configuration(fc4_lot_config_manager* m, uint64_t cfg, bool add);
bool fc4_lot_config_remove_configuration(fc4_lot_config_manager* m, uint32_t id);
uint64_t fc4_lot_config_configuration(const fc4_lot_config_manager* m, uint32_t id);
/* Slots 20 (the map) and 21 (the family set) in bucket order. Return the
 * count; copy at most `capacity` (pointers may be NULL). */
size_t fc4_lot_config_configuration_map(const fc4_lot_config_manager* m, uint32_t* ids, uint64_t* configurations,
                                        size_t capacity);
size_t fc4_lot_config_family_id_set(const fc4_lot_config_manager* m, uint32_t* out, size_t capacity);
/* Slots 9-11 GetLotConfigurationsBy{Filter,Family,Size}: `which` 0 = by filter
 * (`a` = the filter, 0 = null; registered filters cache their result), 1 = by
 * family (`a`), 2 = by size (`a` = x, `b` = z). The matches go to `out` in
 * bucket order, each with a new reference for the caller; returns the count. */
uint32_t fc4_lot_config_configurations(fc4_lot_config_manager* m, uint32_t which, uint64_t a, uint32_t b,
                                       fc4_token_callback out, void* ctx);
/* Slot 12 ClearLotConfigurationSet: releases every element once. */
void fc4_lot_config_clear_set(fc4_lot_config_manager* m, const uint64_t* set, size_t n);
/* Slots 13-15 GetLotConfigurationIDsBy{Filter,Family,Size}, as above (no
 * cache). Return the count; copy at most `capacity`. */
size_t fc4_lot_config_ids(const fc4_lot_config_manager* m, uint32_t which, uint64_t a, uint32_t b, uint32_t* out,
                          size_t capacity);
/* Slots 16 / 17 LotConfigurationHasBuilding{Family,Type}(cfg, id, obj)
 * (`by_type` false / true): the matching object's index in the array to
 * `index` (may be NULL). */
bool fc4_lot_config_has_building(const fc4_lot_config_manager* m, uint64_t cfg, uint32_t id, bool by_type,
                                 uint32_t* index);
/* Slots 18 / 19 and the flush (0x0048A080). */
void fc4_lot_config_register_filter(fc4_lot_config_manager* m, uint64_t filter);
void fc4_lot_config_unregister_filter(fc4_lot_config_manager* m, uint64_t filter);
void fc4_lot_config_flush(fc4_lot_config_manager* m);
/* Diagnostics: a registered filter's cached result (false when not
 * registered; *cached false when nothing is cached). Configurations (no
 * references) copied, at most `capacity`; *count = their number. */
bool fc4_lot_config_filter_cache(const fc4_lot_config_manager* m, uint64_t filter, bool* cached, uint64_t* out,
                                 size_t capacity, size_t* count);
/* Slots 24-26 GetMaxFootprintFor{Building,Prop,Flora}Families(ids) (`kind` 0,
 * 1 or 4: the object types): (w, d, 0, 0) into out4. */
bool fc4_lot_config_max_footprint(const fc4_lot_config_manager* m, uint32_t kind, const uint32_t* ids, size_t n,
                                  float* out4);
/* Slot 27 GetObjectIDArray(obj, vec): families expanded (quirk 13). Returns the
 * count; copies at most `capacity`. */
size_t fc4_lot_config_object_id_array(const fc4_lot_config_manager* m, const fc4_lotdev_cfg_object* obj, uint32_t* out,
                                      size_t capacity);
/* Slot 22 CreateGenericLotConfiguration(type): one reference for the caller,
 * 0 when BDS rejects the type (PL8). */
uint64_t fc4_lot_config_create_generic(fc4_lot_config_manager* m, uint32_t building_type);
/* Slot 23 FixLotConfiguration(cfg): the first problem's code (0 none, 1 an ID
 * removed, 2 a record removed, 3 a footprint changed, 4 outside the lot, 5 a
 * centre moved, 6 a retaining wall removed; PL9, PL10). */
uint32_t fc4_lot_config_fix(fc4_lot_config_manager* m, uint64_t cfg);
bool fc4_lot_config_get_state(const fc4_lot_config_manager* m, fc4_lot_config_state* out);
/* The record (§B6): version 1 and the three held services. Read is false for
 * another version and then changes nothing. */
bool fc4_lot_config_write(const fc4_lot_config_manager* m, const fc4_ostream* out);
bool fc4_lot_config_read(fc4_lot_config_manager* m, const fc4_istream* in);

/* ---- tutorial system (docs/specs/tutorial.md) -------------------------------
 * cSC4TutorialSystem (0xAA3836FB, city component 48). The tutorial window, its
 * tasks, the registry, the 3D view, the cheat manager and the Lua context are
 * host objects named by tokens (0 = none). The simulation reads one thing,
 * fc4_tutorial_is_tutorial (the fire simulator's no-burn-down flag and the
 * flammability fire roll take it from there). A missing callback reads as
 * 0 / false / none. */
typedef struct fc4_tutorial fc4_tutorial;

typedef struct fc4_tutorial_host {
    uint32_t struct_size; /* sizeof(fc4_tutorial_host) */
    void* user;
    /* ---- lifecycle (§4.3) ---- message server; the cheat manager exists and
     * cISC4App slot 21 is true (Init); the cheat manager exists (Shutdown,
     * TU4); register / unregister the handler and the cheat 0x6A382BAF
     * "Tutorial". */
    void (*subscribe)(void* user, uint32_t msg_type);
    void (*unsubscribe)(void* user, uint32_t msg_type);
    bool (*cheats_available)(void* user);
    bool (*has_cheat_manager)(void* user);
    void (*register_cheat)(void* user);
    void (*unregister_cheat)(void* user);
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    /* ---- the window (§4.2, §4.4) ---- a new cSC4TutorialUIWinProc (no
     * reference yet; the core takes one), its init and close; window slot 9
     * the current task (no reference; 0 = none); task slot 25; window slots 6
     * (a message's data field), 7, 8, 10, 11 (the new task number) and the
     * tutorial event. */
    uint64_t (*create_window)(void* user);
    void (*init_window)(void* user, uint64_t window);
    void (*close_window)(void* user, uint64_t window);
    uint64_t (*window_current_task)(void* user, uint64_t window);
    bool (*task_edge_scrolling_allowed)(void* user, uint64_t task);
    void (*window_filter_value)(void* user, uint64_t window, uint32_t value);
    void (*window_show_hints)(void* user, uint64_t window);
    void (*window_catalog_menu)(void* user, uint64_t window, uint64_t menu);
    bool (*window_left_click)(void* user, uint64_t window, int32_t* x, int32_t* z, int32_t n);
    int32_t (*window_reset)(void* user, uint64_t window, int32_t a, int32_t b);
    void (*window_event)(void* user, uint64_t window, uint32_t event);
    /* ---- the registry ---- SetActive(guid, active): what the call left in AL;
     * the entry of `guid` (0x004B5D10): false when absent, else *enabled = its
     * byte +0x10. */
    bool (*registry_set_active)(void* user, uint32_t guid, bool active);
    bool (*registry_entry_enabled)(void* user, uint32_t guid, bool* enabled);
    /* ---- the rest ---- the 3D view's slots 60 and 59 (when there is one);
     * the city's Lua context (cISC4City slot 81, then slot 10) with a
     * reference, 0 = unavailable. */
    void (*view_restore_controls)(void* user);
    uint64_t (*fetch_lua_context)(void* user);
} fc4_tutorial_host;

/* Every field of §2 (tokens 0 = none). */
typedef struct fc4_tutorial_state {
    uint32_t struct_size;
    uint8_t is_tutorial; /* +0x1F (saved): the byte as read */
    uint8_t destroyed;
    uint8_t end_pending; /* +0x2C */
    uint8_t start_tried; /* +0x2D */
    uint32_t ref_count;
    int32_t task_number; /* +0x20 (saved) */
    uint32_t guid;       /* +0x24 (saved; 0xFFFFFFFF = none) */
    uint32_t field_28;   /* +0x28 (3) */
    uint64_t lua_context; /* +0x30 (one reference held) */
    uint64_t window;      /* +0x40 (one reference held) */
} fc4_tutorial_state;

/* Constructed (§2: GUID none, reference count 0). The host table is copied
 * (NULL for a NULL or short table). */
fc4_tutorial* fc4_tutorial_create(const fc4_tutorial_host* host, uint64_t token);
void fc4_tutorial_destroy(fc4_tutorial* t);
bool fc4_tutorial_query_interface(uint32_t iid);
uint32_t fc4_tutorial_add_ref(fc4_tutorial* t);
/* The last one marks the object destroyed (TU3) and returns 0; the handle
 * stays valid until destroy. */
uint32_t fc4_tutorial_release(fc4_tutorial* t);
bool fc4_tutorial_init(fc4_tutorial* t);
bool fc4_tutorial_shutdown(fc4_tutorial* t);
/* DoMessage (§4.3): city init complete, Tutorial_EndTutorial (`data` = the
 * message's data field; 0 also ends the registry entry) and pre-save. Always
 * true. */
bool fc4_tutorial_do_message(fc4_tutorial* t, uint32_t msg_type, uint32_t data);
/* Slots 5 startTutorial (the registry call's result) and 6
 * endTutorial(end_registry) (returns end_registry). */
bool fc4_tutorial_start(fc4_tutorial* t);
bool fc4_tutorial_end(fc4_tutorial* t, bool end_registry);
/* Slot 7: the Lua context (fetched once, kept with a reference), 0 = none. */
uint64_t fc4_tutorial_scripting_context(fc4_tutorial* t);
/* Slot 8 IsTutorial (§4.1): 1 or 0. */
uint32_t fc4_tutorial_is_tutorial(const fc4_tutorial* t);
/* Slots 9 IsEdgeScrollingAllowed, 10 GetCurrentTask (0 = none), 11
 * GetTutorialTaskNumber, 12 GetTutorialGUID. */
bool fc4_tutorial_is_edge_scrolling_allowed(const fc4_tutorial* t);
uint64_t fc4_tutorial_current_task(const fc4_tutorial* t);
int32_t fc4_tutorial_task_number(const fc4_tutorial* t);
uint32_t fc4_tutorial_guid(const fc4_tutorial* t);
/* Slot 13 FilterMessage: the message's kind, ID and data fields 2 and 3. Always true. */
bool fc4_tutorial_filter_message(fc4_tutorial* t, uint32_t kind, uint32_t id, uint32_t field2, uint32_t field3);
/* Slots 14 ShowUIHintsForUsers, 15 HandleCatalogMenu (true), 16
 * FilterLeftMouseClick (x, z in/out; true without a window), 17
 * ResetTutorialNumAndIndex (false without a window), 18 OnTutorialEvent
 * (true), 19 SetTutorialTaskNumber, 20 IncrementTutorialTaskNumber. */
void fc4_tutorial_show_ui_hints(fc4_tutorial* t);
bool fc4_tutorial_handle_catalog_menu(fc4_tutorial* t, uint64_t menu);
bool fc4_tutorial_filter_left_mouse_click(fc4_tutorial* t, int32_t* x, int32_t* z, int32_t n);
bool fc4_tutorial_reset_num_and_index(fc4_tutorial* t, int32_t a, int32_t b);
bool fc4_tutorial_on_event(fc4_tutorial* t, uint32_t event);
void fc4_tutorial_set_task_number(fc4_tutorial* t, int32_t n);
void fc4_tutorial_increment_task_number(fc4_tutorial* t);
bool fc4_tutorial_get_state(const fc4_tutorial* t, fc4_tutorial_state* out);
/* The 11-byte record (§3): version 2, is-tutorial, the task number, the GUID.
 * Write is false (nothing written) on a stream with an error; Read is false
 * unless the version is 2 and maps the GUID (0 -> 0x8A5B7A6C, 1 -> 0xEA5D6DC8). */
bool fc4_tutorial_write(const fc4_tutorial* t, const fc4_ostream* out);
bool fc4_tutorial_read(fc4_tutorial* t, const fc4_istream* in);

/* ---- network manager (docs/specs/network-manager.md; engine/capi/src/network_manager.rs) ----
 * cSC4NetworkManager (component 53) and cSC4NetworkLotManager (component 11).
 * The shim owns the GZCOM objects and forwards their slots here. Tokens are
 * opaque host identities (0 = none): network occupants, tools, construction
 * crews, lots, street props, flora and effect props. Occupants, the terrain
 * and the network lot configurations are host inputs; the traffic graph is
 * traffic-network.md's (fc4_traffic), the riot's removal lock (§10) the
 * disaster layer's (fc4_disasters). A missing callback reads as 0 / false /
 * none. One object's calls must be serialised; a callback must not call back
 * into its object, except that crew_shutdown may call
 * fc4_netmgr_unregister_construction_crew. */
typedef struct fc4_netmgr fc4_netmgr;
typedef struct fc4_netlot fc4_netlot;

/* The network manager's host. */
typedef struct fc4_netmgr_host {
    uint32_t struct_size; /* sizeof(fc4_netmgr_host) */
    void* user;
    /* messages and the city */
    void (*subscribe)(void* user, uint32_t msg_type);
    void (*unsubscribe)(void* user, uint32_t msg_type);
    bool (*uk_paths)(void* user);
    void (*city_size)(void* user, int32_t* x_cells, int32_t* z_cells);
    bool (*is_night)(void* user);
    void (*view_rotation_changed)(void* user);
    /* references */
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    /* a saved reference (fc4_ref token) as the interface `iid`, one reference held; 0 = none */
    uint64_t (*resolve_ref)(void* user, uint64_t token, uint32_t iid);
    /* tools (§7.1): created with one reference for the core */
    uint64_t (*create_tool)(void* user, uint32_t network_type);
    void (*tool_init)(void* user, uint64_t tool);
    void (*tool_shutdown)(void* user, uint64_t tool);
    void (*tool_destroy_occupants)(void* user, uint64_t tool, const uint64_t* occupants, size_t count);
    /* UpdateUndergroundOccupantTerrainHeights: `cells` holds `count` (x, z) pairs */
    void (*tool_update_underground_heights)(void* user, uint64_t tool, const int32_t* cells,
                                            const uint64_t* occupants, size_t count, uint32_t network_type);
    uint64_t (*tool_cell_info)(void* user, uint64_t tool, uint32_t packed_cell); /* (z << 16) + x */
    void (*tool_reinsert_piece)(void* user, uint64_t tool, uint64_t info);
    /* network occupants */
    uint64_t (*query_network_occupant)(void* user, uint64_t occupant); /* one reference for the core; 0 = not one */
    bool (*is_network_occupant)(void* user, uint64_t occupant);
    uint32_t (*network_flags)(void* user, uint64_t n);
    bool (*is_of_type)(void* user, uint64_t n, uint32_t network_type);
    uint32_t (*occupant_flags)(void* user, uint64_t occupant);
    bool (*occupant_flag_set)(void* user, uint64_t occupant, uint32_t flag);
    void (*occupied_cell)(void* user, uint64_t n, int32_t* x, int32_t* z);
    float (*occupant_altitude)(void* user, uint64_t n, float x, float z);
    bool (*is_managed_globally)(void* user, uint64_t n);
    bool (*can_be_lit)(void* user, uint64_t n);
    void (*set_lit)(void* user, uint64_t n, bool lit);
    bool (*check_for_retrofit)(void* user, uint64_t n);
    void (*retrofit)(void* user, uint64_t n);
    uint64_t (*creation_number)(void* user, uint64_t occupant); /* missing: the token */
    /* the occupant manager and the terrain */
    bool (*remove_occupant)(void* user, uint64_t occupant);
    void (*occupants_in_rect)(void* user, const fc4_cell_rect* rect, uint32_t network_mask, fc4_token_callback cb,
                              void* ctx);
    void (*network_occupants)(void* user, fc4_token_callback cb, void* ctx);
    void (*post_refresh)(void* user, uint64_t n); /* posts 0x99EF1145 */
    float (*terrain_altitude)(void* user, float x, float z);
    /* construction crews: slot 5 */
    void (*crew_shutdown)(void* user, uint64_t crew);
} fc4_netmgr_host;

typedef struct fc4_netmgr_state {
    uint32_t struct_size;
    uint8_t initialized, destroyed, uk_paths, long_terrain_op;
    uint8_t queueing, reserved[3];
    uint32_t ref_count;
    uint32_t managed, pending, retrofit, crews;
    uint32_t tools;      /* cached tools that are not null */
    fc4_cell_rect dirty; /* (0x7FFFFFFF, 0x7FFFFFFF, -1, -1) when empty */
} fc4_netmgr_state;

/* A network flag change (§5). */
typedef struct fc4_network_flag_change {
    uint32_t flags;       /* the new flags */
    int32_t usable_post;  /* data1 of the 0x89D3E9F3 to post, or -1 for none */
    uint8_t translucency; /* the translucency changes */
    uint8_t reserved[3];
} fc4_network_flag_change;

/* Pure rules. */
bool fc4_network_set_flag(uint32_t flags, uint32_t bits, bool in_city, fc4_network_flag_change* out);
bool fc4_network_clear_flag(uint32_t flags, uint32_t bits, fc4_network_flag_change* out);
uint32_t fc4_network_pipe_model(uint32_t flags); /* 0 normal, 1 distressed, 2 burst */
/* §7.6 step 1: the rectangle of 0x278E6003, clamped to the city. */
bool fc4_network_terrain_rect(uint32_t data1, uint32_t data2, int32_t x_size, int32_t z_size, fc4_cell_rect* out);
/* The §7.6 corner rule for one corner's height difference (NaN: false). */
bool fc4_network_corner_mismatch(float d);

/* Lifecycle. fc4_netmgr_release returns 0 once the object is destroyed; the
 * handle stays valid until fc4_netmgr_destroy. */
fc4_netmgr* fc4_netmgr_create(const fc4_netmgr_host* host, uint64_t token);
void fc4_netmgr_destroy(fc4_netmgr* m);
bool fc4_netmgr_query_interface(uint32_t iid);
uint32_t fc4_netmgr_add_ref(fc4_netmgr* m);
uint32_t fc4_netmgr_release(fc4_netmgr* m);
bool fc4_netmgr_init(fc4_netmgr* m); /* §3.1: 11 subscriptions, 13 tools */
bool fc4_netmgr_shutdown(fc4_netmgr* m);
/* DoMessage (§6.1). void1: the occupant of 0x99EF1142 / 0x99EF1143 (0 =
 * none); view_old / view_new: the two values of 0xA6B79602. */
bool fc4_netmgr_do_message(fc4_netmgr* m, uint32_t msg_type, uint32_t data1, uint32_t data2, uint32_t data3,
                           uint32_t data4, uint64_t void1, uint32_t view_old, uint32_t view_new);
/* Slot 5 (§7.1): the cached tool (borrowed) or, with `unique`, a new one carrying the caller's reference. */
uint64_t fc4_netmgr_get_network_tool(fc4_netmgr* m, uint32_t network_type, bool unique);
void fc4_netmgr_destroy_network_occupants(fc4_netmgr* m, const uint64_t* list, size_t count); /* slot 6 */
void fc4_netmgr_queue_removals(fc4_netmgr* m, bool on);                                     /* slot 7 */
/* Slot 8 (§7.4): `*replaced` is written (true) only when an intersection was replaced. */
bool fc4_netmgr_remove_or_replace_occupant(fc4_netmgr* m, uint64_t tool, uint64_t occupant, bool* replaced);
void fc4_netmgr_register_construction_crew(fc4_netmgr* m, uint64_t crew);   /* slot 16 */
void fc4_netmgr_unregister_construction_crew(fc4_netmgr* m, uint64_t crew); /* slot 17 */
void fc4_netmgr_add_to_retrofit_list(fc4_netmgr* m, uint64_t n);            /* slot 23 */
/* The globally managed list (0 = a null entry read from a save). Returns the length; copies at most `capacity`. */
size_t fc4_netmgr_managed(const fc4_netmgr* m, uint64_t* out, size_t capacity);
bool fc4_netmgr_get_state(const fc4_netmgr* m, fc4_netmgr_state* out);
/* The record (§4.1). Read appends to the managed list (resolve_ref, IID 0xA821EF94). */
bool fc4_netmgr_write(const fc4_netmgr* m, const fc4_ostream* out);
bool fc4_netmgr_read(fc4_netmgr* m, const fc4_istream* in);

/* The network lot manager's host. */
typedef struct fc4_netlot_host {
    uint32_t struct_size; /* sizeof(fc4_netlot_host) */
    void* user;
    /* messages and references */
    void (*subscribe)(void* user, uint32_t msg_type);
    void (*unsubscribe)(void* user, uint32_t msg_type);
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    uint64_t (*resolve_ref)(void* user, uint64_t token, uint32_t iid); /* one reference held; 0 = none */
    /* network occupants */
    uint64_t (*query_network_occupant)(void* user, uint64_t unknown); /* one reference for the core; 0 = not one */
    uint32_t (*network_flags)(void* user, uint64_t n);
    uint32_t (*piece_id)(void* user, uint64_t n);
    int32_t (*rotation)(void* user, uint64_t n);
    int32_t (*flip)(void* user, uint64_t n);
    void (*position_cell)(void* user, uint64_t n, int32_t* x, int32_t* z);
    void (*occupied_cell)(void* user, uint64_t n, int32_t* x, int32_t* z);
    float (*occupant_altitude)(void* user, uint64_t n, float x, float z);
    bool (*is_intersection)(void* user, uint64_t n);
    uint32_t (*edge_types_flag)(void* user, uint64_t n, uint32_t kind);
    uint32_t (*active_edges)(void* user, uint64_t n);
    void (*set_network_flag)(void* user, uint64_t n, uint32_t bits);
    void (*set_under_texture)(void* user, uint64_t n, uint32_t texture);
    void (*set_variation)(void* user, uint64_t n, uint32_t variation);
    /* the traffic network map (traffic-network.md) */
    bool (*traffic_map_exists)(void* user);
    uint64_t (*find_network_tile)(void* user, int32_t x, int32_t z, uint32_t mask); /* borrowed; 0 = none */
    void (*update_intersection)(void* user, uint64_t n);
    /* the city, the terrain, zones and lots */
    void (*city_size)(void* user, int32_t* x_cells, int32_t* z_cells);
    void (*cell_center)(void* user, int32_t x, int32_t z, float* wx, float* wz);
    float (*terrain_altitude)(void* user, float x, float z);
    int32_t (*zone_type)(void* user, int32_t x, int32_t z);
    uint64_t (*lot_at)(void* user, int32_t x, int32_t z); /* borrowed; 0 = none */
    int32_t (*lot_wealth)(void* user, uint64_t lot);
    bool (*lot_is_livable)(void* user, uint64_t lot);
    uint32_t (*lot_building_type)(void* user, uint64_t lot);
    bool (*building_profile_wealth)(void* user, uint32_t building_type, int32_t* wealth);
    uint64_t (*query_lot)(void* user, uint64_t unknown); /* one reference for the core; 0 = not a lot */
    void (*lot_bounding_rect)(void* user, uint64_t lot, fc4_cell_rect* out);
    float (*area_slope)(void* user, int32_t x, int32_t z);
    /* street props and flora (§8.6); a position is float[3] (x, y, z) */
    void (*remove_street_props)(void* user, int32_t x, int32_t z);
    uint64_t (*add_street_prop)(void* user, uint32_t id, const float* pos, uint32_t orientation); /* borrowed */
    void (*street_prop_slot28)(void* user, uint64_t prop, uint32_t value);
    void (*street_prop_slot29)(void* user, uint64_t prop, uint32_t value);
    void (*demolish_flora)(void* user, int32_t x, int32_t z);
    uint64_t (*add_flora)(void* user, uint32_t id, const float* pos); /* borrowed */
    void (*flora_slot28)(void* user, uint64_t flora, uint32_t index);
    /* the candidate IDs of object `index` of configuration `config` (its token) */
    void (*object_ids)(void* user, uint64_t config, uint32_t index, fc4_u32_callback cb, void* ctx);
    /* potholes (§8.8): the effect prop is created with one reference for the core */
    uint64_t (*create_effect_prop)(void* user);
    void (*effect_set_name)(void* user, uint64_t prop, const char* name);
    void (*effect_start)(void* user, uint64_t prop);
    void (*effect_stop)(void* user, uint64_t prop);
    void (*set_position)(void* user, uint64_t prop, const float* pos);
    void (*insert_occupant)(void* user, uint64_t occupant);
    void (*remove_occupant)(void* user, uint64_t occupant);
    uint64_t (*first_network_occupant_at)(void* user, int32_t x, int32_t z, uint32_t mask); /* borrowed */
} fc4_netlot_host;

/* One configuration object (§8.6). */
typedef struct fc4_netlot_object {
    uint8_t kind;  /* 1 prop, 4 flora */
    uint8_t flags; /* bit 0: no prop is created */
    uint8_t orientation;
    uint8_t reserved;
    float offset[3];
    uint32_t field_24; /* passed to a created prop's slot 28 */
} fc4_netlot_object;

/* A network lot configuration (§2.4). */
typedef struct fc4_netlot_config {
    uint32_t struct_size; /* sizeof(fc4_netlot_config) */
    uint32_t piece_id;
    uint64_t token; /* the host's identity of the configuration (object_ids) */
    uint32_t id;
    uint16_t zones;  /* bit z: zone type z allowed */
    uint8_t wealths; /* bit w: wealth w allowed */
    uint8_t period;
    uint8_t mask[4];
    uint8_t rotations;
    uint8_t flip_rule;
    uint16_t reserved;
    float min_slope, max_slope;
    uint32_t object_count;
    const fc4_netlot_object* objects;
} fc4_netlot_config;

typedef struct fc4_netlot_lot {
    uint64_t occupant;  /* 0 = a reference that did not resolve on load */
    uint32_t config_id; /* the live configuration's, or the one read */
    uint8_t has_config; /* false only between Read and Init */
    uint8_t locked;
    uint16_t reserved;
} fc4_netlot_lot;

typedef struct fc4_netlot_state {
    uint32_t struct_size;
    uint8_t initialized, destroyed, has_forced, reserved;
    uint32_t ref_count;
    uint32_t lots, potholes, pending, configs;
    uint32_t forced_id;
} fc4_netlot_state;

typedef bool (*fc4_netlot_visitor)(void* ctx, int32_t x, int32_t z, const fc4_netlot_lot* lot);

/* Lifecycle. `session_seed` seeds the generator (§17). */
fc4_netlot* fc4_netlot_create(const fc4_netlot_host* host, uint64_t token, uint32_t session_seed);
void fc4_netlot_destroy(fc4_netlot* l);
bool fc4_netlot_query_interface(uint32_t iid);
uint32_t fc4_netlot_add_ref(fc4_netlot* l);
uint32_t fc4_netlot_release(fc4_netlot* l);
/* Configurations for the next Init (load order = call order); objects are copied. The props
 * form is ReadLotConfiguration (§2.4): false unless property 0x10 is 0x21 and the piece ID is present. */
bool fc4_netlot_add_configuration(fc4_netlot* l, const fc4_netlot_config* cfg);
bool fc4_netlot_add_configuration_props(fc4_netlot* l, uint64_t token, const fc4_props* props,
                                        const fc4_netlot_object* objects, uint32_t object_count);
void fc4_netlot_clear_configurations(fc4_netlot* l);
bool fc4_netlot_init(fc4_netlot* l); /* §3.2 */
bool fc4_netlot_shutdown(fc4_netlot* l);
/* DoMessage (§6.2). rect: *data2 of 0x07699455 (NULL otherwise). */
bool fc4_netlot_do_message(fc4_netlot* l, uint32_t msg_type, uint32_t data1, uint32_t data2, uint32_t data3,
                           uint64_t void1, uint64_t unknown, const fc4_cell_rect* rect);
bool fc4_netlot_get_lot(const fc4_netlot* l, int32_t x, int32_t z, fc4_netlot_lot* out); /* slot 5; out may be NULL */
int32_t fc4_netlot_zone_type_for_network(const fc4_netlot* l, int32_t x, int32_t z);    /* slot 9, §8.2 */
int32_t fc4_netlot_wealth_for_network(const fc4_netlot* l, int32_t x, int32_t z);       /* slot 10, §8.3 */
/* IsLotCompatible (§8.4) for the loaded configuration `config_id` (false when none is loaded). */
bool fc4_netlot_is_lot_compatible(const fc4_netlot* l, uint32_t config_id, int32_t x, int32_t z, int32_t zone,
                                  int32_t wealth, int32_t rotation, int32_t flip);
bool fc4_netlot_configuration_by_id(const fc4_netlot* l, uint32_t id);     /* slot 7: loaded */
bool fc4_netlot_set_next_configuration(fc4_netlot* l, uint32_t config_id); /* slot 13 */
void fc4_netlot_clear_next_configuration(fc4_netlot* l);                  /* slot 13 with null */
bool fc4_netlot_iterate_lots(const fc4_netlot* l, fc4_netlot_visitor cb, void* ctx); /* slot 11 */
void fc4_netlot_update_roads_in_cell(fc4_netlot* l, int32_t x, int32_t z);          /* §8.5 */
void fc4_netlot_add_pending(fc4_netlot* l, int32_t x, int32_t z);
void fc4_netlot_process_update_list(fc4_netlot* l);
bool fc4_netlot_redevelop_lot(fc4_netlot* l, int32_t x, int32_t z); /* slot 12; false: no lot */
/* Slot 8 CreateProp for the cell's lot: the street prop (borrowed) or 0. */
uint64_t fc4_netlot_create_prop(fc4_netlot* l, int32_t x, int32_t z, uint32_t index, uint32_t id);
void fc4_netlot_road_damage_change(fc4_netlot* l, int32_t x, int32_t z, bool damaged); /* §8.8 */
bool fc4_netlot_get_state(const fc4_netlot* l, fc4_netlot_state* out);
/* The record (§4.2). Read: false unless the version is 2; occupants (IID 0xA821EF94) and
 * effect props (IID 0x09820792) come back through resolve_ref. */
bool fc4_netlot_write(const fc4_netlot* l, const fc4_ostream* out);
bool fc4_netlot_read(fc4_netlot* l, const fc4_istream* in);

/* ---- network rules (docs/specs/network-rules.md; engine/capi/src/network_rules.rs) ----
 * The rule database (the tile rule files with #Transmogrify, RUL0 intersection
 * rules, RUL1 multi-type solutions and overrides, RUL2 rule overrides, the
 * network INI's tile verifiers), tile rule evaluation, and the rule stages of
 * SolveNetwork (§6.1) over an explicit cell grid. No callbacks. Network types
 * are 0..12 (Road, Rail, Highway, Street, Pipe, PowerLine, Avenue, Subway,
 * LightRail, Monorail, OneWayRoad, DirtRoad, GroundHighway). A cell (x, z) has
 * the key (z << 16) + x. */
typedef struct fc4_network_rules fc4_network_rules;
typedef struct fc4_network_rules_grid fc4_network_rules_grid;

/* fc4_network_rules_report.status for a tile file; -1 for an INI resource. */
#define FC4_NR_STATUS_MISSING    0  /* no bytes: the evaluator exists and never matches */
#define FC4_NR_STATUS_COMPLETE   1
#define FC4_NR_STATUS_REJECTED   2  /* the first line is not a '#' header: no rules */
#define FC4_NR_STATUS_BAD_CENTRE 3  /* a centre token other than 1: the rules before it stay */
#define FC4_NR_STATUS_TRUNCATED  4  /* the file ended inside a centre line */

/* The network INI's instance (fc4_network_rules_load). */
#define FC4_NR_NETWORK_INI 0x8A5993B9u

typedef struct fc4_network_rules_report {
    uint32_t struct_size;      /* 36 */
    int32_t status;            /* FC4_NR_STATUS_* or -1 */
    uint32_t transmogrify;     /* the file started with #Transmogrify */
    uint32_t source_rules;     /* rules read from the file */
    uint32_t rules;            /* rules kept (after the Transmogrify expansion) */
    uint32_t dropped_variants; /* Transmogrify variants dropped as repeats */
    uint32_t entries;          /* INI entries used */
    uint32_t ignored;          /* INI entries ignored or failed */
    uint32_t sections;         /* INI sections read */
} fc4_network_rules_report;

typedef struct fc4_network_rules_output {
    uint32_t id;          /* the piece */
    uint8_t m;            /* metaposition 0..24 */
    uint8_t orientation;  /* rot | 0x80 when flipped */
    uint8_t reserved[2];
} fc4_network_rules_output;

typedef struct fc4_network_rules_cell {
    uint32_t struct_size; /* 116 */
    int32_t x, z;
    uint32_t flags;       /* +0x08: type bits 0x1FFF plus other flags (0x40000 bridge, ...) */
    uint32_t edges[13];   /* the edge word per type */
    uint32_t union_edges; /* read back only: the OR of edges[] */
    uint8_t one_way;
    uint8_t in_run;       /* the cell belongs to the drag (its step counts for the unsolved result) */
    uint8_t intersection; /* an intersection piece is there */
    uint8_t fixed;        /* the cell cannot change */
    uint8_t node;         /* a connection node (+0x56) */
    uint8_t has_occupant;
    uint8_t occ_has_node_edges; /* the occupant's slot 13 answers occ_node_edges */
    uint8_t occ_has_anchor;
    int32_t step;         /* the drag step */
    int32_t solved;       /* +0xB8: the solved-record index, -1 for none */
    uint32_t occ_piece;
    uint32_t occ_orientation;
    uint32_t occ_network_flags; /* GetNetworkFlag() */
    uint32_t occ_node_edges[2];
    int32_t occ_anchor[2];
} fc4_network_rules_cell;

typedef struct fc4_network_rules_solved {
    uint32_t id;
    uint32_t orientation;
    int32_t x, z;
} fc4_network_rules_solved;

typedef struct fc4_network_rules_potential {
    int32_t x, z;
    uint32_t types; /* the type bits, or 4 for a run lasting to the last step */
} fc4_network_rules_potential;

/* An empty database; NULL only on failure. */
fc4_network_rules* fc4_network_rules_create(void);
void fc4_network_rules_destroy(fc4_network_rules* r);
/* LoadResource (§2.1) from bytes: `instance` is a rule resource's instance
 * (type 0x0A5BCF4B, group 0xAA5BCF57: a tile file, 0x10000000 RUL0,
 * 0x10000001 RUL1, 0x10000002 RUL2) or FC4_NR_NETWORK_INI. A tile instance
 * appends one evaluator to its type's list; NULL bytes with len 0 is a
 * missing tile file. `out` may be NULL. False on a bad argument. */
bool fc4_network_rules_load(fc4_network_rules* r, uint32_t instance, const uint8_t* bytes, size_t len,
                            fc4_network_rules_report* out);
/* The tile instances of type t in load order (§2.2); the count (0 for PowerLine). */
uint32_t fc4_network_rules_tile_instances(uint32_t t, uint32_t* out, uint32_t cap);
uint32_t fc4_network_rules_evaluator_count(const fc4_network_rules* r, uint32_t t);
uint32_t fc4_network_rules_rule_count(const fc4_network_rules* r, uint32_t t, uint32_t index);
uint32_t fc4_network_rules_intersection_count(const fc4_network_rules* r);
/* Evaluate (§5) with evaluator `index` of type t at (x, z): -1 when no rule
 * passes (or a bad argument), else the output count (at most cap written). */
int32_t fc4_network_rules_evaluate(const fc4_network_rules* r, uint32_t t, uint32_t index,
                                   const fc4_network_rules_grid* g, int32_t x, int32_t z,
                                   fc4_network_rules_output* out, uint32_t cap);
/* The rule stages of SolveNetwork (§6.1) with the tool's type; *result gets
 * its result (the valid count, or the lowest unsolved in-run step, §6.3). */
bool fc4_network_rules_solve(const fc4_network_rules* r, fc4_network_rules_grid* g, uint32_t tool_type,
                             bool resolve, int32_t* result);

/* The cell grid and the solve state (S, the solved list, the potential list). */
fc4_network_rules_grid* fc4_network_rules_grid_create(uint32_t width, uint32_t height);
void fc4_network_rules_grid_destroy(fc4_network_rules_grid* g);
/* Puts a cell info (replacing the one there); false for a NULL or short cell. */
bool fc4_network_rules_grid_set_cell(fc4_network_rules_grid* g, const fc4_network_rules_cell* cell);
bool fc4_network_rules_grid_get_cell(const fc4_network_rules_grid* g, int32_t x, int32_t z,
                                     fc4_network_rules_cell* out);
bool fc4_network_rules_grid_remove_cell(fc4_network_rules_grid* g, int32_t x, int32_t z);
/* Adds (x, z) to S; whether it was new. */
bool fc4_network_rules_grid_add_to_solve(fc4_network_rules_grid* g, int32_t x, int32_t z);
bool fc4_network_rules_grid_set_valid_count(fc4_network_rules_grid* g, int32_t count);
/* Clears the solve state; the cells stay. */
bool fc4_network_rules_grid_reset_solve(fc4_network_rules_grid* g);
/* The solved list, the potential list and the cells left in S (as x, z
 * pairs, 2 * cap integers), in order; each returns the full count. */
uint32_t fc4_network_rules_grid_solved(const fc4_network_rules_grid* g, fc4_network_rules_solved* out, uint32_t cap);
uint32_t fc4_network_rules_grid_potential(const fc4_network_rules_grid* g, fc4_network_rules_potential* out,
                                          uint32_t cap);
uint32_t fc4_network_rules_grid_unsolved(const fc4_network_rules_grid* g, int32_t* xz, uint32_t cap);

/* ---- occupant manager (docs/specs/occupant-manager.md) ---------------------
 * cSC4OccupantManager (0x098F964D): the city's spatial index of occupants, a
 * fixed 192 x 192 grid of 64 m world cells, each a newest-first list of the
 * occupants whose manager box touches it. The shim owns the GZCOM object (and
 * its cIGZSerializable / message-target views) and forwards its slots here.
 * Occupants and the caller's occupant filters are opaque host tokens (0 =
 * null). A missing callback reads as 0 / false / none.
 *
 * Threads (spec §14): each object owns a recursive lock, entered at
 * exactly the points of the §14 table. Any entry point may be called
 * from any thread; the host's callbacks must be safe on the threads that call
 * in, and may call back into the manager. Slot 42 (fc4_occmgr_iterate_by_bbox)
 * runs its callbacks WITHOUT the lock; slot 44 (fc4_occmgr_iterate) holds it
 * for the whole walk. fc4_occmgr_destroy must not race other calls. */
typedef struct fc4_occmgr fc4_occmgr;

/* An occupant's bounding box (occupant slot 10): x, y, z. */
typedef struct fc4_occ_bbox {
    float min[3];
    float max[3];
} fc4_occ_bbox;

/* An occupant's manager box (§4.3): world cells, inclusive, one byte each. */
typedef struct fc4_occ_cell_box {
    uint8_t x0, z0, x1, z1;
} fc4_occ_cell_box;

/* fc4_occ_filter.kind. A NULL filter pointer is a null filter. */
#define FC4_OCC_FILTER_ALL 0u  /* null filter: everything is included */
#define FC4_OCC_FILTER_HOST 1u /* a caller's cISC4OccupantFilter (token; 0 = null): filter_includes decides */
#define FC4_OCC_FILTER_TYPE 2u /* the manager's shared one-type filter with occ_type (§4.4) */

typedef struct fc4_occ_filter {
    uint32_t kind;
    uint32_t occ_type;
    uint64_t token;
} fc4_occ_filter;

typedef struct fc4_occmgr_host {
    uint32_t struct_size; /* sizeof(fc4_occmgr_host) */
    void* user;
    /* AddRef / Release on an occupant. */
    void (*add_ref)(void* user, uint64_t occ);
    void (*release)(void* user, uint64_t occ);
    /* Occupant slots 3 Init / 4 Shutdown (the base posts Create / Destroy, §5.6). */
    void (*init)(void* user, uint64_t occ);
    void (*shutdown)(void* user, uint64_t occ);
    /* Slot 6 AsPropertyHolder() is non-NULL; slot 7 GetType(). */
    bool (*has_property_holder)(void* user, uint64_t occ);
    uint32_t (*occupant_type)(void* user, uint64_t occ);
    /* Slot 10 GetBoundingBox (out starts zeroed). */
    void (*bounding_box)(void* user, uint64_t occ, fc4_occ_bbox* out);
    /* Slot 15 PostOccupantMessage(msg_type, data4) (queued, or sent at once). */
    void (*post_message)(void* user, uint64_t occ, uint32_t msg_type, uint32_t data4);
    /* Slots 25 / 26 Get- / SetOccupantManagerBBox; false = "no box". */
    bool (*manager_bbox)(void* user, uint64_t occ, fc4_occ_cell_box* out);
    void (*set_manager_bbox)(void* user, uint64_t occ, const fc4_occ_cell_box* box);
    /* Slots 29 / 31 / 32 SetFlag / ClearFlag / IsFlagSet. */
    void (*set_flag)(void* user, uint64_t occ, uint32_t flag);
    void (*clear_flag)(void* user, uint64_t occ, uint32_t flag);
    bool (*is_flag_set)(void* user, uint64_t occ, uint32_t flag);
    /* filter.IsOccupantIncluded(occ) of a FC4_OCC_FILTER_HOST filter. */
    bool (*filter_includes)(void* user, uint64_t filter, uint64_t occ);
    /* Read: the occupant a saved reference names, holding one reference for
     * the manager (iid 0x26BB767E); 0 = does not resolve. */
    uint64_t (*resolve_ref)(void* user, uint64_t token, uint32_t iid);
} fc4_occmgr_host;

/* Diagnostics (fc4_occmgr_get_state). */
typedef struct fc4_occmgr_state {
    uint32_t struct_size;
    uint8_t initialized, was_read, destroyed, filter_present;
    uint32_t ref_count;
    uint32_t filter_type; /* the shared filter's type, 0 while it is null */
    float size_x, size_z;
    int32_t counts[4]; /* manager cells X, Z; standard cells X, Z */
    uint32_t rows, holders, nodes;
    uint32_t lock_waits; /* times a thread waited for the lock */
} fc4_occmgr_state;

/* A list query's output: once per appended occupant, in order, after the query
 * returns; each holds one reference for the caller (release with
 * fc4_occmgr_release_list). */
typedef void (*fc4_occ_list_sink)(void* ctx, uint64_t occ);
/* An iteration callback: false stops the walk. */
typedef bool (*fc4_occ_iterate_fn)(void* ctx, uint64_t occ);

/* The object (constructed: reference count 0, not initialised). NULL when host
 * is NULL or too small. */
fc4_occmgr* fc4_occmgr_create(const fc4_occmgr_host* host);
void fc4_occmgr_destroy(fc4_occmgr* m);
/* QueryInterface (§1): the view's offset (0, 4 or 8), -1 for other IIDs. */
int32_t fc4_occmgr_query_interface(uint32_t iid);
uint32_t fc4_occmgr_add_ref(fc4_occmgr* m);
/* At the last reference: Shutdown, then destroyed (the handle stays valid). */
uint32_t fc4_occmgr_release(fc4_occmgr* m);
bool fc4_occmgr_init(fc4_occmgr* m);
bool fc4_occmgr_shutdown(fc4_occmgr* m);
bool fc4_occmgr_set_city_size(fc4_occmgr* m, float fx, float fz);
/* Slots 6-9 and the stored sizes; outputs may be NULL (cell bounds: 4 values). */
bool fc4_occmgr_get_cell_sizes(const fc4_occmgr* m, float* x, float* z);
bool fc4_occmgr_get_world_cell_count(const fc4_occmgr* m, int32_t* x, int32_t* z);
bool fc4_occmgr_get_cell_count(const fc4_occmgr* m, int32_t* x, int32_t* z);
bool fc4_occmgr_get_standard_cell_count(const fc4_occmgr* m, int32_t* x, int32_t* z);
bool fc4_occmgr_get_city_size(const fc4_occmgr* m, float* x, float* z);
bool fc4_occmgr_get_cell_bounds(const fc4_occmgr* m, int32_t out[4]);

/* Conversions, slots 10-19 (§3.2-§3.4; single precision). Outputs may be NULL.
 * Slot 12 [quirk]: *ox = 4*z - 256, *oz = 4 * (the value already in *oz; NULL
 * reads 0); x is ignored. */
void fc4_occ_world_cell_to_manager_cell(int32_t x, int32_t z, int32_t* ox, int32_t* oz);
void fc4_occ_manager_cell_to_world_cell(int32_t x, int32_t z, int32_t* ox, int32_t* oz);
void fc4_occ_world_cell_to_standard_city_cell(int32_t x, int32_t z, int32_t* ox, int32_t* oz);
void fc4_occ_standard_city_cell_to_world_cell(int32_t x, int32_t z, int32_t* ox, int32_t* oz);
void fc4_occ_world_cell_to_position(int32_t x, int32_t z, float* fx, float* fz);
void fc4_occ_position_to_world_cell(float fx, float fz, int32_t* x, int32_t* z);
void fc4_occ_manager_cell_to_position(int32_t x, int32_t z, float* fx, float* fz);
void fc4_occ_position_to_manager_cell(float fx, float fz, int32_t* x, int32_t* z);
void fc4_occ_standard_city_cell_to_position(int32_t x, int32_t z, float* fx, float* fz);
void fc4_occ_position_to_standard_city_cell(float fx, float fz, int32_t* x, int32_t* z);
/* The manager box of a bounding box (§5.1 steps 5-6: bytes, no clamp). */
bool fc4_occ_cell_box_of(const fc4_occ_bbox* box, fc4_occ_cell_box* out);
/* Slot 26 GetBoundingCells: up to 8 (x, z) pairs; returns their number. */
uint32_t fc4_occ_bounding_cells(int32_t x, int32_t z, int32_t out[16]);

/* Slots 20-25, 27 (§5, §6). */
bool fc4_occmgr_insert(fc4_occmgr* m, uint64_t occ, uint32_t data4);
bool fc4_occmgr_remove(fc4_occmgr* m, uint64_t occ, bool shutdown, uint32_t data4);
bool fc4_occmgr_remove_occupants(fc4_occmgr* m, const fc4_occ_filter* filter, bool shutdown, uint32_t data4);
bool fc4_occmgr_remove_occupants_of_type(fc4_occmgr* m, uint32_t occ_type, bool shutdown, uint32_t data4);
bool fc4_occmgr_move(fc4_occmgr* m, uint64_t occ, bool post);
bool fc4_occmgr_is_cell_empty(const fc4_occmgr* m, int32_t x, int32_t z);
/* x / z are written only by the full scan (§15 quirk 7). */
bool fc4_occmgr_find_occupant(const fc4_occmgr* m, uint64_t occ, bool search_all, int32_t* x, int32_t* z);

/* First-occupant queries, slots 28-35 (§7): *out = the occupant (one
 * reference for the caller) or 0. Standard-cell min / max: (x, z) or NULL. */
bool fc4_occmgr_first_by_position(const fc4_occmgr* m, float fx, float fz, const fc4_occ_filter* filter, uint64_t* out);
bool fc4_occmgr_first_by_position_typed(const fc4_occmgr* m, float fx, float fz, uint32_t occ_type, uint64_t* out);
bool fc4_occmgr_first_in_cell(const fc4_occmgr* m, int32_t x, int32_t z, const fc4_occ_filter* filter, uint64_t* out);
bool fc4_occmgr_first_in_cell_typed(const fc4_occmgr* m, int32_t x, int32_t z, uint32_t occ_type, uint64_t* out);
bool fc4_occmgr_first_by_standard_cell(const fc4_occmgr* m, int32_t x, int32_t z, const fc4_occ_filter* filter,
                                       uint64_t* out);
bool fc4_occmgr_first_by_standard_cell_typed(const fc4_occmgr* m, int32_t x, int32_t z, uint32_t occ_type,
                                             uint64_t* out);
bool fc4_occmgr_first_by_standard_cells(const fc4_occmgr* m, const int32_t* min, const int32_t* max,
                                        const fc4_occ_filter* filter, uint64_t* out);
bool fc4_occmgr_first_by_standard_cells_typed(const fc4_occmgr* m, const int32_t* min, const int32_t* max,
                                              uint32_t occ_type, uint64_t* out);

/* List queries, slots 36-41 (§8). bbox min / max: (x, y, z) or NULL; cells:
 * (x, z) or NULL. max_count is compared unsigned (-1 = all; 0 = false). */
bool fc4_occmgr_occupants_by_bbox(const fc4_occmgr* m, const float* min, const float* max,
                                  const fc4_occ_filter* filter, int32_t max_count, fc4_occ_list_sink sink, void* ctx);
bool fc4_occmgr_occupants_by_bbox_typed(const fc4_occmgr* m, const float* min, const float* max, uint32_t occ_type,
                                        int32_t max_count, fc4_occ_list_sink sink, void* ctx);
bool fc4_occmgr_occupants_by_cells(const fc4_occmgr* m, const int32_t* min, const int32_t* max,
                                   const fc4_occ_filter* filter, int32_t max_count, fc4_occ_list_sink sink, void* ctx);
bool fc4_occmgr_occupants_by_cells_typed(const fc4_occmgr* m, const int32_t* min, const int32_t* max,
                                         uint32_t occ_type, int32_t max_count, fc4_occ_list_sink sink, void* ctx);
bool fc4_occmgr_occupants_by_standard_cells(const fc4_occmgr* m, const int32_t* min, const int32_t* max,
                                            const fc4_occ_filter* filter, int32_t max_count, fc4_occ_list_sink sink,
                                            void* ctx);
bool fc4_occmgr_occupants_by_standard_cells_typed(const fc4_occmgr* m, const int32_t* min, const int32_t* max,
                                                  uint32_t occ_type, int32_t max_count, fc4_occ_list_sink sink,
                                                  void* ctx);

/* Iteration, slots 42-47 (§9). A NULL callback is false. Slot 42: min / max
 * (x, y, z) required; slot 44: world cells (x, y, z; y ignored) or NULL; slots
 * 46 / 47: standard cells (x, z) or NULL. */
bool fc4_occmgr_iterate_by_bbox(const fc4_occmgr* m, fc4_occ_iterate_fn fn, void* ctx, const float* min,
                                const float* max, const fc4_occ_filter* filter);
bool fc4_occmgr_iterate_by_bbox_typed(const fc4_occmgr* m, fc4_occ_iterate_fn fn, void* ctx, const float* min,
                                      const float* max, uint32_t occ_type);
bool fc4_occmgr_iterate(const fc4_occmgr* m, fc4_occ_iterate_fn fn, void* ctx, const int32_t* min, const int32_t* max,
                        const fc4_occ_filter* filter);
bool fc4_occmgr_iterate_typed(const fc4_occmgr* m, fc4_occ_iterate_fn fn, void* ctx, const int32_t* min,
                              const int32_t* max, uint32_t occ_type);
bool fc4_occmgr_iterate_by_standard_cells(const fc4_occmgr* m, fc4_occ_iterate_fn fn, void* ctx, const int32_t* min,
                                          const int32_t* max, const fc4_occ_filter* filter);
bool fc4_occmgr_iterate_by_standard_cells_typed(const fc4_occmgr* m, fc4_occ_iterate_fn fn, void* ctx,
                                                const int32_t* min, const int32_t* max, uint32_t occ_type);
/* Slot 48 ReleaseOccupantList: releases each of the n occupants once. */
void fc4_occmgr_release_list(const fc4_occmgr* m, const uint64_t* list, size_t n);

/* Diagnostics. fc4_occmgr_cell: a cell's list, front first (at most capacity
 * occupants and single-cell bits; either output may be NULL); returns the
 * length, -1 for a null cell or one outside the grid. fc4_occmgr_lock_depth:
 * how deep the calling thread holds the lock (0 = not held). */
bool fc4_occmgr_get_state(const fc4_occmgr* m, fc4_occmgr_state* out);
int32_t fc4_occmgr_cell(const fc4_occmgr* m, int32_t x, int32_t z, uint64_t* out, uint8_t* single, size_t capacity);
uint32_t fc4_occmgr_lock_depth(const fc4_occmgr* m);

/* Save record (§13), version 1. Write: occupants as host references (class
 * 0). Read: false unless version 1; occupants through resolve_ref, appended
 * in the saved order; [choice] atomic (nothing but the was-read flag changes
 * when the record fails). */
bool fc4_occmgr_write(const fc4_occmgr* m, const fc4_ostream* out);
bool fc4_occmgr_read(fc4_occmgr* m, const fc4_istream* in);

/* ---- terrain (docs/specs/terrain.md) ------------------------------------------
 * The terrain's core (cSTETerrain, CLSID 0xE98F9525) and the simulation side of
 * its surface water (cSTESurfaceWater) and of the volcano lava (cSTEVolcanoLava):
 * the height field and the per-vertex water depths (depth relative to the ground;
 * "water" = depth > 0), the queries, the height setters with their depth
 * bookkeeping, RedisplayTerrain and the other messages, kind-0 brushes, draft
 * clearance, navigability, the bodies of water, the lava flow and the saved
 * records. No plugin glue yet.
 *
 * Messages: every message the core posts is queued (MessagePost, §8.1). With
 * `post` in the host table, each entry point hands them over in order after it
 * released the terrain; without it they wait for fc4_terrain_take_posts.
 *
 * Threads: every entry point holds a re-entrant lock (one thread at a time; the
 * owner may re-enter). From inside a callback, a read of the same terrain works
 * when the outer call only reads (is_navigable, the bodies of water) and is
 * refused (fallback value, counted in reentrant_refused) when the outer call
 * changes the terrain (a brush's random_uniform, the lava's fire callbacks).
 * Missing callbacks read as 0 / false / none. */
typedef struct fc4_terrain fc4_terrain;

/* An inclusive vertex rectangle (x0, z0, x1, z1): left, top, right, bottom. */
typedef struct fc4_terrain_rect {
    int32_t x0, z0, x1, z1;
} fc4_terrain_rect;

/* A queued message: MessagePost(msg_type, false) with data1..data4 (§8). */
typedef struct fc4_terrain_post {
    uint32_t msg_type;
    uint32_t data1, data2, data3, data4;
} fc4_terrain_post;

/* "Terrain Properties" (§2.4). */
typedef struct fc4_terrain_tuning {
    uint32_t struct_size;
    float max_height;        /* 0x48CD7B26, the raise clamp */
    float min_height;        /* 0x48CD7B27, the lower clamp */
    float new_sea_level;     /* 0x48CD7B21 */
    float new_flat_height;   /* 0x48CD7B2A */
    float image_height_scale;
    float sea_level_min;     /* SetSeaLevel's range (0x48CD7B70..71) */
    float sea_level_max;
    float sea_level_editor[2];
    float cliff_slope;
    float saved_78;          /* 0x48CD7B20, saved in the misc record */
} fc4_terrain_tuning;

/* "Lava Properties" the flow uses (§11.1). */
typedef struct fc4_lava_tuning {
    uint32_t struct_size;
    float max_flow_rate;        /* per microsecond (shipped 4e-6) */
    float full_rate_difference; /* 12 */
    float height_factor;        /* 0 */
    float water_multiplier;     /* 2 */
    float ignite_level;         /* 1.5 */
    float burn_level;           /* 3 */
    float solidify_threshold;   /* 3 */
} fc4_lava_tuning;

/* One brush component (§9.1). The bitmap (bitmap_width x bitmap_height bytes,
 * row-major) is copied; NULL pixels or a zero size = none (the component is skipped). */
typedef struct fc4_brush_component {
    uint32_t kind;          /* 0-11; only kind 0 (raise / lower) changes the terrain */
    float radius[5];        /* per brush level */
    float strength[5];
    uint8_t oriented;
    uint8_t neutral;
    uint8_t _pad[2];
    uint32_t bitmap_width, bitmap_height;
    const uint8_t* pixels;
} fc4_brush_component;

/* tBrushParameters (§9.2). `out` / `out_valid` receive the touched vertex rectangle. */
typedef struct fc4_brush_params {
    float from_x, from_z, to_x, to_z;
    int32_t level;          /* 0-4; < 0 = the terrain's default level */
    uint8_t redisplay;
    uint8_t out_valid;
    uint8_t _pad[2];
    float strength_scale, radius_scale;
    uint32_t data4;         /* RedisplayTerrain's data4 (3 = disaster) */
    fc4_terrain_rect out;
    float target_height;
} fc4_brush_params;

#define FC4_TERRAIN_BRIDGE_NONE 0
#define FC4_TERRAIN_BRIDGE_UNKNOWN_DECK 1
#define FC4_TERRAIN_BRIDGE_DECK 2

typedef struct fc4_terrain_host {
    uint32_t struct_size;
    void* user;
    void (*post)(void* user, const fc4_terrain_post* msg);
    /* RandomUint32Uniform(n), for the random-walk brushes. */
    uint32_t (*random_uniform)(void* user, uint32_t n);
    /* IsNavigable (§10.4): the lot at the cell centre (0 = none); a bridge tile
     * (FC4_TERRAIN_BRIDGE_*, the deck height written for _DECK); a power pole
     * (type 0x2890D4DE) whose cell is (x, z). */
    uint64_t (*lot_at_cell)(void* user, int32_t x, int32_t z);
    int32_t (*bridge_at)(void* user, int32_t x, int32_t z, float* deck);
    bool (*has_power_pole)(void* user, int32_t x, int32_t z);
    /* The lava (§11.4, §11.5): flammability slot 6, fire slots 5 (IgniteCell(x, z,
     * false, true, 0)), 7 (QuickBurnCell) and 9 (SetCause); the burn rectangle's
     * random iterator (false = exhausted: the host resets it, the draw is lost). */
    bool (*is_cell_flammable)(void* user, int32_t x, int32_t z);
    void (*ignite_cell)(void* user, int32_t x, int32_t z);
    void (*quick_burn_cell)(void* user, int32_t x, int32_t z);
    void (*set_cause)(void* user, uint32_t cause);
    bool (*next_burn_cell)(void* user, int32_t* x, int32_t* z);
} fc4_terrain_host;

typedef struct fc4_terrain_state {
    uint32_t struct_size;
    uint32_t cells_x, cells_z, verts_x, verts_z;
    int32_t terrain_type;
    int32_t default_brush_level;
    float sea_level;
    float min_height, max_height; /* the cached (possibly stale) values, §4.6 */
    float saved_78;
    uint8_t view_ready;
    uint8_t water_display_state;
    uint8_t lava_active;
    uint8_t bodies_dirty;
    uint32_t posts_pending;
    uint32_t brush_count;
    uint32_t reentrant_refused;
} fc4_terrain_state;

bool fc4_terrain_tuning_shipped(fc4_terrain_tuning* out);
/* NULL props = no exemplar: the code defaults 200 / 5 for the height clamps. */
bool fc4_terrain_tuning_from_props(const fc4_props* props, fc4_terrain_tuning* out);
bool fc4_lava_tuning_shipped(fc4_lava_tuning* out);

/* Create (§4.6): a flat terrain at the tuning's flat height, the sea at its sea
 * level. NULL tuning = shipped. NULL for a bad host table or 0 / > 4096 cells. */
fc4_terrain* fc4_terrain_create(const fc4_terrain_host* host, const fc4_terrain_tuning* tuning, uint32_t cells_x,
                                uint32_t cells_z);
/* From (cells_x + 1) x (cells_z + 1) altitudes in index order (x + z * vertsX). */
fc4_terrain* fc4_terrain_create_from_altitudes(const fc4_terrain_host* host, const fc4_terrain_tuning* tuning,
                                               uint32_t cells_x, uint32_t cells_z, const float* altitudes, size_t count,
                                               float sea_level);
/* Load (slot 96, §13.1) from the misc, altitude and vertex property payloads.
 * *error := FC4_OK, FC4_ERR_TRUNCATED, FC4_ERR_UNSUPPORTED_VERSION or
 * FC4_ERR_INVALID_ARGUMENT. The water is rebuilt from the tuning's sea level. */
fc4_terrain* fc4_terrain_load(const fc4_terrain_host* host, const fc4_terrain_tuning* tuning, const uint8_t* misc,
                              size_t misc_len, const uint8_t* altitudes, size_t altitudes_len,
                              const uint8_t* properties, size_t properties_len, int32_t* error);
void fc4_terrain_destroy(fc4_terrain* t);
/* DoPostCityInit (slot 40): RedisplayTerrain posts nothing before it. */
bool fc4_terrain_post_city_init(fc4_terrain* t);
bool fc4_terrain_set_view_ready(fc4_terrain* t, bool ready);
/* Copies and removes up to `capacity` queued messages, oldest first. */
size_t fc4_terrain_take_posts(fc4_terrain* t, fc4_terrain_post* out, size_t capacity);
bool fc4_terrain_get_state(const fc4_terrain* t, fc4_terrain_state* out);

/* Grid and height queries (§2-§4). axis / which: 0 = x / minimum, 1 = z / maximum. */
uint32_t fc4_terrain_cell_count(const fc4_terrain* t, uint32_t axis);
uint32_t fc4_terrain_vert_index(const fc4_terrain* t, int32_t x, int32_t z);
bool fc4_terrain_vertex_of(const fc4_terrain* t, uint32_t index, uint32_t* x, uint32_t* z);
bool fc4_terrain_location_is_in_bounds(const fc4_terrain* t, float x, float z);
float fc4_terrain_altitude(const fc4_terrain* t, float x, float z);
float fc4_terrain_altitude_at_index(const fc4_terrain* t, uint32_t index);
float fc4_terrain_altitude_at_vertex(const fc4_terrain* t, int32_t x, int32_t z);
/* The functions that copy into a buffer copy only when `capacity` suffices and
 * always return the full count (call with NULL / 0 to size the buffer). */
size_t fc4_terrain_altitudes_in(const fc4_terrain* t, fc4_terrain_rect rect, float* out, size_t capacity);
size_t fc4_terrain_altitudes(const fc4_terrain* t, float* out, size_t capacity);
/* out[4]: (x, z), (x, z + 1), (x + 1, z + 1), (x + 1, z); untouched (false) out of bounds. */
bool fc4_terrain_cell_corner_altitudes(const fc4_terrain* t, int32_t x, int32_t z, float* out);
float fc4_terrain_altitude_at_nearest_grid(const fc4_terrain* t, float x, float z);
float fc4_terrain_altitude_checksum(const fc4_terrain* t, fc4_terrain_rect rect);
float fc4_terrain_cached_height(const fc4_terrain* t, uint32_t which);
float fc4_terrain_allowed_height(const fc4_terrain* t, uint32_t which);
bool fc4_terrain_set_cached_heights(fc4_terrain* t, float min, float max);
/* GetFaceNormal (slot 26): out[3], untouched (false) unless 0 <= x < sizeX and 0 <= z < sizeZ. */
bool fc4_terrain_face_normal(const fc4_terrain* t, float x, float z, float* out);

/* Water queries (§5, §10.2). */
float fc4_terrain_sea_level(const fc4_terrain* t);
float fc4_terrain_water_level(const fc4_terrain* t, float x, float z);
float fc4_terrain_water_level_at_index(const fc4_terrain* t, uint32_t index);
float fc4_terrain_water_depth_at_vertex(const fc4_terrain* t, int32_t x, int32_t z);
float fc4_terrain_water_depth_at_index(const fc4_terrain* t, uint32_t index);
float fc4_terrain_max_surface_level(const fc4_terrain* t, int32_t x, int32_t z);
bool fc4_terrain_cell_has_water(const fc4_terrain* t, int32_t x, int32_t z);
bool fc4_terrain_cell_has_land(const fc4_terrain* t, int32_t x, int32_t z);
bool fc4_terrain_is_land_vertex(const fc4_terrain* t, int32_t x, int32_t z);
bool fc4_terrain_is_water_vertex(const fc4_terrain* t, int32_t x, int32_t z);
bool fc4_terrain_is_sea_vertex(const fc4_terrain* t, int32_t x, int32_t z);
uint32_t fc4_terrain_number_of_water_vertices(const fc4_terrain* t, fc4_terrain_rect rect);
float fc4_terrain_sediment(const fc4_terrain* t, int32_t x, int32_t z);
bool fc4_terrain_set_water_depth(fc4_terrain* t, int32_t x, int32_t z, float depth);
bool fc4_terrain_reset_water_to_sea_level(fc4_terrain* t);
/* Surface water slots 6, 13, 12 (0 land, 2 fresh, 3 sea) and 8 (through the host). */
float fc4_terrain_water_depth_for_cell(const fc4_terrain* t, int32_t x, int32_t z);
bool fc4_terrain_has_draft_clearance(const fc4_terrain* t, int32_t x, int32_t z);
uint32_t fc4_terrain_water_type(const fc4_terrain* t, int32_t x, int32_t z);
bool fc4_terrain_is_navigable(const fc4_terrain* t, int32_t x, int32_t z, int32_t r, uint64_t lot);
/* ABI 69. GetLerpedWaterLevel (slot 32, §5.1: no clamp at x = sizeX);
 * AltitudeAtWhichFloodingWillOccur (surface water slot 10, §10.2); GetWaterVelocity (surface
 * water slot 11: out[3], a zero vector past the arrays); GetSedimentValue (slot 55, world
 * metres, 0 unless LocationIsInBounds). */
float fc4_terrain_lerped_water_level(const fc4_terrain* t, float x, float z);
float fc4_terrain_flooding_altitude(const fc4_terrain* t, int32_t x, int32_t z);
bool fc4_terrain_water_velocity(const fc4_terrain* t, int32_t x, int32_t z, float* out);
float fc4_terrain_sediment_at(const fc4_terrain* t, float x, float z);

/* Setters (§6). Every setter skips a locked vertex (flag 0x4000). */
bool fc4_terrain_raise_vertex(fc4_terrain* t, uint32_t index, float d);
bool fc4_terrain_lower_vertex(fc4_terrain* t, uint32_t index, float d);
bool fc4_terrain_set_altitude_at_vertex(fc4_terrain* t, uint32_t index, float h);
bool fc4_terrain_set_altitude(fc4_terrain* t, int32_t x, int32_t z, float h);
bool fc4_terrain_set_altitude_rect(fc4_terrain* t, fc4_terrain_rect rect, float h, bool redisplay, uint32_t data4);
bool fc4_terrain_lock_altitude(fc4_terrain* t, fc4_terrain_rect rect, bool lock);
/* SetSeaLevel (slot 45): false when out of the tuning's range (nothing posted). */
bool fc4_terrain_set_sea_level(fc4_terrain* t, float h);
/* No range check, message or water reset. */
bool fc4_terrain_set_sea_level_raw(fc4_terrain* t, float h);
int32_t fc4_terrain_terrain_type(const fc4_terrain* t);
bool fc4_terrain_set_terrain_type(fc4_terrain* t, int32_t type);

/* Vertex flags (§7). */
uint16_t fc4_terrain_vertex_flags(const fc4_terrain* t, int32_t x, int32_t z);
bool fc4_terrain_set_vertex_flags(fc4_terrain* t, int32_t x, int32_t z, uint16_t flags);
bool fc4_terrain_is_triangulation_flipped(const fc4_terrain* t, int32_t x, int32_t z);
uint32_t fc4_terrain_cell_flags(const fc4_terrain* t, int32_t x, int32_t z);
bool fc4_terrain_set_cell_flags(fc4_terrain* t, int32_t x, int32_t z, uint32_t flags);

/* RedisplayTerrain (slot 68) and slot 69 (two cell corners), the long-operation
 * messages (slots 87, 88) and the water display state (slots 83, 84). */
bool fc4_terrain_redisplay(fc4_terrain* t, bool terrain, bool water, fc4_terrain_rect rect, uint32_t data4);
bool fc4_terrain_redisplay_cells(fc4_terrain* t, bool terrain, bool water, int32_t ax, int32_t az, int32_t bx,
                                 int32_t bz, uint32_t data4);
bool fc4_terrain_begin_long_operation(fc4_terrain* t);
bool fc4_terrain_end_long_operation(fc4_terrain* t);
uint8_t fc4_terrain_water_display_state(const fc4_terrain* t);
bool fc4_terrain_set_water_display_state(fc4_terrain* t, uint8_t state);

/* Brushes (§9). add_brush: at most 4 components, kinds 0-11 (false otherwise);
 * random_walk_step in radians (not 0 sets the walk flag); `angle` is the walk angle's start
 * (the host's RandomRange(0, 2*pi) draw); the stroke angle starts at 0 (TR8).
 * add_shipped_brush: the brushes 0x11, 0x12 and 0x14 with the host's bitmap; each walks with
 * the 0.5 degree default step (so every in-bounds ApplyBrush draws random_uniform(5) once) from
 * a walk angle of 0 (set the host's draw with set_brush_angles).
 * brush_angle: the stroke angle. brush_angles / set_brush_angles (ABI 69): the walk angle
 * (+0x68) and the stroke angle (+0x6C); the original's descriptions are process-wide, so a
 * host carries them across cities. talus_angle (ABI 69): kind 2's TalusAngle in radians. */
bool fc4_terrain_add_brush(fc4_terrain* t, uint32_t id, const fc4_brush_component* components, size_t count,
                           float random_walk_step, float angle);
bool fc4_terrain_add_shipped_brush(fc4_terrain* t, uint32_t id, uint32_t bitmap_width, uint32_t bitmap_height,
                                   const uint8_t* pixels);
bool fc4_terrain_brush_angle(const fc4_terrain* t, uint32_t id, float* out);
bool fc4_terrain_brush_angles(const fc4_terrain* t, uint32_t id, float* walk, float* stroke);
bool fc4_terrain_set_brush_angles(fc4_terrain* t, uint32_t id, float walk, float stroke);
float fc4_terrain_talus_angle(const fc4_terrain* t);
bool fc4_terrain_set_talus_angle(fc4_terrain* t, float radians);
/* ApplyBrush (slot 57): false for an unknown brush. */
bool fc4_terrain_apply_brush(fc4_terrain* t, uint32_t id, fc4_brush_params* params);
int32_t fc4_terrain_default_brush_level(const fc4_terrain* t);
bool fc4_terrain_set_default_brush_level(fc4_terrain* t, int32_t level);

/* Bodies of water (§10.3). on_new_day returns whether it rebuilt the map. */
bool fc4_terrain_update_bodies_of_water(fc4_terrain* t);
bool fc4_terrain_bodies_on_new_day(fc4_terrain* t);
bool fc4_terrain_bodies_note_power_pole(fc4_terrain* t, int32_t x, int32_t z);
bool fc4_terrain_bodies_dirty(const fc4_terrain* t);
bool fc4_terrain_set_bodies_dirty(fc4_terrain* t, bool dirty);
uint16_t fc4_terrain_body_of_water(const fc4_terrain* t, int32_t x, int32_t z);
/* x + z * cellsX. */
size_t fc4_terrain_bodies_map(const fc4_terrain* t, uint16_t* out, size_t capacity);

/* The volcano lava (§11): one per terrain; not saved. lava_flow is DoLavaFlow(dt)
 * with dt in microseconds; lava_burn is BurnSurroundingStuff. NULL tuning = shipped. */
bool fc4_terrain_lava_start(fc4_terrain* t, float x, float z, float radius, const fc4_lava_tuning* tuning);
bool fc4_terrain_lava_stop(fc4_terrain* t);
bool fc4_terrain_lava_set_rates(fc4_terrain* t, float production, float solidification);
bool fc4_terrain_lava_flow(fc4_terrain* t, float dt);
bool fc4_terrain_lava_burn(fc4_terrain* t);
float fc4_terrain_lava_level(const fc4_terrain* t, int32_t x, int32_t z);
bool fc4_terrain_lava_set_level(fc4_terrain* t, int32_t x, int32_t z, float v);
bool fc4_terrain_lava_rects(const fc4_terrain* t, fc4_terrain_rect* source, fc4_terrain_rect* active,
                            fc4_terrain_rect* burn);

/* Saved records (§13.1): the payload (version word included) of the misc (0),
 * altitude (1), vertex property (2) or edge (0x64 + side) record; 0 if unknown. */
#define FC4_TERRAIN_RECORD_MISC 0u
#define FC4_TERRAIN_RECORD_ALTITUDES 1u
#define FC4_TERRAIN_RECORD_PROPERTIES 2u
#define FC4_TERRAIN_RECORD_EDGE 0x64u
size_t fc4_terrain_write_record(const fc4_terrain* t, uint32_t record, uint8_t* out, size_t capacity);
/* An edge record's altitudes: the count, or -1 for a bad record. */
int64_t fc4_terrain_read_edge(const uint8_t* record, size_t len, float* out, size_t capacity);

/* ---- effects manager (docs/specs/effects.md; engine/capi/src/effects.rs) ------
 * The visual effects' simulation side: the effect directory (resource
 * 0xEA5118B0 / 0xEA5118B1 / 1, parsed from the bytes the host reads), the
 * manager (creation by name under the detail setting, LOD ranges and select
 * groups, the animation tick, finishing), the brush effect (terrain brushes
 * through the host, e.g. fc4_terrain_apply_brush) and the game effect (fire,
 * toxic spills, demolition, extinguishing, message posts, pauses). Rendering is
 * the host's. Effects are named by IDs from 1. A missing callback does nothing
 * and reads as 0 / false; `simulator` missing or returning false = no simulator.
 * Finishing (EF2): by default the host calls fc4_effects_finish_hook once per
 * frame after the tick; in the compat mode (FC4_EFFECTS_FINISH_DRAW_PASS) only
 * fc4_effects_draw_pass finishes effects, as the original's draw pass does.
 * A call into the same manager from inside a callback is refused (the fallback
 * value). */
typedef struct fc4_effects_directory fc4_effects_directory;
typedef struct fc4_effects fc4_effects;

#define FC4_EFFECTS_FINISH_HOOK      0u
#define FC4_EFFECTS_FINISH_DRAW_PASS 1u
#define FC4_EFFECTS_UNKNOWN          (-2)

/* A post: an effect's end message (data2 = data3 = 0) or a game record's message
 * (data1 = the instance parameter, data2 / data3 = the cell). */
typedef struct fc4_effects_post {
    uint32_t msg_type;
    int32_t data2;
    uint64_t data1;
    int32_t data3;
    uint32_t reserved;
} fc4_effects_post;

/* An inclusive cell rectangle, clipped to the city. */
typedef struct fc4_effects_rect {
    int32_t x0, z0, x1, z1;
} fc4_effects_rect;

/* A game record's demolition filter (§9.3). */
typedef struct fc4_effects_demolish_filter {
    bool no_networks;
    bool no_flora;
    uint8_t _pad[2];
    float min_size;
    float max_size;
} fc4_effects_demolish_filter;

/* What the demolition filter needs of an occupant. */
typedef struct fc4_effects_occupant_facts {
    bool signpost;
    bool text_decal;
    bool network;
    bool flora;
    float extents[3];
} fc4_effects_occupant_facts;

typedef struct fc4_effects_host {
    uint32_t struct_size;
    void* user;
    /* Whether there is a simulator; *dilation = its slot 32. */
    bool (*simulator)(void* user, float* dilation);
    void (*sim_pause)(void* user);
    void (*sim_resume)(void* user);
    void (*sim_hidden_pause)(void* user);
    void (*sim_hidden_resume)(void* user);
    void (*clock_pause)(void* user, bool on);
    void (*city_cells)(void* user, int32_t* x, int32_t* z);
    bool (*location_in_bounds)(void* user, float x, float z);
    /* ApplyBrush(brush_id, params) on the terrain. */
    bool (*apply_brush)(void* user, uint32_t brush_id, fc4_brush_params* params);
    void (*begin_long_terrain_operation)(void* user);
    void (*end_long_terrain_operation)(void* user);
    /* The fire simulator's IgniteCellRadius and SetCause. */
    void (*ignite_cell_radius)(void* user, int32_t x, int32_t z, int32_t r0, int32_t r1);
    void (*set_fire_cause)(void* user, uint32_t cause);
    /* The toxic-spill disaster's attempt at a cell. */
    void (*toxic_attempt_start)(void* user, int32_t x, int32_t z);
    /* Demolish the occupants of `rect` the filter does not skip (fc4_effects_demolish_filter_skips). */
    void (*demolish_region)(void* user, const fc4_effects_rect* rect, uint32_t flags, bool dezone,
                            const fc4_effects_demolish_filter* filter);
    void (*extinguish_cell)(void* user, int32_t x, int32_t z, int32_t amount);
    void (*post)(void* user, const fc4_effects_post* msg);
    /* IsActive of a render component (`component` its index in the effect); missing = inactive. */
    bool (*render_component_active)(void* user, uint32_t effect, uint32_t component, uint8_t kind);
} fc4_effects_host;

typedef struct fc4_effects_directory_counts {
    uint32_t brushes, attractors, games, effects, names;
    uint16_t root_version;
    uint16_t effects_version;
} fc4_effects_directory_counts;

typedef struct fc4_effects_state {
    uint64_t created, recycled, end_posts, select_draws, ticks, sweeps;
    uint64_t total_us;          /* accumulated dilated microseconds */
    float total_seconds;
    uint32_t effect_count, active_count, level_of_detail, max_priority, finish_mode;
} fc4_effects_state;

typedef struct fc4_effect_info {
    int32_t index;              /* the description; -1 = none (a no-op) */
    uint32_t flags, refs;
    bool started, stopping, active;
    uint8_t _pad;
    uint32_t components;        /* built */
    uint32_t end_type;
    uint64_t end_data1;
    float position[3];
    uint32_t reserved;
} fc4_effect_info;

typedef struct fc4_effects_brush_state {
    uint32_t brush_id;
    float rate, length, strength, radius, acc, t;
    float prev[2], cur[2];
    bool one_shot, active, long_op;
    uint8_t _pad;
    uint64_t applications;
} fc4_effects_brush_state;

typedef struct fc4_effects_game_state {
    bool started, pending, map_applied;
    uint8_t _pad;
    float timer;
    uint32_t count, applies;
    int32_t cell_x, cell_z;
    uint64_t data;
} fc4_effects_game_state;

/* The directory: parsed from the resource's bytes (NULL if it does not parse), or the
 * small synthetic volcano / meteor-crater directory (tests, hosts without an install). */
fc4_effects_directory* fc4_effects_directory_parse(const uint8_t* data, size_t len);
fc4_effects_directory* fc4_effects_directory_volcano(void);
void fc4_effects_directory_destroy(fc4_effects_directory* d);
bool fc4_effects_directory_get_counts(const fc4_effects_directory* d, fc4_effects_directory_counts* out);
/* The description `name` resolves to under `max_priority`: its index, -1 if none
 * qualifies, FC4_EFFECTS_UNKNOWN for an unknown name. */
int32_t fc4_effects_directory_find(const fc4_effects_directory* d, const char* name, uint32_t max_priority);
/* Writes the resource (the parser's inverse) when `cap` suffices; returns the size needed. */
size_t fc4_effects_directory_write(const fc4_effects_directory* d, uint8_t* buf, size_t cap);

/* The manager. NULL for a NULL or short host table. `session_seed` seeds the brush
 * generator; the select generator uses the constant 0x1105. */
fc4_effects* fc4_effects_create(const fc4_effects_host* host, uint32_t session_seed);
void fc4_effects_destroy(fc4_effects* fx);
/* Shares the directory (the caller may destroy its handle afterwards). */
bool fc4_effects_set_directory(fc4_effects* fx, const fc4_effects_directory* d);
/* The detail setting (default 5). */
void fc4_effects_set_max_priority(fc4_effects* fx, uint32_t max_priority);
/* Level of detail = max(zoom - offset + 1, 1); rebuilds every effect. */
void fc4_effects_set_zoom(fc4_effects* fx, int32_t zoom, int32_t offset);
bool fc4_effects_set_finish_mode(fc4_effects* fx, uint32_t mode);
/* Whether meta components also update their children (the original's double update, EF9; default true). */
void fc4_effects_set_nested_update(fc4_effects* fx, bool on);
bool fc4_effects_has(const fc4_effects* fx, const char* name);
/* CreateVisualEffect: the new ID (two references: the caller's and the manager's).
 * False for an unknown name; no description under the detail setting = a no-op (index -1). */
bool fc4_effects_create_effect(fc4_effects* fx, const char* name, uint32_t* out_id);
bool fc4_effects_create_effect_by_index(fc4_effects* fx, uint32_t index, uint32_t* out_id);
bool fc4_effects_add_ref(fc4_effects* fx, uint32_t id);
bool fc4_effects_release(fc4_effects* fx, uint32_t id);
bool fc4_effects_start(fc4_effects* fx, uint32_t id, uint32_t mode);
/* mode 0 = soft (unless the description's hard-stop flag), otherwise hard. */
bool fc4_effects_stop(fc4_effects* fx, uint32_t id, uint32_t mode);
bool fc4_effects_is_started(const fc4_effects* fx, uint32_t id);
bool fc4_effects_is_stopping(const fc4_effects* fx, uint32_t id);
bool fc4_effects_set_position(fc4_effects* fx, uint32_t id, float x, float y, float z);
/* Posted when the effect finishes (type 0 = none). */
bool fc4_effects_set_end_message(fc4_effects* fx, uint32_t id, uint32_t msg_type, uint64_t data1);
/* SetParameter, forwarded to the components and children (13 = the game records' message data). */
bool fc4_effects_forward_parameter(fc4_effects* fx, uint32_t id, uint32_t index, uint64_t value);
/* The animation tick (microseconds, scaled by the simulator's dilation; runs while paused). */
void fc4_effects_tick(fc4_effects* fx, uint32_t us);
/* The finishing pass of the mode: whether it ran. */
bool fc4_effects_finish_hook(fc4_effects* fx);
bool fc4_effects_draw_pass(fc4_effects* fx);
/* Stops every effect (hard) and forgets them. */
void fc4_effects_shutdown(fc4_effects* fx);
bool fc4_effects_get_state(const fc4_effects* fx, fc4_effects_state* out);
bool fc4_effects_get_effect(const fc4_effects* fx, uint32_t id, fc4_effect_info* out);
/* The name, NUL-terminated (truncated to cap - 1); returns its length (0 = unknown ID). */
size_t fc4_effects_get_name(const fc4_effects* fx, uint32_t id, char* buf, size_t cap);
/* The child of the effect's n-th built meta component. */
bool fc4_effects_get_child(const fc4_effects* fx, uint32_t id, uint32_t n, uint32_t* out_id);
bool fc4_effects_get_brush(const fc4_effects* fx, uint32_t id, uint32_t n, fc4_effects_brush_state* out);
bool fc4_effects_get_game(const fc4_effects* fx, uint32_t id, uint32_t n, fc4_effects_game_state* out);
/* Whether the filter keeps (skips) the occupant: signposts and text decals always; networks
 * and flora by the flags; max_size > 0: a largest extent >= max; min_size > 0: a smallest <= min. */
bool fc4_effects_demolish_filter_skips(const fc4_effects_demolish_filter* f, const fc4_effects_occupant_facts* o);

/* ---- automata layer (docs/specs/automata.md, short form; engine/capi/src/automata.rs) ----
 * The saved records of components 40, 41 and 43-46 and of the objects they
 * reference, the slots other specs call, and the riot, fire and U-Drive-It
 * hooks. Movement and animation are not modelled. Main thread only.
 *
 * Objects: an fc4_automata_object is a handle to one core controller
 * (generator 0x4A1628C5, attractor 0x6A1628BA) or automaton (pedestrian
 * 0x896E75AF, vehicle 0x896E714A, patrol car 0x4A222B98, trailer 0x4A5B69ED,
 * emergency vehicle); each handle owns one reference on the shared object
 * and the managers hold their own. When a stream reads a reference to one of
 * these classes, the host passes the object's handle as fc4_ref.core_handle
 * (and nothing else there). A core object is written with the token given by
 * fc4_automata_object_set_token.
 *
 * References: a reference read from a stream comes with one reference owned
 * by the reader. The controller manager and the automata managers release,
 * through their host tables, every host reference they drop (removal, the
 * record a successful Read replaces, a failed Read, destroy) and take one
 * (add_ref) on every host object they start listing. The references inside
 * an object's record are the host's (fc4_automata_object_refs). A missing
 * callback reads as 0 / false / none. */
typedef struct fc4_automata_object fc4_automata_object;
typedef struct fc4_automata_scripts fc4_automata_scripts;
typedef struct fc4_automata_cm fc4_automata_cm;
typedef struct fc4_automata_manager fc4_automata_manager;
typedef struct fc4_automata_riot fc4_automata_riot;
typedef struct fc4_automata_fire fc4_automata_fire;

/* fc4_automata_scripts_define_template kinds. */
#define FC4_AUTOMATA_GENERATOR 0u
#define FC4_AUTOMATA_ATTRACTOR 1u
/* fc4_automata_automaton_state.current_state when the stack is empty. */
#define FC4_AUTOMATA_NO_STATE 0xFFFFFFFFu
/* State types of §6.2 that this ABI names. DISAPPEAR is what SetState(1, ...)
 * sets at a riot's end and what PopState pushes in place of a `final` state. */
#define FC4_AUTOMATA_STATE_DEFAULT 0u
#define FC4_AUTOMATA_STATE_DISAPPEAR 1u

/* A controller time (§1 `time`): hour 0 <= h < 24, day of week 0-6. */
typedef struct fc4_automata_time {
    float hour;
    int16_t day;
    uint16_t reserved;
} fc4_automata_time;

/* CreateAutomaton's seven arguments (§7.2): group, position, flags, the
 * model key (a `key3`, all zero = chosen for the group), the callback target
 * (a host token, 0 = null), the priority (0xFF forces the creation) and the
 * orientation (`const vec3*`, present when `has_orientation` is 1). */
typedef struct fc4_automata_create_args {
    uint32_t group;
    float position[3];
    uint32_t flags;
    uint32_t model_key[3]; /* type, group, instance */
    uint64_t callback;
    uint8_t priority;
    uint8_t has_orientation;
    uint16_t reserved;
    float orientation[3];
} fc4_automata_create_args;

/* AddRef / Release on host objects (managers, failed object reads). */
typedef struct fc4_automata_ref_host {
    uint32_t struct_size; /* sizeof(fc4_automata_ref_host) */
    void* user;
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
} fc4_automata_ref_host;

typedef struct fc4_automata_cm_host {
    uint32_t struct_size; /* sizeof(fc4_automata_cm_host) */
    void* user;
    /* A log line (§3.4 step 2), UTF-8, NUL-terminated. */
    void (*log)(void* user, const char* text);
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    /* A host controller's flags (its slot 12 CheckFlag); flag 8 = never saved (§3.2). */
    uint32_t (*controller_flags)(void* user, uint64_t controller);
} fc4_automata_cm_host;

/* The game calls of the riot leader hooks (§8.1), in the order the core makes them. */
typedef struct fc4_automata_riot_host {
    uint32_t struct_size; /* sizeof(fc4_automata_riot_host) */
    void* user;
    /* The nearest road tile's position (3 floats); false = none. */
    bool (*nearest_road_tile)(void* user, int32_t x, int32_t z, float* position);
    /* Whether `leaders` leaders reach the cap. */
    bool (*leader_cap_reached)(void* user, uint32_t leaders);
    /* Pedestrian manager (city slot 44) slot 15; 0 = null. */
    uint64_t (*create_automaton)(void* user, const fc4_automata_create_args* args);
    void (*add_ref)(void* user, uint64_t automaton);
    /* Automaton slot 51 SetState(type, a, b, c, d, e, f). */
    void (*set_state)(void* user, uint64_t automaton, uint32_t state_type, uint32_t a, float b, uint32_t c,
                      uint32_t d, uint32_t e, uint32_t f);
    /* Automaton slot 9 SetVisible. */
    void (*set_visible)(void* user, uint64_t automaton, bool visible);
    /* Automaton slot 5 AsOccupant. */
    uint64_t (*as_occupant)(void* user, uint64_t automaton);
    /* Disaster layer (city slot 72): register the leader's location (3 floats). */
    void (*register_location)(void* user, uint64_t automaton, const float* position);
    /* Controller manager slot 5 AddControllerFromTemplateName(occupant, name, extra). */
    bool (*add_controller_from_template_name)(void* user, uint64_t occupant, const char* name, uint32_t extra);
    /* Controller manager slot 14 RemoveControllers(occupant). */
    void (*remove_controllers)(void* user, uint64_t occupant);
    /* Disaster layer slot 15: unregister the location by occupant. */
    void (*unregister_location)(void* user, uint64_t occupant);
    void (*release)(void* user, uint64_t automaton);
} fc4_automata_riot_host;

/* The game calls of the fire occupant hooks (§8.2). */
typedef struct fc4_automata_fire_host {
    uint32_t struct_size; /* sizeof(fc4_automata_fire_host) */
    void* user;
    uint32_t (*occupant_type)(void* user, uint64_t occupant);
    /* Controller manager slot 10 AddControllersFromOccupantGroup(name, occupant, extra). */
    bool (*add_controllers_from_occupant_group_name)(void* user, const char* name, uint64_t occupant, uint32_t extra);
    /* Controller manager slot 14 RemoveControllers(occupant). */
    void (*remove_controllers)(void* user, uint64_t occupant);
} fc4_automata_fire_host;

typedef struct fc4_automata_controller_state {
    uint32_t struct_size;
    uint8_t template_bound, initialized, in_grid, shut_down;
    uint32_t flags;
    uint32_t automata;       /* the automata it controls */
    uint64_t occupant_token; /* the occupant it is attached to */
    uint32_t occupant_class;
    uint32_t reserved;
} fc4_automata_controller_state;

typedef struct fc4_automata_automaton_state {
    uint32_t struct_size;
    uint32_t group; /* the automata group it was created for (0 = unknown; not saved) */
    float position[3];
    uint8_t visible, removed, returning, emergency;
    uint32_t states;        /* the state stack's depth */
    uint32_t current_state; /* the top state's type, or FC4_AUTOMATA_NO_STATE */
} fc4_automata_automaton_state;

typedef struct fc4_automata_cm_state {
    uint32_t struct_size;
    uint8_t loaded;
    uint8_t reserved[3];
    uint32_t occupants, controllers, sets;
    fc4_automata_time time_a, time_b;
    uint32_t clock_class;
    uint64_t clock_token;
} fc4_automata_cm_state;

typedef struct fc4_automata_manager_state {
    uint32_t struct_size;
    uint32_t class_id; /* the manager's class (§0) */
    uint8_t initialized, active, paused, loaded;
    uint8_t old_version, time_scaling;
    uint8_t reserved[2];
    int32_t max_count, hard_limit;
    uint32_t automata, group_counts;
    uint16_t version, base_version; /* as read; a Write keeps them unless the data needs more */
} fc4_automata_manager_state;

typedef struct fc4_automata_riot_leader {
    uint64_t automaton;
    float position[3];
    int32_t stage, max_stage;
    uint32_t reserved;
} fc4_automata_riot_leader;

/* Objects. create: an empty record of the current versions; NULL for any
 * other class (an emergency vehicle loads as a vehicle, §12 quirk 3). */
fc4_automata_object* fc4_automata_object_create(uint32_t class_id);
fc4_automata_object* fc4_automata_object_create_emergency(void);
/* Another handle to the same object. */
fc4_automata_object* fc4_automata_object_clone(const fc4_automata_object* o);
void fc4_automata_object_destroy(fc4_automata_object* o);
bool fc4_automata_object_same(const fc4_automata_object* a, const fc4_automata_object* b);
uint32_t fc4_automata_object_class(const fc4_automata_object* o);
bool fc4_automata_object_is_controller(const fc4_automata_object* o);
/* Controllers answer 0x6A1857F8; automata 0xA9B40F05 and 0x26BB767E; both
 * cIGZSerializable 0xE4FDA3D4 and cIGZUnknown 1. */
bool fc4_automata_object_query_interface(const fc4_automata_object* o, uint32_t iid);
void fc4_automata_object_set_token(const fc4_automata_object* o, uint64_t token);
uint64_t fc4_automata_object_token(const fc4_automata_object* o);
/* The record of the object's class (§3.5, §6). Read: on failure nothing
 * changes and the host references read are released through `refs` (NULL = none). */
bool fc4_automata_object_write(const fc4_automata_object* o, const fc4_ostream* out);
bool fc4_automata_object_read(const fc4_automata_object* o, const fc4_istream* in, const fc4_automata_ref_host* refs);
/* The host references its record writes (each occurrence). Returns the count; copies at most `capacity`. */
size_t fc4_automata_object_refs(const fc4_automata_object* o, uint64_t* out, size_t capacity);
/* Controllers (no-ops on automata). */
uint32_t fc4_automata_controller_flags(const fc4_automata_object* o);
void fc4_automata_controller_set_flags(const fc4_automata_object* o, uint32_t flags);
/* The template name's length; copies it NUL-terminated (truncated to capacity - 1). */
size_t fc4_automata_controller_template(const fc4_automata_object* o, char* out, size_t capacity);
bool fc4_automata_controller_get_state(const fc4_automata_object* o, fc4_automata_controller_state* out);
/* Automata (no-ops on controllers): slots 51 SetState (the stack becomes one
 * state of that type), 52 PushState, 53 PopState, 9 SetVisible, 8
 * SetPosition (3 floats), 13 Remove, 29 ReturnToSourceOccupant.
 * SetState and PushState take the full PushState signature (§6.2):
 * controller, timeout_ms, final, animId, sfxId, behaviourTemplate. Both return
 * false and change nothing while the top state has `final` (§12 quirk 11).
 * PushState(0, ...) delegates to SetState. PopState on a `final` state that
 * is not already type 1 pushes DISAPPEAR (type 1) instead of revealing the
 * state below. */
bool fc4_automata_automaton_set_state(const fc4_automata_object* o, uint32_t state_type, uint32_t controller,
                                      float timeout_ms, uint32_t is_final, uint32_t anim_id, uint32_t sfx_id,
                                      uint32_t behaviour_template);
bool fc4_automata_automaton_push_state(const fc4_automata_object* o, uint32_t state_type, uint32_t controller,
                                       float timeout_ms, uint32_t is_final, uint32_t anim_id, uint32_t sfx_id,
                                       uint32_t behaviour_template);
bool fc4_automata_automaton_pop_state(const fc4_automata_object* o);
void fc4_automata_automaton_set_visible(const fc4_automata_object* o, bool visible);
void fc4_automata_automaton_set_position(const fc4_automata_object* o, const float* position);
void fc4_automata_automaton_remove(const fc4_automata_object* o);
void fc4_automata_automaton_return_to_source(const fc4_automata_object* o);
bool fc4_automata_automaton_get_state(const fc4_automata_object* o, fc4_automata_automaton_state* out);

/* Component 40, the script system (§2): the templates and occupant groups
 * the scripts define (not saved). The record is u16 1; Read fails on anything else. */
fc4_automata_scripts* fc4_automata_scripts_create(void);
void fc4_automata_scripts_destroy(fc4_automata_scripts* s);
bool fc4_automata_scripts_init(fc4_automata_scripts* s);
bool fc4_automata_scripts_shutdown(fc4_automata_scripts* s);
/* Slot 16 CreateControllerTemplate: kind FC4_AUTOMATA_GENERATOR / _ATTRACTOR;
 * a template of the same name is replaced. */
bool fc4_automata_scripts_define_template(fc4_automata_scripts* s, const char* name, uint32_t kind, uint32_t flags);
/* Slot 13 CreateOccupantGroup: the template names it adds, in order; the same ID is replaced. */
bool fc4_automata_scripts_define_group(fc4_automata_scripts* s, uint32_t id, const char* name,
                                       const char* const* templates, size_t count);
size_t fc4_automata_scripts_template_count(const fc4_automata_scripts* s);
size_t fc4_automata_scripts_group_count(const fc4_automata_scripts* s);
bool fc4_automata_scripts_write(const fc4_automata_scripts* s, const fc4_ostream* out);
bool fc4_automata_scripts_read(fc4_automata_scripts* s, const fc4_istream* in);

/* Component 41, the controller manager (§3, §7.1). `scripts` may be NULL
 * (no script system). The host table is copied; NULL = no callbacks. */
fc4_automata_cm* fc4_automata_cm_create(const fc4_automata_cm_host* host);
/* Releases every host reference the record holds. */
void fc4_automata_cm_destroy(fc4_automata_cm* c);
/* Write (§3.1): controllers with flag 8 are left out, and occupants left with none. */
bool fc4_automata_cm_write(const fc4_automata_cm* c, const fc4_ostream* out);
/* Read (§3.1): sets the loaded flag; a repeated occupant's controllers are
 * dropped (§12 quirk 2); on failure nothing changes. */
bool fc4_automata_cm_read(fc4_automata_cm* c, const fc4_istream* in);
bool fc4_automata_cm_is_loaded(const fc4_automata_cm* c);
/* The 24-hour clock reference (§3.1 field 2); the manager takes a reference. */
void fc4_automata_cm_set_clock(fc4_automata_cm* c, const fc4_ref* clock);
bool fc4_automata_cm_set_times(fc4_automata_cm* c, const fc4_automata_time* a, const fc4_automata_time* b);
/* Slot 24 GetControllerTime: time B. */
bool fc4_automata_cm_controller_time(const fc4_automata_cm* c, fc4_automata_time* out);
/* Slot 5: NULL without such a template; else a new handle (the caller destroys it). */
fc4_automata_object* fc4_automata_cm_add_controller_from_template_name(fc4_automata_cm* c,
                                                                       const fc4_automata_scripts* scripts,
                                                                       const fc4_ref* occupant, const char* name,
                                                                       uint32_t extra);
/* Slots 9 / 10: false with no script system or no such group. */
bool fc4_automata_cm_add_controllers_from_group_id(fc4_automata_cm* c, const fc4_automata_scripts* scripts,
                                                   uint32_t group_id, const fc4_ref* occupant, uint32_t extra);
bool fc4_automata_cm_add_controllers_from_group_name(fc4_automata_cm* c, const fc4_automata_scripts* scripts,
                                                     const char* name, const fc4_ref* occupant, uint32_t extra);
/* Slots 11 / 12: the occupant's controllers whose template is one of the group's. */
bool fc4_automata_cm_remove_controllers_from_group_id(fc4_automata_cm* c, const fc4_automata_scripts* scripts,
                                                      uint32_t group_id, const fc4_ref* occupant);
bool fc4_automata_cm_remove_controllers_from_group_name(fc4_automata_cm* c, const fc4_automata_scripts* scripts,
                                                        const char* name, const fc4_ref* occupant);
/* Slot 14 RemoveControllers: the count removed. */
size_t fc4_automata_cm_remove_controllers(fc4_automata_cm* c, const fc4_ref* occupant);
/* RemoveOccupant (0x99EF1143, §10): its controllers and automaton sets go. */
void fc4_automata_cm_occupant_removed(fc4_automata_cm* c, const fc4_ref* occupant);
/* PostCityInit (§3.4): only when loaded; unknown templates are logged and
 * dropped. The number dropped; -1 for NULL scripts (nothing happens). */
int32_t fc4_automata_cm_post_city_init(fc4_automata_cm* c, const fc4_automata_scripts* scripts);
size_t fc4_automata_cm_controller_count(const fc4_automata_cm* c);
/* Controller `index` (entry by entry): `out` (may be NULL) gets its
 * reference; a new handle for a core controller, NULL for a host one or out of range. */
fc4_automata_object* fc4_automata_cm_controller_at(const fc4_automata_cm* c, size_t index, fc4_ref* out);
bool fc4_automata_cm_get_state(const fc4_automata_cm* c, fc4_automata_cm_state* out);

/* Components 43-46 (§4, §5, §7.3): vehicle 0xA98F9D3B, pedestrian
 * 0xC98F9D17, aircraft 0xC98F9D4A, watercraft 0xA98F9D28; NULL for any
 * other class. `refs` is copied (NULL = none). */
fc4_automata_manager* fc4_automata_manager_create(uint32_t class_id, const fc4_automata_ref_host* refs);
void fc4_automata_manager_destroy(fc4_automata_manager* m);
bool fc4_automata_manager_write(const fc4_automata_manager* m, const fc4_ostream* out);
/* Read: sets the loaded and old-version flags; on failure nothing changes. */
bool fc4_automata_manager_read(fc4_automata_manager* m, const fc4_istream* in);
/* Slot 3 Init applies the saved max count. */
bool fc4_automata_manager_init(fc4_automata_manager* m);
bool fc4_automata_manager_shutdown(fc4_automata_manager* m);
void fc4_automata_manager_activate(fc4_automata_manager* m, bool on);
bool fc4_automata_manager_is_active(const fc4_automata_manager* m);
/* Slots 9-11 (U-Drive-It, §9). */
void fc4_automata_manager_pause(fc4_automata_manager* m);
void fc4_automata_manager_resume(fc4_automata_manager* m);
bool fc4_automata_manager_is_paused(const fc4_automata_manager* m);
/* Slot 13: `view` is the U-Drive-It view 0x2B709676. */
bool fc4_automata_manager_is_driving_mode(const fc4_automata_manager* m, uint32_t view);
/* Slot 23 SetMaxCount (§7.3): clamp 0-5000, remove the newest beyond it,
 * hard limit trunc(n x 1.25). The number removed. */
size_t fc4_automata_manager_set_max_count(fc4_automata_manager* m, int32_t n);
size_t fc4_automata_manager_automata_count(const fc4_automata_manager* m);
/* As fc4_automata_cm_controller_at. */
fc4_automata_object* fc4_automata_manager_automaton_at(const fc4_automata_manager* m, size_t index, fc4_ref* out);
/* Slots 7 / 8. */
bool fc4_automata_manager_add_automaton(fc4_automata_manager* m, const fc4_ref* automaton);
bool fc4_automata_manager_remove_automaton(fc4_automata_manager* m, const fc4_ref* automaton);
/* Slot 15 (stub): a pedestrian / vehicle at the position, listed; NULL at
 * the hard limit and for aircraft and watercraft. A new handle. */
fc4_automata_object* fc4_automata_manager_create_automaton(fc4_automata_manager* m,
                                                           const fc4_automata_create_args* args);
/* Slots 31 / 32. */
void fc4_automata_manager_track_group(fc4_automata_manager* m, uint32_t group, int32_t delta);
uint32_t fc4_automata_manager_instances_of_group(const fc4_automata_manager* m, uint32_t group);
/* RemoveOccupant (§10): its airport pairs and ferry entries go. */
void fc4_automata_manager_occupant_removed(fc4_automata_manager* m, const fc4_ref* occupant);
bool fc4_automata_manager_get_state(const fc4_automata_manager* m, fc4_automata_manager_state* out);
/* U-Drive-It (§9): Pause / Resume on each manager of the array (NULL entries skipped). */
void fc4_automata_pause_all(fc4_automata_manager* const* managers, size_t count);
void fc4_automata_resume_all(fc4_automata_manager* const* managers, size_t count);

/* Riot leaders (§8.1). NULL for a NULL or short host table. */
fc4_automata_riot* fc4_automata_riot_create(const fc4_automata_riot_host* host);
void fc4_automata_riot_destroy(fc4_automata_riot* r);
/* AddRiotLeader: the new leader's index, or -1. */
int32_t fc4_automata_riot_add(fc4_automata_riot* r, int32_t x, int32_t z);
/* A new maximum stage below 6 adds the "riot_leader_<stage>" controller; whether it did. */
bool fc4_automata_riot_set_stage(fc4_automata_riot* r, uint32_t index, int32_t stage);
/* The end; the leader is erased (later indices move down). */
bool fc4_automata_riot_end(fc4_automata_riot* r, uint32_t index);
uint32_t fc4_automata_riot_count(const fc4_automata_riot* r);
bool fc4_automata_riot_get_leader(const fc4_automata_riot* r, uint32_t index, fc4_automata_riot_leader* out);

/* The fire occupant (§8.2). NULL for a NULL or short host table. */
fc4_automata_fire* fc4_automata_fire_create(const fc4_automata_fire_host* host);
void fc4_automata_fire_destroy(fc4_automata_fire* f);
/* SetTarget: a building (0x278128A0) gets the "fire_occupant" controllers; whether the call was made. */
bool fc4_automata_fire_set_target(fc4_automata_fire* f, uint64_t target);
/* Shutdown: RemoveControllers(target); false without a target. */
bool fc4_automata_fire_shutdown(fc4_automata_fire* f);
uint64_t fc4_automata_fire_target(const fc4_automata_fire* f);
/* ---- advisor system and MySims (docs/specs/advisors.md, docs/specs/mysims.md) --
 * One fc4_advisors handle holds the advisor system (cSC4AdvisorSystem
 * 0x4A0DD616) and, when an fc4_mysim_host is given, the MySim classes
 * (0x4A1DBBBF, and 0x0BF606A5 run as a MySim) and the MySim agent
 * (0x29E37DEC). Missing callbacks read as 0 / false / none.
 *
 * Core objects (the system, advisors, advices, subjects) cross the ABI as
 * fc4_adv_obj handles. The class store creates them with
 * fc4_advisors_create_object, writes / reads them with fc4_adv_obj_write /
 * _read, and returns them in fc4_ref.core_handle (an fc4_adv_obj*) on
 * reference reads. object_created is called for every object the core
 * creates, with a new handle the host owns (fc4_adv_obj_destroy); its return
 * value is the object's token, written for references to it.
 *
 * Pointers the host returns inside structs (entry strings, effects, building
 * names and groups, tooltips) only have to stay valid until the callback
 * returns. Lists use the two-call pattern: called with (NULL, 0) for the
 * length, then with a buffer; -1 = no list (the source is missing).
 * Threads: single-threaded core; serialise every call on one handle. */
typedef struct fc4_advisors fc4_advisors;
typedef struct fc4_adv_obj fc4_adv_obj;

#define FC4_ADV_OBJ_SYSTEM 1u
#define FC4_ADV_OBJ_ADVISOR 2u
#define FC4_ADV_OBJ_ADVICE 3u
#define FC4_ADV_OBJ_SUBJECT 4u
#define FC4_ADV_OBJ_HOST 5u /* fc4_adv_out_message only: a host object (obj_token) */

/* One entry of the Lua `advisors` table (§2.1). */
typedef struct fc4_adv_advisor_entry {
    uint32_t class_id;
    int32_t id, advice_type;
    uint32_t reserved;
    const uint8_t* caption; /* the whole caption; Init keeps the text before the first '|' */
    size_t caption_len;
    const uint8_t* name;
    size_t name_len;
} fc4_adv_advisor_entry;

/* One entry of the Lua `advices` table (§2.2). */
typedef struct fc4_adv_advice_entry {
    uint32_t guid, class_id;
    int32_t advice_type, priority, mood, frequency, timeout;
    uint32_t event, command;
    int32_t nd_type;
    uint8_t once, news_only, persist, no_timeout, has_trigger, reserved[3];
    const uint8_t* trigger;
    size_t trigger_len;
    const uint32_t* effects;
    size_t effect_count;
} fc4_adv_advice_entry;

/* A protected Lua call's result (§6.1, §6.6). */
#define FC4_ADV_LUA_OTHER 0u /* nil, table, userdata, ... (also: no callback) */
#define FC4_ADV_LUA_BOOL 1u
#define FC4_ADV_LUA_NUMBER 2u
#define FC4_ADV_LUA_STRING 3u
#define FC4_ADV_LUA_ERROR 4u
typedef struct fc4_adv_lua_result {
    uint32_t kind, boolean;
    double number;
    const uint8_t* str;
    size_t len;
} fc4_adv_lua_result;

typedef struct fc4_adv_date {
    int32_t day_number, year, month, day;
} fc4_adv_date;

/* An outgoing message. obj_kind: 0 none, FC4_ADV_OBJ_ADVISOR (obj_id = the
 * advisor ID), FC4_ADV_OBJ_ADVICE (obj_id = the GUID), FC4_ADV_OBJ_HOST. */
typedef struct fc4_adv_out_message {
    uint32_t msg_type, data1, data2, obj_kind;
    uint64_t obj_token;
    uint32_t obj_id, obj_serial;
} fc4_adv_out_message;

/* A variable source read (§7): `kind` is an FC4_ADV_SRC_* code; a, b and
 * object as each kind documents (engine/core/src/advisors/mod.rs SrcKind). */
typedef struct fc4_adv_src {
    uint32_t kind, a, b, reserved;
    uint64_t object;
} fc4_adv_src;

typedef struct fc4_adv_val {
    int64_t i;
    double f;
    uint64_t object, object2, n, m;
    float cell_x, cell_z;
    uint8_t has_cell, reserved[7];
} fc4_adv_val;

/* List reads (§7.5-§7.8): kind 1 hospitals, 2 schools, 3 fire stations,
 * 4 police stations, 5 port lots, 6 connections, 7 deal proposals. */
typedef struct fc4_adv_item {
    uint64_t object;
    float f0, f1, x, z;
    uint32_t flags, kind;
} fc4_adv_item;

typedef struct fc4_adv_conditional_status {
    uint8_t status, reserved;
    uint16_t building_count;
    uint32_t reserved2;
    const uint8_t* tooltip;
    size_t tooltip_len;
} fc4_adv_conditional_status;

/* A subject's fields; object = the host object's token (0 = none). */
typedef struct fc4_adv_subject_state {
    float x, z;
    uint8_t has_location, reserved[3];
    uint32_t data1, data2, reserved2;
    uint64_t object;
} fc4_adv_subject_state;

/* An incoming message (DoMessage); void1 / unknown 0 = none. */
typedef struct fc4_adv_message {
    uint32_t msg_type, data1, data2, data3, data4, reserved;
    uint64_t void1, unknown;
} fc4_adv_message;

/* A Lua value of fc4_advisors_call_lua. */
#define FC4_ADV_VALUE_NIL 0u
#define FC4_ADV_VALUE_BOOL 1u
#define FC4_ADV_VALUE_NUMBER 2u
#define FC4_ADV_VALUE_STRING 3u
#define FC4_ADV_VALUE_SUBJECT 4u
typedef struct fc4_adv_lua_value {
    uint32_t kind, boolean;
    double number;
    const uint8_t* str; /* results: valid until the next call */
    size_t len;
    fc4_adv_obj* subject; /* results: a new handle the caller destroys (NULL = nil userdata) */
} fc4_adv_lua_value;

typedef struct fc4_advisors_state {
    uint32_t struct_size, advisor_count;
    int32_t last_day;
    uint32_t idle_start;
    uint8_t has_idle_start, loaded_from_save, initialized, reload_pending, dolly_llama, has_mysims, reserved[2];
    int32_t thresholds[3], tag_subject, tag_string;
    uint32_t context_depth, subscribed_events, reserved2;
} fc4_advisors_state;

typedef struct fc4_advisor_state {
    uint32_t struct_size, class_id;
    int32_t id, table_index, advice_type, month_counter, current_day, cursor;
    uint32_t advice_count, active_count, queue_length, pending;
    uint8_t dirty, base_subscribed, own_subscribed, debug_subscribed, fire_active, riot_active, user_disaster,
        is_mysim;
    int32_t building_count, park_count, recreation_count;
    uint32_t landmark_count, zot_count, serial;
    uint64_t token;
} fc4_advisor_state;

typedef struct fc4_advice_state {
    uint32_t struct_size, guid, class_id, flags;
    int32_t advisor_id, table_index, priority, frequency, timeout, mood;
    uint32_t event_id, command_id;
    int32_t trigger, nd_type;
    uint32_t serial, effect_count;
    uint8_t loaded, active, has_subject, has_trigger_text;
    int32_t next_day, expiry, last_month;
    uint32_t reserved, reserved2;
    uint64_t token;
} fc4_advice_state;

typedef struct fc4_advisor_host {
    uint32_t struct_size; /* sizeof(fc4_advisor_host) */
    void* user;
    /* Lua: script data (§2, §4.2) */
    bool (*advisor_count)(void* user, uint32_t* n); /* false: `advisors` is not a table */
    bool (*advisor_entry)(void* user, uint32_t i, fc4_adv_advisor_entry* out); /* 1-based */
    uint32_t (*advice_count)(void* user);
    bool (*advice_entry)(void* user, uint32_t i, fc4_adv_advice_entry* out);
    bool (*compile_trigger)(void* user, const uint8_t* text, size_t len, int32_t* ref);
    void (*release_trigger)(void* user, int32_t ref);
    void (*call_trigger)(void* user, int32_t ref, fc4_adv_lua_result* out);
    void (*call_condition)(void* user, int32_t table_index, fc4_adv_lua_result* out);
    bool (*detokenize)(void* user, const uint8_t* text, size_t len, fc4_bytes_sink sink, void* ctx); /* false: unchanged */
    void (*reload_scripts)(void* user);
    void (*register_api)(void* user, bool global);
    void (*set_tutorial_lua)(void* user, bool on);
    void (*create_tags)(void* user, int32_t* subject_tag, int32_t* string_tag);
    void (*register_detokenizer)(void* user, bool on);
    bool (*popup_priority)(void* user, const char* key, double* out);
    void (*collect_garbage)(void* user);
    void (*close_lua)(void* user);
    void (*set_global)(void* user, const char* name, double value);
    /* subject NULL = nil; subject_token = its token */
    void (*set_local)(void* user, const char* name, double value, const fc4_adv_subject_state* subject,
                      uint64_t subject_token);
    void (*set_global_string)(void* user, const char* name, const uint8_t* value, size_t len);
    bool (*debug_enabled)(void* user);
    void (*register_cheat)(void* user, uint32_t id, const char* name);
    void (*unregister_cheat)(void* user, uint32_t id);
    void (*advisor_window)(void* user, bool start);
    /* city and simulator */
    bool (*has_city)(void* user);
    bool (*city_established)(void* user);
    bool (*difficulty)(void* user, int32_t* out);
    void (*cell_counts)(void* user, uint32_t* x, uint32_t* z);
    void (*sim_date)(void* user, fc4_adv_date* out);
    int32_t (*start_year)(void* user);
    bool (*is_any_paused)(void* user);
    uint32_t (*now_ms)(void* user);
    /* messages: target_kind FC4_ADV_OBJ_SYSTEM or FC4_ADV_OBJ_ADVISOR (advisor_id, token) */
    void (*subscribe)(void* user, uint32_t target_kind, int32_t advisor_id, uint64_t token, uint32_t msg_type);
    void (*unsubscribe)(void* user, uint32_t target_kind, int32_t advisor_id, uint64_t token, uint32_t msg_type);
    void (*post)(void* user, const fc4_adv_out_message* m);
    void (*send)(void* user, const fc4_adv_out_message* m); /* synchronous; must not call back */
    void (*post_update_self)(void* user, int32_t advisor_id, uint64_t token, uint32_t data1, uint32_t data2);
    void (*post_ticker)(void* user);
    void (*global_effect)(void* user, uint32_t id, bool assert_);
    void (*rebuild_text)(void* user, uint32_t guid, uint64_t token);
    /* the variables' sources (§7) */
    bool (*value)(void* user, const fc4_adv_src* src, fc4_adv_val* out);
    int64_t (*list)(void* user, uint32_t kind, uint32_t a, const uint32_t* groups, size_t group_count,
                    fc4_adv_item* out, size_t capacity);
    void (*record_history)(void* user, uint32_t history_type, int32_t value);
    /* occupants */
    uint32_t (*occupant_type)(void* user, uint64_t occupant);
    int32_t (*occupant_bool_property)(void* user, uint64_t occupant, uint32_t id); /* -1 none, 0, 1 */
    bool (*building_key_instance)(void* user, uint64_t occupant, uint32_t* out);
    bool (*is_occupant_group)(void* user, uint64_t occupant, uint32_t group);
    /* civic and rewards (§6.6) */
    bool (*conditional_status)(void* user, uint32_t guid, fc4_adv_conditional_status* out);
    void (*update_conditional_status)(void* user, uint32_t guid, const fc4_adv_conditional_status* status);
    bool (*reward_subject)(void* user, uint32_t guid, fc4_adv_subject_state* out);
    /* neighbour deals */
    bool (*deal_type)(void* user, uint64_t object, uint32_t* out);
    void (*cancel_deal)(void* user, uint64_t object);
    /* history, ordinances, region (§8.3) */
    bool (*history_value)(void* user, uint32_t history_type, int32_t day, double* out);
    bool (*history_trend)(void* user, uint32_t history_type, int32_t from, int32_t to, float* out);
    bool (*history_count)(void* user, uint32_t history_type, int32_t from, int32_t to, double* out);
    int32_t (*ordinance_flag)(void* user, uint32_t id, bool on); /* -1 none, 0, 1 */
    void (*set_ordinance_on)(void* user, uint32_t id, bool on);
    bool (*ordinance_amount)(void* user, uint32_t id, uint32_t which, int64_t* out);
    bool (*ordinance_text)(void* user, uint32_t id, uint32_t which, fc4_bytes_sink sink, void* ctx);
    bool (*tutorial_guid)(void* user, uint32_t* out);
    /* references */
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    uint64_t (*resolve_ref)(void* user, uint64_t token, uint32_t iid);
    uint64_t (*object_created)(void* user, uint32_t kind, uint32_t class_id, fc4_adv_obj* handle);
} fc4_advisor_host;

/* MySim Lua field values (§4.4). */
#define FC4_MYSIM_FIELD_NUMBER 0u
#define FC4_MYSIM_FIELD_STRING 1u
#define FC4_MYSIM_FIELD_BUILDING 2u /* object = the building token, 0 = nil */
#define FC4_MYSIM_FIELD_SUBJECT 3u  /* subject NULL = nil; object = its token */
typedef struct fc4_mysim_field {
    uint32_t kind, reserved;
    double number;
    uint64_t object;
    const uint8_t* str;
    size_t len;
    const fc4_adv_subject_state* subject;
} fc4_mysim_field;

typedef struct fc4_mysim_disaster_location {
    uint32_t disaster_type;
    int32_t x, z;
} fc4_mysim_disaster_location;

typedef struct fc4_mysim_building {
    uint32_t building_type, purpose; /* purpose 1 = residence */
    float completion;
    float position[3];
    const uint8_t* name;
    size_t name_len;
    const uint32_t* groups;
    size_t group_count;
} fc4_mysim_building;

typedef struct fc4_mysim_lot {
    uint64_t building; /* 0 = none */
    int32_t cell_x, cell_z, x0, z0, x1, z1;
    uint8_t condition, road_access, wealth, reserved;
    uint32_t reserved2;
} fc4_mysim_lot;

typedef struct fc4_mysim_station {
    uint64_t station;
    float funding;
    uint32_t reserved;
} fc4_mysim_station;

typedef struct fc4_mysim_dispatch {
    int32_t state; /* 2 travelling, 3 at a location */
    uint32_t flags;
    int32_t cell_x, cell_z;
    float position[3];
    uint32_t travel_type, wait_minutes;
    uint8_t at_home, at_work, by_commute, reserved;
    uint64_t destination, location;
} fc4_mysim_dispatch;

#define FC4_MYSIM_JOB_LOT 1u
#define FC4_MYSIM_JOB_NEIGHBOR 2u
typedef struct fc4_mysim_job_destination {
    uint32_t kind;
    int32_t cell_x, cell_z;
    uint32_t reserved;
    uint64_t building;
} fc4_mysim_job_destination;

typedef struct fc4_mysim_post {
    uint32_t msg_type, data1, data2;
    int32_t sim; /* the MySim's advisor ID, 0 = none */
    uint64_t void1;
} fc4_mysim_post;

/* Grids of fc4_mysim_host.grid / grid_at: `arg` = the pollution grid 0..3 or
 * the desirability developer (0x1010 / 0x1020 / 0x1030). */
#define FC4_MYSIM_GRID_CRIME 1u
#define FC4_MYSIM_GRID_POLLUTION 2u
#define FC4_MYSIM_GRID_AURA 3u
#define FC4_MYSIM_GRID_AVERAGE_EQ 4u
#define FC4_MYSIM_GRID_AVERAGE_HQ 5u
#define FC4_MYSIM_GRID_CONGESTION 6u
#define FC4_MYSIM_GRID_DESIRABILITY 7u

/* `sim` is the MySim's advisor ID. Version-12 MySim records (inline dispatch)
 * are not supported over the ABI. */
typedef struct fc4_mysim_host {
    uint32_t struct_size; /* sizeof(fc4_mysim_host) */
    void* user;
    /* Lua (§4.4) */
    bool (*create_table)(void* user, int32_t sim);
    void (*set_field)(void* user, int32_t sim, const char* name, const fc4_mysim_field* value);
    bool (*activate_lua)(void* user, int32_t sim);
    bool (*tuning)(void* user, const char* key, double* out);
    bool (*ui_blocked)(void* user);
    void (*register_agent_lua)(void* user, bool on);
    /* city */
    void (*cell_counts)(void* user, uint32_t* x, uint32_t* z);
    void (*cell_center)(void* user, int32_t cx, int32_t cz, float* x, float* z);
    bool (*disaster_ongoing)(void* user);
    size_t (*disaster_locations)(void* user, fc4_mysim_disaster_location* out, size_t capacity);
    /* buildings and lots */
    bool (*building)(void* user, uint64_t building, fc4_mysim_building* out);
    bool (*lot)(void* user, uint64_t building, fc4_mysim_lot* out);
    bool (*occupant_position)(void* user, uint64_t occupant, float* out3);
    int64_t (*buildings_in_box)(void* user, int32_t x0, int32_t z0, int32_t x1, int32_t z1, uint64_t* out,
                                size_t capacity);
    int64_t (*all_buildings)(void* user, uint64_t* out, size_t capacity);
    size_t (*lots_in_rect)(void* user, int32_t x0, int32_t z0, int32_t x1, int32_t z1, fc4_mysim_lot* out,
                           size_t capacity);
    size_t (*special_buildings)(void* user, uint64_t* out, size_t capacity);
    size_t (*education_buildings)(void* user, bool hospitals, uint64_t* out, size_t capacity);
    bool (*grade)(void* user, uint64_t building, bool hospital, float* out);
    void (*job_title)(void* user, uint64_t building, fc4_bytes_sink sink, void* ctx);
    bool (*landmark_funding)(void* user, uint64_t building, float* out);
    /* residential */
    bool (*average_age)(void* user, int32_t x, int32_t z, int32_t* out);
    bool (*eq_by_wealth)(void* user, int32_t x, int32_t z, uint8_t wealth, float* out);
    bool (*hq_by_wealth)(void* user, int32_t x, int32_t z, uint8_t wealth, float* out);
    bool (*life_expectancy)(void* user, int32_t x, int32_t z, uint8_t* out);
    bool (*new_age_by_wealth)(void* user, uint8_t wealth, uint8_t* out);
    /* simulators */
    bool (*grid)(void* user, uint32_t grid, uint32_t arg, int32_t x, int32_t z, int32_t* out);
    bool (*grid_at)(void* user, uint32_t grid, uint32_t arg, float x, float z, int32_t* out);
    bool (*coverage)(void* user, bool police, int32_t x0, int32_t z0, int32_t w, int32_t h, int32_t* out);
    int32_t (*powered)(void* user, uint64_t building); /* -1 none, 0, 1 */
    int32_t (*watered)(void* user, uint64_t building);
    int64_t (*stations)(void* user, bool police, fc4_mysim_station* out, size_t capacity);
    bool (*travel_time_ratio)(void* user, int32_t x, int32_t z, uint32_t travel_type, float* out);
    bool (*trip_simulators)(void* user);
    /* the dispatch (automata, §4.7) */
    bool (*dispatch)(void* user, int32_t sim, fc4_mysim_dispatch* out);
    bool (*find_new_job)(void* user, int32_t sim);
    bool (*change_job)(void* user, int32_t sim, uint64_t building);
    bool (*job_destination)(void* user, int32_t sim, fc4_mysim_job_destination* out);
    bool (*commute_time)(void* user, int32_t sim, float* out);
    bool (*neighbor_city_name)(void* user, int32_t x, int32_t z, fc4_bytes_sink sink, void* ctx);
    /* create, start_commute, reset_commute, shutdown, leave_for_work, return_home,
     * night_sound, update_residence, move_in_control, ghost */
    void (*dispatch_command)(void* user, int32_t sim, const char* what);
    /* messages and effects */
    void (*post)(void* user, const fc4_mysim_post* m);
    void (*automata_trigger)(void* user, const char* name, const float* position3);
    void (*subscribe)(void* user, uint32_t msg_type, bool on); /* the agent's subscriptions */
    /* references */
    void (*add_ref)(void* user, uint64_t token);
    void (*release)(void* user, uint64_t token);
    uint64_t (*resolve_ref)(void* user, uint64_t token, uint32_t iid);
} fc4_mysim_host;

typedef struct fc4_mysims_agent_state {
    uint32_t struct_size;
    uint8_t initialised, reserved[3];
    int32_t slots[6]; /* advisor IDs, 0 = empty */
    uint32_t last_disaster_type;
    int32_t active_ui_index;
} fc4_mysims_agent_state;

typedef struct fc4_mysim_state {
    uint32_t struct_size;
    uint8_t state, slot, wealth, sign, is_male, age, birth_month, retired, neighbor_job, commute_check, lua_table,
        unnamed_flag, eq_index, hq_index;
    uint16_t generation;
    uint32_t home_cell_x, home_cell_z;
    int32_t neighbor_cell[2], commute_cell[2];
    uint32_t jobless_days, bitmap_id, last_commute_time, last_local_disaster;
    float home_position[3], eq, hq, eq_delta, hq_delta, eq_ring[6], hq_ring[6];
    fc4_res_key vehicle_key, car_key, pedestrian_key;
    int32_t unnamed_pair[2], last_player_trip_day, last_trip_update_day, local_area_size;
    uint8_t pylon_units, pylon_running, reserved[2];
    uint32_t local_group_count;
    uint64_t pylon_elapsed, residence, job, dispatch, dispatch_tool;
} fc4_mysim_state;

/* Creates the system (constructed, not initialised); the tables are copied.
 * mysim_host NULL = no MySim classes (their advisors are skipped as unknown
 * classes). session_seed seeds the system's and each MySim's generator. */
fc4_advisors* fc4_advisors_create(const fc4_advisor_host* host, const fc4_mysim_host* mysim_host,
                                  uint32_t session_seed);
void fc4_advisors_destroy(fc4_advisors* p);
bool fc4_advisors_query_interface(uint32_t iid);
bool fc4_advisors_init(fc4_advisors* p);        /* §4.1 */
bool fc4_advisors_shutdown(fc4_advisors* p);    /* §4.6 */
bool fc4_advisors_reload_data(fc4_advisors* p); /* §4.2 */
/* The system's DoMessage (§5): the ticker, PostCityInit, city established. */
bool fc4_advisors_do_message(fc4_advisors* p, const fc4_adv_message* m);
bool fc4_advisors_cheat(fc4_advisors* p, uint32_t id, const char* text); /* §5.2 */
bool fc4_advisors_push_event(fc4_advisors* p, uint32_t event_id, const fc4_adv_obj* subject);
bool fc4_advisors_get_state(const fc4_advisors* p, fc4_advisors_state* out);
/* Advisor IDs in map order: returns the count, copies at most `capacity`. */
size_t fc4_advisors_advisor_ids(const fc4_advisors* p, int32_t* out, size_t capacity);
bool fc4_advisors_advisor_state(const fc4_advisors* p, int32_t id, fc4_advisor_state* out);
/* which 0 = caption, 1 = personal name; NUL-terminated; returns the full length. */
size_t fc4_advisors_advisor_text(const fc4_advisors* p, int32_t id, uint32_t which, char* out, size_t capacity);
bool fc4_advisors_advisor_simulate(fc4_advisors* p, int32_t id, int32_t kind); /* 0 daily, 1 monthly */
bool fc4_advisors_advisor_do_message(fc4_advisors* p, int32_t id, const fc4_adv_message* m);
bool fc4_advisors_advisor_command_would_trigger(fc4_advisors* p, int32_t id, uint32_t command);
bool fc4_advisors_advisor_push_event(fc4_advisors* p, int32_t id, uint32_t event_id, const fc4_adv_obj* subject);
bool fc4_advisors_advice_state(const fc4_advisors* p, int32_t advisor_id, uint32_t index, fc4_advice_state* out);
bool fc4_advisors_push_context_advice(fc4_advisors* p, int32_t advisor_id, uint32_t index); /* §6.7 */
bool fc4_advisors_pop_context_advice(fc4_advisors* p);
/* The game.* functions and globals (§8.3): the number of results (copies at
 * most `capacity`), -1 when `name` is not one of them. */
int32_t fc4_advisors_call_lua(fc4_advisors* p, const char* name, const fc4_adv_lua_value* args, size_t nargs,
                              fc4_adv_lua_value* out, size_t capacity);
/* Objects (new handles; fc4_adv_obj_destroy). create_object: the class
 * store's factory (§11), NULL for an unknown class. */
fc4_adv_obj* fc4_advisors_system_object(const fc4_advisors* p);
fc4_adv_obj* fc4_advisors_advisor_object(const fc4_advisors* p, int32_t id);
fc4_adv_obj* fc4_advisors_advice_object(const fc4_advisors* p, int32_t advisor_id, uint32_t index);
fc4_adv_obj* fc4_advisors_create_object(fc4_advisors* p, uint32_t class_id);
fc4_adv_obj* fc4_advisors_new_subject(fc4_advisors* p);
bool fc4_advisors_add_advisor(fc4_advisors* p, const fc4_adv_obj* advisor); /* AddAdvisor (§4.3) */
void fc4_adv_obj_destroy(fc4_adv_obj* o);
uint32_t fc4_adv_obj_kind(const fc4_adv_obj* o); /* FC4_ADV_OBJ_*, 0 for NULL */
uint32_t fc4_adv_obj_class(const fc4_adv_obj* o);
bool fc4_adv_obj_same(const fc4_adv_obj* a, const fc4_adv_obj* b);
uint64_t fc4_adv_obj_token(const fc4_adv_obj* o);
bool fc4_adv_obj_set_token(const fc4_adv_obj* o, uint64_t token);
bool fc4_adv_obj_write(const fc4_adv_obj* o, const fc4_ostream* out); /* §11 */
bool fc4_adv_obj_read(const fc4_adv_obj* o, const fc4_istream* in);
bool fc4_adv_subject_get(const fc4_adv_obj* subject, fc4_adv_subject_state* out);
/* SetLocation (clamped to the city) when has_location, SetObject, data1, data2. */
bool fc4_adv_subject_set(const fc4_adv_obj* subject, const fc4_adv_subject_state* value);

/* The MySim agent (mysims.md §1.3, §3.2, §6.1, §7.1); false / 0 without MySims.
 * AdviceCommand names its advisor by ID in `unknown`. MySims are named by
 * their advisor IDs. */
bool fc4_mysims_agent_init(fc4_advisors* p);
bool fc4_mysims_agent_shutdown(fc4_advisors* p);
bool fc4_mysims_agent_do_message(fc4_advisors* p, const fc4_adv_message* m);
bool fc4_mysims_agent_get_state(const fc4_advisors* p, fc4_mysims_agent_state* out);
size_t fc4_mysims_active(const fc4_advisors* p, int32_t* out, size_t capacity); /* GetActiveMySims */
/* MoveIn (§4.1): the MySim that moves in, 0 if all five are active. */
int32_t fc4_mysims_move_in(fc4_advisors* p, const uint8_t* name, size_t name_len, bool is_male, uint8_t sign,
                           const uint8_t* bio, size_t bio_len, uint32_t bitmap_id, const uint8_t* path,
                           size_t path_len, uint8_t slot);
int32_t fc4_mysims_lives_here(const fc4_advisors* p, uint64_t building);
bool fc4_mysims_set_active_ui_panel_sim(fc4_advisors* p, int32_t slot);
/* GetActiveUIPanelSim (§7.4): the MySim (0 = none); *index = its vector index, unchanged without one. */
int32_t fc4_mysims_get_active_ui_panel_sim(const fc4_advisors* p, int32_t* index);
bool fc4_mysims_agent_write(const fc4_advisors* p, const fc4_ostream* out); /* u16 1 */
bool fc4_mysims_agent_read(fc4_advisors* p, const fc4_istream* in);
bool fc4_mysim_get_state(const fc4_advisors* p, int32_t id, fc4_mysim_state* out);
/* which 0 = bio, 1 = portrait path; NUL-terminated; returns the full length. */
size_t fc4_mysim_text(const fc4_advisors* p, int32_t id, uint32_t which, char* out, size_t capacity);
bool fc4_mysim_set_residence(fc4_advisors* p, int32_t id, uint64_t building); /* §4.1 */
bool fc4_mysim_change_residence(fc4_advisors* p, int32_t id, uint64_t building); /* §4.7 */
bool fc4_mysim_find_new_residence(fc4_advisors* p, int32_t id); /* always false */
bool fc4_mysim_test_for_residence(const fc4_advisors* p, int32_t id, uint64_t building, bool filter,
                                  bool match_wealth);
bool fc4_mysim_change_job(fc4_advisors* p, int32_t id, uint64_t building);
bool fc4_mysim_delete_from_game(fc4_advisors* p, int32_t id);
bool fc4_mysim_announce_move_in(fc4_advisors* p, int32_t id);
bool fc4_mysim_do_script_command(fc4_advisors* p, int32_t id, uint32_t command); /* §4.8 */
bool fc4_mysim_update_variables(fc4_advisors* p, int32_t id);                    /* §4.5 */
uint32_t fc4_mysim_distance_to_closest(const fc4_advisors* p, int32_t id, uint32_t group); /* 0xFFFFFFFF = none */
uint32_t fc4_mysim_distance_to_closest_special(const fc4_advisors* p, int32_t id, uint32_t building_type);
/* LocalSchoolGrade / LocalHospitalGrade (§4.9): the grade as a fraction. */
float fc4_mysim_local_grade(const fc4_advisors* p, int32_t id, uint32_t group, bool hospital);

/* Variable source codes (fc4_adv_src.kind), generated from SrcKind. */
#define FC4_ADV_SRC_MAYOR_RATING 1u
#define FC4_ADV_SRC_MIN_MAYOR_RATING 2u
#define FC4_ADV_SRC_MAX_MAYOR_RATING 3u
#define FC4_ADV_SRC_DEMAND_SUPPLY 4u
#define FC4_ADV_SRC_DEMAND_VALUE 5u
#define FC4_ADV_SRC_DEMAND_ACTIVE 6u
#define FC4_ADV_SRC_DEMAND_CAP 7u
#define FC4_ADV_SRC_JOBS_BY_CENSUS 8u
#define FC4_ADV_SRC_NEUTRAL_TAX_RATE 9u
#define FC4_ADV_SRC_ZONE_COUNT 10u
#define FC4_ADV_SRC_CONNECTIONS_OF_TYPE 11u
#define FC4_ADV_SRC_NEIGHBOR_CITIES 12u
#define FC4_ADV_SRC_CITY_HAS_SEA 13u
#define FC4_ADV_SRC_REGION_POPULATION 14u
#define FC4_ADV_SRC_EXTRAPOLATED_POPULATION 15u
#define FC4_ADV_SRC_CITY_SIZE_X 16u
#define FC4_ADV_SRC_CITY_SIZE_Z 17u
#define FC4_ADV_SRC_WATER_TILE_COUNT 18u
#define FC4_ADV_SRC_DEPARTMENT_FUNDING 19u
#define FC4_ADV_SRC_TOTAL_FUNDS 20u
#define FC4_ADV_SRC_INCOME_MONTHLY 21u
#define FC4_ADV_SRC_EXPENSE_MONTHLY 22u
#define FC4_ADV_SRC_TAX_RATE 23u
#define FC4_ADV_SRC_TAX_INCOME 24u
#define FC4_ADV_SRC_BORROWED 25u
#define FC4_ADV_SRC_BORROWING_LIMIT 26u
#define FC4_ADV_SRC_BOND_PAYMENTS_MONTHLY 27u
#define FC4_ADV_SRC_DEAL_INCOME 28u
#define FC4_ADV_SRC_DEAL_EXPENSE 29u
#define FC4_ADV_SRC_GLOBAL_LIFE_EXPECTANCY 30u
#define FC4_ADV_SRC_GLOBAL_HEALTH 31u
#define FC4_ADV_SRC_GLOBAL_EDUCATION 32u
#define FC4_ADV_SRC_HQ_MIN 33u
#define FC4_ADV_SRC_HQ_MAX 34u
#define FC4_ADV_SRC_EQ_MIN 35u
#define FC4_ADV_SRC_EQ_MAX 36u
#define FC4_ADV_SRC_HEALTH_TOTALS 37u
#define FC4_ADV_SRC_SCHOOL_TOTALS 38u
#define FC4_ADV_SRC_FIRE_STRIKE_CHANCE 40u
#define FC4_ADV_SRC_POPULATION 41u
#define FC4_ADV_SRC_FLAMMABILITY_HOTSPOT 42u
#define FC4_ADV_SRC_AVERAGE_CRIME 43u
#define FC4_ADV_SRC_CRIME_HOTSPOT 44u
#define FC4_ADV_SRC_JAIL_POPULATION 45u
#define FC4_ADV_SRC_JAILED_COUNT 46u
#define FC4_ADV_SRC_MONTHLY_CRIME_COUNT 47u
#define FC4_ADV_SRC_ARREST_COUNT 48u
#define FC4_ADV_SRC_POLICE_STRIKE_CHANCE 49u
#define FC4_ADV_SRC_SCHOOL_STRIKE_CHANCE 50u
#define FC4_ADV_SRC_HEALTH_STRIKE_CHANCE 51u
#define FC4_ADV_SRC_FIRE_STATION_COUNT 52u
#define FC4_ADV_SRC_POLICE_STATION_COUNT 53u
#define FC4_ADV_SRC_JAIL_COUNT 54u
#define FC4_ADV_SRC_AVERAGE_FLAMMABILITY 55u
#define FC4_ADV_SRC_FIRE_COVERAGE 56u
#define FC4_ADV_SRC_FIRE_COVERAGE_PERCENT 57u
#define FC4_ADV_SRC_POLICE_COVERAGE 58u
#define FC4_ADV_SRC_POLICE_COVERAGE_PERCENT 59u
#define FC4_ADV_SRC_JAIL_CAPACITY 60u
#define FC4_ADV_SRC_DISASTER_DAMAGE_BUILDINGS 61u
#define FC4_ADV_SRC_DISASTER_DAMAGE_COST 62u
#define FC4_ADV_SRC_DISASTER_POLICE_EFFECTIVENESS 63u
#define FC4_ADV_SRC_DISASTER_FIRE_EFFECTIVENESS 64u
#define FC4_ADV_SRC_DISASTER_FIRE_CAUSE 65u
#define FC4_ADV_SRC_CONGESTION_AVERAGE 70u
#define FC4_ADV_SRC_CONGESTION_TILES 71u
#define FC4_ADV_SRC_CONGESTION_WORST 72u
#define FC4_ADV_SRC_STATION_UTILIZATION 73u
#define FC4_ADV_SRC_STATION_UTILIZATION_LOW 74u
#define FC4_ADV_SRC_TRANSPORT_FUNDING 75u
#define FC4_ADV_SRC_FREIGHT_TRIP_LENGTH 76u
#define FC4_ADV_SRC_PORT_EFFICIENCY 77u
#define FC4_ADV_SRC_CONGESTION_AT 78u
#define FC4_ADV_SRC_NUM_PORTS 79u
#define FC4_ADV_SRC_SEAPORT_VOLUME 80u
#define FC4_ADV_SRC_NETWORK_TILES 81u
#define FC4_ADV_SRC_POTHOLE_COUNT 82u
#define FC4_ADV_SRC_FREIGHT_DEPOT_COUNT 83u
#define FC4_ADV_SRC_RAPID_TRANSIT_TRANSITIONS 84u
#define FC4_ADV_SRC_TRANSIT_STRIKE 85u
#define FC4_ADV_SRC_STATION_COUNT 86u
#define FC4_ADV_SRC_POWER_PRODUCTION 90u
#define FC4_ADV_SRC_POWER_CONSUMED 91u
#define FC4_ADV_SRC_OVERWORKED_PLANTS 92u
#define FC4_ADV_SRC_UNPOWERED_BUILDINGS 93u
#define FC4_ADV_SRC_POWER_STRIKE 94u
#define FC4_ADV_SRC_POWER_STRIKE_CHANCE 95u
#define FC4_ADV_SRC_REGION_EXPORTED 96u
#define FC4_ADV_SRC_REGION_IMPORTED 97u
#define FC4_ADV_SRC_WATER_PRODUCTION 98u
#define FC4_ADV_SRC_WATER_CONSUMED 99u
#define FC4_ADV_SRC_INCINERATOR_CAPACITY 100u
#define FC4_ADV_SRC_WASTE_TO_ENERGY_CAPACITY 101u
#define FC4_ADV_SRC_RECYCLING_EFFECT 102u
#define FC4_ADV_SRC_AIR_POLLUTION 103u
#define FC4_ADV_SRC_WATER_POLLUTION 104u
#define FC4_ADV_SRC_GARBAGE_POLLUTION 105u
#define FC4_ADV_SRC_UNCOLLECTED_GARBAGE 106u
#define FC4_ADV_SRC_LANDFILL_USAGE 107u
#define FC4_ADV_SRC_GARBAGE_PRODUCED 108u
#define FC4_ADV_SRC_GARBAGE_RECYCLED 109u
#define FC4_ADV_SRC_GARBAGE_TO_ENERGY 110u
#define FC4_ADV_SRC_GARBAGE_TO_LANDFILL 111u
#define FC4_ADV_SRC_WORST_AIR_POLLUTION 112u
#define FC4_ADV_SRC_WORST_WATER_POLLUTION 113u
#define FC4_ADV_SRC_WORST_UNCOLLECTED_GARBAGE 114u
#define FC4_ADV_SRC_POWER_PLANT_COUNT 115u
#define FC4_ADV_SRC_POWER_PLANTS_OF_TYPE 116u
#define FC4_ADV_SRC_AGED_POWER_PLANTS 117u
#define FC4_ADV_SRC_POWER_POLE_COUNT 118u
#define FC4_ADV_SRC_POWER_PLANT_EXTREMES 119u
#define FC4_ADV_SRC_WATER_SOURCE_COUNT 120u
#define FC4_ADV_SRC_WATER_PIPE_COUNT 121u
#define FC4_ADV_SRC_DISTRESSED_PIPES 122u
#define FC4_ADV_SRC_BURST_PIPES 123u
#define FC4_ADV_SRC_WATERED_BUILDINGS 124u
#define FC4_ADV_SRC_UNWATERED_BUILDINGS 125u
#define FC4_ADV_SRC_PUMP_SHUTDOWNS 126u
#define FC4_ADV_SRC_WATER_BUILDING_EXTREMES 127u
#define FC4_ADV_SRC_INCINERATOR_COUNT 128u
#define FC4_ADV_SRC_WASTE_TO_ENERGY_COUNT 129u
#define FC4_ADV_SRC_WATER_TREATMENT_COUNT 130u
#define FC4_ADV_SRC_RECYCLING_CENTER_COUNT 131u
#define FC4_ADV_SRC_DEAL_COUNT 132u
#define FC4_ADV_SRC_POWER_FUNDING 133u

/* ---- network geometry (docs/specs/network-geometry.md §3-§9; engine/capi/src/network_geometry.rs) ----
 * The network tool's geometry: the drag (BreakIntoStraightAndDiagSegments,
 * ComputeDraggedCells), the validation passes and their failure codes
 * (0xF0000001 bad terrain, 0xF0000002 reserved, 0xF0000003 bridge,
 * 0xF0000005 intersection, 0xF0000006 smoothing, 0xF0000007 unsolved,
 * 0xF0000008 terrain support), the cell infos, SmoothenNetwork levels 0-3,
 * CanTilesBeSupportedOnTerrain and the grading (UpdateTerrainHeights), plus
 * the standalone height satisfier (fc4_vhcs) and the pure helpers of §10,
 * §11 and §13. No plugin glue yet.
 *
 * The terrain is read through the host table, or through a core terrain bound
 * with fc4_netgeom_set_terrain: then it is read under the terrain's lock and
 * the grading writes it (its messages go to the terrain host's post). The rest
 * of the world goes through the host table; a missing callback is the empty
 * world (no occupants, no connection nodes, no bridge types; the bridge checks
 * pass; the highlight pass keeps the count). The land-bridge pass (NG4) and the
 * piece solver's extra constraints (NG5) are not exposed yet.
 *
 * Threads: one thread at a time. A callback that calls back into the same
 * geometry is refused (fallback value, counted in reentrant_refused). */
typedef struct fc4_netgeom fc4_netgeom;
typedef struct fc4_vhcs fc4_vhcs;

#define FC4_NETGEOM_SAT_NETWORK 0u
#define FC4_NETGEOM_SAT_TERRAIN 1u

#define FC4_NETGEOM_OCC_OTHER 0u
#define FC4_NETGEOM_OCC_LANDFILL 1u
#define FC4_NETGEOM_OCC_NETWORK 2u
#define FC4_NETGEOM_OCC_BUILDING 3u

#define FC4_NETGEOM_TV_FIX 0u
#define FC4_NETGEOM_TV_RANGE 1u
#define FC4_NETGEOM_TV_UNSUPPORTED 2u
#define FC4_NETGEOM_TV_INVALID 0xFFFFFFFFu

/* One type's placement parameters (§2.4, exemplar group 0x084344E0): angles in
 * degrees, heights in metres. Types: 0 road, 1 rail, 2 highway, 3 street,
 * 4 pipe, 5 power line, 6 avenue, 7 subway, 8 light rail, 9 monorail,
 * 10 one-way road, 11 dirt road, 12 ground highway. */
typedef struct fc4_netgeom_params {
    uint32_t struct_size;
    float max_slope;            /* 0x879D12E7 */
    float slope_04;             /* 0x279D1707 */
    float max_slope_change;     /* 0x07B36C71 */
    float max_raise;            /* 0xC7B36CA4 */
    float max_cut;              /* 0xC7B36CA5 */
    float resolve_tolerance;    /* 0xC804DD86 */
    float tunnel_trigger_slope; /* 0xC804DD87 */
    float tunnel_end_slope;     /* 0xC804DD88 */
    float new_cell_tolerance;   /* 0xC804DD93 */
    float support_threshold;    /* 0xC804DD94 */
    float max_support_height;   /* 0xC804DD95 */
    float tunnel_clearance;     /* 0xE974EAA6, the first entry (the only one read) */
    float earthwork_cost;       /* 0x6A2D49F4 */
    int32_t cell_cost;          /* 0x4A2D49B9 */
    int32_t bridge_cost;        /* 0x8A2D49EA */
    int32_t tunnel_cost;        /* 0x8A2D49F0 */
    uint32_t elevated_id_count; /* 0xE974EAA7's length (only emptiness is read) */
    uint8_t straight_only;      /* p+0x25 (street) */
    uint8_t reserved[3];
} fc4_netgeom_params;

/* The per-type limits (§2.3). */
typedef struct fc4_netgeom_type_info {
    uint32_t struct_size;
    uint32_t type_id;
    uint32_t width1, width2;
    float slope_rise;           /* +0x100: r24(tan(max_slope) x 16) */
    float diagonal_slope_rise;  /* +0x104 */
    float smoothness;           /* +0x108 */
    float diagonal_smoothness;  /* +0x10C */
    float rise_04;              /* +0x110 */
    float tunnel_trigger_rise;  /* tool +0x23C */
    float tunnel_end_slope_rad; /* tool +0x2AC */
} fc4_netgeom_type_info;

/* BreakIntoStraightAndDiagSegments's outputs (§3.2). */
typedef struct fc4_netgeom_segments {
    uint32_t struct_size;
    int32_t end_x, end_z;
    uint32_t dragged_count, straight_count;
    uint8_t axis0, axis1;
    uint8_t pen_direction;      /* 0 -x, 2 -z, 4 +x, 6 +z */
    uint8_t end_direction;      /* 1, 3, 5 or 7 */
} fc4_netgeom_segments;

typedef struct fc4_netgeom_cell {
    int32_t x, z;
} fc4_netgeom_cell;

/* A dragged step (§2.2): kind 0 regular, 1 bridge, 2 tunnel portal, 3 tunnel interior. */
typedef struct fc4_netgeom_step {
    uint32_t first, last, kind;
} fc4_netgeom_step;

/* A tunnel or bridge: its end steps (bridge_type 0 for tunnels). */
typedef struct fc4_netgeom_span {
    uint32_t start, end, bridge_type;
} fc4_netgeom_span;

typedef struct fc4_netgeom_vertex_height {
    int32_t vertex;
    float height;
} fc4_netgeom_vertex_height;

/* A cell's occupant for the reserved-tile test (§5.1); the host fills it. */
typedef struct fc4_netgeom_occupant {
    uint32_t kind;              /* FC4_NETGEOM_OCC_* */
    uint8_t immovable;          /* network occupants: slot 17 */
    uint8_t connection_node;    /* network occupants: flag 0x200000 */
    uint8_t reserved[2];
    uint32_t type_id;           /* network occupants: slot 22 */
} fc4_netgeom_occupant;

/* The existing network piece in a cell (§8.1); the host fills it. */
typedef struct fc4_netgeom_piece {
    uint32_t flags;
    uint8_t immovable;
    uint8_t has_supports;
    uint8_t reserved[2];
    float corners[4];           /* (x,z), (x,z+1), (x+1,z+1), (x+1,z) */
    uint32_t edges[13];
} fc4_netgeom_piece;

/* cSC4NetworkCellInfo (§8.1), the fields the height solver reads. A connection
 * node's fixed vertices are not carried (an inserted info has none). */
typedef struct fc4_netgeom_cell_info {
    uint32_t struct_size;
    int32_t x, z;
    uint32_t flags;             /* +0x08 */
    int32_t step;               /* +0x14, -1 off the run */
    uint32_t edges[13];         /* +0x18 */
    uint32_t edge_union;        /* +0x4C */
    uint8_t in_run, placed, intersection, fixed, tunnel_interior, connection_node, has_supports;
    uint8_t reserved;
    float corners[4];           /* +0x58 */
    float allowed[8];           /* +0x68, (lo, hi) per corner */
    float preferred[8];         /* +0x88 */
    int32_t vertices[4];        /* +0xA8 */
} fc4_netgeom_cell_info;

/* Diagnostics (fc4_netgeom_get_state). */
typedef struct fc4_netgeom_state {
    uint32_t struct_size;
    uint32_t type_id;
    int32_t start_x, start_z, end_x, end_z;
    uint32_t dragged_count, straight_count;
    uint8_t axis0, axis1, pen_direction, end_direction;
    uint32_t valid;             /* tool +0x78 */
    uint32_t failure;           /* tool +0x20C, 0 = none */
    uint32_t cell_count, step_count, extra_cell_count, tunnel_count, bridge_count;
    uint32_t cell_info_count, solved_count, smoothing_cell_count, boundary_count;
    int32_t level;              /* the smoothing loop's level, -1 = none */
    float tunnel_start_height;  /* NaN = none */
    uint32_t reentrant_refused;
    uint8_t terrain_bound;
    uint8_t reserved[3];
} fc4_netgeom_state;

/* What the geometry needs from the game. Vertex ids are z * (cells_x + 1) + x. */
typedef struct fc4_netgeom_host {
    uint32_t struct_size;
    void* user;
    /* terrain (unused while a core terrain is bound) */
    void (*cells)(void* user, uint32_t* cells_x, uint32_t* cells_z);
    float (*vertex_altitude)(void* user, int32_t vertex);            /* slot 12 */
    bool (*cell_has_water)(void* user, int32_t x, int32_t z);        /* slot 34 */
    float (*surface_water_level)(void* user, int32_t vertex);        /* NaN = none */
    float (*max_allowed_height)(void* user);                         /* slot 24 */
    void (*set_altitude_at_vertex)(void* user, int32_t vertex, float h); /* slot 50 */
    void (*begin_long_terrain_operation)(void* user);                /* slot 87 */
    void (*redisplay_terrain)(void* user, const fc4_terrain_rect* rect, uint32_t kind); /* slot 68 */
    void (*end_long_terrain_operation)(void* user);                  /* slot 88 */
    /* world */
    bool (*is_connection_node)(void* user, int32_t x, int32_t z);
    /* Writes up to `capacity` occupants; returns the total (called again with a larger buffer). */
    uint32_t (*cell_occupants)(void* user, int32_t x, int32_t z, fc4_netgeom_occupant* out, uint32_t capacity);
    bool (*existing_piece)(void* user, int32_t x, int32_t z, fc4_netgeom_piece* out);
    /* §3.3 step 3: the cell holds an auto-connect occupant (or, for road, a cell info with edges). */
    bool (*extends_diagonal_end)(void* user, int32_t x, int32_t z, uint32_t type_id);
    /* Slot 59 over the run's cells: the new valid count (at most `valid`). */
    uint32_t (*update_occupant_highlights)(void* user, uint32_t type_id, const fc4_netgeom_cell* cells,
                                           uint32_t count, uint32_t valid);
    uint32_t (*bridge_type_count)(void* user, uint32_t type_id);
    /* NG2: IsStepGoodForBridge, CheckBridgeStepEdgeTypes, AdjustBridgeSpan, FindBestBridgeType. */
    bool (*is_step_good_for_bridge)(void* user, uint32_t type_id, uint32_t step, const fc4_netgeom_cell* cells,
                                    uint32_t count);
    bool (*bridge_step_edge_types_ok)(void* user, uint32_t type_id, uint32_t step, const fc4_netgeom_cell* cells,
                                      uint32_t count);
    bool (*adjust_bridge_span)(void* user, uint32_t type_id, uint32_t* start, uint32_t* end);
    bool (*find_best_bridge_type)(void* user, uint32_t type_id, uint32_t start, uint32_t end,
                                  uint32_t* bridge_type, uint32_t* min_span);
} fc4_netgeom_host;

/* Parameters and pure helpers. */
bool fc4_netgeom_params_shipped(uint32_t type_id, fc4_netgeom_params* out);
/* From the exemplar's properties (NULL = none: the defaults of §2.4). */
bool fc4_netgeom_params_from_props(uint32_t type_id, const fc4_props* props, fc4_netgeom_params* out);
/* params NULL = shipped. */
bool fc4_netgeom_params_type_info(uint32_t type_id, const fc4_netgeom_params* params, fc4_netgeom_type_info* out);
bool fc4_netgeom_break_into_segments(int32_t start_x, int32_t start_z, int32_t end_x, int32_t end_z, bool snap,
                                     bool straight_only, uint32_t width1, uint32_t width2,
                                     fc4_netgeom_segments* out);
/* §9.6 step 3 for one vertex: FC4_NETGEOM_TV_*; for _FIX lo = hi = preferred = the height. */
uint32_t fc4_netgeom_terrain_vertex(float network_height, float terrain_altitude, bool has_supports, bool fixed,
                                    uint32_t type_id, const fc4_netgeom_params* params, float* lo, float* hi,
                                    float* preferred);
/* §10.2: 0, 1 or 2 from four corner and four terrain heights. */
uint32_t fc4_netgeom_support_level(const float* corners, const float* terrain, float threshold);
uint32_t fc4_netgeom_support_flags(uint32_t level);
/* §10.1: the draft flag (0x80000000) is set for a draft occupant, cleared for a real one. */
uint32_t fc4_netgeom_occupant_flags(uint32_t cell_flags, bool real, bool tunnel_occupant);
/* §11: 0 for at most one pole. */
int32_t fc4_netgeom_power_line_cost(uint32_t poles, uint32_t wire_cells, int32_t per_pole, float per_wire);
/* §13: (x << 16) + z. */
int32_t fc4_netgeom_destroy_key(int32_t x, int32_t z);

/* The geometry. params NULL = the shipped table; else `count` types in type
 * order (missing types: all-zero parameters). NULL on a bad host or params size. */
fc4_netgeom* fc4_netgeom_create(const fc4_netgeom_host* host, const fc4_netgeom_params* params, uint32_t count);
void fc4_netgeom_destroy(fc4_netgeom* g);
/* Binds a core terrain (NULL = the host's terrain callbacks); it must outlive the binding. */
bool fc4_netgeom_set_terrain(fc4_netgeom* g, fc4_terrain* terrain);
bool fc4_netgeom_get_type_info(const fc4_netgeom* g, uint32_t type_id, fc4_netgeom_type_info* out);
bool fc4_netgeom_set_land_bridges(fc4_netgeom* g, uint32_t type_id, bool on);
bool fc4_netgeom_reset(fc4_netgeom* g);
/* DrawNetworkLine (slot 18, §3.1). */
bool fc4_netgeom_draw_line(fc4_netgeom* g, uint32_t type_id, int32_t start_x, int32_t start_z, int32_t end_x,
                           int32_t end_z, bool snap);
/* The validation passes (§4): the valid step count; the code is in the state's failure. */
uint32_t fc4_netgeom_validate(fc4_netgeom* g);
bool fc4_netgeom_create_cell_infos(fc4_netgeom* g);
bool fc4_netgeom_insert_cell_info(fc4_netgeom* g, const fc4_netgeom_cell_info* info);
bool fc4_netgeom_get_cell_info(const fc4_netgeom* g, int32_t x, int32_t z, fc4_netgeom_cell_info* out);
bool fc4_netgeom_smoothen(fc4_netgeom* g, int32_t level);
bool fc4_netgeom_can_tiles_be_supported(fc4_netgeom* g);
/* The smoothing loop (§9.4): 0 and *level, else 0xF0000006 / 0xF0000008. */
uint32_t fc4_netgeom_smooth(fc4_netgeom* g, int32_t* level);
/* Passes, cell infos and smoothing (network-tools.md §5.1 steps 4-5 without the
 * piece solver): the valid count, 0 = refused (see the failure code). */
uint32_t fc4_netgeom_place(fc4_netgeom* g);
/* GetCostOfSolution (network-tools.md §8.1) of the solved cells. */
int32_t fc4_netgeom_cost(const fc4_netgeom* g, int32_t demolition);
/* UpdateTerrainHeights (§9.7): changes above 0.001, RedisplayTerrain kind 4 inside
 * Begin/EndLongTerrainOperation; the changed count, *rect (may be NULL) when not 0. */
uint32_t fc4_netgeom_update_terrain_heights(fc4_netgeom* g, fc4_terrain_rect* rect);
bool fc4_netgeom_get_state(const fc4_netgeom* g, fc4_netgeom_state* out);
/* Lists: copy when `capacity` suffices; return the count. */
uint32_t fc4_netgeom_get_cells(const fc4_netgeom* g, fc4_netgeom_cell* out, uint32_t capacity);
uint32_t fc4_netgeom_get_steps(const fc4_netgeom* g, fc4_netgeom_step* out, uint32_t capacity);
uint32_t fc4_netgeom_get_tunnels(const fc4_netgeom* g, fc4_netgeom_span* out, uint32_t capacity);
uint32_t fc4_netgeom_get_bridges(const fc4_netgeom* g, fc4_netgeom_span* out, uint32_t capacity);
uint32_t fc4_netgeom_get_solved(const fc4_netgeom* g, fc4_netgeom_cell* out, uint32_t capacity);
/* The geometry's satisfiers (FC4_NETGEOM_SAT_*). */
bool fc4_netgeom_sat_height(const fc4_netgeom* g, uint32_t which, int32_t vertex, float* out);
bool fc4_netgeom_sat_range(const fc4_netgeom* g, uint32_t which, int32_t vertex, float* lo, float* hi);
uint32_t fc4_netgeom_sat_solution(const fc4_netgeom* g, uint32_t which, fc4_netgeom_vertex_height* out,
                                  uint32_t capacity);

/* The standalone height satisfier (cSC4VertexHtConstraintSatisfier, §9.1). The
 * pick tie-break is insertion order ([choice], NG1). */
fc4_vhcs* fc4_vhcs_create(void);
void fc4_vhcs_destroy(fc4_vhcs* s);
bool fc4_vhcs_init(fc4_vhcs* s, float lo, float hi);
bool fc4_vhcs_shutdown(fc4_vhcs* s);
/* AddVertexToSolve: false when already present. */
bool fc4_vhcs_add_vertex(fc4_vhcs* s, int32_t vertex, float initial);
bool fc4_vhcs_insert_range(fc4_vhcs* s, int32_t vertex, float lo, float hi);
bool fc4_vhcs_insert_fixed(fc4_vhcs* s, int32_t vertex, float height);
bool fc4_vhcs_insert_equality(fc4_vhcs* s, int32_t a, int32_t b);
bool fc4_vhcs_insert_slope(fc4_vhcs* s, int32_t a, int32_t b, float limit);
/* |h(a) - 2 h(b) + h(c)| <= limit, b the middle vertex. */
bool fc4_vhcs_insert_smoothness(fc4_vhcs* s, int32_t a, int32_t b, int32_t c, float limit);
bool fc4_vhcs_set_preferred_value(fc4_vhcs* s, int32_t vertex, float height);
bool fc4_vhcs_set_preferred_range(fc4_vhcs* s, int32_t vertex, float lo, float hi);
/* Solve: false when a range became empty. */
bool fc4_vhcs_solve(fc4_vhcs* s);
bool fc4_vhcs_height(fc4_vhcs* s, int32_t vertex, float* out);
bool fc4_vhcs_range(fc4_vhcs* s, int32_t vertex, float* lo, float* hi);
uint32_t fc4_vhcs_solution(fc4_vhcs* s, fc4_netgeom_vertex_height* out, uint32_t capacity);

/* ---- turbo clock (tooling, not simulator.md; fc4_core::Turbo) --------------
 * Tooling deviation for test runs (the plugin's sim_turbo). Off (the default)
 * changes nothing. On: the day gate's "ready" (spec §5.2 step 3) is always true
 * (the done[...] conditions still hold a day for its agents), and frameBudgetMs
 * is `frame_ms` while neither paused nor hidden-paused (the §5.1 value
 * otherwise). Pause and hidden pause stop time as without turbo.
 * `until_julian` (0 = none): turbo turns itself off on the first new day on or
 * after it, and clears it; fc4_sim_take_turbo_auto_off then returns that day
 * once. Takes effect from the next tick. Always returns true for a live sim. */
bool fc4_sim_set_turbo(fc4_sim* sim, bool on, uint32_t frame_ms, int32_t until_julian);
/* Pointers may be NULL. False only for a NULL sim. */
bool fc4_sim_get_turbo(const fc4_sim* sim, bool* on, uint32_t* frame_ms, int32_t* until_julian);
/* The Julian day of the last automatic switch-off, once; 0 if none since the last call. */
int32_t fc4_sim_take_turbo_auto_off(fc4_sim* sim);

/* ---- pacing (simulator.md §5.7 [choice]; core fc4_core::pacing) ----------------
 * FC4_PACING_ORIGINAL keeps the original's frame- and clock-bound pacing (one
 * step per idle delivery, the hosts' millisecond limit timers): the conformance
 * shadows. FC4_PACING_DETERMINISTIC: every kind-1 / kind-2 cycle (growth
 * developers, aura, plumbing, the demand census, land value, crime, power, the
 * BDS rebuild) runs to completion in its day's single settle idle delivery,
 * millisecond budgets are item counts, and the simulator's day has a settle
 * point (after the day's agents, before the next day opens) where the host
 * delivers the day's deferred posted messages.
 *
 * The process mode is captured by every core when it is created (default
 * original); set it once before any core exists (the plugin: at game start). */
#define FC4_PACING_ORIGINAL      0u
#define FC4_PACING_DETERMINISTIC 1u
/* False (nothing changed) for an unknown mode. */
bool fc4_pacing_set_process(uint32_t mode);
uint32_t fc4_pacing_process(void);

/* The simulator's side. Under FC4_PACING_DETERMINISTIC each opened day has one
 * settle point, after levels 2-7 have drained: every enabled idle agent that
 * is_paced() accepts gets one SimIdle (data1 = the day, data2 = 0, data3 = 22),
 * in idle order; an agent that returns false (its cycle waits for another
 * agent) is delivered again after the others, at most 4 passes; then settle()
 * runs. The next day opens only after it. The per-frame idle passes (§5.4)
 * skip paced agents. A load, init or a mode change counts the current day as
 * settled. Under FC4_PACING_ORIGINAL nothing of this happens. */
typedef struct fc4_pacing_hooks {
    uint32_t struct_size; /* sizeof(fc4_pacing_hooks) */
    void* user;
    /* NULL: every idle agent is paced. */
    bool (*is_paced)(void* user, fc4_agent agent);
    /* May be NULL. */
    void (*settle)(void* user, int32_t julian);
    /* How many SimIdle a paced agent gets at each settle point before the
     * re-delivery passes (clamped to 1..1024): 1 for our cores (they run their
     * cycle to completion), a fixed count for an original agent that does a
     * fixed amount of work per delivery. NULL: 1. */
    uint32_t (*idle_steps)(void* user, fc4_agent agent);
} fc4_pacing_hooks;
/* `hooks` may be NULL (every idle agent paced, no settle callback); it is
 * copied. False for an unknown mode, a short struct or a NULL sim. */
bool fc4_sim_set_pacing(fc4_sim* sim, uint32_t mode, const fc4_pacing_hooks* hooks);
/* Pointers may be NULL. `day_settled` is always true under original pacing. */
bool fc4_sim_get_pacing(const fc4_sim* sim, uint32_t* mode, bool* day_settled);

/* ---- audio (docs/specs/audio.md) --------------------------------------------
 * The sound-event system, the sound managers and the sound data. Four opaque
 * handles: fc4_audio is one instance of the whole layer (the sound-event pool
 * of §4, the automaton's SFX slots of §6, the audio event handler of §7, the
 * sound manager of §8 and the process-global instance-id counter of §8.4);
 * fc4_audio_msg is a cRZMessage2Standard (§3.2), fc4_audio_snd_event a
 * cGZSndEventStd (§3.1) and fc4_audio_component the effects sound component
 * (§9). Tokens are opaque host identities (the automaton's occupant, instance
 * handles); 0 is null.
 *
 * Threads: the game calls getters from a second thread while the main thread
 * is inside OnTick, so every main-thread call publishes an fc4_audio_state
 * snapshot and fc4_audio_get_state reads only that. fc4_audio_instance_id is a
 * counter and is safe from either thread.
 *
 * Nothing in the audio layer is saved (§12): there is no Read/Write pair. */
typedef struct fc4_audio fc4_audio;
typedef struct fc4_audio_msg fc4_audio_msg;
typedef struct fc4_audio_snd_event fc4_audio_snd_event;
typedef struct fc4_audio_component fc4_audio_component;

/* One posted message (§6.4): the four fields and the message-server priority. */
typedef struct fc4_audio_post {
    uint32_t struct_size; /* sizeof(fc4_audio_post) */
    uint32_t type;        /* +0x0C */
    uint32_t data1;       /* +0x10: the SFX id */
    uint32_t data2;       /* +0x14: the instance id */
    uint32_t high_priority; /* MessagePost's second argument: always 0 */
    uint32_t flags;       /* +0x08: the presence flags (0x87 for both SFX) */
    uint64_t void1;       /* +0x24: the automaton's occupant */
} fc4_audio_post;

/* One sound-event pool page (§4.2). The page a pointer belongs to is
 * `p & 0xFFFFC000`; +0x0C is the count/state word, marker included. */
typedef struct fc4_audio_pool_page {
    uint32_t struct_size;
    uint32_t base;
    uint32_t free_head;   /* +0x08 */
    uint32_t state;       /* +0x0C */
    uint32_t live;        /* state with the 0x40000000 marker removed */
    uint32_t in_service;  /* 1 when the marker is set */
    uint32_t active;
    uint32_t retired;
    uint32_t handed_back;
} fc4_audio_pool_page;

/* The published snapshot the second-thread getters read. */
typedef struct fc4_audio_state {
    uint32_t struct_size;
    uint32_t instance_id;      /* automaton +0x8C */
    uint32_t posts;
    uint32_t live_sounds;
    uint32_t snd_sys_events;
    uint32_t rate_window_ms;   /* the handler's +0x48 */
    uint32_t pooled_events;
    uint32_t master_volume;    /* the manager's +0x14 */
    uint32_t voice_gate;       /* tuning 0xAABAA0D3, +0x64 */
    uint32_t one_shot;         /* tuning 0x0A946A57, +0x68 */
    uint32_t update_period;    /* tuning 0x4A946A5D, +0x6C */
    uint32_t component_active;
    uint32_t reserved;
} fc4_audio_state;

/* fc4_audio_pool_free's result (§4.2). */
#define FC4_AUDIO_RELEASE_NONE        0
#define FC4_AUDIO_RELEASE_HANDED_BACK 1 /* post-dec 0: pool vtable slot 3 */
#define FC4_AUDIO_RELEASE_RETIRED     2 /* post-dec 0x40000045: marker cleared */
/* fc4_audio_manager_stop's result (§8.3). */
#define FC4_AUDIO_STOP_UNKNOWN  0 /* the id is not present: nothing happens */
#define FC4_AUDIO_STOP_STOPPED  1
/* fc4_audio_data_extension's segment (§10.2). */
#define FC4_AUDIO_CTX_HITLISTS 0
#define FC4_AUDIO_CTX_TLO      1
#define FC4_AUDIO_CTX_SAMPLE   2

/* ---- the layer ---- */
fc4_audio* fc4_audio_create(void);
/* NULL is a no-op. */
void fc4_audio_destroy(fc4_audio* a);
/* Safe from the second thread. False for a NULL handle or out. */
bool fc4_audio_get_state(const fc4_audio* a, fc4_audio_state* out);

/* ---- the message object (§3.2) ---- SetType(0) stores 0 and CLEARS presence
 * bit 0 (§14.4); SetVoid1(0) clears bit 7 but still writes +0x24. */
fc4_audio_msg* fc4_audio_msg_create(void);
void fc4_audio_msg_destroy(fc4_audio_msg* m);
void fc4_audio_msg_set_type(fc4_audio_msg* m, uint32_t t);
void fc4_audio_msg_set_data1(fc4_audio_msg* m, uint32_t v);
void fc4_audio_msg_set_data2(fc4_audio_msg* m, uint32_t v);
void fc4_audio_msg_set_data3(fc4_audio_msg* m, uint32_t v);
void fc4_audio_msg_set_data4(fc4_audio_msg* m, uint32_t v);
void fc4_audio_msg_set_void1(fc4_audio_msg* m, uint64_t p);
uint32_t fc4_audio_msg_flags(const fc4_audio_msg* m);

/* ---- the sound event (§3.1) ---- the default constructor leaves +0x30 and
 * +0x34 at 0; the five-argument one stores `payload` and writes 1 to +0x34. */
fc4_audio_snd_event* fc4_audio_snd_event_create(void);
fc4_audio_snd_event* fc4_audio_snd_event_create5(uint32_t payload, uint32_t data1, uint32_t data2,
                                                 uint32_t data3, uint32_t data4);
void fc4_audio_snd_event_destroy(fc4_audio_snd_event* e);
/* 0x5B523BA4 answers this+0x2C and AddRefs; any other IID falls through and
 * the result is 0xFFFFFFFF. */
uint32_t fc4_audio_snd_event_qi(fc4_audio_snd_event* e, uint32_t iid);
void fc4_audio_snd_event_set_value(fc4_audio_snd_event* e, uint32_t v);
uint32_t fc4_audio_snd_event_get_value(const fc4_audio_snd_event* e);
uint32_t fc4_audio_snd_event_get_flag(const fc4_audio_snd_event* e);
/* GetOuter: `this - 0x2C`. */
uint32_t fc4_audio_snd_event_get_outer(uint32_t secondary);

/* ---- the automaton's SFX slots (§6) ---- StartSFX is slot 61 (+0xF4,
 * 0x00509950) and returns 1, or 0 when the id is 0; StopSFX is slot 62
 * (+0xF8, 0x00505B50) and always returns 1 (§14.3). Both post queued at
 * normal priority and return before any sound starts or stops (§6.4).
 * StopSFX never clears automaton +0x8C (§14.1). */
uint32_t fc4_audio_start_sfx(fc4_audio* a, uint32_t sfx_id, uint64_t occupant);
uint32_t fc4_audio_stop_sfx(fc4_audio* a, uint32_t sfx_id, uint64_t occupant);
/* HandleEnterState / HandleExitState (§6.1): the slot is called only when
 * state+0x18 is non-zero. Returns 1 when it was called. */
uint32_t fc4_audio_handle_enter_state(fc4_audio* a, uint32_t sfx_id, uint64_t occupant);
uint32_t fc4_audio_handle_exit_state(fc4_audio* a, uint32_t sfx_id, uint64_t occupant);
bool fc4_audio_get_post(const fc4_audio* a, uint32_t index, fc4_audio_post* out);

/* ---- the instance-id counter (§8.4) ---- one process-wide counter, the
 * pre-increment value, never 0 (including after a 32-bit wrap). */
uint32_t fc4_audio_instance_id(fc4_audio* a);

/* ---- the sound-event pool (§4) ---- alloc is FUN_005E16E0, free is
 * FUN_005E1780 (a null pointer is a no-op, §4.3). Only a request of 0x38
 * bytes uses the pool (§4.1, §14.7). */
uint32_t fc4_audio_pool_alloc(fc4_audio* a);
uint32_t fc4_audio_pool_free(fc4_audio* a, uint32_t p);
uint32_t fc4_audio_pool_page_of(uint32_t p);
uint32_t fc4_audio_pool_size_uses(uint32_t size);
bool fc4_audio_pool_get_page(const fc4_audio* a, uint32_t base, fc4_audio_pool_page* out);

/* ---- the audio event handler (§7) ---- Init subscribes the 13 main types;
 * 0x26D31EC1 adds the city set of 11 and 0x26D31EC2 removes exactly that set,
 * so the automata SFX messages are only heard while a city is loaded (§7.2). */
uint32_t fc4_audio_handler_init(fc4_audio* a);
void fc4_audio_handler_city_message(fc4_audio* a, uint32_t type);
/* HandleAutomataMessages (§7.3). The rate limiter of 1000-4000 ms applies only
 * when `payload` is 0xA9F14C47 (§14.5). */
uint32_t fc4_audio_handler_play(fc4_audio* a, uint32_t payload, uint32_t rand, uint32_t sound_id,
                                uint32_t instance);
uint32_t fc4_audio_handler_stop(fc4_audio* a, uint32_t sound_id, uint32_t instance);

/* ---- the sound manager (§8) ---- DoMessage handles exactly the eight types
 * of §8.3; 0x8A385DD8 and 0xAA396960 are NOT among them (§14.11). */
uint32_t fc4_audio_manager_message(fc4_audio* a, uint32_t type);
/* _StartSound: an instance id that is already live is stopped first (§8.3). */
uint32_t fc4_audio_manager_start(fc4_audio* a, uint32_t sound_id, uint32_t instance, uint32_t param);
/* _StopSound: a miss returns FC4_AUDIO_STOP_UNKNOWN and does nothing (§14.12). */
uint32_t fc4_audio_manager_stop(fc4_audio* a, uint32_t instance, uint32_t hard);
/* _ReadTuningValues (§10.8): no clamping; read_ok 0 restores the defaults and
 * logs the failure. */
void fc4_audio_manager_tuning(fc4_audio* a, uint32_t read_ok, uint32_t voice_gate, uint32_t one_shot,
                              uint32_t update_period);

/* ---- the effects sound component (§9) ---- the description's sound id,
 * 1/locationUpdateRate and length (§10.9). Start's and Stop's mode arguments
 * are never read (§9.1, §14.14). */
fc4_audio_component* fc4_audio_component_create(uint32_t sound_id, float inv_location_rate,
                                                uint32_t length);
void fc4_audio_component_destroy(fc4_audio_component* c);
void fc4_audio_component_start(fc4_audio_component* c, uint32_t mode, uint32_t this_addr,
                               uint32_t snd_sys);
void fc4_audio_component_stop(fc4_audio_component* c, uint32_t mode);
void fc4_audio_component_update(fc4_audio_component* c, float dt);
void fc4_audio_component_set_transform(fc4_audio_component* c, float x, float y, float z);
/* IsActive is the +0x1C started byte (§9.3). */
uint32_t fc4_audio_component_is_active(const fc4_audio_component* c);
/* Component record +0x10 bit 0 -> runtime flag bit 2 (§9.3). */
uint32_t fc4_audio_ignore_flags(uint32_t record_flags, uint32_t runtime_flags);
uint32_t fc4_audio_check_finished_considers(uint32_t runtime_flags);

/* ---- the data (§10) ---- */
/* Resolve a resource type to its extension in a DB segment (§10.2, §10.11).
 * Copies up to cap-1 bytes NUL-terminated and returns the length without the
 * NUL, or 0xFFFFFFFF when the type resolves to nothing. `out` may be NULL. */
uint32_t fc4_audio_data_extension(uint32_t type, uint32_t context, uint8_t* out, uint32_t cap);
uint32_t fc4_audio_data_entries(void); /* Sound.dat: 2776 (§10.1) */
uint64_t fc4_audio_data_bytes(void);   /* Sound.dat: 122,372,241 (§10.1) */
/* AU1: the "AB" magic is reported, not required to load a bank (§10.7). */
uint32_t fc4_audio_aems_magic_present(uint32_t version, uint8_t magic0, uint8_t magic1);
/* 0x1C for version >= 8, 0x14 below (§10.7). */
uint32_t fc4_audio_aems_module_stride(uint32_t version);

/* ---- the director (§1.1) ---- */
uint32_t fc4_audio_director_id(void);          /* 0xFDB5CA17 */
uint32_t fc4_audio_director_class_count(void); /* 92 */
/* Copies the index-th OnStart name NUL-terminated; returns its length without
 * the NUL, or 0xFFFFFFFF when index is out of range. `out` may be NULL. */
uint32_t fc4_audio_director_on_start(uint32_t index, uint8_t* out, uint32_t cap);

/* ---- nothing is saved (§12) ---- always 0. */
uint32_t fc4_audio_has_save_record(void);

/* ---- inspection ------------------------------------------------------------ */
bool fc4_sim_get_state(const fc4_sim* sim, fc4_state* out);
/* Diagnostics: reads the clock once, changes nothing. */
bool fc4_sim_get_debug(const fc4_sim* sim, fc4_debug* out);
/* Diagnostics: the registration at `index` of `level` (registration order).
 * The name is copied NUL-terminated (truncated to name_capacity - 1); `name`
 * may be NULL. Returns false if there is no such registration. */
bool fc4_sim_get_agent_info(const fc4_sim* sim, uint32_t level, size_t index, fc4_agent_info* out, char* name,
                            size_t name_capacity);

/* ---- prop occupant (docs/specs/prop-occupant.md) ---------------------------- */

/* The process-wide prop type ID table of 5.1 (0x00B231A4): the +0x5C u16 is an
 * index into it, not an exemplar key. GetPropTypeIndex appends an unknown ID,
 * so the table grows across a session (quirk 2). */
typedef struct fc4_prop_type_table fc4_prop_type_table;
/* One cSC4PropOccupant (class 0x2977AA47, 0x68 bytes). */
typedef struct fc4_prop_occupant fc4_prop_occupant;
/* One cSC4PropOccupantVisualEffect (class 0x2977AA49, 0x94 bytes). */
typedef struct fc4_prop_effect fc4_prop_effect;

/* What the occupant reaches outside itself (3, 6.1, network-tools 13.1).
 * Every callback receives `user`. */
typedef struct fc4_prop_occupant_host {
  uint32_t struct_size;           /* sizeof(fc4_prop_occupant_host) */
  void* user;
  /* Whether the occupant is in the city: SetState posts only when true. */
  int32_t (*is_in_city)(void* user);
  /* Post an occupant message carrying this prop and data1. */
  void (*post_occupant_message)(void* user, uint32_t msg_type, uint32_t data1);
  /* Whether the exemplar has property 0x4A70D491: 1 true, 0 false,
   * -1 no exemplar is attached (quirk 6). */
  int32_t (*has_model_state_property)(void* user);
  /* Switch the drawn model instances to `state`. */
  void (*set_model_state)(void* user, uint8_t state);
  /* Register or clear the 24-hour clock entry; (0, 0) clears it. */
  void (*set_clock_entry)(void* user, uint8_t start, uint8_t end);
  /* One draw from the prop's own generator 0x00B231B0, uniform in 0..100. */
  uint8_t (*draw_chance)(void* user);
  /* Remove this prop from the occupant manager; 1 when it was there. */
  int32_t (*remove_from_city)(void* user);
} fc4_prop_occupant_host;

/* What the visual-effect prop reaches outside itself (6.2). */
typedef struct fc4_prop_effect_host {
  uint32_t struct_size;
  void* user;
  /* (Re)create the effect object; returns the handle or 0. */
  uint64_t (*restart_effect)(void* user, uint32_t id, const uint8_t* name, uint32_t name_len);
  void (*effect_slot3)(void* user, uint64_t handle);
  void (*effect_slot4)(void* user, uint64_t handle);
  int32_t (*flag2)(void* user);
} fc4_prop_effect_host;

/* One compiled slot of cISC4PropOccupant (4.1). */
typedef struct fc4_prop_slot_info {
  uint8_t slot;                   /* compiled slot, 0..23 */
  uint8_t sdk_index;              /* compiled slot - 3 */
  uint8_t callee_pop;
  uint8_t arg_dwords;
  uint32_t offset;
  uint32_t address;               /* the Windows address of the body */
  uint32_t ret;                   /* FC4_RET_* */
} fc4_prop_slot_info;

/* Measured return channels (4.1). docs/abi/interfaces.tsv labels all 24 rows
 * eax-or-void; the measurements differ. */
#define FC4_RET_EAX 0u            /* a real value in EAX */
#define FC4_RET_AL 1u             /* only AL: mask to 0xFF, never a bool */
#define FC4_RET_VOID 2u           /* no result; ST(0) empty at return */
#define FC4_RET_VIA_CALL 3u       /* a tail call */
#define FC4_RET_EAX_OR_VOID 4u    /* the one ambiguous slot (8) */

/* TagMatch of 5.3 / prop-developer 4.3 step 2.3.2. */
#define FC4_TAG_MATCHED 0
#define FC4_TAG_CONFIG_LACKS_TAG 1
#define FC4_TAG_NO_CONFIGURATION 2
#define FC4_TAG_UNKNOWN_CONFIGURATION 3
#define FC4_TAG_NONE 4

/* ReadOutcome of 7 / lot-records 10.1. */
#define FC4_PROP_RECORD_OK 0
#define FC4_PROP_RECORD_BASE_FAILED 1  /* a failing base read reports success */
#define FC4_PROP_RECORD_TRUNCATED 2

/* The type table (5.1). */
fc4_prop_type_table* fc4_prop_type_table_create(void);
void fc4_prop_type_table_destroy(fc4_prop_type_table* t);
uint32_t fc4_prop_type_table_len(const fc4_prop_type_table* t);
/* GetPropTypeIndex: interning at the end when the ID is unknown. */
uint32_t fc4_prop_type_table_intern(const fc4_prop_type_table* t, uint32_t id);
uint32_t fc4_prop_type_table_id_at(const fc4_prop_type_table* t, uint32_t index);

/* The prop occupant. */
fc4_prop_occupant* fc4_prop_occupant_create(const fc4_prop_occupant_host* host,
                                            const fc4_prop_type_table* types);
void fc4_prop_occupant_destroy(fc4_prop_occupant* p);
/* QueryInterface (1.1): the sub-object offset, or -1 when the base refuses. */
int32_t fc4_prop_occupant_query_interface(uint32_t iid);
uint32_t fc4_prop_occupant_add_ref(fc4_prop_occupant* p);
uint32_t fc4_prop_occupant_release(fc4_prop_occupant* p);
uint32_t fc4_prop_occupant_refcount(fc4_prop_occupant* p);

uint32_t fc4_prop_occupant_get_prop_type(const fc4_prop_occupant* p);
void fc4_prop_occupant_set_prop_type(fc4_prop_occupant* p, uint32_t id);
uint32_t fc4_prop_occupant_prop_type_index(const fc4_prop_occupant* p);
int32_t fc4_prop_occupant_get_orientation(const fc4_prop_occupant* p);
/* Slot 10: channel al, clamps to 0..3, always returns 1. */
int32_t fc4_prop_occupant_set_orientation(fc4_prop_occupant* p, int32_t orientation);
/* Slot 12: channel eax, a clean zero-extended value. */
uint32_t fc4_prop_occupant_get_state(const fc4_prop_occupant* p);
void fc4_prop_occupant_set_state(fc4_prop_occupant* p, uint8_t state);
/* Slots 13/14: void, ST(0) empty at return. */
void fc4_prop_occupant_set_time_of_day_range(fc4_prop_occupant* p, float start, float end);
void fc4_prop_occupant_get_time_of_day_range(const fc4_prop_occupant* p, float* a, float* b);
void fc4_prop_occupant_set_date_range(fc4_prop_occupant* p, uint32_t interval, uint32_t duration,
                                      uint32_t start_day, uint32_t end_day);
int32_t fc4_prop_occupant_has_date_range(const fc4_prop_occupant* p);
void fc4_prop_occupant_set_random_chance(fc4_prop_occupant* p, uint8_t chance);
/* Slot 18: channel al. The value is a percent 1..100, never a bool. */
uint8_t fc4_prop_occupant_get_random_chance(const fc4_prop_occupant* p);
uint32_t fc4_prop_occupant_get_timer_bitfield(const fc4_prop_occupant* p);
/* Slot 20: channel eax, a real 0/1. */
int32_t fc4_prop_occupant_get_timer_state(const fc4_prop_occupant* p, uint8_t mask);
void fc4_prop_occupant_set_timer_state(fc4_prop_occupant* p, uint8_t mask, int32_t on);
/* Slot 22: channel al. The value is a code 1..4, never a bool. */
uint8_t fc4_prop_occupant_get_prop_usage(const fc4_prop_occupant* p);
void fc4_prop_occupant_set_prop_usage(fc4_prop_occupant* p, uint8_t usage);
/* The usage each Add*Prop path writes (3): kind 1 city, 2 lot, 3 building,
 * 4 street. */
uint8_t fc4_prop_add_usage(uint32_t kind);

uint32_t fc4_prop_occupant_lot_tag(const fc4_prop_occupant* p);
int32_t fc4_prop_occupant_has_lot_tag(const fc4_prop_occupant* p);
void fc4_prop_occupant_set_lot_tag(fc4_prop_occupant* p, uint32_t tag);
void fc4_prop_occupant_set_bounding_box(fc4_prop_occupant* p, const float* min, const float* max);
void fc4_prop_occupant_get_bounding_box(const fc4_prop_occupant* p, float* min, float* max);
int32_t fc4_prop_occupant_init_after_load(fc4_prop_occupant* p);
int32_t fc4_prop_occupant_shutdown(fc4_prop_occupant* p);
void fc4_prop_occupant_set_loaded(fc4_prop_occupant* p, int32_t loaded);

/* The slot table (4.1). */
uint32_t fc4_prop_occupant_slot_count(void);
int32_t fc4_prop_occupant_slot_info(uint32_t index, fc4_prop_slot_info* out);
int32_t fc4_prop_occupant_slot_channel(uint32_t index);
/* Object size by occupant class (1.2, test 3). */
uint32_t fc4_prop_object_size(uint32_t class_id);

/* The save record (7 / lot-records 10.1). */
int32_t fc4_prop_occupant_read_record(fc4_prop_occupant* p, const uint8_t* bytes, uint32_t len);
int32_t fc4_prop_occupant_write_record(fc4_prop_occupant* p, uint8_t* out, uint32_t cap,
                                       uint32_t* written);
/* Returns the record version, or -1. */
int32_t fc4_prop_record_read_summary(const uint8_t* bytes, uint32_t len, uint32_t* out_prop_type,
                                     uint8_t* out_timer_bits, uint32_t* out_lot_tag);

/* The lot tag and the PV2 rule (5.3). */
int32_t fc4_prop_tag_classify(uint32_t tag, const uint32_t* ids, uint32_t ids_len, int32_t unknown);
/* Corpus figures: which 0 tagged, 1 unmatched, 2 config-lacks-tag,
 * 3 no-configuration, 4 unknown-configuration, 5 percent x 100. */
uint32_t fc4_prop_tag_corpus(uint32_t which);

/* The visual-effect prop (6.2). */
fc4_prop_effect* fc4_prop_effect_create(const fc4_prop_effect_host* host,
                                        const fc4_prop_type_table* types);
void fc4_prop_effect_destroy(fc4_prop_effect* e);
int32_t fc4_prop_effect_set_type_name(fc4_prop_effect* e, const uint8_t* name, uint32_t name_len);
int32_t fc4_prop_effect_set_type_id(fc4_prop_effect* e, uint32_t id);
/* Returns 1 only when an effect object exists at +0x6C. */
int32_t fc4_prop_effect_start(fc4_prop_effect* e, int32_t b);
/* Returns 1 only when one existed and was started. */
int32_t fc4_prop_effect_stop(fc4_prop_effect* e, int32_t b);
int32_t fc4_prop_effect_is_live(const fc4_prop_effect* e);
int32_t fc4_prop_effect_is_started(const fc4_prop_effect* e);
/* 1 when Write drops the name and the trailing u32 (lot-records 10.2). */
int32_t fc4_prop_effect_should_drop_name(const fc4_prop_effect* e);
int32_t fc4_prop_effect_sub_object_offsets(uint32_t* out);
int32_t fc4_prop_effect_write_record(fc4_prop_effect* e, uint8_t* out, uint32_t cap,
                                     uint32_t* written);
int32_t fc4_prop_effect_read_record(fc4_prop_effect* e, const uint8_t* bytes, uint32_t len,
                                    uint32_t* out_name_len);

/* Constants (1, 2.2, 5.3, 7). */
uint32_t fc4_prop_class_id(void);
uint32_t fc4_prop_occupant_type(void);
uint32_t fc4_prop_no_lot_tag(void);
/* out[0] record version 6, out[1] effect version 3, out[2] legacy timer bits
 * 0x0F, out[3] occupant base version 4. */
int32_t fc4_prop_record_versions(uint16_t* out);

/* ---- network tools (docs/specs/network-tools.md §2-§11; engine/capi/src/network_tools.rs) ----
 * The network tool's own half of PlaceNetwork over the geometry core
 * (fc4_netgeom): the tool state (§2), PlaceNetworkOccupants (slot 7, §3),
 * PlaceNetworkOccupantById (slot 8, §4), the PlaceNetwork pipeline hand-off
 * (§5) with the commit order (§6), DestroyOccupantsAndResolve (slot 50, §10)
 * and UpdateUndergroundOccupantTerrainHeights (§11). No plugin glue yet.
 *
 * The world (occupants, poles, the terrain, the geometry core, the crew)
 * goes through the host table; a missing callback is the empty world (no
 * occupants, no neighbours, no structures, the passes keep the counts). The
 * geometry itself runs through the host's draw/validate callbacks, which a
 * glue host binds to an fc4_netgeom.
 *
 * Threads: one thread at a time. A callback that calls back into the same
 * tool is refused (fallback value, counted in reentrant_refused). */
typedef struct fc4_nettool fc4_nettool;

/* One cell (§2.2's 8-byte cell, §10's (x, z)). */
typedef struct fc4_nettool_cell {
    int32_t x, z;
} fc4_nettool_cell;

/* A neighbour of the destroy closure (§10 step 3): flags 0x400000 immovable,
 * 0x100000 intersection. */
typedef struct fc4_nettool_neighbour {
    uint64_t occ;
    int32_t x, z;
    uint8_t immovable, intersection;
    uint8_t reserved[6];
} fc4_nettool_neighbour;

/* One collected occupant of §10 step 2. */
typedef struct fc4_nettool_listed {
    uint64_t occ;
    int32_t x, z;
} fc4_nettool_listed;

/* One underground piece of §11: slot 94's stored terrain heights, slot 34's
 * vertex heights, slot 38-with-type's edge mask. */
typedef struct fc4_nettool_underground {
    uint64_t occ;
    int32_t x, z;
    float stored_terrain[4]; /* (x,z), (x,z+1), (x+1,z+1), (x+1,z) */
    float vertices[4];
    uint32_t edges;
} fc4_nettool_underground;

/* FindUndergroundOccupant's solved record (§11.1 step 4): the occupant and
 * the four solved corner heights. */
typedef struct fc4_nettool_solved {
    uint64_t occ;
    float corners[4];
} fc4_nettool_solved;

/* The pass/solve result (§5.1 steps 4-5): the valid count and the failure
 * code (0 = none; §14 of network-geometry.md). */
typedef struct fc4_nettool_pass {
    uint32_t valid_count;
    uint32_t failure;
} fc4_nettool_pass;

/* The largest cell list one run carries. */
#define FC4_NETTOOL_MAX_CELLS 4096u

/* One run of DrawNetworkLine/PlaceNetwork (§3.2 step 2 / §3.3 step 5.4):
 * the drag inputs, the counters (+0x70/+0x74/+0x78), the current +0x208 and
 * the cell lists. */
typedef struct fc4_nettool_run {
    uint32_t struct_size;
    uint32_t type_id;
    int32_t start_x, start_z, end_x, end_z;
    uint32_t dragged_count, straight_count, valid_count;
    uint32_t data4; /* +0x208 at the insert */
    uint32_t cell_count;
    fc4_nettool_cell cells[FC4_NETTOOL_MAX_CELLS];
    uint32_t resolve_count;
    fc4_nettool_cell resolve_cells[FC4_NETTOOL_MAX_CELLS];
} fc4_nettool_run;

/* UpdateUndergroundOccupantTerrainHeights' outcomes (§11.1 steps 4-5). */
#define FC4_NETTOOL_U_UPDATED 0u
#define FC4_NETTOOL_U_KEPT 1u
#define FC4_NETTOOL_U_DESTROY 2u

/* What the network tool needs from the game. Occupants are opaque u64 host
 * tokens; 0xFFFFFFFF as a type means "none". */
typedef struct fc4_nettool_host {
    uint32_t struct_size;
    void* user;
    /* ---- the drag and the pipeline (§3.2, §5) ---- */
    bool (*draw_network_line)(void* user, fc4_nettool_run* run, bool snap); /* slot 18 */
    bool (*do_network_specific_placement)(void* user, fc4_nettool_run* run, uint32_t arg3,
                                          uint32_t neighbour_mode, bool real); /* slot 53 */
    /* §5.1 steps 4-5 through the geometry core; false = no host. */
    bool (*validate_and_solve)(void* user, fc4_nettool_run* run, bool real, fc4_nettool_pass* out);
    bool (*attempt_neighbor_connections)(void* user, fc4_nettool_run* run);
    void (*place_neighbor_connections)(void* user, fc4_nettool_run* run);
    /* §3.3: GetNetworkOccupant with the OR filter (occupant manager slot 32 +
     * QI 0xA821EF94): the occupant's network type, or 0xFFFFFFFF = none. */
    uint32_t (*get_network_occupant)(void* user, int32_t x, int32_t z, uint32_t mask);
    /* §3.3 step 6.2: the callback tool's PlaceNetworkOccupants (one-cell
     * join in the callback tool's own type). */
    void (*callback_place)(void* user, uint32_t callback, int32_t ax, int32_t az, int32_t bx, int32_t bz,
                           bool real);
    /* ---- the commit (§6) ---- */
    void (*commit_pre)(void* user, const fc4_nettool_run* run, bool real);
    void (*insert_occupants_for_solved_cells)(void* user, const fc4_nettool_run* run, bool preview_only);
    void (*insert_bridge_pieces)(void* user, const fc4_nettool_run* run);
    void (*insert_tunnel_pieces)(void* user, const fc4_nettool_run* run);
    void (*insert_highway_intersection_pieces)(void* user, const fc4_nettool_run* run);
    bool (*crew_parameters_both_one)(void* user, uint32_t type_id);
    void (*crew_add_network_segment)(void* user, uint64_t crew, const fc4_nettool_run* run);
    /* §6.3 MarkOccupantsUsable(set, release) — unconditional in real mode
     * (quirk 5). */
    void (*mark_occupants_usable)(void* user, const fc4_nettool_run* run, bool release);
    void (*commit_draft)(void* user, const fc4_nettool_run* run);
    /* ---- §4 / §6.4 ---- */
    void (*insert_network_occupant_by_id)(void* user, const fc4_nettool_run* run, int32_t x, int32_t z,
                                          uint32_t piece, bool usable, uint32_t type_id);
    /* Slot 9 OccupantExists(x, z, flags, &mask) (zones.md §8.8): true with
     * mask[0..4] written, false with mask untouched. */
    bool (*occupant_exists)(void* user, int32_t x, int32_t z, uint32_t flags, uint8_t* mask);
    /* ---- §10 DestroyOccupantsAndResolve ---- */
    /* Network occupant slot 59 GetOccupiedCell. */
    bool (*occupied_cell)(void* user, uint64_t occ, fc4_nettool_cell* out);
    /* The occupant's neighbours by network type; writes up to `capacity`,
     * returns the total (called again with a larger buffer). */
    uint32_t (*neighbours_by_network_type)(void* user, uint64_t occ, fc4_nettool_neighbour* out,
                                           uint32_t capacity);
    bool (*is_intersection_piece)(void* user, uint64_t occ); /* flag 0x100000 */
    /* Network manager slot 12 GetAllIntersectionPieces. */
    uint32_t (*intersection_structure)(void* user, uint64_t occ, uint64_t* out, uint32_t capacity);
    bool (*is_tunnel_portal)(void* user, uint64_t occ); /* flag 0x80000 */
    /* The portal at the other end (QI 0x8A4BD52B); 0 = none. */
    uint64_t (*paired_portal)(void* user, uint64_t occ);
    /* §10 step 5.1: MessageSend of 0xAA7A014D (synchronous). */
    void (*send_pre_demolish_sync)(void* user, uint64_t occ);
    /* §10 step 5.3 (destroyed cell): occupantManager.RemoveOccupant(o, false, 0). */
    void (*remove_occupant)(void* user, uint64_t occ, bool shutdown);
    /* §10 step 5.3 (closure piece): RemoveOrReplaceOccupant — true when a
     * plain piece was put back. */
    bool (*remove_or_replace_occupant)(void* user, uint64_t occ);
    void (*occupant_shutdown)(void* user, uint64_t occ);
    void (*release)(void* user, uint64_t occ);
    /* §10 step 6.1: SolveNetwork(−1, true) over the re-solve set. */
    void (*solve_network_resolve)(void* user, fc4_nettool_run* run);
    /* ---- §11 the underground refit ---- */
    /* Terrain slot 17: the cell's current corner altitudes. */
    void (*corner_altitudes)(void* user, int32_t x, int32_t z, float out[4]);

    /* §11.1 step 3: SmoothenNetwork(level) over the solved records. */
    bool (*smoothen_underground)(void* user, fc4_nettool_run* run, int32_t level);
    /* §11.1 step 4: FindUndergroundOccupant(x, z, class). */
    bool (*find_underground_solved)(void* user, int32_t x, int32_t z, fc4_nettool_solved* out);
    /* §11.1 step 5.4: DestroyNetworkOccupants / ReleaseOccupantList. */
    void (*destroy_network_occupants)(void* user, const uint64_t* occ, uint32_t count);
    void (*release_occupant_list)(void* user, const uint64_t* occ, uint32_t count);
    /* §11.1 step 6: slot 86 (true), SetUsable(false), slot 50, SetUsable(true),
     * Release — in that order; the usable toggle posts 0x89D3E9F3 data1 = 0
     * then 1. vertex_heights is set for U_UPDATED, stored_terrain for
     * U_UPDATED and U_KEPT. */
    void (*underground_rebuild)(void* user, uint64_t occ, uint32_t outcome, const float* vertex_heights,
                                const float* stored_terrain);
} fc4_nettool_host;

/* Diagnostics (fc4_nettool_get_state). */
typedef struct fc4_nettool_state {
    uint32_t struct_size;
    uint32_t type_id;
    uint8_t real, stop_on_failure, destroyed_flag, preview_update;
    uint8_t reserved[4];
    uint32_t data4;      /* +0x208 */
    uint32_t failure;    /* +0x20C, 0 = none */
    int32_t demolition_cost; /* +0x220 */
    uint32_t layer_mask;     /* +0x360 */
    float depth;             /* +0x36C */
    float cover_margin_a;    /* +0x370 */
    float cover_margin_b;    /* +0x378 */
    uint32_t city_x, city_z; /* +0x240, +0x244 */
    uint32_t cache_count;
    uint32_t reentrant_refused;
} fc4_nettool_state;

/* A tool of `type_id` over `host`. NULL on a bad host table size. */
fc4_nettool* fc4_nettool_create(const fc4_nettool_host* host, uint32_t type_id);
void fc4_nettool_destroy(fc4_nettool* t);
/* Init (slot 3): the city size and the underground fields from the host's
 * exemplar reads (ReloadNetworkSpecificInfo). */
bool fc4_nettool_init(fc4_nettool* t, uint32_t city_x, uint32_t city_z, float depth, float cover_a,
                      float cover_b, float piece_tolerance, bool land_bridges);
/* Reset (slot 5): empties the world cache and the step counts. */
bool fc4_nettool_reset(fc4_nettool* t);
/* ClearFailureReason (slot 13). */
bool fc4_nettool_clear_failure(fc4_nettool* t);
/* EnablePreviewUpdate (slot 21): +0x234. */
bool fc4_nettool_enable_preview_update(fc4_nettool* t, bool on);
/* GetFailureState (slot 14): +0x20C. */
uint32_t fc4_nettool_get_failure_state(const fc4_nettool* t);
/* OccupantExists (slot 9, zones.md §8.8). */
bool fc4_nettool_occupant_exists(const fc4_nettool* t, int32_t x, int32_t z, uint32_t flags, uint8_t* mask);
/* PlaceNetworkOccupants (slot 7, §3): AL. callback 0 = none. */
bool fc4_nettool_place_network_occupants(fc4_nettool* t, int32_t x1, int32_t z1, int32_t x2, int32_t z2,
                                         uint32_t mask, uint32_t callback, bool real, int32_t type_id);
/* PlaceNetworkOccupantById (slot 8, §4). */
bool fc4_nettool_place_network_occupant_by_id(fc4_nettool* t, int32_t x, int32_t z, uint32_t piece,
                                              bool usable, int32_t type_id);
/* PlaceNetwork (§5) on a caller's run: true on success, false on refusal
 * (the failure code is in the state). crew 0 = none. */
bool fc4_nettool_place_network(fc4_nettool* t, const fc4_nettool_run* run, uint64_t crew, uint32_t arg3,
                               uint32_t neighbour_mode, bool real);
/* DestroyOccupantsAndResolve (slot 50, §10). */
bool fc4_nettool_destroy_occupants_and_resolve(fc4_nettool* t, const fc4_nettool_listed* list, uint32_t count);
/* UpdateUndergroundOccupantTerrainHeights (§11): the pipe/subway refit. */
bool fc4_nettool_update_underground_heights(fc4_nettool* t, const fc4_nettool_underground* pieces,
                                            uint32_t count, uint32_t network_type);
/* Diagnostics. */
bool fc4_nettool_get_state(const fc4_nettool* t, fc4_nettool_state* out);

/* ---- DBPF packages and exemplars (data-formats §2, §4; glue-formats4) ---------
 * A package opened and parsed by the core's fc4_formats, for the plug-in's
 * formats shadow (FC4FormatsShadow.h). Additive; no FC4_ABI_VERSION bump. */
typedef struct fc4_dbpf fc4_dbpf;
/* `path`: UTF-8 bytes, no terminator. NULL when unreadable or the header or
 * index fail data-formats §2.1-§2.3. */
fc4_dbpf* fc4_dbpf_open(const uint8_t* path, size_t len);
void fc4_dbpf_close(fc4_dbpf* p);
/* Index entries of `type_id` (duplicates included, index order). */
size_t fc4_dbpf_count_type(const fc4_dbpf* p, uint32_t type_id);
/* Group and instance of the index-th entry of `type_id`; false out of range. */
bool fc4_dbpf_key_of_type(const fc4_dbpf* p, uint32_t type_id, size_t index, uint32_t* group, uint32_t* instance);
/* Entry (type, group, instance), first in index order (Q3), parsed as an
 * exemplar or cohort (§4): a new fc4_props with its OWN properties in the
 * game's variant types and shapes (Empty and kind-1 resource properties left
 * out); `parent` (3 words, may be NULL) gets the parent cohort key. NULL when
 * absent, undecodable or not an exemplar of that type. Caller destroys. */
fc4_props* fc4_dbpf_read_exemplar(const fc4_dbpf* p, uint32_t type_id, uint32_t group, uint32_t instance,
                                  uint32_t* parent);

/* Packages combined as the resource manager combines them (data-formats §3,
 * each added at the front) and resolved with parents and effective lists
 * (§4.4) by the core's ExemplarStore. Keys are uint32_t[3] (type, group,
 * instance). See engine/capi/src/dbpf.rs. */
typedef struct fc4_exstore fc4_exstore;
fc4_exstore* fc4_exstore_create(void);
void fc4_exstore_destroy(fc4_exstore* p);
bool fc4_exstore_add_package(fc4_exstore* p, const uint8_t* path, size_t len, uint32_t* counts3);
size_t fc4_exstore_resolve(fc4_exstore* p, size_t* failed);
int32_t fc4_exstore_key(const fc4_exstore* p, size_t index, uint32_t* key3);
fc4_props* fc4_exstore_props(const fc4_exstore* p, size_t index, bool effective, size_t* total, uint32_t* parent3);
size_t fc4_exstore_untyped_ids(const fc4_exstore* p, size_t index, bool effective, uint32_t* ids, size_t cap);
bool fc4_exstore_failure(const fc4_exstore* p, size_t index, uint32_t* key3, uint8_t* buf, size_t cap);

/* One exemplar/cohort record parsed for the plug-in resource factory
 * (data-formats §4.1/§4.2/§8.1): every property in record order, Empty and
 * kind-1 resource-key properties reported by kind (0 value, 1 Empty,
 * 2 resource key, -1 out of range). Value data is borrowed from the record. */
typedef struct fc4_exrec fc4_exrec;
fc4_exrec* fc4_exrec_parse(uint32_t type_id, const uint8_t* data, size_t len);
void fc4_exrec_destroy(fc4_exrec* p);
bool fc4_exrec_parent(const fc4_exrec* p, uint32_t* key3);
size_t fc4_exrec_count(const fc4_exrec* p);
int32_t fc4_exrec_prop(const fc4_exrec* p, size_t index, uint32_t* id, uint16_t* mem_type, bool* is_array,
                       const uint8_t** data, size_t* count, uint32_t* key3);

#ifdef __cplusplus
} /* extern "C" */
#endif

/* Layout checks (must match #[repr(C)] in engine/capi/src/lib.rs). */
#ifdef __cplusplus
static_assert(sizeof(fc4_audio_post) == 32, "fc4_audio_post layout");
static_assert(sizeof(fc4_audio_pool_page) == 36, "fc4_audio_pool_page layout");
static_assert(sizeof(fc4_audio_state) == 52, "fc4_audio_state layout");
static_assert(sizeof(fc4_message) == 32, "fc4_message layout");
static_assert(sizeof(fc4_date) == 24, "fc4_date layout");
static_assert(sizeof(fc4_debug) == 76, "fc4_debug layout");
static_assert(sizeof(fc4_agent_info) == 16, "fc4_agent_info layout");
static_assert(sizeof(fc4_timer_source) == (sizeof(void*) == 4 ? 32 : 48), "fc4_timer_source layout");
static_assert(sizeof(fc4_state) == 4 + 4 + 5 * 4 + 5 * 4 + 3 * 4 + 5 * 4 + 11 * 4 + 12 + 11 * 4, "fc4_state layout");
static_assert(sizeof(fc4_budget_message) == 48, "fc4_budget_message layout");
static_assert(sizeof(fc4_lot_info) == 32, "fc4_lot_info layout");
static_assert(sizeof(fc4_budget_item) == 32, "fc4_budget_item layout");
static_assert(sizeof(fc4_loan_info) == 32, "fc4_loan_info layout");
static_assert(sizeof(fc4_bond) == 56, "fc4_bond layout");
static_assert(sizeof(fc4_budget_group) == 24, "fc4_budget_group layout");
static_assert(sizeof(fc4_budget_state) == 432, "fc4_budget_state layout");
static_assert(sizeof(fc4_budget_host) == (sizeof(void*) == 4 ? 128 : 256), "fc4_budget_host layout");
static_assert(sizeof(fc4_tax_lot) == 24, "fc4_tax_lot layout");
static_assert(sizeof(fc4_tax_scan) == 408, "fc4_tax_scan layout");
static_assert(sizeof(fc4_grid_geometry) == 32, "fc4_grid_geometry layout");
static_assert(sizeof(fc4_landvalue_state) == 56, "fc4_landvalue_state layout");
static_assert(sizeof(fc4_landvalue_host) == (sizeof(void*) == 4 ? 96 : 192), "fc4_landvalue_host layout");
static_assert(sizeof(fc4_landvalue_tuning) == (sizeof(void*) == 4 ? 72 : 128), "fc4_landvalue_tuning layout");
static_assert(sizeof(fc4_weather_state) == 56, "fc4_weather_state layout");
static_assert(sizeof(fc4_weather_host) == (sizeof(void*) == 4 ? 56 : 112), "fc4_weather_host layout");
static_assert(sizeof(fc4_ordinance_sim_state) == 40, "fc4_ordinance_sim_state layout");
static_assert(sizeof(fc4_ordinance_host) == (sizeof(void*) == 4 ? 104 : 208), "fc4_ordinance_host layout");
static_assert(sizeof(fc4_demand_connection) == 24, "fc4_demand_connection layout");
static_assert(sizeof(fc4_demand_growth_developer) == 24, "fc4_demand_growth_developer layout");
static_assert(sizeof(fc4_demand_sim_state) == 44, "fc4_demand_sim_state layout");
static_assert(sizeof(fc4_demand_host) == (sizeof(void*) == 4 ? 172 : 344), "fc4_demand_host layout");
static_assert(sizeof(fc4_demand_node_state) == 48, "fc4_demand_node_state layout");
static_assert(sizeof(fc4_demand_shadow_info) == 24, "fc4_demand_shadow_info layout");
static_assert(sizeof(fc4_demand_shadow_stats) == 32, "fc4_demand_shadow_stats layout");
static_assert(sizeof(fc4_demand_modifiers) == 400, "fc4_demand_modifiers layout");
static_assert(sizeof(fc4_grid_api) == (sizeof(void*) == 4 ? 40 : 80), "fc4_grid_api layout");
static_assert(sizeof(fc4_cell_rect) == 16, "fc4_cell_rect layout");
static_assert(sizeof(fc4_power_post) == 32, "fc4_power_post layout");
static_assert(sizeof(fc4_power_pole) == 12, "fc4_power_pole layout");
static_assert(sizeof(fc4_power_line) == 12, "fc4_power_line layout");
static_assert(sizeof(fc4_power_deal) == 24, "fc4_power_deal layout");
static_assert(sizeof(fc4_power_budget_item) == 24, "fc4_power_budget_item layout");
static_assert(sizeof(fc4_power_message) == 48, "fc4_power_message layout");
static_assert(sizeof(fc4_power_found) == 32, "fc4_power_found layout");
static_assert(sizeof(fc4_power_state) == 88, "fc4_power_state layout");
static_assert(sizeof(fc4_power_host) == (sizeof(void*) == 4 ? 400 : 800), "fc4_power_host layout");
static_assert(sizeof(fc4_power_capture_ref) == 32, "fc4_power_capture_ref layout");
static_assert(sizeof(fc4_power_capture) == (sizeof(void*) == 4 ? 24 : 40), "fc4_power_capture layout");
static_assert(sizeof(fc4_power_shadow_counts) == 24, "fc4_power_shadow_counts layout");
static_assert(sizeof(fc4_power_shadow_options) == (sizeof(void*) == 4 ? 20 : 24), "fc4_power_shadow_options layout");
static_assert(sizeof(fc4_power_shadow_stats) == 72, "fc4_power_shadow_stats layout");
static_assert(sizeof(fc4_plumbing_budget_item) == 24, "fc4_plumbing_budget_item layout");
static_assert(sizeof(fc4_plumbing_message) == 40, "fc4_plumbing_message layout");
static_assert(sizeof(fc4_plumbing_producer_info) == 20, "fc4_plumbing_producer_info layout");
static_assert(sizeof(fc4_plumbing_found) == 32, "fc4_plumbing_found layout");
static_assert(sizeof(fc4_plumbing_state) == 144, "fc4_plumbing_state layout");
static_assert(sizeof(fc4_plumbing_host) == (sizeof(void*) == 4 ? 356 : 712), "fc4_plumbing_host layout");
static_assert(sizeof(fc4_plumbing_capture_ref) == 40, "fc4_plumbing_capture_ref layout");
static_assert(sizeof(fc4_plumbing_capture) == (sizeof(void*) == 4 ? 24 : 40), "fc4_plumbing_capture layout");
static_assert(sizeof(fc4_plumbing_original) == (sizeof(void*) == 4 ? 32 : 64), "fc4_plumbing_original layout");
static_assert(sizeof(fc4_plumbing_shadow_stats) == 72, "fc4_plumbing_shadow_stats layout");
static_assert(sizeof(fc4_flammability_message) == 24, "fc4_flammability_message layout");
static_assert(sizeof(fc4_flammability_state) == 80, "fc4_flammability_state layout");
static_assert(sizeof(fc4_flammability_host) == (sizeof(void*) == 4 ? 136 : 272), "fc4_flammability_host layout");
static_assert(sizeof(fc4_pollution_budget_item) == 32, "fc4_pollution_budget_item layout");
static_assert(sizeof(fc4_pollution_grid_stats) == 24, "fc4_pollution_grid_stats layout");
static_assert(sizeof(fc4_pollution_state) == 408, "fc4_pollution_state layout");
static_assert(sizeof(fc4_pollution_host) == (sizeof(void*) == 4 ? 256 : 512), "fc4_pollution_host layout");
static_assert(sizeof(fc4_pollution_capture_ref) == 32, "fc4_pollution_capture_ref layout");
static_assert(sizeof(fc4_pollution_capture) == (sizeof(void*) == 4 ? 24 : 40), "fc4_pollution_capture layout");
static_assert(sizeof(fc4_pollution_shadow_stats) == 64, "fc4_pollution_shadow_stats layout");
static_assert(sizeof(fc4_protection_budget_item) == 32, "fc4_protection_budget_item layout");
static_assert(sizeof(fc4_protection_dispatch_info) == 32, "fc4_protection_dispatch_info layout");
static_assert(sizeof(fc4_protection_message) == 32, "fc4_protection_message layout");
static_assert(sizeof(fc4_protection_state) == 152, "fc4_protection_state layout");
static_assert(sizeof(fc4_protection_host) == (sizeof(void*) == 4 ? 220 : 440), "fc4_protection_host layout");
static_assert(sizeof(fc4_crime_message) == 24, "fc4_crime_message layout");
static_assert(sizeof(fc4_crime_event) == 28, "fc4_crime_event layout");
static_assert(sizeof(fc4_crime_nearby) == 32, "fc4_crime_nearby layout");
static_assert(sizeof(fc4_crime_state) == 152, "fc4_crime_state layout");
static_assert(sizeof(fc4_crime_host) == (sizeof(void*) == 4 ? 200 : 400), "fc4_crime_host layout");
static_assert(sizeof(fc4_crime_shadow_info) == 24, "fc4_crime_shadow_info layout");
static_assert(sizeof(fc4_crime_shadow_incident) == 24, "fc4_crime_shadow_incident layout");
static_assert(sizeof(fc4_crime_shadow_month_in) == (sizeof(void*) == 4 ? 64 : 88), "fc4_crime_shadow_month_in layout");
static_assert(sizeof(fc4_crime_shadow_month_out) == 40, "fc4_crime_shadow_month_out layout");
static_assert(sizeof(fc4_aura_lot) == 32, "fc4_aura_lot layout");
static_assert(sizeof(fc4_aura_ratings) == 64, "fc4_aura_ratings layout");
static_assert(sizeof(fc4_aura_state) == 120, "fc4_aura_state layout");
static_assert(sizeof(fc4_aura_host) == (sizeof(void*) == 4 ? 176 : 352), "fc4_aura_host layout");
static_assert(sizeof(fc4_aura_capture_ref) == 16, "fc4_aura_capture_ref layout");
static_assert(sizeof(fc4_aura_capture) == (sizeof(void*) == 4 ? 24 : 40), "fc4_aura_capture layout");
static_assert(sizeof(fc4_aura_shadow_stats) == 64, "fc4_aura_shadow_stats layout");
static_assert(sizeof(fc4_aura_shadow_step_in) == 24, "fc4_aura_shadow_step_in layout");
static_assert(sizeof(fc4_flora_record) == 360, "fc4_flora_record layout");
static_assert(sizeof(fc4_flora_occupant) == 16, "fc4_flora_occupant layout");
static_assert(sizeof(fc4_flora_param) == 8, "fc4_flora_param layout");
static_assert(sizeof(fc4_flora_message) == (sizeof(void*) == 4 ? 40 : 56), "fc4_flora_message layout");
static_assert(sizeof(fc4_flora_state) == 180, "fc4_flora_state layout");
static_assert(sizeof(fc4_flora_registry_host) == (sizeof(void*) == 4 ? 20 : 40), "fc4_flora_registry_host layout");
static_assert(sizeof(fc4_flora_host) == (sizeof(void*) == 4 ? 260 : 520), "fc4_flora_host layout");
static_assert(sizeof(fc4_flora_census_entry) == 72, "fc4_flora_census_entry layout");
static_assert(sizeof(fc4_flora_shadow_month_in) == (sizeof(void*) == 4 ? 56 : 96),
              "fc4_flora_shadow_month_in layout");
static_assert(sizeof(fc4_flora_shadow_stats) == 64, "fc4_flora_shadow_stats layout");
static_assert(sizeof(fc4_landfill_region) == (sizeof(void*) == 4 ? 24 : 32), "fc4_landfill_region layout");
static_assert(sizeof(fc4_landfill_effects) == 16, "fc4_landfill_effects layout");
static_assert(sizeof(fc4_landfill_state) == 112, "fc4_landfill_state layout");
static_assert(sizeof(fc4_landfill_host) == (sizeof(void*) == 4 ? 252 : 504), "fc4_landfill_host layout");
static_assert(sizeof(fc4_landfill_shadow_stats) == 72, "fc4_landfill_shadow_stats layout");
static_assert(sizeof(fc4_landvalue_original) == (sizeof(void*) == 4 ? 132 : 152), "fc4_landvalue_original layout");
static_assert(sizeof(fc4_landvalue_shadow_stats) == 48, "fc4_landvalue_shadow_stats layout");
static_assert(sizeof(fc4_flammability_shadow_hotspot) == 12, "fc4_flammability_shadow_hotspot layout");
static_assert(sizeof(fc4_flammability_original) == (sizeof(void*) == 4 ? 128 : 144), "fc4_flammability_original layout");
static_assert(sizeof(fc4_flammability_shadow_stats) == 48, "fc4_flammability_shadow_stats layout");
static_assert(sizeof(fc4_residential_budget_item) == 32, "fc4_residential_budget_item layout");
static_assert(sizeof(fc4_residential_query_data) == 56, "fc4_residential_query_data layout");
static_assert(sizeof(fc4_residential_min_max) == 24, "fc4_residential_min_max layout");
static_assert(sizeof(fc4_residential_snapshot) == 160, "fc4_residential_snapshot layout");
static_assert(sizeof(fc4_residential_state) == 144, "fc4_residential_state layout");
static_assert(sizeof(fc4_residential_host) == (sizeof(void*) == 4 ? 292 : 584), "fc4_residential_host layout");
static_assert(sizeof(fc4_residential_capture_ref) == 32, "fc4_residential_capture_ref layout");
static_assert(sizeof(fc4_residential_capture) == (sizeof(void*) == 4 ? 24 : 40), "fc4_residential_capture layout");
static_assert(sizeof(fc4_residential_shadow_scalars) == 24, "fc4_residential_shadow_scalars layout");
static_assert(sizeof(fc4_residential_shadow_stats) == 72, "fc4_residential_shadow_stats layout");
static_assert(sizeof(fc4_res_key) == 12, "fc4_res_key layout");
static_assert(sizeof(fc4_growth_profile) == 24, "fc4_growth_profile layout");
static_assert(sizeof(fc4_growth_item) == 12, "fc4_growth_item layout");
static_assert(sizeof(fc4_construction_profile) == 0x24, "fc4_construction_profile layout");
static_assert(sizeof(fc4_lot_config) == (sizeof(void*) == 4 ? 36 : 40), "fc4_lot_config layout");
static_assert(sizeof(fc4_building_profile) == 48, "fc4_building_profile layout");
static_assert(sizeof(fc4_development_message) == 40, "fc4_development_message layout");
static_assert(sizeof(fc4_command_param) == 8, "fc4_command_param layout");
static_assert(sizeof(fc4_advice_subject) == 24, "fc4_advice_subject layout");
static_assert(sizeof(fc4_plan_entry) == 24, "fc4_plan_entry layout");
static_assert(sizeof(fc4_development_state) == 208, "fc4_development_state layout");
static_assert(sizeof(fc4_development_host) == (sizeof(void*) == 4 ? 432 : 864), "fc4_development_host layout");
static_assert(sizeof(fc4_growth_message) == 48, "fc4_growth_message layout");
static_assert(sizeof(fc4_growth_state) == 136, "fc4_growth_state layout");
static_assert(sizeof(fc4_growth_host) == (sizeof(void*) == 4 ? 200 : 400), "fc4_growth_host layout");
static_assert(sizeof(fc4_zone_region) == (sizeof(void*) == 4 ? 24 : 32), "fc4_zone_region layout");
static_assert(sizeof(fc4_zones_place_result) == 32, "fc4_zones_place_result layout");
static_assert(sizeof(fc4_zones_dezone) == 32, "fc4_zones_dezone layout");
static_assert(sizeof(fc4_zones_state) == 176, "fc4_zones_state layout");
static_assert(sizeof(fc4_zones_tuning) == 584, "fc4_zones_tuning layout");
static_assert(sizeof(fc4_zones_host) == (sizeof(void*) == 4 ? 188 : 376), "fc4_zones_host layout");
static_assert(sizeof(fc4_zonedev_profile) == 12, "fc4_zonedev_profile layout");
static_assert(sizeof(fc4_zonedev_parcel_result) == 32, "fc4_zonedev_parcel_result layout");
static_assert(sizeof(fc4_zonedev_state) == 56, "fc4_zonedev_state layout");
static_assert(sizeof(fc4_zonedev_host) == (sizeof(void*) == 4 ? 400 : 800), "fc4_zonedev_host layout");
static_assert(sizeof(fc4_zonedev_cfg_object) == (sizeof(void*) == 4 ? 28 : 40), "fc4_zonedev_cfg_object layout");
static_assert(sizeof(fc4_neighbor_deal_data) == 72, "fc4_neighbor_deal_data layout");
static_assert(sizeof(fc4_neighbor_connection_state) == 24, "fc4_neighbor_connection_state layout");
static_assert(sizeof(fc4_neighbors_figures) == 128, "fc4_neighbors_figures layout");
static_assert(sizeof(fc4_neighbors_message) == 24, "fc4_neighbors_message layout");
static_assert(sizeof(fc4_neighbors_state) == 48, "fc4_neighbors_state layout");
static_assert(sizeof(fc4_neighbors_host) == (sizeof(void*) == 4 ? 216 : 432), "fc4_neighbors_host layout");
static_assert(sizeof(fc4_neighbors_shadow_city) == 168, "fc4_neighbors_shadow_city layout");
static_assert(sizeof(fc4_neighbors_shadow_conn) == 40, "fc4_neighbors_shadow_conn layout");
static_assert(sizeof(fc4_neighbors_shadow_input) == (sizeof(void*) == 4 ? 208 : 232), "fc4_neighbors_shadow_input layout");
static_assert(sizeof(fc4_neighbors_shadow_counters) == 32, "fc4_neighbors_shadow_counters layout");
static_assert(sizeof(fc4_lotdev_cfg_object) == 48 + sizeof(void*), "fc4_lotdev_cfg_object layout");
static_assert(sizeof(fc4_lotdev_texture) == 20, "fc4_lotdev_texture layout");
static_assert(sizeof(fc4_lotdev_burnt) == 32, "fc4_lotdev_burnt layout");
static_assert(sizeof(fc4_lotdev_profile) == 12, "fc4_lotdev_profile layout");
static_assert(sizeof(fc4_lotdev_message) == 40, "fc4_lotdev_message layout");
static_assert(sizeof(fc4_lotdev_tuning) == 172, "fc4_lotdev_tuning layout");
static_assert(sizeof(fc4_lotdev_state) == 112, "fc4_lotdev_state layout");
static_assert(sizeof(fc4_lotdev_host) == (sizeof(void*) == 4 ? 8 + 178 * 4 : 16 + 178 * 8), "fc4_lotdev_host layout");
static_assert(sizeof(fc4_history_host) == (sizeof(void*) == 4 ? 36 : 72), "fc4_history_host layout");
static_assert(sizeof(fc4_history_point) == 8, "fc4_history_point layout");
static_assert(sizeof(fc4_history_state) == 28, "fc4_history_state layout");
static_assert(sizeof(fc4_clock24_host) == (sizeof(void*) == 4 ? 52 : 104), "fc4_clock24_host layout");
static_assert(sizeof(fc4_clock24_tuning) == 20, "fc4_clock24_tuning layout");
static_assert(sizeof(fc4_clock24_state) == 56, "fc4_clock24_state layout");
static_assert(sizeof(fc4_commercial_state) == 56, "fc4_commercial_state layout");
static_assert(sizeof(fc4_commercial_host) == (sizeof(void*) == 4 ? 112 : 224), "fc4_commercial_host layout");
static_assert(sizeof(fc4_civic_state) == 40, "fc4_civic_state layout");
static_assert(sizeof(fc4_civic_host) == (sizeof(void*) == 4 ? 160 : 320), "fc4_civic_host layout");
static_assert(sizeof(fc4_park_state) == 48, "fc4_park_state layout");
static_assert(sizeof(fc4_park_host) == (sizeof(void*) == 4 ? 160 : 320), "fc4_park_host layout");
static_assert(sizeof(fc4_demolition_state) == 44, "fc4_demolition_state layout");
static_assert(sizeof(fc4_demolition_host) == (sizeof(void*) == 4 ? 344 : 688), "fc4_demolition_host layout");
static_assert(sizeof(fc4_dispatch_info) == 56, "fc4_dispatch_info layout");
static_assert(sizeof(fc4_station_units) == 24, "fc4_station_units layout");
static_assert(sizeof(fc4_dispatch_state) == 56, "fc4_dispatch_state layout");
static_assert(sizeof(fc4_dispatch_host) == (sizeof(void*) == 4 ? 164 : 328), "fc4_dispatch_host layout");
static_assert(sizeof(fc4_citysit_advice_info) == 40, "fc4_citysit_advice_info layout");
static_assert(sizeof(fc4_citysit_track_entry) == 24, "fc4_citysit_track_entry layout");
static_assert(sizeof(fc4_citysit_success_count) == 8, "fc4_citysit_success_count layout");
static_assert(sizeof(fc4_citysit_state) == 96, "fc4_citysit_state layout");
static_assert(sizeof(fc4_citysit_host) == (sizeof(void*) == 4 ? 324 : 648), "fc4_citysit_host layout");
static_assert(sizeof(fc4_disaster_post) == 32, "fc4_disaster_post layout");
static_assert(sizeof(fc4_disaster_message) == 48, "fc4_disaster_message layout");
static_assert(sizeof(fc4_world_box) == 16, "fc4_world_box layout");
static_assert(sizeof(fc4_disasters_host) == (sizeof(void*) == 4 ? 328 : 656), "fc4_disasters_host layout");
static_assert(sizeof(fc4_disasters_state) == 80, "fc4_disasters_state layout");
static_assert(sizeof(fc4_disaster_fire) == 56, "fc4_disaster_fire layout");
static_assert(sizeof(fc4_terrain_rect) == 16, "fc4_terrain_rect layout");
static_assert(sizeof(fc4_terrain_post) == 20, "fc4_terrain_post layout");
static_assert(sizeof(fc4_terrain_tuning) == 48, "fc4_terrain_tuning layout");
static_assert(sizeof(fc4_lava_tuning) == 32, "fc4_lava_tuning layout");
static_assert(sizeof(fc4_brush_params) == 56, "fc4_brush_params layout");
static_assert(sizeof(fc4_terrain_state) == 60, "fc4_terrain_state layout");
static_assert(sizeof(fc4_brush_component) == (sizeof(void*) == 4 ? 60 : 64), "fc4_brush_component layout");
static_assert(sizeof(fc4_terrain_host) == (sizeof(void*) == 4 ? 48 : 96), "fc4_terrain_host layout");
static_assert(sizeof(fc4_automata_time) == 8, "fc4_automata_time layout");
static_assert(sizeof(fc4_automata_create_args) == 56, "fc4_automata_create_args layout");
static_assert(sizeof(fc4_automata_ref_host) == (sizeof(void*) == 4 ? 16 : 32), "fc4_automata_ref_host layout");
static_assert(sizeof(fc4_automata_cm_host) == (sizeof(void*) == 4 ? 24 : 48), "fc4_automata_cm_host layout");
static_assert(sizeof(fc4_automata_riot_host) == (sizeof(void*) == 4 ? 56 : 112), "fc4_automata_riot_host layout");
static_assert(sizeof(fc4_automata_fire_host) == (sizeof(void*) == 4 ? 20 : 40), "fc4_automata_fire_host layout");
static_assert(sizeof(fc4_automata_controller_state) == 32, "fc4_automata_controller_state layout");
static_assert(sizeof(fc4_automata_automaton_state) == 32, "fc4_automata_automaton_state layout");
static_assert(sizeof(fc4_automata_cm_state) == 48, "fc4_automata_cm_state layout");
static_assert(sizeof(fc4_automata_manager_state) == 36, "fc4_automata_manager_state layout");
static_assert(sizeof(fc4_automata_riot_leader) == 32, "fc4_automata_riot_leader layout");
static_assert(sizeof(fc4_disaster_instance) == 24, "fc4_disaster_instance layout");
static_assert(sizeof(fc4_riot_leader) == 32, "fc4_riot_leader layout");
static_assert(sizeof(fc4_adv_advisor_entry) == (sizeof(void*) == 4 ? 32 : 48), "fc4_adv_advisor_entry layout");
static_assert(sizeof(fc4_adv_advice_entry) == (sizeof(void*) == 4 ? 64 : 80), "fc4_adv_advice_entry layout");
static_assert(sizeof(fc4_adv_lua_result) == (sizeof(void*) == 4 ? 24 : 32), "fc4_adv_lua_result layout");
static_assert(sizeof(fc4_adv_date) == 16, "fc4_adv_date layout");
static_assert(sizeof(fc4_adv_out_message) == 32, "fc4_adv_out_message layout");
static_assert(sizeof(fc4_adv_src) == 24, "fc4_adv_src layout");
static_assert(sizeof(fc4_adv_val) == 64, "fc4_adv_val layout");
static_assert(sizeof(fc4_adv_item) == 32, "fc4_adv_item layout");
static_assert(sizeof(fc4_adv_conditional_status) == (sizeof(void*) == 4 ? 16 : 24), "fc4_adv_conditional_status layout");
static_assert(sizeof(fc4_adv_subject_state) == 32, "fc4_adv_subject_state layout");
static_assert(sizeof(fc4_adv_message) == 40, "fc4_adv_message layout");
static_assert(sizeof(fc4_adv_lua_value) == (sizeof(void*) == 4 ? 32 : 40), "fc4_adv_lua_value layout");
static_assert(sizeof(fc4_advisors_state) == 56, "fc4_advisors_state layout");
static_assert(sizeof(fc4_advisor_state) == 88, "fc4_advisor_state layout");
static_assert(sizeof(fc4_advice_state) == 96, "fc4_advice_state layout");
static_assert(sizeof(fc4_advisor_host) == (sizeof(void*) == 4 ? 264 : 528), "fc4_advisor_host layout");
static_assert(sizeof(fc4_mysim_field) == (sizeof(void*) == 4 ? 40 : 48), "fc4_mysim_field layout");
static_assert(sizeof(fc4_mysim_disaster_location) == 12, "fc4_mysim_disaster_location layout");
static_assert(sizeof(fc4_mysim_building) == (sizeof(void*) == 4 ? 40 : 56), "fc4_mysim_building layout");
static_assert(sizeof(fc4_mysim_lot) == 40, "fc4_mysim_lot layout");
static_assert(sizeof(fc4_mysim_station) == 16, "fc4_mysim_station layout");
static_assert(sizeof(fc4_mysim_dispatch) == 56, "fc4_mysim_dispatch layout");
static_assert(sizeof(fc4_mysim_job_destination) == 24, "fc4_mysim_job_destination layout");
static_assert(sizeof(fc4_mysim_post) == 24, "fc4_mysim_post layout");
static_assert(sizeof(fc4_mysim_host) == (sizeof(void*) == 4 ? 196 : 392), "fc4_mysim_host layout");
static_assert(sizeof(fc4_mysims_agent_state) == 40, "fc4_mysims_agent_state layout");
static_assert(sizeof(fc4_mysim_state) == 240, "fc4_mysim_state layout");
static_assert(sizeof(fc4_tract_profile) == 24, "fc4_tract_profile layout");
static_assert(sizeof(fc4_tract_construction) == 32, "fc4_tract_construction layout");
static_assert(sizeof(fc4_tract_building_profile) == 12, "fc4_tract_building_profile layout");
static_assert(sizeof(fc4_tract_host) == (sizeof(void*) == 4 ? 368 : 736), "fc4_tract_host layout");
static_assert(sizeof(fc4_tract_grow_result) == 112, "fc4_tract_grow_result layout");
static_assert(sizeof(fc4_tract_state) == 288, "fc4_tract_state layout");
static_assert(sizeof(fc4_tract_tuning) == 120, "fc4_tract_tuning layout");
static_assert(sizeof(fc4_prop_manager_host) == (sizeof(void*) == 4 ? 188 : 376), "fc4_prop_manager_host layout");
static_assert(sizeof(fc4_prop_profile) == 28, "fc4_prop_profile layout");
static_assert(sizeof(fc4_prop_command_arg) == (sizeof(void*) == 4 ? 12 : 16), "fc4_prop_command_arg layout");
static_assert(sizeof(fc4_prop_manager_state) == 48, "fc4_prop_manager_state layout");
static_assert(sizeof(fc4_prop_catalogue_state) == 24, "fc4_prop_catalogue_state layout");
static_assert(sizeof(fc4_lot_config_host) == (sizeof(void*) == 4 ? 168 : 336), "fc4_lot_config_host layout");
static_assert(sizeof(fc4_lot_config_building_info) == (sizeof(void*) == 4 ? 16 : 24), "fc4_lot_config_building_info layout");
static_assert(sizeof(fc4_lot_config_state) == 40, "fc4_lot_config_state layout");
static_assert(sizeof(fc4_lot_config_catalogue_state) == 20, "fc4_lot_config_catalogue_state layout");
static_assert(sizeof(fc4_tutorial_host) == (sizeof(void*) == 4 ? 100 : 200), "fc4_tutorial_host layout");
static_assert(sizeof(fc4_tutorial_state) == 40, "fc4_tutorial_state layout");
static_assert(sizeof(fc4_lot_cfg_object) == (sizeof(void*) == 4 ? 20 : 24), "fc4_lot_cfg_object layout");
static_assert(sizeof(fc4_object_placement) == 16, "fc4_object_placement layout");
static_assert(sizeof(fc4_city_detail_state) == 40, "fc4_city_detail_state layout");
static_assert(sizeof(fc4_city_detail_host) == (sizeof(void*) == 4 ? 72 : 144), "fc4_city_detail_host layout");
static_assert(sizeof(fc4_port_lot) == 40, "fc4_port_lot layout");
static_assert(sizeof(fc4_port_state) == 64, "fc4_port_state layout");
static_assert(sizeof(fc4_port_host) == (sizeof(void*) == 4 ? 168 : 336), "fc4_port_host layout");
static_assert(sizeof(fc4_prop_ref) == 16, "fc4_prop_ref layout");
static_assert(sizeof(fc4_prop_message) == 32, "fc4_prop_message layout");
static_assert(sizeof(fc4_prop_state) == 104, "fc4_prop_state layout");
static_assert(sizeof(fc4_prop_block) == 52, "fc4_prop_block layout");
static_assert(sizeof(fc4_prop_host) == (sizeof(void*) == 4 ? 272 : 544), "fc4_prop_host layout");
static_assert(sizeof(fc4_occ_bbox) == 24, "fc4_occ_bbox layout");
static_assert(sizeof(fc4_occ_cell_box) == 4, "fc4_occ_cell_box layout");
static_assert(sizeof(fc4_occ_filter) == 16, "fc4_occ_filter layout");
static_assert(sizeof(fc4_occmgr_state) == 56, "fc4_occmgr_state layout");
static_assert(sizeof(fc4_occmgr_host) == (sizeof(void*) == 4 ? 68 : 136), "fc4_occmgr_host layout");
static_assert(sizeof(fc4_prop_occupant_host) == (sizeof(void*) == 4 ? 36 : 72), "fc4_prop_occupant_host layout");
static_assert(sizeof(fc4_prop_effect_host) == (sizeof(void*) == 4 ? 24 : 48), "fc4_prop_effect_host layout");
static_assert(sizeof(fc4_prop_slot_info) == 16, "fc4_prop_slot_info layout");

static_assert(sizeof(fc4_effects_post) == 24, "fc4_effects_post layout");
static_assert(sizeof(fc4_effects_rect) == 16, "fc4_effects_rect layout");
static_assert(sizeof(fc4_effects_demolish_filter) == 12, "fc4_effects_demolish_filter layout");
static_assert(sizeof(fc4_effects_occupant_facts) == 16, "fc4_effects_occupant_facts layout");
static_assert(sizeof(fc4_effects_directory_counts) == 24, "fc4_effects_directory_counts layout");
static_assert(sizeof(fc4_effects_state) == 80, "fc4_effects_state layout");
static_assert(sizeof(fc4_effect_info) == 48, "fc4_effect_info layout");
static_assert(sizeof(fc4_effects_brush_state) == 56, "fc4_effects_brush_state layout");
static_assert(sizeof(fc4_effects_game_state) == 32, "fc4_effects_game_state layout");
static_assert(sizeof(fc4_effects_host) == (sizeof(void*) == 4 ? 80 : 160), "fc4_effects_host layout");
#else
_Static_assert(sizeof(fc4_message) == 32, "fc4_message layout");
_Static_assert(sizeof(fc4_date) == 24, "fc4_date layout");
_Static_assert(sizeof(fc4_debug) == 76, "fc4_debug layout");
_Static_assert(sizeof(fc4_agent_info) == 16, "fc4_agent_info layout");
_Static_assert(sizeof(fc4_power_capture_ref) == 32, "fc4_power_capture_ref layout");
_Static_assert(sizeof(fc4_power_shadow_counts) == 24, "fc4_power_shadow_counts layout");
_Static_assert(sizeof(fc4_power_shadow_stats) == 72, "fc4_power_shadow_stats layout");
_Static_assert(sizeof(fc4_neighbor_deal_data) == 72, "fc4_neighbor_deal_data layout");
_Static_assert(sizeof(fc4_neighbors_shadow_city) == 168, "fc4_neighbors_shadow_city layout");
_Static_assert(sizeof(fc4_lotdev_cfg_object) == 48 + sizeof(void*), "fc4_lotdev_cfg_object layout");
_Static_assert(sizeof(fc4_lotdev_texture) == 20, "fc4_lotdev_texture layout");
_Static_assert(sizeof(fc4_lotdev_burnt) == 32, "fc4_lotdev_burnt layout");
_Static_assert(sizeof(fc4_lotdev_profile) == 12, "fc4_lotdev_profile layout");
_Static_assert(sizeof(fc4_lotdev_message) == 40, "fc4_lotdev_message layout");
_Static_assert(sizeof(fc4_lotdev_tuning) == 172, "fc4_lotdev_tuning layout");
_Static_assert(sizeof(fc4_lotdev_state) == 112, "fc4_lotdev_state layout");
_Static_assert(sizeof(fc4_lotdev_host) == (sizeof(void*) == 4 ? 8 + 178 * 4 : 16 + 178 * 8), "fc4_lotdev_host layout");
_Static_assert(sizeof(fc4_history_host) == (sizeof(void*) == 4 ? 36 : 72), "fc4_history_host layout");
_Static_assert(sizeof(fc4_history_point) == 8, "fc4_history_point layout");
_Static_assert(sizeof(fc4_history_state) == 28, "fc4_history_state layout");
_Static_assert(sizeof(fc4_clock24_host) == (sizeof(void*) == 4 ? 52 : 104), "fc4_clock24_host layout");
_Static_assert(sizeof(fc4_clock24_tuning) == 20, "fc4_clock24_tuning layout");
_Static_assert(sizeof(fc4_clock24_state) == 56, "fc4_clock24_state layout");
_Static_assert(sizeof(fc4_commercial_state) == 56, "fc4_commercial_state layout");
_Static_assert(sizeof(fc4_civic_state) == 40, "fc4_civic_state layout");
_Static_assert(sizeof(fc4_park_state) == 48, "fc4_park_state layout");
_Static_assert(sizeof(fc4_lot_cfg_object) == (sizeof(void*) == 4 ? 20 : 24), "fc4_lot_cfg_object layout");
_Static_assert(sizeof(fc4_object_placement) == 16, "fc4_object_placement layout");
_Static_assert(sizeof(fc4_city_detail_state) == 40, "fc4_city_detail_state layout");
_Static_assert(sizeof(fc4_city_detail_host) == (sizeof(void*) == 4 ? 72 : 144), "fc4_city_detail_host layout");
_Static_assert(sizeof(fc4_port_lot) == 40, "fc4_port_lot layout");
_Static_assert(sizeof(fc4_port_state) == 64, "fc4_port_state layout");
_Static_assert(sizeof(fc4_port_host) == (sizeof(void*) == 4 ? 168 : 336), "fc4_port_host layout");
_Static_assert(sizeof(fc4_prop_ref) == 16, "fc4_prop_ref layout");
_Static_assert(sizeof(fc4_prop_message) == 32, "fc4_prop_message layout");
_Static_assert(sizeof(fc4_prop_state) == 104, "fc4_prop_state layout");
_Static_assert(sizeof(fc4_prop_block) == 52, "fc4_prop_block layout");
_Static_assert(sizeof(fc4_prop_host) == (sizeof(void*) == 4 ? 272 : 544), "fc4_prop_host layout");
_Static_assert(sizeof(fc4_demolition_state) == 44, "fc4_demolition_state layout");
_Static_assert(sizeof(fc4_demolition_host) == (sizeof(void*) == 4 ? 344 : 688), "fc4_demolition_host layout");
_Static_assert(sizeof(fc4_dispatch_info) == 56, "fc4_dispatch_info layout");
_Static_assert(sizeof(fc4_station_units) == 24, "fc4_station_units layout");
_Static_assert(sizeof(fc4_dispatch_state) == 56, "fc4_dispatch_state layout");
_Static_assert(sizeof(fc4_dispatch_host) == (sizeof(void*) == 4 ? 164 : 328), "fc4_dispatch_host layout");
_Static_assert(sizeof(fc4_citysit_advice_info) == 40, "fc4_citysit_advice_info layout");
_Static_assert(sizeof(fc4_citysit_track_entry) == 24, "fc4_citysit_track_entry layout");
_Static_assert(sizeof(fc4_citysit_success_count) == 8, "fc4_citysit_success_count layout");
_Static_assert(sizeof(fc4_citysit_state) == 96, "fc4_citysit_state layout");
_Static_assert(sizeof(fc4_citysit_host) == (sizeof(void*) == 4 ? 324 : 648), "fc4_citysit_host layout");
_Static_assert(sizeof(fc4_disaster_post) == 32, "fc4_disaster_post layout");
_Static_assert(sizeof(fc4_disaster_message) == 48, "fc4_disaster_message layout");
_Static_assert(sizeof(fc4_disasters_host) == (sizeof(void*) == 4 ? 328 : 656), "fc4_disasters_host layout");
_Static_assert(sizeof(fc4_disasters_state) == 80, "fc4_disasters_state layout");
_Static_assert(sizeof(fc4_disaster_fire) == 56, "fc4_disaster_fire layout");
_Static_assert(sizeof(fc4_prop_manager_host) == (sizeof(void*) == 4 ? 188 : 376), "fc4_prop_manager_host layout");
_Static_assert(sizeof(fc4_prop_profile) == 28, "fc4_prop_profile layout");
_Static_assert(sizeof(fc4_prop_command_arg) == (sizeof(void*) == 4 ? 12 : 16), "fc4_prop_command_arg layout");
_Static_assert(sizeof(fc4_prop_manager_state) == 48, "fc4_prop_manager_state layout");
_Static_assert(sizeof(fc4_prop_catalogue_state) == 24, "fc4_prop_catalogue_state layout");
_Static_assert(sizeof(fc4_lot_config_host) == (sizeof(void*) == 4 ? 168 : 336), "fc4_lot_config_host layout");
_Static_assert(sizeof(fc4_lot_config_building_info) == (sizeof(void*) == 4 ? 16 : 24), "fc4_lot_config_building_info layout");
_Static_assert(sizeof(fc4_lot_config_state) == 40, "fc4_lot_config_state layout");
_Static_assert(sizeof(fc4_lot_config_catalogue_state) == 20, "fc4_lot_config_catalogue_state layout");
_Static_assert(sizeof(fc4_tutorial_host) == (sizeof(void*) == 4 ? 100 : 200), "fc4_tutorial_host layout");
_Static_assert(sizeof(fc4_tutorial_state) == 40, "fc4_tutorial_state layout");
_Static_assert(sizeof(fc4_occ_bbox) == 24, "fc4_occ_bbox layout");
_Static_assert(sizeof(fc4_occ_cell_box) == 4, "fc4_occ_cell_box layout");
_Static_assert(sizeof(fc4_occ_filter) == 16, "fc4_occ_filter layout");
_Static_assert(sizeof(fc4_occmgr_state) == 56, "fc4_occmgr_state layout");
_Static_assert(sizeof(fc4_occmgr_host) == (sizeof(void*) == 4 ? 68 : 136), "fc4_occmgr_host layout");
_Static_assert(sizeof(fc4_terrain_rect) == 16, "fc4_terrain_rect layout");
_Static_assert(sizeof(fc4_terrain_post) == 20, "fc4_terrain_post layout");
_Static_assert(sizeof(fc4_terrain_tuning) == 48, "fc4_terrain_tuning layout");
_Static_assert(sizeof(fc4_lava_tuning) == 32, "fc4_lava_tuning layout");
_Static_assert(sizeof(fc4_brush_params) == 56, "fc4_brush_params layout");
_Static_assert(sizeof(fc4_terrain_state) == 60, "fc4_terrain_state layout");
_Static_assert(sizeof(fc4_brush_component) == (sizeof(void*) == 4 ? 60 : 64), "fc4_brush_component layout");
_Static_assert(sizeof(fc4_terrain_host) == (sizeof(void*) == 4 ? 48 : 96), "fc4_terrain_host layout");
_Static_assert(sizeof(fc4_automata_time) == 8, "fc4_automata_time layout");
_Static_assert(sizeof(fc4_automata_create_args) == 44, "fc4_automata_create_args layout");
_Static_assert(sizeof(fc4_automata_ref_host) == (sizeof(void*) == 4 ? 16 : 32), "fc4_automata_ref_host layout");
_Static_assert(sizeof(fc4_automata_cm_host) == (sizeof(void*) == 4 ? 24 : 48), "fc4_automata_cm_host layout");
_Static_assert(sizeof(fc4_automata_riot_host) == (sizeof(void*) == 4 ? 56 : 112), "fc4_automata_riot_host layout");
_Static_assert(sizeof(fc4_automata_fire_host) == (sizeof(void*) == 4 ? 20 : 40), "fc4_automata_fire_host layout");
_Static_assert(sizeof(fc4_automata_controller_state) == 32, "fc4_automata_controller_state layout");
_Static_assert(sizeof(fc4_automata_automaton_state) == 32, "fc4_automata_automaton_state layout");
_Static_assert(sizeof(fc4_automata_cm_state) == 48, "fc4_automata_cm_state layout");
_Static_assert(sizeof(fc4_automata_manager_state) == 36, "fc4_automata_manager_state layout");
_Static_assert(sizeof(fc4_automata_riot_leader) == 32, "fc4_automata_riot_leader layout");
_Static_assert(sizeof(fc4_adv_advisor_entry) == (sizeof(void*) == 4 ? 32 : 48), "fc4_adv_advisor_entry layout");
_Static_assert(sizeof(fc4_adv_advice_entry) == (sizeof(void*) == 4 ? 64 : 80), "fc4_adv_advice_entry layout");
_Static_assert(sizeof(fc4_adv_lua_result) == (sizeof(void*) == 4 ? 24 : 32), "fc4_adv_lua_result layout");
_Static_assert(sizeof(fc4_adv_date) == 16, "fc4_adv_date layout");
_Static_assert(sizeof(fc4_adv_out_message) == 32, "fc4_adv_out_message layout");
_Static_assert(sizeof(fc4_adv_src) == 24, "fc4_adv_src layout");
_Static_assert(sizeof(fc4_adv_val) == 64, "fc4_adv_val layout");
_Static_assert(sizeof(fc4_adv_item) == 32, "fc4_adv_item layout");
_Static_assert(sizeof(fc4_adv_conditional_status) == (sizeof(void*) == 4 ? 16 : 24), "fc4_adv_conditional_status layout");
_Static_assert(sizeof(fc4_adv_subject_state) == 32, "fc4_adv_subject_state layout");
_Static_assert(sizeof(fc4_adv_message) == 40, "fc4_adv_message layout");
_Static_assert(sizeof(fc4_adv_lua_value) == (sizeof(void*) == 4 ? 32 : 40), "fc4_adv_lua_value layout");
_Static_assert(sizeof(fc4_advisors_state) == 56, "fc4_advisors_state layout");
_Static_assert(sizeof(fc4_advisor_state) == 88, "fc4_advisor_state layout");
_Static_assert(sizeof(fc4_advice_state) == 96, "fc4_advice_state layout");
_Static_assert(sizeof(fc4_advisor_host) == (sizeof(void*) == 4 ? 264 : 528), "fc4_advisor_host layout");
_Static_assert(sizeof(fc4_mysim_field) == (sizeof(void*) == 4 ? 40 : 48), "fc4_mysim_field layout");
_Static_assert(sizeof(fc4_mysim_disaster_location) == 12, "fc4_mysim_disaster_location layout");
_Static_assert(sizeof(fc4_mysim_building) == (sizeof(void*) == 4 ? 40 : 56), "fc4_mysim_building layout");
_Static_assert(sizeof(fc4_mysim_lot) == 40, "fc4_mysim_lot layout");
_Static_assert(sizeof(fc4_mysim_station) == 16, "fc4_mysim_station layout");
_Static_assert(sizeof(fc4_mysim_dispatch) == 56, "fc4_mysim_dispatch layout");
_Static_assert(sizeof(fc4_mysim_job_destination) == 24, "fc4_mysim_job_destination layout");
_Static_assert(sizeof(fc4_mysim_post) == 24, "fc4_mysim_post layout");
_Static_assert(sizeof(fc4_mysim_host) == (sizeof(void*) == 4 ? 196 : 392), "fc4_mysim_host layout");
_Static_assert(sizeof(fc4_mysims_agent_state) == 40, "fc4_mysims_agent_state layout");
_Static_assert(sizeof(fc4_mysim_state) == 240, "fc4_mysim_state layout");
_Static_assert(sizeof(fc4_prop_occupant_host) == (sizeof(void*) == 4 ? 36 : 72), "fc4_prop_occupant_host layout");
_Static_assert(sizeof(fc4_prop_effect_host) == (sizeof(void*) == 4 ? 24 : 48), "fc4_prop_effect_host layout");
_Static_assert(sizeof(fc4_prop_slot_info) == 16, "fc4_prop_slot_info layout");_Static_assert(sizeof(fc4_netgeom_params) == 76, "fc4_netgeom_params layout");
_Static_assert(sizeof(fc4_netgeom_type_info) == 44, "fc4_netgeom_type_info layout");
_Static_assert(sizeof(fc4_netgeom_segments) == 24, "fc4_netgeom_segments layout");
_Static_assert(sizeof(fc4_netgeom_cell) == 8, "fc4_netgeom_cell layout");
_Static_assert(sizeof(fc4_netgeom_step) == 12, "fc4_netgeom_step layout");
_Static_assert(sizeof(fc4_netgeom_span) == 12, "fc4_netgeom_span layout");
_Static_assert(sizeof(fc4_netgeom_vertex_height) == 8, "fc4_netgeom_vertex_height layout");
_Static_assert(sizeof(fc4_netgeom_occupant) == 12, "fc4_netgeom_occupant layout");
_Static_assert(sizeof(fc4_netgeom_piece) == 76, "fc4_netgeom_piece layout");
_Static_assert(sizeof(fc4_netgeom_cell_info) == 180, "fc4_netgeom_cell_info layout");
_Static_assert(sizeof(fc4_netgeom_state) == 96, "fc4_netgeom_state layout");
_Static_assert(sizeof(fc4_netgeom_host) == (sizeof(void*) == 4 ? 84 : 168), "fc4_netgeom_host layout");#endif
_Static_assert(sizeof(fc4_effects_post) == 24, "fc4_effects_post layout");
_Static_assert(sizeof(fc4_effects_rect) == 16, "fc4_effects_rect layout");
_Static_assert(sizeof(fc4_effects_demolish_filter) == 12, "fc4_effects_demolish_filter layout");
_Static_assert(sizeof(fc4_effects_occupant_facts) == 16, "fc4_effects_occupant_facts layout");
_Static_assert(sizeof(fc4_effects_directory_counts) == 24, "fc4_effects_directory_counts layout");
_Static_assert(sizeof(fc4_effects_state) == 80, "fc4_effects_state layout");
_Static_assert(sizeof(fc4_effect_info) == 48, "fc4_effect_info layout");
_Static_assert(sizeof(fc4_effects_brush_state) == 56, "fc4_effects_brush_state layout");
_Static_assert(sizeof(fc4_effects_game_state) == 32, "fc4_effects_game_state layout");
_Static_assert(sizeof(fc4_effects_host) == (sizeof(void*) == 4 ? 80 : 160), "fc4_effects_host layout");
#endif

#endif /* FC4_H */
