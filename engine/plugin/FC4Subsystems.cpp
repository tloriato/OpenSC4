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

#include "FC4Subsystems.h"

#include "FC4Autotest.h"
#include "FC4Checked.h"
#include "FC4Config.h"
#include "FC4BudgetShadow.h"
#include "FC4CrimeShadow.h"
#include "FC4DemandShadow.h"
#include "FC4Log.h"
#include "FC4Observer.h"
#include "FC4Dialogs.h"  // the harness lane: close_dialogs
#include "FC4Seed.h"     // the seeding lane: session_seed
#include "FC4Pacing.h"   // the detpacing lane: pacing
#include "FC4Popups.h"
#include "FC4RefAudit.h"
#include "cFC4Demand.h"
#include "cFC4Ordinance.h"
#include "cFC4Power.h"
#include "cFC4Plumbing.h"
#include "FC4PlumbingShadow.h"
#include "cFC4Flammability.h"
#include "FC4PollutionShadow.h"
#include "FC4PowerShadow.h"
#include "cFC4Pollution.h"
#include "FC4ProtectionShadow.h"
#include "cFC4Protection.h"
#include "cFC4Crime.h"
#include "FC4AuraShadow.h"
#include "cFC4Aura.h"
#include "FC4FloraShadow.h"
#include "cFC4Flora.h"
#include "FC4LandfillShadow.h"
#include "cFC4Landfill.h"
#include "FC4LandValueShadow.h"
#include "FC4FlammabilityShadow.h"
#include "FC4Clock24Shadow.h"
#include "FC4AutomataShadow.h"  // the glue19 lane
#include "FC4MySimsShadow.h"  // the mysims1 lane: mysims_shadow
#include "cFC4Automata.h"  // the glue19 lane
#include "FC4HistoryShadow.h"
#include "FC4NetMgrShadow.h"
#include "FC4TerrainShadow.h"  // the glue16 lane
#include "FC4CitySitShadow.h"     // the glue17 lane: citysit_shadow
#include "FC4DemolitionPriceShadow.h"  // the glue23 lane: demolition_price_shadow
#include "FC4DemolitionShadow.h"  // the glue17 lane: demolition_shadow
#include "FC4DispatchShadow.h"    // the glue17 lane: dispatch_shadow
#include "cFC4Dispatch.h"        // the glue17 lane: enable = dispatch
#include "cFC4CitySituation.h"    // the glue17 lane: enable = citysit
#include "cFC4Demolition.h"       // the glue23 lane: enable = demolition
#include "cFC4NetworkTool.h"      // the glue29 lane: enable = nettool
#include "cFC4Terrain.h"        // the glue18 lane: the terrain replacement
#include "cFC4NetworkManager.h"    // the glue24 lane: enable = netmgr
#include "cFC4NetworkLotManager.h" // the glue24 lane: enable = netlot
#include "cFC4OccMgr.h"          // the glue25 lane: enable = occmgr
#include "FC4ResidentialShadow.h"
#include "FC4ZonesShadow.h"
#include "cFC4Residential.h"
#include "cFC4ZoneManager.h"
#include "FC4ZoneDevShadow.h"
#include "cFC4ZoneDeveloper.h"
#include "FC4NeighborsShadow.h"
#include "FC4DevelopmentShadow.h"
#include "cFC4Neighbors.h"
#include "cFC4Development.h"
#include "cFC4GrowthDeveloper.h"
#include "cFC4TrafficSimulator.h"
#include "cFC4Clock24.h"
#include "cFC4History.h"
#include "FC4TractShadow.h"
#include "FC4CommercialShadow.h"  // the cmglue lane
#include "FC4CivicShadow.h"       // the cmglue lane
#include "FC4ParksShadow.h"       // the cmglue lane
#include "FC4DisasterShadow.h"    // the glue13 lane
#include "FC4EffectsShadow.h"      // the glue22 lane
#include "cFC4Effects.h"           // the glue22 lane
#include "FC4TrafficShadow.h"
#include "FC4OccMgrShadow.h"  // the glue14b lane
#include "cFC4TractDeveloper.h"
#include "FC4LotDevShadow.h"
#include "FC4PropLotShadow.h"  // the glue15 lane
#include "cFC4LotDeveloper.h"
#include "cFC4Commercial.h"  // the cmglue lane: commercial, industrial
#include "cFC4Civic.h"       // the cmglue lane: civic
#include "cFC4Parks.h"       // the cmglue lane: parks
#include "cFC4Tutorial.h"    // the glue15 lane: tutorial
#include "FC4PortsShadow.h"    // the glue12b2 lane: ports
#include "cFC4PortDeveloper.h"  // the glue12b2 lane: ports
#include "cFC4PropManager.h"  // the glue21 lane: propmgr
#include "cFC4LotConfigManager.h"  // the glue21 lane: lotcfg
#include "FC4Turbo.h"  // the harness lane: sim_turbo
#include "FC4Script.h"  // the harness lane: script
#include "FC4PropDevShadow.h"     // the glue12b1 lane: propdev_shadow
#include "FC4CityDetailShadow.h"  // the glue12b1 lane: citydetail_shadow
#include "cFC4PropDeveloper.h"    // the glue12b1 lane: propdev
#include "cFC4CityDetail.h"       // the glue12b1 lane: citydetail
#include "FC4FormatsShadow.h"     // the glue-formats4 lane
#include "FC4ExemplarFactory.h"   // the glue-exemplars4 lane

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>

#include <cctype>
#include <cstring>
#include <string>

// Factories (defined next to each class).
cIGZUnknown* CreateFC4Simulator();
cIGZUnknown* CreateFC4BudgetSimulator();
cIGZUnknown* CreateFC4DepartmentBudget();
cIGZUnknown* CreateFC4LineItem();
cIGZUnknown* CreateFC4LandValueSimulator();
cIGZUnknown* CreateFC4WeatherSimulator();
cIGZUnknown* CreateFC4EffectsManager();

namespace fc4 {
namespace {

// ---- the table ------------------------------------------------------------------------------------

const ClassEntry kSimulatorClasses[] = {
    {0x2990C1E5, "cSC4Simulator", &CreateFC4Simulator},
};

// docs/specs/budget.md §1: the budget simulator, departments and line items.
const ClassEntry kBudgetClasses[] = {
    {0xE990BE01, "cSC4BudgetSimulator", &CreateFC4BudgetSimulator},
    {0xE990BFFC, "cSC4DepartmentBudget", &CreateFC4DepartmentBudget},
    {0xAA313C9F, "cSC4LineItem", &CreateFC4LineItem},
};

// docs/specs/landvalue.md §1. Off by default (M3 trial): `enable = landvalue`.
// The game's own grid classes (0x49B9E602..0x49B9E60B) are never registered
// (simgrid.md §6.3).
const ClassEntry kLandValueClasses[] = {
    {0xE990C18E, "cSC4LandValueSimulator", &CreateFC4LandValueSimulator},
};

// docs/specs/weather.md §1. Off by default (M2 glue trial): `enable = weather`.
const ClassEntry kWeatherClasses[] = {
    {0x2990C1BC, "cSC4WeatherSimulator", &CreateFC4WeatherSimulator},
};

// docs/specs/ordinance.md §1, §3.6. Off by default (M2 glue trial):
// `enable = ordinance`. The simulator and the ordinance classes go together
// (§10.2).
const ClassEntry kOrdinanceClasses[] = {
    {0xA990C08E, "cSC4OrdinanceSimulator", &CreateFC4OrdinanceSimulator},
    {0x8BD4FC74, "cSC4OrdinanceSimple", &CreateFC4Ordinance<0x8BD4FC74>},
    {0xA2BF1DDC, "cSC4OrdinanceCarpoolIncentive", &CreateFC4Ordinance<0xA2BF1DDC>},
    {0xA2BF1E43, "cSC4OrdinanceCleanAir", &CreateFC4Ordinance<0xA2BF1E43>},
    {0x00D0723D, "cSC4OrdinanceCommunityCPRTraining", &CreateFC4Ordinance<0x00D0723D>},
    {0x40D07236, "cSC4OrdinanceFreeClinics", &CreateFC4Ordinance<0x40D07236>},
    {0x815B4CEF, "cSC4OrdinanceJuniorSportsLeague", &CreateFC4Ordinance<0x815B4CEF>},
    {0x22F6E80C, "cSC4OrdinanceLandfillGasRecovery", &CreateFC4Ordinance<0x22F6E80C>},
    {0xA0D07129, "cSC4OrdinanceLegalizeGambling", &CreateFC4Ordinance<0xA0D07129>},
    {0x62BF1DAA, "cSC4OrdinanceMandatorySmokeDetectors", &CreateFC4Ordinance<0x62BF1DAA>},
    {0x62BF1DB9, "cSC4OrdinanceNeighborhoodWatch", &CreateFC4Ordinance<0x62BF1DB9>},
    {0xE0D0722E, "cSC4OrdinanceNuclearFreeZone", &CreateFC4Ordinance<0xE0D0722E>},
    {0x22F6E81B, "cSC4OrdinancePaperReductionAct", &CreateFC4Ordinance<0x22F6E81B>},
    {0x82B9999B, "cSC4OrdinancePowerConservation", &CreateFC4Ordinance<0x82B9999B>},
    {0xE0D07233, "cSC4OrdinanceProReadingCampaign", &CreateFC4Ordinance<0xE0D07233>},
    {0xA2BF1DE5, "cSC4OrdinanceShuttleService", &CreateFC4Ordinance<0xA2BF1DE5>},
    {0xC2F6E81F, "cSC4OrdinanceTireRecycling", &CreateFC4Ordinance<0xC2F6E81F>},
    {0x62F6E7CF, "cSC4OrdinanceTouristPromotion", &CreateFC4Ordinance<0x62F6E7CF>},
    {0x42BF1E18, "cSC4OrdinanceTrashPresort", &CreateFC4Ordinance<0x42BF1E18>},
    {0xC2BF1E04, "cSC4OrdinanceVehicleEmissionStandard", &CreateFC4Ordinance<0xC2BF1E04>},
    {0x02BF1DFA, "cSC4OrdinanceWaterConservation", &CreateFC4Ordinance<0x02BF1DFA>},
    {0xC2BF1DC5, "cSC4OrdinanceYouthCurfew", &CreateFC4Ordinance<0xC2BF1DC5>},
};

// docs/specs/demand.md §1. Off by default (M2 glue trial): `enable = demand`.
// The node class is registered because saves store nodes by class (§1).
const ClassEntry kDemandClasses[] = {
    {0x2990BFF2, "cSC4DemandSimulator", &CreateFC4DemandSimulator},
    {0x8990C12D, "cSC4Demand", &CreateFC4Demand},
};

// docs/specs/power.md §1. Off by default (M3 glue trial): `enable = power`.
const ClassEntry kPowerClasses[] = {
    {0x0990C005, "cSC4PowerSimulator", &CreateFC4PowerSimulator},
};

// docs/specs/plumbing.md §1. Off by default (M3 glue trial): `enable = plumbing`.
const ClassEntry kPlumbingClasses[] = {
    {0x0990C075, "cSC4PlumbingSimulator", &CreateFC4PlumbingSimulator},
};

// docs/specs/flammability.md §1. Off by default (M3 glue trial):
// `enable = flammability`.
const ClassEntry kFlammabilityClasses[] = {
    {0xE990C045, "cSC4FlammabilitySimulator", &CreateFC4FlammabilitySimulator},
};

// docs/specs/pollution.md §1. Off by default (M3 glue trial): `enable = pollution`.
const ClassEntry kPollutionClasses[] = {
    {0x8990C065, "cSC4PollutionSimulator", &CreateFC4PollutionSimulator},
};

// docs/specs/protection.md §1, §13.2. Off by default (M3 glue trial):
// `enable = protection` replaces both simulators together ([choice] §13.2).
const ClassEntry kProtectionClasses[] = {
    {0x2990C05A, "cSC4FireProtectionSimulator", &CreateFC4FireProtectionSimulator},
    {0x6990C04F, "cSC4PoliceSimulator", &CreateFC4PoliceSimulator},
};

// docs/specs/crime.md §1. Off by default (M3 glue trial): `enable = crime`.
const ClassEntry kCrimeClasses[] = {
    {0x8990C09A, "cSC4CrimeSimulator", &CreateFC4CrimeSimulator},
};

// docs/specs/aura.md §1. Off by default (M3 glue trial): `enable = aura`.
const ClassEntry kAuraClasses[] = {
    {0x0990BDD8, "cSC4AuraSimulator", &CreateFC4AuraSimulator},
};

// docs/specs/flora.md §1. Off by default (M3 glue trial): `enable = flora`.
// The flora occupant class (0xA9C05C85) stays the game's (§13.2).
const ClassEntry kFloraClasses[] = {
    {kFC4_CLSID_cSC4FloraSimulator, "cSC4FloraSimulator", &CreateFC4FloraSimulator},
};

// docs/specs/landfill.md §1. Off by default (M3 glue trial): `enable = landfill`.
const ClassEntry kLandfillClasses[] = {
    {0xEA09D4AE, "cSC4LandfillDeveloper", &CreateFC4LandfillDeveloper},
};

// docs/specs/residential.md §1 and residential-education-health.md. Off by
// default (M4 glue trial): `enable = residential`.
const ClassEntry kResidentialClasses[] = {
    {kFC4_CLSID_cSC4ResidentialSimulator, "cSC4ResidentialSimulator", &CreateFC4ResidentialSimulator},
};

// docs/specs/zones.md §1. Off by default (M4 glue trial): `enable = zones`.
// The zone developer / lot manager (0x498F9B01) stays the game's (§13.1).
const ClassEntry kZonesClasses[] = {
    {kFC4_CLSID_cSC4ZoneManager, "cSC4ZoneManager", &CreateFC4ZoneManager},
};

// docs/specs/neighbors.md §1 (the simulator; its connection and deal objects
// have no factory, §15.2). Off by default (glue trial): `enable = neighbors`.
const ClassEntry kNeighborsClasses[] = {
    {kFC4_CLSID_cSC4NeighborsSimulator, "cSC4NeighborsSimulator", &CreateFC4NeighborsSimulator},
};

// docs/specs/building-development.md and growth-developer.md: the building
// development simulator and the growth developers, replaced as one unit (§9,
// growth §12). Off by default (M4 glue trial): `enable = development`.
const ClassEntry kDevelopmentClasses[] = {
    {kFC4_CLSID_cSC4BuildingDevelopmentSimulator, "cSC4BuildingDevelopmentSimulator",
     &CreateFC4BuildingDevelopmentSimulator},
    {kFC4_CLSID_cSC4GrowthDeveloper, "cSC4GrowthDeveloper", &CreateFC4GrowthDeveloper},
};

// docs/specs/traffic-simulator.md, traffic-network.md and pathfinder.md: the
// traffic simulator, its network model and the pathfinders of slot 6, replaced
// as one unit (traffic-simulator.md §9). Off by default (M5 glue trial):
// `enable = traffic`.
const ClassEntry kTrafficClasses[] = {
    {kFC4_CLSID_cSC4TrafficSimulator, "cSC4TrafficSimulator", &CreateFC4TrafficSimulator},
};

// docs/specs/history.md §1: the history warehouse (city component 1). Off by
// default (glue trial): `enable = history`.
// docs/specs/automata.md 0: the script system (40), the controller manager
// (41) and the four automata managers (43-46). Records and call contracts
// only (0: animation stubbed). The Init fault that kept these out of
// kSubsystems (glue19 question A4) was the create path's IID: the city's
// ensure-components helper calls CreateObject(CLSID, IID) and the classes
// answered the CLSID only, so the city's component slot stayed null and its
// Init dispatcher dereferenced it unconditionally at SimCity 4.exe+0x0045BF1B
// (automata.md §15). Every QueryInterface now answers its interface IID too.
static const ClassEntry kAutomataClasses[] = {
    {kFC4_CLSID_cSC4AutomataScriptSystem, "cSC4AutomataScriptSystem", &CreateFC4AutomataScriptSystem},
    {kFC4_CLSID_cSC4AutomataControllerManager, "cSC4AutomataControllerManager",
     &CreateFC4AutomataControllerManager},
    {kFC4_CLSID_cSC4VehicleManager, "cSC4VehicleManager", &CreateFC4AutomataVehicleManager},
    {kFC4_CLSID_cSC4PedestrianManager, "cSC4PedestrianManager", &CreateFC4AutomataPedestrianManager},
    {kFC4_CLSID_cSC4AircraftManager, "cSC4AircraftManager", &CreateFC4AutomataAircraftManager},
    {kFC4_CLSID_cSC4WatercraftManager, "cSC4WatercraftManager", &CreateFC4AutomataWatercraftManager},
};

const ClassEntry kHistoryClasses[] = {
    {kFC4_CLSID_cSC4HistoryWarehouse, "cSC4HistoryWarehouse", &CreateFC4HistoryWarehouse},
};

// docs/specs/clock24.md §1: the 24-hour clock (city component 50). Off by
// default (glue trial): `enable = clock24`.
const ClassEntry kClock24Classes[] = {
    {kFC4_CLSID_cSC424HourClock, "cSC424HourClock", &CreateFC4Clock24},
};

// docs/specs/terrain.md §1 (the glue18 lane). The terrain is city component 49
// and the surface water (CLSID 0x0B8DA7DB) is the object the city keeps next to
// it (city slot 83). Both are registered so the city never builds the original
// of either; both are stateless views over the one fc4_terrain handle, so which
// of the two the city constructs does not matter. Off by default:
// `enable = terrain`. With it on the city builds ours from the start and the
// original cSTETerrain never runs (registry CLSID precedence).
const ClassEntry kTerrainClasses[] = {
    {fc4::kFC4_CLSID_cSTETerrain, "cSTETerrain", &CreateFC4Terrain},
    {fc4::kFC4_CLSID_cSTESurfaceWater, "cSTESurfaceWater", &CreateFC4SurfaceWater},
};

// docs/specs/tract-developer.md §1. Off by default (M4 glue trial):
// `enable = tract`. The BDS, the growth developers, the lot developer, the
// lot manager and the lot configuration manager stay whoever the city runs.
const ClassEntry kTractClasses[] = {
    {kFC4_CLSID_cSC4TractDeveloper, "cSC4TractDeveloper", &CreateFC4TractDeveloper},
};

// docs/specs/lot-developer.md §1. Off by default (M4 glue trial):
// `enable = lotdev`. The BDS, the tract developer, the lot manager, the
// occupant and prop managers stay whoever the city runs.
const ClassEntry kLotDevClasses[] = {
    {kFC4_CLSID_cSC4LotDeveloper, "cSC4LotDeveloper", &CreateFC4LotDeveloper},
};

// docs/specs/commercial-industrial.md §1. Off by default (M4 glue trial):
// `enable = commercial` / `enable = industrial`. The growth developers and the
// query tool read the commercial proximity grids through slot 5.
const ClassEntry kCommercialClasses[] = {
    {kFC4_CLSID_cSC4CommercialSimulator, "cSC4CommercialSimulator", &CreateFC4CommercialSimulator},
};
const ClassEntry kIndustrialClasses[] = {
    {kFC4_CLSID_cSC4IndustrialSimulator, "cSC4IndustrialSimulator", &CreateFC4IndustrialSimulator},
};
// docs/specs/civic-buildings.md §1. Off by default (M4 glue trial):
// `enable = civic`. The building menu, the advisors and Lua reach in through
// slots 5-8.
const ClassEntry kCivicClasses[] = {
    {kFC4_CLSID_cSC4CivicBuildingSimulator, "cSC4CivicBuildingSimulator", &CreateFC4CivicBuildingSimulator},
};
// docs/specs/parks.md §1. Off by default (M4 glue trial): `enable = parks`.
// Nothing calls into it beyond Init and Shutdown (§9).
const ClassEntry kParkClasses[] = {
    {kFC4_CLSID_cSC4ParkManager, "cSC4ParkManager", &CreateFC4ParkManager},
};
// docs/specs/port-developers.md §1: the airport and seaport developers (city
// components 13 and 14). Off by default (glue12b2): `enable = ports`.
const ClassEntry kPortClasses[] = {
    {kFC4_CLSID_cSC4AirportDeveloper, "cSC4AirportDeveloper", &CreateFC4AirportDeveloper},
    {kFC4_CLSID_cSC4SeaportDeveloper, "cSC4SeaportDeveloper", &CreateFC4SeaportDeveloper},
};
// docs/specs/zones.md §1, §1.2, §11.2, §11.3: the zone developer / lot manager
// (city components 7 and 8). Off by default (M4 glue trial): enable = zonedev.
// Lot creation waits for re-specs11 (CreateLot fails, logged).
const ClassEntry kZoneDevClasses[] = {
    {kFC4_CLSID_cSC4ZoneDeveloper, "cSC4ZoneDeveloper", &CreateFC4ZoneDeveloper},
};
// docs/specs/tutorial.md §1: the tutorial system (city component 48). Off by
// default (glue trial): `enable = tutorial`. The fire and flammability
// IsTutorial readers then read fc4_tutorial_is_tutorial (cFC4Tutorial.h).
const ClassEntry kTutorialClasses[] = {
    {kFC4_CLSID_cSC4TutorialSystem, "cSC4TutorialSystem", &CreateFC4TutorialSystem},
};
// docs/specs/prop-lotconfig.md Part A: the prop manager (city component 5).
// Off by default (the glue21 lane): `enable = propmgr`. Replaces the game's
// cSC4PropManager; propmgr_shadow still compares the live class against the
// core over a read-only host (FC4PropLotShadow.h).
const ClassEntry kPropMgrClasses[] = {
    {kFC4_CLSID_cSC4PropManager, "cSC4PropManager", &CreateFC4PropManager},
};
// docs/specs/prop-lotconfig.md Part B: the lot configuration manager (city
// component 9). Off by default (the glue21 lane): `enable = lotcfg`.
const ClassEntry kLotCfgClasses[] = {
    {kFC4_CLSID_cSC4LotConfigManager, "cSC4LotConfigManager", &CreateFC4LotConfigManager},
};

// docs/specs/dispatch.md §1. Off by default (glue17): `enable = dispatch`.
// The protection simulators and the air sources register with it (§8), and the
// dispatch tool drives it (§4.5).
const ClassEntry kDispatchClasses[] = {
    {kFC4_CLSID_cSC4DispatchManager, "cSC4DispatchManager", &CreateFC4DispatchManager},
};

// docs/specs/city-situation.md §1, and the §8 "minimal replacement" [choice]:
// the record byte for byte and the interface answered with "no situation".
// Off by default (glue17): `enable = citysit`. The mission gameplay (targets,
// pursuit, coverage, the player-drive camera, the popup) is deliberately out
// of scope (§0).
const ClassEntry kCitySitClasses[] = {
    {kFC4_CLSID_cSC4CitySituationManager, "cSC4CitySituationManager", &CreateFC4CitySituationManager},
};

// docs/specs/demolition.md §1. Off by default (glue23): `enable = demolition`.
// Its host is the full 86-callback table of FC4DemolitionHost.h; a half-wired
// one would price utility demolition at 0 and demolish nothing.
const ClassEntry kDemolitionClasses[] = {
    {kFC4_CLSID_cSC4Demolition, "cSC4Demolition", &CreateFC4Demolition},
};

// docs/specs/prop-developer.md §1: the prop developer (city component 10)
// and its wealth requester (class 0x89C49986, resolved from saved records).
// Off by default: `enable = propdev`.
const ClassEntry kPropDevClasses[] = {
    {kFC4_CLSID_cSC4PropDeveloper, "cSC4PropDeveloper", &CreateFC4PropDeveloper},
    {kFC4_CLSID_cSC4WealthRequester, "cSC4WealthRequester", &CreateFC4WealthRequester},
};
// docs/specs/city-detail.md §1: the city detail manager. Off by default:
// `enable = citydetail`.
const ClassEntry kCityDetailClasses[] = {
    {kFC4_CLSID_cSC4CityDetailManager, "cSC4CityDetailManager", &CreateFC4CityDetailManager},
};

// docs/specs/effects.md §1: the effects manager (city slot 85, city +0x1FC),
// its visual effects and their components. Off by default (glue trial):
// `enable = effects`. Not a saved component (§13).
const ClassEntry kEffectsClasses[] = {
    {kFC4_CLSID_cSC4EffectsManager, "cSC4EffectsManager", &CreateFC4EffectsManager},
};

// docs/specs/network-manager.md §1: the network manager (city component 53)
// and the network lot manager (component 11). Off by default (glue24):
// `enable = netmgr` / `enable = netlot`. The network tools stay the game's
// unless `enable = nettool` also turns the tool replacement on (glue29):
// §13 sends the tools to their own subsystem, and the manager's own factory
// creates the stand-ins (cFC4NetworkTool.h) that glue29's real tools replace
// when the key is on.
const ClassEntry kNetworkManagerClasses[] = {
    {kFC4_CLSID_cSC4NetworkManager, "cSC4NetworkManager", &CreateFC4NetworkManager},
};
const ClassEntry kNetworkLotClasses[] = {
    {kFC4_CLSID_cSC4NetworkLotManager, "cSC4NetworkLotManager", &CreateFC4NetworkLotManager},
};
// docs/specs/network-tools.md §1: the network tools (glue29). The tool
// classes have no GZCOM class ID of their own (question NM11: the manager's
// factory builds them directly), so the replacement is registered as one
// synthetic class and reaches the simulation through the registry: with
// `enable = nettool` the plugin's own callers (the scripted `network`
// action, the zone developer's street tools) take their tools from
// NetToolRegistry::Take instead of the network manager's factory, and the
// netmgr replacement's own create_tool callback hands out the real tool.
const ClassEntry kNetToolClasses[] = {
    {kFC4_CLSID_cFC4NetworkTool, "cFC4NetworkTool", &CreateFC4NetworkToolEntry},
};

// docs/specs/occupant-manager.md §1, CLSID 0x098F964D. Off by default
// (glue25): `enable = occmgr`. The city component table's #4 (foundations
// §3.6); every manager query and the v1 save record go through it.
const ClassEntry kOccMgrClasses[] = {
    {kFC4_CLSID_cSC4OccupantManager, "cSC4OccupantManager", &CreateFC4OccupantManager},
};

const Subsystem kSubsystems[] = {
    {"simulator", "FC4_DISABLE_SIMULATOR", kSimulatorClasses, sizeof(kSimulatorClasses) / sizeof(kSimulatorClasses[0]),
     true},
    {"budget", nullptr, kBudgetClasses, sizeof(kBudgetClasses) / sizeof(kBudgetClasses[0]), true},
    {"landvalue", nullptr, kLandValueClasses, sizeof(kLandValueClasses) / sizeof(kLandValueClasses[0]), false},
    {"weather", nullptr, kWeatherClasses, sizeof(kWeatherClasses) / sizeof(kWeatherClasses[0]), false},
    {"ordinance", nullptr, kOrdinanceClasses, sizeof(kOrdinanceClasses) / sizeof(kOrdinanceClasses[0]), false},
    {"demand", nullptr, kDemandClasses, sizeof(kDemandClasses) / sizeof(kDemandClasses[0]), false},
    {"power", nullptr, kPowerClasses, sizeof(kPowerClasses) / sizeof(kPowerClasses[0]), false},
    {"plumbing", nullptr, kPlumbingClasses, sizeof(kPlumbingClasses) / sizeof(kPlumbingClasses[0]), false},
    {"flammability", nullptr, kFlammabilityClasses, sizeof(kFlammabilityClasses) / sizeof(kFlammabilityClasses[0]),
     false},
    {"pollution", nullptr, kPollutionClasses, sizeof(kPollutionClasses) / sizeof(kPollutionClasses[0]), false},
    {"protection", nullptr, kProtectionClasses, sizeof(kProtectionClasses) / sizeof(kProtectionClasses[0]), false},
    {"crime", nullptr, kCrimeClasses, sizeof(kCrimeClasses) / sizeof(kCrimeClasses[0]), false},
    {"aura", nullptr, kAuraClasses, sizeof(kAuraClasses) / sizeof(kAuraClasses[0]), false},
    {"flora", nullptr, kFloraClasses, sizeof(kFloraClasses) / sizeof(kFloraClasses[0]), false},
    {"landfill", nullptr, kLandfillClasses, sizeof(kLandfillClasses) / sizeof(kLandfillClasses[0]), false},
    {"residential", nullptr, kResidentialClasses, sizeof(kResidentialClasses) / sizeof(kResidentialClasses[0]),
     false},
    {"zones", nullptr, kZonesClasses, sizeof(kZonesClasses) / sizeof(kZonesClasses[0]), false},
    {"neighbors", nullptr, kNeighborsClasses, sizeof(kNeighborsClasses) / sizeof(kNeighborsClasses[0]), false},
    {"development", nullptr, kDevelopmentClasses, sizeof(kDevelopmentClasses) / sizeof(kDevelopmentClasses[0]),
     false},
    {"traffic", nullptr, kTrafficClasses, sizeof(kTrafficClasses) / sizeof(kTrafficClasses[0]), false},
    {"tract", nullptr, kTractClasses, sizeof(kTractClasses) / sizeof(kTractClasses[0]), false},
    {"lotdev", nullptr, kLotDevClasses, sizeof(kLotDevClasses) / sizeof(kLotDevClasses[0]), false},
    {"commercial", nullptr, kCommercialClasses, sizeof(kCommercialClasses) / sizeof(kCommercialClasses[0]), false},
    {"industrial", nullptr, kIndustrialClasses, sizeof(kIndustrialClasses) / sizeof(kIndustrialClasses[0]), false},
    {"civic", nullptr, kCivicClasses, sizeof(kCivicClasses) / sizeof(kCivicClasses[0]), false},
    {"parks", nullptr, kParkClasses, sizeof(kParkClasses) / sizeof(kParkClasses[0]), false},
    {"zonedev", nullptr, kZoneDevClasses, sizeof(kZoneDevClasses) / sizeof(kZoneDevClasses[0]), false},
    {"history", nullptr, kHistoryClasses, sizeof(kHistoryClasses) / sizeof(kHistoryClasses[0]), false},
    {"clock24", nullptr, kClock24Classes, sizeof(kClock24Classes) / sizeof(kClock24Classes[0]), false},
    {"dispatch", nullptr, kDispatchClasses, sizeof(kDispatchClasses) / sizeof(kDispatchClasses[0]), false},
    {"citysit", nullptr, kCitySitClasses, sizeof(kCitySitClasses) / sizeof(kCitySitClasses[0]), false},
    {"demolition", nullptr, kDemolitionClasses, sizeof(kDemolitionClasses) / sizeof(kDemolitionClasses[0]),
     false},
    {"tutorial", nullptr, kTutorialClasses, sizeof(kTutorialClasses) / sizeof(kTutorialClasses[0]), false},
    {"propdev", nullptr, kPropDevClasses, sizeof(kPropDevClasses) / sizeof(kPropDevClasses[0]), false},
    {"citydetail", nullptr, kCityDetailClasses, sizeof(kCityDetailClasses) / sizeof(kCityDetailClasses[0]), false},
    {"ports", nullptr, kPortClasses, sizeof(kPortClasses) / sizeof(kPortClasses[0]), false},
    {"effects", nullptr, kEffectsClasses, sizeof(kEffectsClasses) / sizeof(kEffectsClasses[0]), false},
    {"nettool", nullptr, kNetToolClasses, sizeof(kNetToolClasses) / sizeof(kNetToolClasses[0]), false},
    {"terrain", nullptr, kTerrainClasses, sizeof(kTerrainClasses) / sizeof(kTerrainClasses[0]), false},
    {"automata", nullptr, kAutomataClasses, sizeof(kAutomataClasses) / sizeof(kAutomataClasses[0]),
     false},  // the automata layer (glue19); off by default: `enable = automata`
    {"netmgr", nullptr, kNetworkManagerClasses, sizeof(kNetworkManagerClasses) / sizeof(kNetworkManagerClasses[0]),
     false},
    {"netlot", nullptr, kNetworkLotClasses, sizeof(kNetworkLotClasses) / sizeof(kNetworkLotClasses[0]), false},
    // the glue21 lane
    {"propmgr", nullptr, kPropMgrClasses, sizeof(kPropMgrClasses) / sizeof(kPropMgrClasses[0]), false},
    {"lotcfg", nullptr, kLotCfgClasses, sizeof(kLotCfgClasses) / sizeof(kLotCfgClasses[0]), false},
    {"occmgr", nullptr, kOccMgrClasses, sizeof(kOccMgrClasses) / sizeof(kOccMgrClasses[0]), false},
    // the glue-exemplars4 lane: no GZCOM class; the resource factories are taken
    // over at PostAppInit (data-formats §8.1, FC4ExemplarFactory.h). `enable = exemplars`.
    {"exemplars", nullptr, nullptr, 0, false},
};

constexpr size_t kSubsystemCount = sizeof(kSubsystems) / sizeof(kSubsystems[0]);

// ---- switches --------------------------------------------------------------------------------------

std::string Env(const char* name) {
    char value[512] = {};
    const DWORD n = GetEnvironmentVariableA(name, value, sizeof(value));
    return (n > 0 && n < sizeof(value)) ? std::string(value, n) : std::string();
}

std::string Lower(std::string s) {
    for (char& c : s) c = static_cast<char>(std::tolower(static_cast<unsigned char>(c)));
    return s;
}

bool ListContains(const std::string& list, const std::string& key) {
    size_t i = 0;
    while (i < list.size()) {
        while (i < list.size() && (list[i] == ',' || list[i] == ' ' || list[i] == ';')) ++i;
        size_t j = i;
        while (j < list.size() && list[j] != ',' && list[j] != ' ' && list[j] != ';') ++j;
        if (j > i) {
            const std::string item = list.substr(i, j - i);
            if (item == key || item == "all") return true;
        }
        i = j;
    }
    return false;
}

bool g_resolved = false;
bool g_enabled[kSubsystemCount] = {};
char g_disabledList[128] = "-";

void Resolve() {
    if (g_resolved) return;
    g_resolved = true;
    // First call outside DllMain: configuration, audit settings, checked-build self-tests.
    ConfigReload("game start");
    SessionSeedStartup();  // session_seed (FC4Seed.h): one line with S
    PacingStartup();       // pacing (FC4Pacing.h): before any core exists; one line
    RefAuditReload();
    CheckedStartup();
    AutotestReload();
    ObserverReload();
    PopupDialogsReload();  // popup_dialogs (FC4Popups.h); the call itself is made at PostAppInit
    DialogsReload();       // close_dialogs (FC4Dialogs.h); the subscription is made at PostAppInit
    // FreeCity4.ini `disable = ...` wins over FC4_DISABLE.
    const char* source = "";
    const std::string disable = Lower(Setting("disable", "FC4_DISABLE", &source));
    const char* enableSource = "";
    const std::string enable = Lower(Setting("enable", "FC4_ENABLE", &enableSource));
    for (size_t i = 0; i < kSubsystemCount; ++i) {
        const Subsystem& s = kSubsystems[i];
        if (!s.defaultOn && !ListContains(enable, s.key)) {
            // Off by default and not enabled: not "disabled" (the lists and the
            // autotest verdict stay as without this subsystem).
            g_enabled[i] = false;
            Log("subsystem '%s': off by default (enable = %s in FreeCity4.ini to use it)", s.key, s.key);
            continue;
        }
        bool off = ListContains(disable, s.key);
        const char* why = off ? (strcmp(source, "FreeCity4.ini") == 0 ? "FreeCity4.ini (disable=)" : "FC4_DISABLE") : "";
        if (!off && s.legacyDisableEnv != nullptr && Env(s.legacyDisableEnv) == "1") {
            off = true;
            why = s.legacyDisableEnv;
        }
        g_enabled[i] = !off;
        if (off) {
            if (strcmp(g_disabledList, "-") == 0) g_disabledList[0] = 0;
            if (g_disabledList[0] != 0) strncat_s(g_disabledList, ",", _TRUNCATE);
            strncat_s(g_disabledList, s.key, _TRUNCATE);
        }
        if (off) {
            Log("subsystem '%s': DISABLED by %s; the original classes are used", s.key, why);
        } else {
            Log("subsystem '%s': enabled (%u class%s replaced)%s", s.key, static_cast<unsigned>(s.classCount),
                s.classCount == 1 ? "" : "es",
                s.defaultOn ? ""
                            : (strcmp(enableSource, "FreeCity4.ini") == 0 ? " by FreeCity4.ini (enable=)"
                                                                         : " by FC4_ENABLE"));
        }
    }
    // kSubsystems[0] is the simulator, [1] the budget, [5] demand.
    BudgetShadowStartup(g_enabled[0], g_enabled[1]);
    DemandShadowStartup(g_enabled[0], g_enabled[5]);
    // The pollution shadow (FC4PollutionShadow.h) and the protection grid shadow
    // (FC4ProtectionShadow.h; it compares the original simulators), by key.
    for (size_t i = 0; i < kSubsystemCount; ++i) {
        if (strcmp(kSubsystems[i].key, "pollution") == 0) PollutionShadowStartup(g_enabled[0], g_enabled[i]);
        if (strcmp(kSubsystems[i].key, "protection") == 0) ProtectionShadowStartup(g_enabled[i]);
    }
    const auto enabledKey = [](const char* key) {
        for (size_t i = 0; i < kSubsystemCount; ++i) {
            if (strcmp(kSubsystems[i].key, key) == 0) return g_enabled[i];
        }
        return false;
    };
    CrimeShadowStartup(g_enabled[0], enabledKey("crime"));
    // The aura shadow (FC4AuraShadow.h; it follows the original aura simulator).
    AuraShadowStartup(g_enabled[0], enabledKey("aura"));
    // The flora census shadow (FC4FloraShadow.h; it compares the original flora simulator).
    FloraShadowStartup(g_enabled[0], enabledKey("flora"));
    // The landfill shadow (FC4LandfillShadow.h; it compares the original landfill developer).
    LandfillShadowStartup(g_enabled[0], enabledKey("landfill"));
    // The power shadow (FC4PowerShadow.h; it compares the original power simulator).
    PowerShadowStartup(g_enabled[0], enabledKey("power"));
    // The plumbing shadow (FC4PlumbingShadow.h; it compares the original plumbing simulator).
    PlumbingShadowStartup(g_enabled[0], enabledKey("plumbing"));
    // The land value and flammability grid shadows (FC4LandValueShadow.h,
    // FC4FlammabilityShadow.h; they compare the original simulators).
    LandValueShadowStartup(enabledKey("landvalue"));
    FlammabilityShadowStartup(enabledKey("flammability"));
    // The residential shadow (FC4ResidentialShadow.h; it compares the original residential simulator).
    ResidentialShadowStartup(g_enabled[0], enabledKey("residential"));
    ZonesShadowStartup(enabledKey("zones"));  // zones_shadow = 1 only
    // The neighbours shadow (FC4NeighborsShadow.h; it compares the original neighbours simulator).
    NeighborsShadowStartup(g_enabled[0], enabledKey("neighbors"));
    // The development shadow (FC4DevelopmentShadow.h; it compares the original BDS and growth developers).
    DevelopmentShadowStartup(g_enabled[0], enabledKey("development"));
    // The tract developer shadow (FC4TractShadow.h; it compares the original tract developer).
    TractShadowStartup(enabledKey("tract"));
    // The lot developer shadow (FC4LotDevShadow.h; it compares the original lot developer).
    LotDevShadowStartup(enabledKey("lotdev"));
    // The commercial shadow (FC4CommercialShadow.h; it compares the original commercial simulator).
    CommercialShadowStartup(g_enabled[0], enabledKey("commercial"));
    // The civic shadow (FC4CivicShadow.h; it compares the original civic building simulator).
    CivicShadowStartup(g_enabled[0], enabledKey("civic"));
    // The parks shadow (FC4ParksShadow.h; it compares the original park manager).
    ParksShadowStartup(g_enabled[0], enabledKey("parks"));
    // The disasters shadow (FC4DisasterShadow.h; it compares the original disaster layer, fire and riot).
    DisastersShadowStartup(enabledKey("disasters"));
    // The zone developer shadow (FC4ZoneDevShadow.h; it compares the original lot manager).
    ZoneDevShadowStartup(enabledKey("zonedev"));
    // The history warehouse shadow (FC4HistoryShadow.h; it follows the original warehouse).
    HistoryShadowStartup(enabledKey("history"));
    // The 24-hour clock shadow (FC4Clock24Shadow.h; it follows the original clock).
    Clock24ShadowStartup(enabledKey("clock24"));
    // The traffic shadow (FC4TrafficShadow.h; it compares the original traffic simulator).
    TrafficShadowStartup(g_enabled[0], enabledKey("traffic"));
    // The network manager shadow (FC4NetMgrShadow.h; it follows the original network managers).
    NetMgrShadowStartup();
    // The occupant manager shadow (FC4OccMgrShadow.h; it follows the original occupant manager).
    OccMgrShadowStartup();
    // The prop developer shadow (FC4PropDevShadow.h; it compares the original prop developer).
    PropDevShadowStartup(g_enabled[0], enabledKey("propdev"));
    // The city detail shadow (FC4CityDetailShadow.h; it compares the original city detail manager).
    CityDetailShadowStartup(enabledKey("citydetail"));
    // The ports shadow (FC4PortsShadow.h; it compares the original airport and seaport developers).
    PortsShadowStartup(g_enabled[0], enabledKey("ports"));
    // The terrain shadow (FC4TerrainShadow.h; it compares the original terrain, never replaced).
// glue18: with `enable = terrain` there is no original terrain left, so the
    // shadow goes inactive instead of comparing ours with itself.
    TerrainShadowStartup(enabledKey("terrain"));
    // The glue17 lane's three shadows (they compare the original city situation
    // manager, demolition utility and dispatch manager).
    CitySitShadowStartup(enabledKey("citysit"));
    DemolitionShadowStartup(enabledKey("demolition"));
    DemolitionPriceShadowStartup(enabledKey("demolition"));  // the glue23 lane
    DispatchShadowStartup(enabledKey("dispatch"));
    // The prop manager, lot configuration manager and tutorial shadows (FC4PropLotShadow.h; the glue15 lane).
    PropLotShadowStartup(enabledKey("propmgr"), enabledKey("lotcfg"), enabledKey("tutorial"));
    // The effects shadow (FC4EffectsShadow.h; a same-process mirror of the
    // effects manager). It runs beside whichever manager the city uses.
    EffectsShadowStartup(enabledKey("effects"));
    TerrainShadowStartup();
    // The automata shadow (FC4AutomataShadow.h; it compares the original automata components).
    AutomataShadowStartup(enabledKey("automata"));
    // The MySims shadow (FC4MySimsShadow.h; it compares the original MySim agent + advisors).
    MySimsShadowStartup();
    // The data-formats shadow (FC4FormatsShadow.h; glue-formats4): core DBPF/exemplar readers vs the game.
    FormatsShadowStartup();
    // The exemplar/cohort factory replacement (FC4ExemplarFactory.h; glue-exemplars4).
    ExemplarFactorySetEnabled(enabledKey("exemplars"));
    // The turbo clock (FC4Turbo.h; tooling): only our simulator has it.
    TurboReload(g_enabled[0]);
    // The network tool registry (glue29): with `enable = nettool` the plugin's
    // own tool callers take replacement tools instead of the manager's.
    NetToolRegistry::SetEnabled(enabledKey("nettool"));
    if (NetToolRegistry::Enabled()) {
        Log("nettool: registry active - scripted 'network' action and zonedev street tools "
            "will use the fc4_nettool core");
    }
    // Scripted actions (FC4Script.h; tooling): the whole file is parsed now.
    ScriptReload();
}

}  // namespace

const char* DisabledSubsystemsList() { return g_disabledList; }

bool SimulatorReplaced() {
    const Subsystem* owner = nullptr;
    return FindEnabledClass(0x2990C1E5, &owner) != nullptr;
}

const Subsystem* Subsystems(size_t* count) {
    if (count != nullptr) *count = kSubsystemCount;
    return kSubsystems;
}

bool SubsystemEnabled(const Subsystem& s) {
    Resolve();
    for (size_t i = 0; i < kSubsystemCount; ++i) {
        if (&kSubsystems[i] == &s) return g_enabled[i];
    }
    return false;
}

const ClassEntry* FindEnabledClass(uint32_t clsid, const Subsystem** owner) {
    Resolve();
    for (size_t i = 0; i < kSubsystemCount; ++i) {
        if (!g_enabled[i]) continue;
        // The effects replacement fails closed (glue22 question 9): the class is
        // registered only when the run also sets effects_replace = 1.
        if (strcmp(kSubsystems[i].key, "effects") == 0) {
            const long force = SettingInt("effects_replace", "FC4_EFFECTS_REPLACE", 0);
            if (force == 0) {
                static bool warned = false;
                if (!warned) {
                    warned = true;
                    Log("subsystem 'effects': the class replacement is NOT registered (glue22 question 9); "
                        "effects_shadow = 1 is the verified path; set effects_replace = 1 to force it");
                }
                continue;
            }
        }
        const Subsystem& s = kSubsystems[i];
        for (size_t k = 0; k < s.classCount; ++k) {
            if (s.classes[k].clsid == clsid) {
                if (owner != nullptr) *owner = &s;
                return &s.classes[k];
            }
        }
    }
    return nullptr;
}

}  // namespace fc4
