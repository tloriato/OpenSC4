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

// Runtime configuration for in-game bisection (tooling only; no spec
// behaviour).
//
// FreeCity4.ini next to FreeCity4.dll is a plain `key = value` file (`#` or
// `;` starts a comment, `[sections]` are ignored, keys are case-insensitive).
// A key in the ini wins over the matching environment variable.
//
//   disable = budget, simulator   subsystems left to the game (as FC4_DISABLE);
//                                 read once, when the game enumerates classes
//   enable = landvalue            default-off subsystems to use (as FC4_ENABLE);
//                                 `disable` wins; read once, like `disable`
//   <switch> = 0 / 1              one budget behaviour that touches game
//                                 objects (below); re-read at every city load
//   budget_default = 0 / 1        value of every switch the ini/env leave unset
//   refaudit = 0 / 1              reference-count audit (default 1)
//   autotest_seconds = N          unattended run: verdict line and process
//                                 exit N s after the first tick of a loaded
//                                 city (FC4Autotest.h); 0 = off
//   autotest_unpause = 0 / 1 / 2  autotest: run the loaded city at speed 1 or 2
//   digest = 0 / 1                monthly DIGEST line (default 1; FC4Observer.h)
//   autoload_city = <path.sc4>    load this save once the region view is up
//                                 (FC4Autoload.h); tooling for unattended runs
//   autoload_delay_ticks = N      framework ticks to wait once the region is up
//                                 (default 60)
//   budget_shadow = 0 / 1         the budget shadow differential (default 0;
//                                 needs `disable = budget`; FC4BudgetShadow.h)
//   budget_shadow_lots = N        its lot lines per mismatched tax type (default 6)
//   demand_shadow = 0 / 1         the demand shadow differential (default 0;
//                                 needs the original demand; FC4DemandShadow.h)
//   demand_shadow_nodes = N       its node lines per mismatched day (default 8)
//   crime_shadow = 0 / 1          the crime grid shadow (default 0; needs the
//                                 original crime simulator; FC4CrimeShadow.h)
//   crime_shadow_cells = N        its differing cells listed per month (default 8)
//   session_seed = N              the process's session seed S (default: the cRZTime
//                                 wall clock at game start; env FC4_SESSION_SEED;
//                                 foundations §6.3, FC4Seed.h); a subsystem's own
//                                 <key>_seed below overrides it for that subsystem
//   crime_seed = N                the crime generator's session seed (default:
//                                 the cRZTime clock, crime.md §9; cFC4Crime.cpp)
//   popup_dialogs = 0 / 1         unattended runs: 0 = cISC4App::SetPopupDialogsEnabled(false)
//                                 once per process (advisor popups dropped without
//                                 the hidden pause, generic dialogs answer without
//                                 asking), 1 = (true); unset = leave the game alone;
//                                 env FC4_POPUP_DIALOGS; read at game start, called at
//                                 PostAppInit (FC4Popups.h)
//   pollution_shadow = 0 / 1      the pollution grid shadow (default 0; needs the
//                                 original pollution simulator; FC4PollutionShadow.h)
//   pollution_shadow_cells = N    its differing cells listed per grid (default 4)
//   protection_shadow = 0 / 1     the fire / police grid shadow (default 0; needs
//                                 the ORIGINAL protection simulators: do not enable
//                                 protection; FC4ProtectionShadow.h)
//   aura_shadow = 0 / 1           the aura grid shadow (default 0; needs the ORIGINAL
//                                 aura simulator: do not enable aura; FC4AuraShadow.h)
//   aura_shadow_cells = N         its differing tracts listed per grid (default 4)
//   flora_shadow = 0 / 1          the flora census shadow (default 0; needs the
//                                 original flora simulator; FC4FloraShadow.h)
//   landfill_shadow = 0 / 1       the landfill shadow (default 0; needs the ORIGINAL
//                                 landfill developer: do not enable landfill;
//                                 FC4LandfillShadow.h)
//   landfill_shadow_diffs = N     its differences listed per month (default 8)
//   power_shadow = 0 / 1          the power shadow (default 0; needs FreeCity4's
//                                 simulator and the ORIGINAL power simulator: do not
//                                 enable power; FC4PowerShadow.h)
//   power_shadow_cells = N        its differing cells listed per line (default 8)
//   power_shadow_lines = N        its cycle lines with a difference per month
//                                 (default 32; -1 = every cycle)
//   landvalue_shadow = 0 / 1      the land value grid shadow (default 0; needs the
//                                 ORIGINAL land value simulator: do not enable
//                                 landvalue; FC4LandValueShadow.h)
//   landvalue_shadow_cells = N    its differing tracts listed per grid (default 4)
//   flammability_shadow = 0 / 1   the flammability grid shadow (default 0; needs the
//                                 ORIGINAL flammability simulator: do not enable
//                                 flammability; FC4FlammabilityShadow.h)
//   flammability_shadow_cells = N its differing tracts listed per grid (default 4)
//   residential_shadow = 0 / 1    the residential shadow (default 0; needs FreeCity4's
//                                 simulator and the ORIGINAL residential simulator: do
//                                 not enable residential; FC4ResidentialShadow.h)
//   residential_shadow_cells = N  its differing cells listed per grid (default 4)
//   residential_seed = N          the residential generator's session seed (default:
//                                 the cRZTime clock, residential.md §3; cFC4Residential.cpp)
//   zones_shadow = 0 / 1          the zone manager shadow (default 0; needs the ORIGINAL
//                                 zone manager: do not enable zones; FC4ZonesShadow.h)
//   zonedev_shadow = 0 / 1        the zone developer / lot manager shadow (default 0; needs
//                                 the ORIGINAL zone developer: do not enable zonedev;
//                                 FC4ZoneDevShadow.h)
//   zonedev_seed = N              the zone developer generator's session seed (default:
//                                 the cRZTime clock; cFC4ZoneDeveloper.cpp)
//   neighbours_shadow = 0 / 1     the neighbours shadow (default 0; needs FreeCity4's
//                                 simulator and the ORIGINAL neighbours simulator: do
//                                 not enable neighbors; FC4NeighborsShadow.h)
//   neighbors_seed = N            the deal proposal generator's session seed (default:
//                                 the cRZTime clock; cFC4Neighbors.cpp)
//   neighbors_two_curves = 0 / 1  neighbors.md §8.3 two price curves [choice] (default 0)
//   development_growth_attempts_per_day = N  the BDS's daily growth budget
//                                 (building-development.md §4.4.1; default 0 = unlimited;
//                                 cFC4Development.cpp)
//   development_seed = N          the development generators' session seed (default: the
//                                 wall clock, building-development.md §4.14; env
//                                 FC4_DEVELOPMENT_SEED)
//   development_shadow = 0 / 1    the development shadow (default 0; needs FreeCity4's
//                                 simulator and the ORIGINAL BDS and growth developers: do
//                                 not enable development; FC4DevelopmentShadow.h)
//   tract_shadow = 0 / 1          the tract developer shadow (default 0; FC4TractShadow.h).
//                                 Without `enable = tract` it is the
//                                 original-vs-core differential; with it, the
//                                 replacement's self-check (glue27)
//   history_shadow = 0 / 1        the history warehouse shadow (default 0; needs the
//                                 ORIGINAL warehouse: do not enable history;
//                                 FC4HistoryShadow.h)
//   clock24_shadow = 0 / 1        the 24-hour clock shadow (default 0; needs the
//                                 ORIGINAL clock: do not enable clock24;
//                                 FC4Clock24Shadow.h)
//   netmgr_shadow = 0 / 1         the network manager / network lot manager shadow
//                                 (default 0; needs the ORIGINAL managers;
//                                 FC4NetMgrShadow.h)
//   netmgr_shadow_order = late / early  where the shadow's network lot core hears
//                                 its messages: after the original (late, default) or
//                                 before every city listener (early)
//   tract_seed = N                the tract developer generator's session seed
//                                 (default: the cRZTime clock; cFC4TractDeveloper.cpp)
//   tract_grow_log = 0 / 1        one line per Grow call (default 0; enable = tract)
//   lotdev_shadow = 0 / 1         the lot developer shadow (default 0; needs the
//                                 ORIGINAL lot developer: do not enable lotdev;
//                                 FC4LotDevShadow.h)
//   lotdev_seed = N               the lot developer generator's session seed
//                                 (default: the cRZTime clock; cFC4LotDeveloper.cpp)
//   lotdev_lot_log = 0 / 1        one line per lot change of the daily pass (SetState,
//                                 SetCondition, zot; default 0; enable = lotdev)
//   commercial_shadow = 0 / 1     the commercial shadow (default 0; needs FreeCity4's
//                                 simulator and the ORIGINAL commercial simulator: do
//                                 not enable commercial; FC4CommercialShadow.h)
//   commercial_shadow_cells = N   its differing cells listed per grid (default 4)
//   civic_shadow = 0 / 1          the civic building shadow (default 0; needs the
//                                 ORIGINAL civic simulator: do not enable civic;
//                                 FC4CivicShadow.h)
//   civic_seed = N                the civic temporaries' session seed (default: the
//                                 cRZTime clock; cFC4Civic.cpp)
//   parks_shadow = 0 / 1          the park manager shadow (default 0; needs the ORIGINAL
//                                 park manager: do not enable parks; FC4ParksShadow.h)
//   disasters_shadow = 0 / 1     the disaster layer shadow (default 0; needs the ORIGINAL
//                                 disaster layer: do not enable disasters; fire and riot;
//                                 FC4DisasterShadow.h)
//   citysit_shadow = 0 / 1        the city situation manager shadow (default 0; needs the
//                                 ORIGINAL manager: do not enable citysit; FC4CitySitShadow.h)
//   demolition_shadow = 0 / 1     the demolition utility shadow (default 0; needs the ORIGINAL
//                                 utility: do not enable demolition; FC4DemolitionShadow.h)
//   demolition_price_shadow = 0 / 1  the demolition PRICING differential: our DemolishOccupant
//                                 and DemolishRegion prices against the original's, side by
//                                 side (default 0; needs the ORIGINAL utility: do not enable
//                                 demolition; FC4DemolitionPriceShadow.h)
//   dispatch_shadow = 0 / 1       the dispatch manager shadow (default 0; needs the ORIGINAL
//                                 manager: do not enable dispatch; FC4DispatchShadow.h)
//   parks_seed = N                the park member generator's session seed (default:
//                                 the cRZTime clock; cFC4Parks.cpp)
//   growth_probe = 0 / 1          the growth calibration probe (default 0; needs the
//                                 ORIGINAL BDS and tract developer; counts the tract
//                                 developer's Grow calls per game day; env
//                                 FC4_GROWTH_PROBE; read at PostAppInit;
//                                 FC4GrowthProbe.h)
//   traffic_units_per_slice = N   the traffic work cycle's units per slice
//                                 (traffic-simulator.md §3.4; default 0 = the core's
//                                 default; env FC4_TRAFFIC_UNITS_PER_SLICE;
//                                 cFC4TrafficSimulator.cpp)
//   traffic_seed = N              the traffic generator's session seed (default: the
//                                 cRZTime clock; env FC4_TRAFFIC_SEED)
//   traffic_shadow = 0 / 1        the traffic shadow (default 0; needs FreeCity4's
//                                 simulator; FC4TrafficShadow.h). Without
//                                 `enable = traffic` it is the original-vs-core
//                                 differential; with it, the replacement's
//                                 self-check (glue27)
//   traffic_shadow_diffs = N      its differences named per line (default 8, 1..64)
//   traffic_shadow_lines = N      its delivery lines per month (default 8)
//   traffic_shadow_hypotheses = N its alternative readings of unclear spec points
//                                 (a bit mask, default 0 = the spec as written; fc4.h
//                                 FC4_TRAFFIC_SHADOW_HYP_*)
//   pacing = deterministic | original
//                                 simulator.md §5.7 [choice] (env FC4_PACING; default
//                                 deterministic, or original when a *_shadow switch is on
//                                 and `pacing` is unset): idle cycles of our cores complete
//                                 within their day, item counts instead of ms budgets,
//                                 posted day/month/StateChanged messages at the day's
//                                 settle point; `original` keeps the clock-bound pacing
//                                 (conformance shadows). Read once at game start;
//                                 FC4Pacing.h
//   occmgr_shadow = 0 / 1         the occupant manager shadow (default 0; follows the
//                                 ORIGINAL occupant manager, which FreeCity4 never
//                                 replaces; FC4OccMgrShadow.h)
//   occmgr_shadow_samples = N     its sampled calls per query slot and month (default 4,
//                                 1..64)
//   occmgr_shadow_probe = 0 / 1   its thread probe on the original's vtable (default 1;
//                                 calls per thread, OM1 / OM3)//   terrain_shadow = 0 / 1        the terrain shadow (default 0; compares FreeCity4's
//   traffic_shadow_hypotheses = N retired (ABI 69): accepted and ignored, logged once;
//                                 its readings (TS55-TS57) are the core's behaviour
//   terrain_shadow = 0 / 1        the terrain shadow (default 0; compares FreeCity4's//                                 terrain core with the ORIGINAL terrain, which is
//                                 never replaced; FC4TerrainShadow.h)
//   terrain_shadow_points = N     its sample points per month (default 256, 16..4096)
//   terrain_shadow_diffs = N      its differences named per line (default 8, 1..64)
//   terrain_shadow_lines = N      its change lines per month (default 8, 0..1000)
//
// A switch that is off turns its behaviour into a safe no-op that returns a
// neutral value; FreeCity4.log lists the switches that are off at each load.
#pragma once

#include <string>

namespace fc4 {

enum class Sw : unsigned {
    LotScan,         // lot_scan: ForEachLot (lot list, GetBuilding, profile, building value)
    PropertyWrites,  // property_writes: AddPropertyVariant / property copies
    PropertyReads,   // property_reads: GetProperty / HasProperty on holders
    MessagePosting,  // message_posting: budget messages posted to the message server
    History,         // history: history warehouse records and queue lengths
    BuildingName,    // building_name: GetBuildingName (line item names)
    ExemplarLoads,   // exemplar_loads: tuning exemplar from the resource manager
    Disaster,        // disaster: AttemptDisasterStart (RiskyMoney)
    Cheats,          // cheats: the budget's cheat manager registration
    HolderRefs,      // holder_refs: references the budget core holds on game objects
    Ltext,           // ltext: localized string lookups (LTEXT resources)
    // Sub-switches of lot_scan (ForEachLot), for bisecting it; lot_scan = 0
    // still skips the whole scan.
    LotList,           // lot_list: 1 = GetLotsInCellRect list (one reference per lot,
                       //   released by us, nodes freed by our SC4List); 0 = walk every
                       //   cell with GetLot(x, z, true) (borrowed; no list, no releases)
    LotBuilding,       // lot_building: lot->GetBuilding() (off: no building)
    LotProfile,        // lot_profile: building->GetBuildingProfile() purpose / wealth
    LotBuildingValue,  // lot_building_value: QI cISCPropertyHolder + property 0xE91A0B5F
    LotLandValue,      // lot_land_value: GetLocation + GetLandValue
    LotSize,           // lot_size: GetSize (off: 1 x 1)
    LotState,          // lot_state: IsInLivableState + GetOccupantWealth (off: livable, wealth 0)
    LotRelease,        // lot_release: Release the list's lots (off: leak one reference per
                       //   lot per scan; the entries are still nulled)
    LotListFree,       // lot_list_free: destroy the SC4List (off: the list is heap-allocated
                       //   and leaked, head and nodes included)
    // Power simulator host callbacks (cFC4Power.cpp), one per group of game
    // calls; only used when the power subsystem is enabled.
    PowerOccupants,   // power_occupants: occupant type, properties, position, bounding box/cells,
                      //   building type, network occupants (off: occupants look empty)
    PowerPoles,       // power_poles: pole and line QIs, GetCell, IsPreview, BreakAllConnections,
                      //   Is/SetDistressed (off: not a pole / line, nothing done)
    PowerLots,        // power_lots: lot manager and lot calls, SetHasPower (off: no lot)
    PowerNeighbors,   // power_neighbors: deals and neighbour connections (off: none)
    PowerBudget,      // power_budget: department, line items, budget items (off: no department)
    PowerGridWrites,  // power_grid_writes: SetTractValue / SetTractValues on the powered grid
    PowerPosts,       // power_posts: messages the power simulator posts
    PowerTerrain,     // power_terrain: RemoveOccupant of poles on terrain changes
    PowerZoneTool,    // power_zone_tool: the power-line tool's UpdateOnZoneChange
    PowerView,        // power_view: the 3D view's current input control (Nuclear Free Zone)
    PowerRadiation,   // power_radiation: AddRadiationToCity (nuclear meltdown)
    PowerOccupantQuery,  // power_occupant_query: GetOccupantsByStandardCityCells (wire rebuild,
                         //   terrain changes; off: no occupants)
    PowerDemolish,       // power_demolish: DemolishLots (Nuclear Free Zone, plant failure)
    PowerDirectCalls,    // power_direct_calls: the pole's non-virtual RemoveConnection
                         //   (docs/abi/direct-calls.md; identified executable only)
    // Plumbing simulator host callbacks (cFC4Plumbing.cpp), one per group of
    // game calls; only used when the plumbing subsystem is enabled.
    PlumbingOccupants,   // plumbing_occupants: occupant type, properties, position, bounding box/cells,
                         //   building type, buildings in a cell (off: occupants look empty)
    PlumbingPipes,       // plumbing_pipes: network-occupant QI, flags, ConnectsToNeighbor, GetOccupiedCell
                         //   (off: not a pipe, nothing done)
    PlumbingLots,        // plumbing_lots: lot manager and lot calls, SetCondition (off: no lot)
    PlumbingTerrain,     // plumbing_terrain: terrain cell counts, CellHasWater, RedisplayTerrain
    PlumbingNeighbors,   // plumbing_neighbors: deals and neighbour connections (off: none)
    PlumbingBudget,      // plumbing_budget: department, line items, budget items (off: no department)
    PlumbingGridWrites,  // plumbing_grid_writes: SetTractValue / SetTractValues on the two water grids
    PlumbingPosts,       // plumbing_posts: messages the plumbing simulator posts
    PlumbingPollution,   // plumbing_pollution: GetWaterValue reads of the pollution simulator
    PlumbingOrdinance,   // plumbing_ordinance: GetCumulativeFactor reads of the ordinance simulator
    PlumbingDisaster,    // plumbing_disaster: AttemptDisasterStart (pipe burst)
    PlumbingHistory,     // plumbing_history: RecordHistoryData
    PlumbingDirectCalls, // plumbing_direct_calls: the pipe's non-virtual SetWateredState
                         //   (docs/abi/direct-calls.md; identified executable only)
    // Flammability simulator host callbacks (cFC4Flammability.cpp), one per
    // group of game calls; only used when the flammability subsystem is enabled.
    FlammabilityOccupants,  // flammability_occupants: occupant type, properties, bounds, position,
                            //   the occupant query (off: occupants look empty, no raw value)
    FlammabilityLots,       // flammability_lots: lot manager and lot calls, GetRandomLot (off: no lot)
    FlammabilityWater,      // flammability_water: IsCellWatered reads of the plumbing simulator
                            //   (off: read as "no plumbing simulator", so the no-water multiplier applies)
    FlammabilityOrdinance,  // flammability_ordinance: GetCumulativeFactor reads of the ordinance simulator
    FlammabilitySeason,     // flammability_season: GetCurrentSeason reads of the weather simulator
    FlammabilityZoning,     // flammability_zoning: GetZoneType reads of the zone manager (off: unzoned)
    FlammabilityFire,       // flammability_fire: IgniteCell / SetCause of the fire disaster manager
                            //   (off: no fire is ever started)
    FlammabilityGridWrites, // flammability_grid_writes: SetTractValue / SetTractValues on the two grids
    FlammabilityPosts,      // flammability_posts: the cycle-complete message
    FlammabilityHistory,    // flammability_history: RecordHistoryData of the average
    // Pollution simulator host callbacks (cFC4Pollution.cpp), one per group of
    // game calls; only used when the pollution subsystem is enabled.
    PollutionOccupants,   // pollution_occupants: occupant property reads / writes / removals, position,
                          //   bounding box (off: occupants look empty, nothing written)
    PollutionLots,        // pollution_lots: GetLot and the lot's SetCondition (off: nothing done)
    PollutionNeighbors,   // pollution_neighbors: garbage deals, SetAmountUsed, MarkDealsDirty (off: no deals)
    PollutionBudget,      // pollution_budget: department, line items, budget items (off: no department)
    PollutionGridWrites,  // pollution_grid_writes: SetTractValue / SetTractValues on the eight grids
    PollutionPosts,       // pollution_posts: messages the pollution simulator posts
    PollutionHistory,     // pollution_history: RecordHistoryData / SetHistoryDataQueueLength
    PollutionOrdinance,   // pollution_ordinance: GetCumulativeFactor reads (off: 1.0)
    PollutionTraffic,     // pollution_traffic: the traffic simulator's air-polluting traffic map (off: 0)
    PollutionZoning,      // pollution_zoning: GetZoneType reads of the zone manager (off: unzoned)
    PollutionPower,       // pollution_power: IsOccupantPowered reads of the power simulator (off: unpowered)
    PollutionLandfill,    // pollution_landfill: the landfill developer's usage and tile count (off: 0)
    PollutionPopulation,  // pollution_population: the residential simulator's GetPopulation (off: 0)
    PollutionProps,       // pollution_props: Add / RemovePropRequester on the prop developer
    PollutionTerrain,     // pollution_terrain: terrain altitude reads (off: 0)
    // Fire and police protection host callbacks (cFC4Protection.cpp), one per
    // group of game calls; only used when the protection subsystem is enabled.
    ProtectionOccupants,   // protection_occupants: occupant type, properties (reads and writes),
                           //   position, the prop QI, lots (road access, the crime message's lot)
                           //   (off: occupants look empty, no lot)
    ProtectionBudget,      // protection_budget: departments, budget items, locally funded objects
                           //   (off: no department, no budget item)
    ProtectionDispatch,    // protection_dispatch: the dispatch manager (register / unregister the
                           //   dispatch source, dispatch counts, IDs and records) (off: no dispatches)
    ProtectionAutomata,    // protection_automata: Add / RemoveControllersFromOccupantGroup
    ProtectionGridWrites,  // protection_grid_writes: SetTractValue / SetTractValues on the two grids
    ProtectionPosts,       // protection_posts: the messages the protection simulators post
    ProtectionHistory,     // protection_history: RecordHistoryData
    ProtectionAirSources,  // protection_air_sources: create the fire plane / police helicopter
                           //   dispatch source through GZCOM and call its non-virtual Init /
                           //   Shutdown (docs/abi/direct-calls.md; identified executable only)
    // Crime simulator host callbacks (cFC4Crime.cpp), one per group of game
    // calls; only used when the crime subsystem is enabled.
    CrimeLots,         // crime_lots: lot manager and lot calls (GetRandomLot, GetLot, bounding rect,
                       //   location, condition, building, GetCenter) (off: no lot, so no crime)
    CrimePolice,       // crime_police: police grid, GetJailPopulation, ResetUnjailedCriminals,
                       //   ProcessMonthlyCrimeStats (off: no coverage, 0 jailed, 0 released)
    CrimeResidential,  // crime_residential: GetPopulationGrids / GetEQGrids (off: empty census)
    CrimeDemand,       // crime_demand: GetDemand active value / max / min (off: absent)
    CrimeOrdinance,    // crime_ordinance: GetCumulativeFactor (off: the factor stays as it is)
    CrimeOccupants,    // crime_occupants: occupant properties and position, the nearest-occupant box
                       //   query (off: no crime effect, no nearest crime sim / prop)
    CrimeFire,         // crime_fire: disaster layer, IgniteOccupant / SetCause (off: arson starts no fire)
    CrimeProps,        // crime_props: AddPropRequester, GetPropProfile (off: not registered; every prop generic)
    CrimePosts,        // crime_posts: the crime-committed and crime-map-updated messages
    CrimeGridWrites,   // crime_grid_writes: SetTractValue / SetTractValues on the two crime grids
    CrimeZoning,       // crime_zoning: GetZoneType (off: unzoned, so no crime type matches a zone mask)
    // Demand simulator (cFC4Demand.cpp); only used when the demand subsystem is enabled.
    DemandCapacity,        // demand_capacity: read the building profile's capacity maps (§7.2;
                           //   plain reads, any executable) (off: no capacity, as before)
    DemandDirectCalls,     // demand_direct_calls: fill GetLocalPopulationSummary's map with the
                           //   game's map functions (docs/abi/direct-calls.md; identified executable
                           //   only) (off: the caller's map is left untouched)
    // Aura simulator host callbacks (cFC4Aura.cpp), one per group of game
    // calls; only used when the aura subsystem is enabled.
    AuraLots,         // aura_lots: lot count, the lot walk (GetLot per cell), bounding rect, zone,
                      //   wealth, building (off: no lots; the cycle ends at state 4)
    AuraPopulations,  // aura_lot_populations: GetPopulation(type) (lot slot 53) for the twelve
                      //   developer types (off: no pairs, every lot -128)
    AuraOccupants,    // aura_occupants: occupant HasProperty / properties / type / position
                      //   (off: occupants look empty)
    AuraBudget,       // aura_budget: GetBudgetItemForPurpose, department funding, tax rates
                      //   (off: no department, funding missing, tax rate 0)
    AuraOrdinance,    // aura_ordinance: GetCumulativeOffset (off: 0)
    AuraLandValue,    // aura_land_value: GetLandValueType (off: 0)
    AuraInputs,       // aura_inputs: the other simulators' grids (pollution, grade maps, crime,
                      //   traffic, population grids) (off: none, the stages are skipped)
    AuraGridWrites,   // aura_grid_writes: SetTractValue / SetTractValues on the aura's grids
    AuraPosts,        // aura_posts: the cycle-completed message
    AuraHistory,      // aura_history: RecordHistoryData of the mayor rating
    AuraDisaster,     // aura_disaster: the riot manager's AttemptDisasterStart
    AuraStrikes,      // aura_strikes: police / fire EndStrike, residential End*Strike (BustStrike)
    // Flora simulator host callbacks (cFC4Flora.cpp), one per group of game
    // calls; only used when the flora subsystem is enabled.
    FloraOccupants,  // flora_occupants: occupant type, property, position, box, flag 1, network flags,
                     //   flora values and setters, change posts (off: occupants look empty, nothing set)
    FloraCreate,     // flora_create: new flora occupants through GZCOM class 0xA9C05C85 and their Init
                     //   (off: none created, so nothing is planted)
    FloraManager,    // flora_manager: InsertOccupant / RemoveOccupant / MoveOccupant (off: refused, nothing done)
    FloraQueries,    // flora_queries: the occupant manager's box and cell queries, and the month line's
                     //   census (off: no occupants found)
    FloraTerrain,    // flora_terrain: terrain altitude, sea level, face normal, initialized, and the lot
                     //   manager's GetLotSavvyTerrainAltitude (off: 0, flat, not initialized)
    FloraWeather,    // flora_weather: the weather simulator's GetMoistureType (off: moisture 0)
    FloraRecords,    // flora_records: the game-layout record blocks (docs/abi/layouts.md) and
                     //   SetStaticFloraData (off: no blocks, GetStaticFloraData returns null)
    FloraPosts,      // flora_posts: FloraTypesUpdated (off: not posted)
    FloraCheats,     // flora_cheats: the "Flora" cheat and the PlaceFlora command registrations
    FloraExemplars,  // flora_exemplars: the flora exemplar enumeration (off: no flora types)
    FloraObliteration,  // flora_obliteration: the obliterated city's segment records (cISC4DBSegment and its
                        //   streams, the occupants' Write / Read; off: nothing saved, nothing restored)
    // Landfill developer host callbacks (cFC4Landfill.cpp), one per group of
    // game calls; only used when the landfill subsystem is enabled.
    LandfillTerrain,     // landfill_terrain: terrain altitude and sea level reads, SetAltitude,
                         //   RedisplayTerrain (off: 0, nothing redrawn)
    LandfillLevelling,   // landfill_levelling: the lot developer's AttemptAreaLevelling (off: not levelled)
    LandfillPollution,   // landfill_pollution: the garbage supply and capacities, AddToGarbageSupply,
                         //   SetGarbageCapacity, the active-landfill flag, GarbagePutInLandfill (off: 0)
    LandfillBudget,      // landfill_budget: the sanitation department, its funding and line item 30
                         //   (off: no department)
    LandfillZoning,      // landfill_zoning: GetZoneCount, GetZoneType, PlaceZone (off: unzoned, refused)
    LandfillOccupants,   // landfill_occupants: occupant lookups, removal, position, flags, messages,
                         //   item costs, flora and base-texture removal (off: no occupants)
    LandfillProps,       // landfill_props: AddLotProp / RemovePropA, the zot (off: no zot)
    LandfillTraffic,     // landfill_traffic: FindNetworkTile, the traffic simulator's subnetworks
                         //   (off: no road tile, no connection)
    LandfillHistory,     // landfill_history: the history record messages (MessageSend)
    LandfillEffects,     // landfill_effects: the zone effects (§4.10, rendering only; not implemented,
                         //   logged once either way)
    // Residential simulator host callbacks (cFC4Residential.cpp), one per
    // group of game calls; only used when the residential subsystem is enabled.
    ResidentialOccupants,  // residential_occupants: the building QueryInterface, occupant groups, positions,
                           //   building type / slot 11 and the per-building properties (off: not a building)
    ResidentialBudget,     // residential_budget: the four departments, budget items, local funding and line
                           //   items (off: no department, no item)
    ResidentialDevelopers,  // residential_developers: the BDS building map and keys, the growth developers'
                            //   population maps (off: no map, population 0)
    ResidentialPollution,  // residential_pollution: GetPollutionValue (off: 0)
    ResidentialOrdinance,  // residential_ordinance: the ordinances on and their properties (off: no effects)
    ResidentialAutomata,   // residential_automata: Add / RemoveControllersFromOccupantGroup (off: not changed)
    ResidentialGridWrites,  // residential_grid_writes: SetTractValue / fill on the residential grids
    ResidentialPosts,      // residential_posts: the strike start / end messages (MessagePost; off: not posted)
    ResidentialHistory,    // residential_history: history records (MessageSend) and queue lengths
    ResidentialLots,       // residential_lots: GetOccupantLot and its centre (off: the building's position)
    ResidentialTerrain,    // residential_terrain: GetAltitude (off: 0)
    ResidentialEffects,    // residential_effects: the buzzards through the effects manager (REH §4.7 step 3,
                           //   RG1; off: not shown, counted as created)
    // Zone manager host callbacks (cFC4ZoneManager.cpp), one per group of game
    // calls; only used when the zones subsystem is enabled.
    ZonesGridWrites,      // zones_grid_writes: SetTractValue / fill on the zone grid (off: not written)
    ZonesBudget,          // zones_budget: GetTotalFunds / WithdrawFunds (off: unlimited funds, nothing charged)
    ZonesLots,            // zones_lots: the lot manager's GetLot / GetLotsInCellRect and the lot reads (off: no lots)
    ZonesOutLots,         // zones_out_lots: PlaceZone's outLots set insertions (off: the set is left alone)
    ZonesDemolition,      // zones_demolition: both DemolishLots calls (off: cost 0, nothing demolished)
    ZonesParcellization,  // zones_parcellization: the zone developer's DoParcellization (off: not re-parcelled)
    ZonesPosts,           // zones_posts: the ZoningChange message (MessagePost; off: not posted)
    ZonesMessages,        // zones_messages: the InsertOccupant subscription (off: no dezone under new networks)
    ZonesTraffic,         // zones_traffic: FindNetworkTile and the tile reads (off: no network tile)
    ZonesTerrain,         // zones_terrain: CellHasWater (off: dry)
    ZonesOccupants,       // zones_occupants: the landfill occupant, bounding cells, CanRemove, network flags
                          //   (off: no occupant)
    ZonesLandfillCost,    // zones_landfill_cost: the landfill developer's GetZoningCost (off: 0)
    ZonesLotDeveloper,    // zones_lot_developer: GetAreaSlope and SetEmptyLotBaseTexture (off: flat, not set)
    // Neighbours simulator host callbacks (cFC4Neighbors.cpp), one per group of
    // game calls; only used when the neighbors subsystem is enabled.
    NeighborsRegionReads,   // neighbors_region_reads: the regional cities (app, region GetCity) and their
                            //   connections (FindConnection / GetAllConnections) (off: no city, none)
    NeighborsRegionWrites,  // neighbors_region_writes: SetBudget / SetIncome / the five statistics and
                            //   UpdateImportExport on regional cities (off: not written)
    NeighborsDeals,         // neighbors_deals: Update / Get / SetLocalDeals (off: none, not stored)
    NeighborsReload,        // neighbors_reload: region ReloadCity in Init (off: not reloaded)
    NeighborsSymmetric,     // neighbors_symmetric: ChangeSymmetricConnection (off: not changed)
    NeighborsMarkers,       // neighbors_markers: the connection marker props, their lookup and moves and
                            //   the terrain heights (off: no marker, height 0)
    NeighborsPosts,         // neighbors_posts: the deal status reports and connection messages (off: not
                            //   sent; the connection log line stays)
    NeighborsHistory,       // neighbors_history: history records (MessageSend)
    NeighborsFigures,       // neighbors_figures: the budget, utility and landfill figures of
                            //   UpdateCommodityStats (off: 0, no departments)
    // Building development simulator and growth developer host callbacks
    // (cFC4Development.cpp, cFC4GrowthDeveloper.cpp), one per group of game
    // calls; only used when the development subsystem is enabled.
    DevelopmentMapWrites,   // development_map_writes: SetTractValue on the building map and the growth
                            //   developers' grids (off: not written)
    DevelopmentOccupants,   // development_occupants: building occupant creation, loading, insertion, removal
                            //   and reads (off: nothing placed, no occupant found)
    DevelopmentZoning,      // development_zoning: GetZoneType and the dezoning PlaceZone (off: unzoned, not dezoned)
    DevelopmentLots,        // development_lots: lot and lot manager reads (off: no lot)
    DevelopmentLotDeveloper,  // development_lot_developer: UpdateLotElements, GetLotSlope and the area altitude
                              //   (off: not updated, 0)
    DevelopmentDemolition,  // development_demolition: DemolishLots and the funds it charges (off: not demolished)
    DevelopmentAdvisor,     // development_advisor: the CityPlanning advisor event (off: not pushed)
    DevelopmentPosts,       // development_posts: kSC4MessageBuildingTypesUpdated (MessagePost; off: not posted)
    DevelopmentCheats,      // development_cheats: the three cheats and the PlaceBuilding command (off: not registered)
    DevelopmentTract,       // development_tract: the tract developer's Grow, Decline and UpdatePopulation
                            //   (off: nothing built, declined or populated)
    DevelopmentCity,        // development_city: land value, power and terrain altitude reads and the growth
                            //   developers' source maps (off: 0, unpowered, no map)
    // Traffic simulator host callbacks (cFC4TrafficSimulator.cpp), one per group
    // of game calls (off: the neutral answer the comment names).
    TrafficGridWrites,   // traffic_grid_writes: SetTractValue / fill on the traffic grids (off: not written)
    TrafficBudget,       // traffic_budget: the two departments, line item incomes and the budget items of
                         //   transit buildings (off: no department, cost 0)
    TrafficLots,         // traffic_lots: the lot manager and the lot reads (off: no lots)
    TrafficSources,      // traffic_sources: the trip lists and the sources' own UpdateTrafficVolume
                         //   (off: no trips, nothing run)
    TrafficHolders,      // traffic_holders: property holder reads and writes (trip capacities; off: absent,
                         //   not written)
    TrafficNeighbors,    // traffic_neighbors: neighbour connections, connected cities, regional demand
                         //   (off: none)
    TrafficOccupants,    // traffic_occupants: the occupant manager's lists and the network occupant reads
                         //   (off: no occupants)
    TrafficTerrain,      // traffic_terrain: CellHasWater and the surface water (off: dry, not navigable)
    TrafficPosts,        // traffic_posts: the posted messages (map changed, strikes, accidents; off: not posted)
    TrafficHistory,      // traffic_history: history records (off: not recorded)
    TrafficSimulators,   // traffic_simulators: residential, building development, ordinance and demand reads
                         //   (off: 0, none)
    TrafficMessages,     // traffic_messages: the message subscriptions (off: no messages)
    // Tract developer host callbacks (FC4TractHost.cpp), one per group of game
    // calls; only used when the tract subsystem is enabled.
    TractConstruction,  // tract_construction: the lot developer's StartConstruction (off: refused, nothing built)
    TractAbandon,       // tract_abandon: AbandonBuilding / ReoccupyBuilding (off: refused)
    TractLotChanges,    // tract_lot_changes: CreateLotIfPossible, DeleteLot, lot SetFacing / SetState and
                        //   RedisplayTerrain (off: no lot made, nothing changed)
    TractDemolition,    // tract_demolition: DemolishLots (off: refused, nothing demolished)
    TractPopulation,    // tract_population: lot SetPopulation (off: not set)
    TractResources,     // tract_resources: Claim / ReleaseConstructionResources (off: claims refused)
    TractFilters,       // tract_filters: the configuration filters and GetLotConfigurationsByFilter (off: no
                        //   configuration)
    TractLotLists,      // tract_lot_lists: GetLotsInCellRect / GetLotsSurroundingLot / Rect (off: no lots)
    TractTraffic,       // tract_traffic: FindNetworkTile, IsOfType and the trip length map (off: no tile, 0)
    TractStyleMap,      // tract_style_map: the growth developer's GetStylePercentages (off: empty)
    TractRoads,         // tract_roads: the configurations' GetRequiredRoads (off: no road required)
    TractFamilySets,    // tract_family_sets: the BDS's GetBuildingTypesByFamily (off: no building)
    TractAgents,        // tract_agents: AddAgent / RemoveAgent (off: not registered, no Year message)
    // Commercial simulator host callbacks (cFC4Commercial.cpp); only used when
    // the commercial subsystem is enabled.
    CommercialGridWrites,  // commercial_grid_writes: SetTractValue / fill on the CO$$ / CO$$$ grids (off: not written)
    // Zone developer / lot manager host callbacks (FC4ZoneDevHost.cpp), one per
    // group of game calls; only used when the zonedev subsystem is enabled.
    ZoneDevGridWrites,    // zonedev_grid_writes: SetTractValue / fill on the maximum-capacity grid (off: not written)
    ZoneDevMessages,      // zonedev_messages: the InsertOccupant, PostCityInit and 0xA6B79602 subscriptions (off: none)
    ZoneDevNetworkTools,  // zonedev_network_tools: the street / dirt-road / road tools, Reset and OccupantExists
                          //   (off: no tool, no draft)
    ZoneDevPosts,         // zonedev_posts: DeferOccupantOperations (MessagePost; off: not posted)
    ZoneDevDemolition,    // zonedev_demolition: OnDezone's DemolishRegion (off: nothing demolished)
    ZoneDevStreets,       // zonedev_streets: the street tool's PlaceNetworkOccupants (off: false, no street)
    ZoneDevOccupants,     // zonedev_occupants: the cell occupant lists and occupant reads, RemoveOccupant (off:
                          //   no occupant)
    ZoneDevTerrain,       // zonedev_terrain: CellHasWater and the altitudes (off: dry, 0)
    ZoneDevLotChanges,    // zonedev_lot_changes: lot Init / Shutdown / PostCityInit / SetState /
                          //   SetVerticesAreLocked (off: not called)
    ZoneDevGrowth,        // zonedev_growth: the BDS, growth developer and lot configuration reads of
                          //   DetermineLotSize (off: none)
    ZoneDevLevelling,     // zonedev_levelling: the lot developer's AttemptAreaLevelling (off: not levelled)
    ZoneDevLotDeveloper,  // zonedev_lot_developer: DeleteLot's lot developer calls (off: not called)
    ZoneDevTraffic,       // zonedev_traffic: GetDesiredLotInsertionPoint (off: null, appended)
    // Lot developer host callbacks (FC4LotDevHost.cpp), one per group of game
    // calls; only used when the lotdev subsystem is enabled.
    LotdevLotWrites,     // lotdev_lot_writes: lot SetState / SetCondition / flags / configuration / properties
                         //   and the other lot setters (off: not written)
    LotdevBuildings,     // lotdev_buildings: BDS AddNewBuildingOccupant and the construction occupant (off: none)
    LotdevDemolition,    // lotdev_demolition: RemoveOccupant, DemolishOccupant, DemolishLots, DeleteLot (off:
                         //   nothing removed)
    LotdevProps,         // lotdev_props: props, zots, flora and rubble (off: none added or removed)
    LotdevTextures,      // lotdev_textures: the base-texture occupants (off: none made or changed)
    LotdevTerrain,       // lotdev_terrain: ModifyTerrainHeight and the foundation occupants (off: refused)
    LotdevNight,         // lotdev_night: building SetLit and the night-light prop messages (off: not sent)
    LotdevUnits,         // lotdev_units: the business units (off: none made, joined or left)
    LotdevAgents,        // lotdev_agents: AddAgent / RemoveAgent and the animation tick (off: not registered)
    Count
};

// True if the switch is on (default) - cheap, any thread.
bool On(Sw s);
const char* SwitchName(Sw s);

// Counts (and, verbose, logs the first few of) the calls a switch skipped.
void NoteSkipped(Sw s, const char* what);
// One line with the skip counts since the last call (budget Shutdown).
void LogSkipCounts(const char* why);

// Re-reads FreeCity4.ini and the environment, and logs the result.
void ConfigReload(const char* why);

// The ini value of `iniKey`, else the environment variable `envName` (may be
// null), else "". `source` (optional) receives "FreeCity4.ini", `envName` or "".
std::string Setting(const char* iniKey, const char* envName, const char** source = nullptr);

// Setting() as an integer: "on/true/yes" = 1, "off/false/no" = 0, a number
// as is; empty or unparsable = `fallback`.
long SettingInt(const char* iniKey, const char* envName, long fallback);

// The switches off at the last ConfigReload, comma-separated without spaces,
// or "-" (static buffer; lock-free readers such as the autotest verdict).
const char* SwitchesOffList();

// Full path of FreeCity4.ini (next to the DLL).
std::string IniPath();

}  // namespace fc4
