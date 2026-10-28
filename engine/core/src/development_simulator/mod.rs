// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The building development simulator (`cSC4BuildingDevelopmentSimulator`,
//! "BDS", class `0x8990BE7A`), `docs/specs/building-development.md`.
//! Section references (`§n`) point at that spec.
//!
//! # Structure
//!
//! - [`DevelopmentSim`]: the state of §2, Init / Shutdown (§1.3, §1.4), the
//!   developer map (§1.5), the agents and messages of §3, the monthly
//!   construction budget and map rebuild (§4.1), the `DoIdle` state machine
//!   (§4.2), decline planning and execution (§4.3, §4.5), the daily `Grow`
//!   (§4.4), construction resources (§4.6), `IsSuitableForConstruction`
//!   (§4.8), placing buildings (§4.9), `DoBuildingChanged` (§4.10),
//!   `DoTerrainChanged` (§4.11), special buildings, cheats and PlaceBuilding
//!   (§4.12), tuning (§4.13), persistence (§6) and the slot behaviour of §7
//!   as plain methods.
//! - [`BdsProcess`]: the **process-wide** statics (§2): the building
//!   catalogue ([`BuildingCatalogue`], §4.7), the tried-tract marks (§4.4)
//!   and the file-local `cRZRandom` (§4.14). The host keeps one per process
//!   and hands it ([`SharedProcess`]) to every BDS it creates.
//! - [`GrowthDevelopers`]: the growth developers (map values,
//!   [`DeveloperId`] tokens), a separate object whose operations map one to
//!   one onto `crate::growth_developer::GrowthDeveloper`, so the developer
//!   map, `CreateDeveloper` and the developers can be replaced as one unit.
//! - [`DevelopmentHost`]: everything else outside the BDS. The tract
//!   developer, the lot developer, lots, the lot manager, the zone manager,
//!   land value, power, the occupant manager, terrain, demolition, budget,
//!   advisors, the framework servers, the demand simulator's nodes and the
//!   resources are host calls. The building map is created and resolved
//!   through the [`GridProvider<u8>`] supertrait (simgrid §6.4); the
//!   construction node is a [`ConstructionNode`] the host creates (a core
//!   [`DemandNode`] in tests and the conformance runner, the game's
//!   `cISC4Demand` in the plugin).
//! - Pure helpers: [`unpack_terrain_rect`], [`monthly_supply`],
//!   [`decline_day`], [`tuning_min_max`]; the developer-type, key, zone and
//!   category-flag helpers are `crate::development`'s.
//!
//! Host tokens are opaque (`cISC4GrowthDeveloper*`, `cISC4Lot*`,
//! `cISC4Occupant*` in the plugin); 0 is never a live object. Host callbacks
//! must not call back into the BDS.
//!
//! # Arithmetic
//!
//! Foundations §18: every float operation is one `f32` rounding in the
//! spec's order; `trunc` is `_ftol` (`crate::x87::ftol`, low 32 bits where
//! the spec keeps 32); `0.5`, `0.001` and the 5° limit are `float32`
//! constants.
//!
//! # Contracts of other components
//!
//! `GetLot(x, z, true)`, `GetLotsInCellRect(…, false)`, `GetZoneType` and
//! `PlaceZone` follow `zones.md`; `GetLotSlope`,
//! `GetLowestOrHighestAreaAltitude` (slot 16) and `UpdateLotElements` follow
//! `lot-developer.md`. Each is one host call with the arguments of
//! §4.8–§4.11 ([`DevelopmentHost::dezone`] carries only the rectangle and
//! the one variable flag).
//!
//! - `UpdateLotElements(lot, kind)` receives the BDS's whole 32-bit `kind`
//!   (§4.11 *[compat]*, lot-developer.md §6.4).
//! - *[inferred]* `DemolishLots` (no demolition spec yet) is one host call
//!   that returns the cost it writes through its out argument, and it is
//!   called even with an empty queue (§4.11 as written).
//! - *[inferred]* `IsBuildingTypeValid` and the type enumerations use the
//!   building-key map (only building exemplars fill it); a density-table
//!   type "loads" (§4.7.4) exactly when it has a key, and its occupant size
//!   is the one its profile read at catalogue time.
//! - *[inferred]* Foundations and retaining walls are stored as instance →
//!   key, like buildings.
//! - *[inferred]* `AddBuildingData` for a type seen before replaces its key,
//!   names and profile and appends it to its families again.
//! - `PlopBuilding` passes the profile's orientation (+0x18, §4.9
//!   *[compat]*).
//! - *[inferred]* The orientation 1/3 box swap exchanges the x and z
//!   components of both corners.
//!
//! # Choices made where the spec is open (`[choice]`)
//!
//! - Seeding (§4.14): [`BdsProcess::from_session`] seeds the generator from
//!   the session seed per foundations §6.3 *[choice]* (`derive_seed(S,
//!   0x0C09C0B8)`); [`BdsProcess::new`] and [`BdsProcess::reseed`] take an
//!   exact seed (tests and replays). [`clock_seed`] is the original's
//!   `cRZTime() XOR 0x0C09C0B8` for a shim that wants it.
//! - Grow (§4.4): a deterministic work budget instead of the original's
//!   3 ms timer (no timer, no spin, no step adaptation): at most
//!   [`DevelopmentSim::grow_attempt_budget`] attempts (tract-developer
//!   `Grow` calls) per day; the day ends when the budget is used up or every
//!   qualifying developer has made its one attempt, whichever comes first.
//!   A pick whose lists are invalid or whose growth list is empty is not an
//!   attempt (no `Grow` call). The budget is checked before each draw, so a
//!   spent budget also skips the final `RandomUint32Uniform(0)`. The default, [`DEFAULT_GROW_ATTEMPT_BUDGET`] = `u32::MAX`, is
//!   effectively unlimited until it is calibrated from in-game measurement.
//!   If a pick's `GetSubType()` is not a pending key (the original would
//!   re-pick the developer until its timer expires), the day's loop ends
//!   after that attempt. If a draw picks nobody (only possible when the
//!   signed weights overflow), the loop ends.
//! - A missing census node (§4.1) counts as 0; a developer without a source
//!   node has active demand 0 (does not qualify for growth or decline). No
//!   construction node: the monthly supply is not set, `GetTotal` and
//!   `GetFree` return 0, `Claim` returns false and `Release` returns false.
//! - `GetStagePopulationMultiplier` returns 1.0 outside the vector (§4.13).
//! - Tuning (§4.13): every code default, the stage vector included, is
//!   assigned before the read. `0x47E2C380` is read as the little-endian
//!   bytes of its values (each element at its own width, floats as their
//!   bits); a word with fewer than 4 bytes behind it keeps its default.
//! - Save (§6): references are written per foundations §2.7; a null or
//!   unresolvable developer or special-building reference is not inserted;
//!   a developer read for a type already in the map releases the old one
//!   (also when it is the same object: each slot holds one reference);
//!   the planned-growth map (always empty in the shipped game) is kept in
//!   key order. A null grid or node reference leaves the BDS without one,
//!   so `Init` creates a new one.
//! - PlaceBuilding with fewer than three parameters does nothing.
//! - `AddNewBuildingOccupant` releases its creation reference after the
//!   insertion and returns the occupant only when the insertion succeeded.
//! - Loops over a lot's bounding rectangle are clamped to the building
//!   map; the decline plan of one month is capped at [`MAX_PLAN_ENTRIES`].
//! - The angle test of §4.11 is `r24(A − B) ≤ 5°` as written (a NaN fails it,
//!   so the change counts as too big).
//! - Debug features: debug growth is never set; the coffee-bar flag is only
//!   stored; the dead growth-plan path (`PlanGrowth`, `ExecuteGrowthPlan`)
//!   is not implemented (its flag is never set, §10.1 Q1).
//! - `Release` from a count ≤ 1 calls `Shutdown`, returns 0 and marks the
//!   object destroyed.

pub mod archive;
mod catalogue;
mod simulator;
#[cfg(test)]
mod tests;

pub use catalogue::{BuildingCatalogue, BuildingConfigObject, LotConfigInfo};
pub use simulator::{
    DevelopmentData, DevelopmentSim, PlanEntry, PlannedGrowth, Tuning, DEFAULT_GROW_ATTEMPT_BUDGET, RECORD_VERSION,
};

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

pub use crate::demand::DemandNode;
pub use crate::development::{ConstructionProfile, GrowthItem, GrowthProfile, Outcome};
pub use crate::grid::{GridProvider, TractGrid};
use crate::property::PropertyHolder;
use crate::rng::{RzRandom, SeedSource};
use crate::stream::SaveObject;
use crate::stream::{RefValue, ResKey};

/// Class and interface IDs (§1.1, §6).
pub const CLSID_BDS: u32 = 0x8990_BE7A;
pub const IID_BDS: u32 = 0x0781_3EAE;
pub const IID_UNKNOWN: u32 = 1;
pub const IID_MESSAGE_TARGET: u32 = 0xE98B_2F57;
pub const IID_MESSAGE_TARGET2: u32 = 0x4522_94AA;
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;
/// The interfaces references in the record are read with (§6).
pub const IID_SIM_GRID: u32 = 0xC9BD_939A;
pub const IID_DEMAND: u32 = 0x0797_A4E8;
pub const IID_GROWTH_DEVELOPER: u32 = 0xE7CD_D662;
pub const IID_OCCUPANT: u32 = 0x26BB_767E;

/// The building map (§1.3 step 6): `cISC4SimGrid<uint8>`.
pub const BUILDING_MAP_CLSID: u32 = 0x49B9_E602;
pub const BUILDING_MAP_INSTANCE: u32 = 0x49D5_B678;
pub const BUILDING_MAP_TRACT_SIZE: i32 = 1;

/// The construction node's values when created (§1.3 step 7).
pub const NEW_NODE_SUPPLY: f32 = 100.0;
pub const NEW_NODE_DEMAND: f32 = 0.0;

/// Which sub-object `QueryInterface` returns (§1.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BdsInterface {
    /// `cISC4BuildingDevelopmentSimulator` and `cIGZUnknown`.
    Primary,
    /// `cIGZMessageTarget` (+4; its `DoMessage` returns true, nothing else).
    MessageTarget,
    /// `cIGZMessageTarget2` (+8).
    MessageTarget2,
    /// `cIGZSerializable` (+0xC).
    Serializable,
}

/// `QueryInterface` (§1.1): `None` = false, the out pointer unchanged.
pub fn query_interface(iid: u32) -> Option<BdsInterface> {
    match iid {
        IID_BDS | IID_UNKNOWN => Some(BdsInterface::Primary),
        IID_MESSAGE_TARGET => Some(BdsInterface::MessageTarget),
        IID_MESSAGE_TARGET2 => Some(BdsInterface::MessageTarget2),
        IID_SERIALIZABLE => Some(BdsInterface::Serializable),
        _ => None,
    }
}

/// Agent registration (§1.3 step 9).
pub const AGENT_NAME: &str = "cSC4BuildingDevelopmentSimulator";
pub const AGENT_LEVELS: [u32; 4] = [0, 4, 6, 10];
pub const AGENT_FLAGS: u32 = 2;

/// The PlaceBuilding command (§1.3 step 8, §4.12).
pub const PLACE_BUILDING_COMMAND: u32 = 0x0B99_AE81;
pub const PLACE_BUILDING_NAME: &str = "PlaceBuilding";

/// The cheats (§1.3 step 10), in registration order.
pub mod cheat {
    pub const HOW_DRY_I_AM: u32 = 0x4918_65E1;
    pub const FIGHT_THE_POWER: u32 = 0x08DA_D89A;
    pub const BARSTUCK: u32 = 0x6AB6_A2D7;
    pub const ALL: [(u32, &str); 3] = [
        (HOW_DRY_I_AM, "HowDryIAm"),
        (FIGHT_THE_POWER, "FightThePower"),
        (BARSTUCK, "Barstuck"),
    ];
}

/// Messages (§3, §5).
pub mod msg {
    /// Agent levels (§3.1).
    pub const SIM_BEGIN: u32 = 0x6695_6810;
    pub const SIM_NEW_DAY: u32 = 0x6695_6814;
    pub const SIM_NEW_MONTH: u32 = 0x6695_6816;
    pub const SIM_IDLE: u32 = 0x6695_681A;
    /// Handled (§3.2).
    pub const CHEAT: u32 = 0x230E_27AC;
    pub const PLACE_BUILDING: u32 = super::PLACE_BUILDING_COMMAND;
    pub const TERRAIN_CHANGED: u32 = 0x278E_6003;
    pub const OCCUPANT_INSERTED: u32 = 0x99EF_1142;
    pub const OCCUPANT_REMOVED: u32 = 0x99EF_1143;
    pub const STATE_CHANGED: u32 = 0xE8BD_A2E7;
    /// The subscriptions of `Init` step 8 (and `Shutdown`), in order.
    pub const SUBSCRIPTIONS: [u32; 5] = [
        STATE_CHANGED,
        TERRAIN_CHANGED,
        OCCUPANT_INSERTED,
        OCCUPANT_REMOVED,
        PLACE_BUILDING,
    ];
    /// Posted once per process (§5).
    pub const BUILDING_TYPES_UPDATED: u32 = 0xA783_EBC8;
}

/// Properties (§1.5, §3.2, §4.7.1, §4.9, §4.13).
pub mod prop {
    pub const EXEMPLAR_TYPE: u32 = 0x10;
    pub const EXEMPLAR_NAME: u32 = 0x20;
    /// The developer type of a developer exemplar.
    pub const DEVELOPER_TYPE: u32 = 0x21;
    /// Localized building name (`SC4LoadExemplarString`).
    pub const LOCALIZED_NAME: u32 = 0x8A41_6A99;
    pub const BUILDING_NAME: u32 = 0x899A_FBAD;
    pub const WEALTH: u32 = 0x2781_2832;
    pub const PURPOSE: u32 = 0x2781_2833;
    pub const CAPACITY_SATISFIED: u32 = 0x2781_2834;
    pub const FLAG_27812836: u32 = 0x2781_2836;
    pub const VALUE_499AFA38: u32 = 0x499A_FA38;
    pub const CATEGORIES: u32 = 0xAA1D_D396;
    pub const OCCUPANT_SIZE: u32 = 0x2781_2810;
    /// A foundation's maximum height (float32, §4.9.2 slot 26).
    pub const FOUNDATION_MAX_HEIGHT: u32 = 0x88FC_C3E3;
    pub const PLOP_COST: u32 = 0x49CA_C341;
    pub const FAMILIES: u32 = 0x2781_2870;
    /// The preview-occupant marker (tested for presence).
    pub const PREVIEW_OCCUPANT: u32 = 0x0292_44DB;
    /// Demolition cost of a building occupant (int64).
    pub const DEMOLITION_COST: u32 = 0x099A_FACD;
    // ---- tuning (§4.13) ----
    pub const CONSTRUCTION_MIN_MAX: u32 = 0x47E2_C380;
    pub const PER_RESIDENT: u32 = 0x47E2_C381;
    pub const BIAS: u32 = 0x47E2_C382;
    pub const STAGE_MULTIPLIER: u32 = 0x47E2_C383;
}

/// ExemplarType values (§1.3).
pub mod exemplar_type {
    pub const BUILDING: u32 = 2;
    pub const DEVELOPER: u32 = 4;
    pub const FOUNDATION: u32 = 0x11;
    pub const RETAINING_WALL: u32 = 0x15;
}

/// The "simulators" tuning exemplar (§4.13).
pub const TUNING_KEY: ResKey = ResKey {
    type_id: 0x6534_284A,
    group: 0xE7E2_C2DB,
    instance: 0xE8DA_7677,
};

/// Census nodes read monthly (§4.1): R, C, I.
pub const CENSUS_R: u32 = 0x1001;
pub const CENSUS_C: u32 = 0x3001;
pub const CENSUS_I: u32 = 0x4001;

/// Occupant groups of special buildings (§3.2) and the building occupant
/// type (§4.9).
pub const SPECIAL_GROUPS: [u32; 2] = [0x150A, 0x150B];
pub const OCCUPANT_TYPE_BUILDING: u32 = 0x2781_28A0;

/// The CityPlanning advisor event (§4.10).
pub const CITY_PLANNING_ADVISOR: u32 = 0x17;
pub const HIGHEST_STAGE_EVENT: u32 = 0x4A64_A9CC;

/// Decline thresholds (§4.3, §4.5) and the growth threshold (§4.4).
pub const DECLINE_PLAN_THRESHOLD: i32 = -20;
pub const GROWTH_THRESHOLD: i32 = 5;
/// Agriculture never declines (§4.3).
pub const AGRICULTURE: u32 = 5;

/// The terrain-change test (§4.11): 5° in radians, float32 `0x3DB2B8C3`.
pub const MAX_SLOPE_CHANGE: f32 = 0.087_266_47;

/// [choice] The most decline-plan entries one plan holds.
pub const MAX_PLAN_ENTRIES: usize = 1 << 20;

/// The original's seed for the file-local generator (§4.14).
pub const CLOCK_SEED_XOR: u32 = crate::rng::file_seed::BUILDING_DEVELOPMENT;

/// `cRZTime() XOR 0x0C09C0B8` (§4.14), for a shim that seeds like the
/// original.
pub fn clock_seed(time: u32) -> u32 {
    crate::rng::clock_seed(time, CLOCK_SEED_XOR)
}

/// A growth developer (`cISC4GrowthDeveloper*` in the plugin).
pub type DeveloperId = u64;
/// An occupant (`cISC4Occupant*`).
pub type OccupantId = u64;
/// A lot (`cISC4Lot*`).
pub type LotId = u64;
/// An inclusive cell rectangle `(x1, z1, x2, z2)`.
pub type CellRect = (i32, i32, i32, i32);
/// A position or box corner (x, y, z), metres.
pub type Vec3 = [f32; 3];

/// One command parameter (`cIGZVariant`) as PlaceBuilding reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CommandParam {
    pub as_u32: u32,
    pub as_f32: f32,
}

/// A message as `DoMessage` sees it (§3).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Message {
    pub msg_type: u32,
    pub data1: u32,
    pub data2: u32,
    pub data3: u32,
    pub data4: u32,
    /// `GetVoid1()` as an occupant (occupant inserted / removed).
    pub occupant: Option<OccupantId>,
    /// `GetIGZUnknown()` as a lot (StateChanged).
    pub lot: Option<LotId>,
    /// The command's parameters (PlaceBuilding).
    pub params: Vec<CommandParam>,
}

impl Message {
    pub fn of_type(msg_type: u32) -> Message {
        Message {
            msg_type,
            ..Message::default()
        }
    }
}

/// The advice subject §4.10 builds and then discards (the event itself is
/// pushed with a null subject *[quirk]*).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AdviceSubject {
    /// `r24(pos.x / CellWidthX)`, `r24(pos.z / CellWidthZ)`.
    pub x: f32,
    pub z: f32,
    pub lot: LotId,
    /// `stage << 24 | developerType`.
    pub data1: u32,
}

/// The growth developers as the BDS uses them (§1.5, §4.1–§4.5, §6).
///
/// The developer map, `CreateDeveloper` and the developers' `Init` and
/// `Shutdown` are replaced together with the growth developer as one unit,
/// so every operation maps one to one onto a method of
/// `crate::growth_developer::GrowthDeveloper`:
///
/// | here | growth developer core |
/// |---|---|
/// | `create_developer` | `GrowthDeveloper::new` + `add_ref` (the BDS's reference) |
/// | `developer_init` | `init(Some(exemplar))` |
/// | `developer_shutdown` | `shutdown()` |
/// | `developer_release` | `release()` |
/// | `developer_sub_type` | `developer_type()` (`GetSubType`, slot 6) |
/// | `developer_purpose` | `purpose()` (slot 7) |
/// | `developer_lists_valid` | `lists_valid()` (slot 10) |
/// | `developer_growth_list` / `developer_decline_list` | `growth_list()` / `decline_list()` (slots 11 / 12), copied |
/// | `developer_source_demand` | `source_demand()` (slot 18) |
/// | `developer_start_map_update` | `start_map_update()` (slot 27) |
/// | `developer_map_update_finished` | `is_map_update_finished()` (slot 28) |
/// | `developer_ref` / `resolve_developer` | the developer as a `SaveObject` (`RefValue::Object`, class `0x0990C138`) / `RefValue::object::<GrowthDeveloper>()` |
///
/// Both receive the exemplar's resource key too: a host whose developers
/// are game objects passes them the game's own holder of that key.
pub trait GrowthDevelopers {
    /// A new `cSC4GrowthDeveloper` (§1.5) holding one reference for the
    /// BDS; `exemplar` is the developer exemplar it is about to be
    /// `Init`ed with.
    fn create_developer(&self, key: &ResKey, exemplar: &Rc<PropertyHolder>) -> Option<DeveloperId>;
    /// `Init(holder)` (slot 3).
    fn developer_init(&self, dev: DeveloperId, key: &ResKey, exemplar: &Rc<PropertyHolder>) -> bool;
    /// `Shutdown()` (slot 4).
    fn developer_shutdown(&self, dev: DeveloperId) -> bool;
    /// `Release()`.
    fn developer_release(&self, dev: DeveloperId);
    /// `GetSubType()` (slot 6): the developer type.
    fn developer_sub_type(&self, dev: DeveloperId) -> u32;
    /// `GetPurpose()` (slot 7).
    fn developer_purpose(&self, dev: DeveloperId) -> u32;
    /// `GrowthAndDeclineListsAreValid()` (slot 10).
    fn developer_lists_valid(&self, dev: DeveloperId) -> bool;
    /// A copy of `GetGrowthList()` (slot 11).
    fn developer_growth_list(&self, dev: DeveloperId) -> Vec<GrowthItem>;
    /// A copy of `GetDeclineList()` (slot 12).
    fn developer_decline_list(&self, dev: DeveloperId) -> Vec<GrowthItem>;
    /// `GetSourceDemand()` (slot 18): the demand node, `None` without one.
    fn developer_source_demand(&self, dev: DeveloperId) -> Option<u64>;
    /// `StartMapUpdate()` (slot 27).
    fn developer_start_map_update(&self, dev: DeveloperId);
    /// `IsMapUpdateFinished()` (slot 28).
    fn developer_map_update_finished(&self, dev: DeveloperId) -> bool;
    /// How a save refers to the developer.
    fn developer_ref(&self, dev: DeveloperId) -> RefValue;
    /// The developer a save reference (read with
    /// [`IID_GROWTH_DEVELOPER`]) points at; it holds the reference the
    /// stream handed over.
    fn resolve_developer(&self, r: &RefValue) -> Option<DeveloperId>;
}

/// Everything the BDS reads from, or does to, the rest of the game.
pub trait DevelopmentHost: GridProvider<u8> {
    // ---- clock, city, terrain ----
    /// Today's day number (`GetSimDate()` → `cIGZDate` slot 26).
    fn sim_day(&self) -> u32;
    /// `cISC4City::GetDifficultyLevel()` (0 easy, 1 medium, 2 hard).
    fn difficulty(&self) -> u32;
    /// `CellWidthX()`, `CellWidthZ()` (city slots 91/92).
    fn cell_widths(&self) -> (f32, f32);
    /// The terrain's cell counts (slots 7 and 8), cached at `Init`.
    fn terrain_cell_counts(&self) -> (i32, i32);
    /// `PositionToCell(x, z)` (city slot 95).
    fn position_to_cell(&self, x: f32, z: f32) -> (i32, i32);
    /// `CellCenterToPosition(cx, cz)` (city slot 97): (x, z).
    fn cell_center(&self, cx: i32, cz: i32) -> (f32, f32);
    /// `cISTETerrain::GetAltitude(x, z)`.
    fn altitude(&self, x: f32, z: f32) -> f32;

    // ---- resources ----
    /// Every exemplar (type `0x6534284A`) in the resource key list's
    /// enumeration order.
    fn exemplars(&self) -> Vec<(ResKey, Rc<PropertyHolder>)>;
    /// The exemplars whose ExemplarType (property `0x10`) is one of `types`,
    /// in the same order (§1.3 steps 1 and 2 keep only those). A host that
    /// converts exemplars on demand can skip the others.
    fn exemplars_of_types(&self, types: &[u32]) -> Vec<(ResKey, Rc<PropertyHolder>)> {
        self.exemplars()
            .into_iter()
            .filter(|(_, h)| crate::props::u32_of(h, prop::EXEMPLAR_TYPE).is_some_and(|t| types.contains(&t)))
            .collect()
    }
    /// `SC4LoadExemplarString(holder, id)`: the localized string the
    /// property refers to.
    fn exemplar_string(&self, holder: &PropertyHolder, id: u32) -> Option<Vec<u8>>;
    /// The lot configurations (`GetLotConfigurationMap`, §4.7.4).
    fn lot_configurations(&self) -> Vec<LotConfigInfo>;
    /// The "simulators" exemplar ([`TUNING_KEY`]) if it loads.
    fn tuning(&self) -> Option<Rc<PropertyHolder>>;

    // ---- framework ----
    /// Registers the static-data cleanup callback (first `Init` of the
    /// process; the callback is [`BdsProcess::app_shutdown`]).
    fn register_static_cleanup(&self);
    /// Posts `kSC4MessageBuildingTypesUpdated` (void1 null, data2 = count).
    fn post_building_types_updated(&self, count: u32);
    fn subscribe(&self, msg_types: &[u32]);
    fn unsubscribe(&self, msg_types: &[u32]);
    fn register_command(&self, id: u32, name: &str);
    fn unregister_command(&self, id: u32);
    /// `AddAgent(target, level, name, flags)`.
    fn add_agent(&self, level: u32, name: &str, flags: u32);
    /// `RemoveAgent(target)`.
    fn remove_agent(&self);
    /// `RegisterCheatCode` and `AddNotification2`.
    fn register_cheat(&self, id: u32, name: &str);
    /// `RemoveNotification2` and `UnregisterCheatCode`.
    fn unregister_cheat(&self, id: u32);
    /// `Init` step 5: hold the services.
    fn acquire_services(&self);
    /// `Shutdown`: release the message server and the services.
    fn release_services(&self);

    // ---- demand ----
    /// `demandSim.GetDemand(id, 0).QuerySupplyValue()`; `None` without the
    /// node.
    fn census_supply(&self, id: u32) -> Option<f32>;
    /// A new `cSC4Demand` (the construction node), not yet `Init`ed.
    fn create_construction_node(&self) -> Option<Rc<dyn ConstructionNode>>;
    /// The node a save reference (read with [`IID_DEMAND`]) points at; it
    /// holds the reference the stream handed over.
    fn resolve_construction_node(&self, r: &RefValue) -> Option<Rc<dyn ConstructionNode>> {
        r.object::<DemandNode>().map(|n| n as Rc<dyn ConstructionNode>)
    }

    /// `QueryActiveDemandValue()` of the demand node a developer's
    /// [`GrowthDevelopers::developer_source_demand`] named.
    fn demand_active_value(&self, node: u64) -> f32;

    // ---- tract developer ----
    /// `Grow(profile, outcomes, flag)` (slot 5): returns the built capacity.
    /// `outcomes` arrives empty and is appended to.
    fn tract_grow(&self, profile: &GrowthProfile, outcomes: &mut Vec<Outcome>, flag: &mut bool) -> i32;
    /// `Decline(profile)` (slot 6).
    fn tract_decline(&self, profile: &GrowthProfile) -> i32;

    // ---- lots, lot manager, lot developer ----
    /// `GetBuildingType(false)` (slot 46).
    fn lot_building_type(&self, lot: LotId) -> u32;
    /// `GetBoundingRect()` (inclusive cells).
    fn lot_bounding_rect(&self, lot: LotId) -> CellRect;
    /// `GetLotConfiguration().GetGrowthStage()`; `None` without a
    /// configuration.
    fn lot_growth_stage(&self, lot: LotId) -> Option<u8>;
    /// `GetBuilding()`.
    fn lot_building(&self, lot: LotId) -> Option<OccupantId>;
    /// `GetState()`.
    fn lot_state(&self, lot: LotId) -> u32;
    /// `GetCenter()`: (x, y, z).
    fn lot_center(&self, lot: LotId) -> Vec3;
    /// `GetSlope()`.
    fn lot_slope(&self, lot: LotId) -> f32;
    /// `IsInDemolishableState()`.
    fn lot_demolishable(&self, lot: LotId) -> bool;
    /// `lotManager.GetLot(x, z, originOnly = true)` (zones.md, lot manager
    /// slot 6): null outside the city; else the cell's lot only if its
    /// `GetLocation()` is `(x, z)`. No reference is taken.
    fn lot_at(&self, x: i32, z: i32) -> Option<LotId>;
    /// `lotManager.GetLotsInCellRect(list, x1, z1, x2, z2, originOnly =
    /// false)` (zones.md, slot 7): the rectangle clamped to the city, x outer
    /// and z inner, each lot once, in that order. The references it hands
    /// over are the host's to release.
    fn lots_in_cell_rect(&self, rect: CellRect) -> Vec<LotId>;
    /// `lotDeveloper.GetLotSlope(lot, water = false)` (lot-developer.md,
    /// slot 30).
    fn lot_developer_slope(&self, lot: LotId) -> f32;
    /// `lotDeveloper.UpdateLotElements(lot, kind)` (lot-developer.md §6.4,
    /// slot 37): the terrain-change kind, the whole 32-bit data4 (§4.11).
    fn update_lot_elements(&self, lot: LotId, kind: u32);
    /// `lotDeveloper.GetLowestOrHighestAreaAltitude(SC4Rect<float>{x1, z1,
    /// x2, z2}, lowest = false, withWater = true)` (lot-developer.md, slot
    /// 16): the highest altitude over the rectangle.
    fn area_altitude(&self, rect: [f32; 4]) -> f32;

    // ---- zones, land value, power, demolition, budget ----
    /// `zoneManager.GetZoneType(x, z)` (zones.md, slot 12): the cell's
    /// int8 zone type (the BDS sign-extends it).
    fn zone_type(&self, x: i32, z: i32) -> i8;
    /// `zoneManager.PlaceZone(region = rect, type 0, commit = true,
    /// skipFunds = flag, checkSlope = false, skipSizeCheck = true, checkLotInside =
    /// true, null, null, null)` (zones.md §6).
    fn dezone(&self, rect: CellRect, flag: bool);
    /// Power simulator slot 19: powered at (x, z).
    fn powered(&self, x: i32, z: i32) -> bool;
    /// `GetLandValueType(x, z)`.
    fn land_value_type(&self, x: i32, z: i32) -> u32;
    /// `GetLandValue(x, z)`.
    fn land_value(&self, x: i32, z: i32) -> u8;
    /// `DemolishLots(true, lots, 0, 1, true, null, &cost, null)`: the cost.
    fn demolish_lots(&self, lots: &[LotId]) -> i64;
    fn total_funds(&self) -> i64;
    fn set_total_funds(&self, v: i64);

    // ---- occupants ----
    fn has_occupant_manager(&self) -> bool;
    /// `InsertOccupant(occ, false)`.
    fn insert_occupant(&self, occ: OccupantId) -> bool;
    /// `RemoveOccupant(occ, true, false)`.
    fn remove_occupant(&self, occ: OccupantId) -> bool;
    /// Whether an occupant of [`OCCUPANT_TYPE_BUILDING`] covers the cell
    /// (occupant manager slot 33).
    fn building_occupant_at_cell(&self, cx: i32, cz: i32) -> bool;
    /// `AsPropertyHolder()->HasProperty(id)`.
    fn occupant_has_property(&self, occ: OccupantId, id: u32) -> bool;
    /// An int64 property of the occupant.
    fn occupant_property_i64(&self, occ: OccupantId, id: u32) -> Option<i64>;
    /// `IsOccupantGroup(group)` (slot 22).
    fn occupant_in_group(&self, occ: OccupantId, group: u32) -> bool;
    fn occupant_position(&self, occ: OccupantId) -> Vec3;
    /// A new building occupant with one reference for the caller (§4.9.1:
    /// `CreateInstance(0xA9BD882D)` already carries it; no extra AddRef).
    fn create_building_occupant(&self) -> Option<OccupantId>;
    fn set_building_type(&self, occ: OccupantId, t: u32);
    /// `occ` slot 4 `GetBuildingType()` (§4.9.1 loader step 2).
    fn building_type(&self, occ: OccupantId) -> u32;
    /// The exemplar half of the loaders (§4.9.1 steps 1, 4–5 and 7; §4.9.2
    /// steps 1, 3–5): `GetResource(key, 0xA52160F5, &ex, 0, 0)`; when it
    /// loads, `SetDefaultExemplar(ex)` on the occupant's
    /// `cISCExemplarPropertyHolder` (QI `0x0AC2B5F7`) and the resource's
    /// reference dropped. False when the resource does not load.
    fn set_default_exemplar(&self, occ: OccupantId, key: ResKey) -> bool;
    /// The float32 values of the occupant's property `id`, through its own
    /// holder (which falls back to the default exemplar); `None` when the
    /// property is missing.
    fn occupant_property_f32s(&self, occ: OccupantId, id: u32) -> Option<Vec<f32>>;
    fn occupant_bounding_box(&self, occ: OccupantId) -> (Vec3, Vec3);
    fn set_occupant_bounding_box(&self, occ: OccupantId, min: Vec3, max: Vec3);
    fn set_occupant_position(&self, occ: OccupantId, pos: Vec3);
    fn set_occupant_orientation(&self, occ: OccupantId, orientation: i32);
    fn init_occupant(&self, occ: OccupantId);
    fn add_ref(&self, occ: OccupantId);
    fn release(&self, occ: OccupantId);
    fn object_ref(&self, occ: OccupantId) -> RefValue;
    /// The occupant of a save reference (read with [`IID_OCCUPANT`]).
    fn resolve_occupant(&self, r: &RefValue) -> Option<OccupantId>;

    // ---- foundations and retaining walls (§4.9.2) ----
    /// A new foundation occupant (class `0x49C05C8F`, occupant type
    /// `0x68FD0C69`) with one reference for the caller.
    fn create_foundation_occupant(&self) -> Option<OccupantId>;
    /// `cISC4FoundationOccupant` slot 4 `GetFoundationType()`.
    fn foundation_type(&self, occ: OccupantId) -> u32;
    /// Slot 5 `SetFoundationType(t)`.
    fn set_foundation_type(&self, occ: OccupantId, t: u32);
    /// Slot 7 `SetMaxHeight(v)`.
    fn set_foundation_max_height(&self, occ: OccupantId, v: f32);
    /// Slot 8 `SetSize(size)`.
    fn set_foundation_size(&self, occ: OccupantId, size: Vec3);
    /// Slot 10 `SetUsage(usage)` (stored as a byte).
    fn set_foundation_usage(&self, occ: OccupantId, usage: i32);
    /// Slot 12 `SetOrientation(orientation)` (clamped to 0..3).
    fn set_foundation_orientation(&self, occ: OccupantId, orientation: i32);
    /// `cISC4LotRetainingWallOccupant` slot 11 `GetRetainingWallType()`, the
    /// wall reached from any occupant by its IID `0x295D1162` (§4.9.2, DG35);
    /// 0 when the occupant is not a wall.
    fn retaining_wall_type(&self, occ: OccupantId) -> u32;
    /// `lotDeveloper.GetLowestOrHighestAreaAltitude(SC4Rect<int32>{x1, z1,
    /// x2, z2}, lowest, withWater)` (lot-developer.md, slot 17).
    fn area_altitude_cells(&self, rect: CellRect, lowest: bool, with_water: bool) -> f32;

    // ---- advisors ----
    /// `GetAdvisor(advisor).PushEvent(event, null)`; `discarded` is the
    /// subject the original fills and releases (§4.10).
    fn push_advisor_event(&self, advisor: u32, event: u32, discarded: &AdviceSubject);
}

/// The construction node (§1.3 step 7, §4.6): a `cSC4Demand` (demand spec
/// §3) owned by the BDS. Every method is the `cISC4Demand` slot of BDS §8.
pub trait ConstructionNode {
    /// `Init()` (slot 3).
    fn init(&self);
    /// `Shutdown()` (slot 4).
    fn shutdown(&self);
    /// `QuerySupplyValue()` (slot 8).
    fn supply(&self) -> f32;
    /// `QueryDemandValue()` (slot 9).
    fn demand(&self) -> f32;
    /// `SetSupplyValue(v)` (slot 18).
    fn set_supply(&self, v: f32);
    /// `SetDemandValue(v)` (slot 19).
    fn set_demand(&self, v: f32);
    /// `AddToDemandValue(v)` (slot 17).
    fn add_to_demand(&self, v: f32);
    /// `EndOfCycle()` (slot 30).
    fn end_of_cycle(&self) -> bool;
    /// How the BDS record refers to the node (§6).
    fn save_ref(self: Rc<Self>) -> RefValue;
}

impl ConstructionNode for DemandNode {
    fn init(&self) {
        DemandNode::init(self);
    }
    fn shutdown(&self) {
        DemandNode::shutdown(self);
    }
    fn supply(&self) -> f32 {
        DemandNode::supply(self)
    }
    fn demand(&self) -> f32 {
        DemandNode::demand(self)
    }
    fn set_supply(&self, v: f32) {
        DemandNode::set_supply(self, v);
    }
    fn set_demand(&self, v: f32) {
        DemandNode::set_demand(self, v);
    }
    fn add_to_demand(&self, v: f32) {
        DemandNode::add_to_demand(self, v);
    }
    fn end_of_cycle(&self) -> bool {
        DemandNode::end_of_cycle(self)
    }
    fn save_ref(self: Rc<Self>) -> RefValue {
        RefValue::Object(self as Rc<dyn SaveObject>)
    }
}

/// One draw of the BDS generator, for draw-sequence checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Draw {
    /// The argument of `RandomUint32Uniform(n)`.
    pub n: u32,
    pub value: u32,
    /// Whether the generator stepped (n ≠ 0).
    pub stepped: bool,
}

/// The process-wide statics of §2: the building catalogue (§4.7), the
/// tried-tract marks (§4.4) and the file-local `cRZRandom` (§4.14).
pub struct BdsProcess {
    catalogue: RefCell<BuildingCatalogue>,
    marks: RefCell<BTreeMap<u32, BTreeSet<(i32, i32)>>>,
    rng: RefCell<RzRandom>,
    steps: Cell<u64>,
    trace: RefCell<Option<Vec<Draw>>>,
}

/// The handle every BDS of a process shares.
pub type SharedProcess = Rc<BdsProcess>;

impl BdsProcess {
    /// The process statics with the generator seeded with `seed`
    /// ([choice], foundations §6.3; `0xFFFFFFFF` takes the wall clock).
    pub fn new(seed: u32, seeds: &dyn SeedSource) -> SharedProcess {
        Rc::new(BdsProcess {
            catalogue: RefCell::new(BuildingCatalogue::new()),
            marks: RefCell::new(BTreeMap::new()),
            rng: RefCell::new(RzRandom::new(seed, seeds)),
            steps: Cell::new(0),
            trace: RefCell::new(None),
        })
    }

    /// The process statics with the generator seeded from the session seed
    /// (foundations §6.3 *[choice]*): `derive_seed(session, 0x0C09C0B8)`,
    /// the original's `cRZTime() XOR 0x0C09C0B8` with the session seed in
    /// place of the clock.
    pub fn from_session(session: u32, seeds: &dyn SeedSource) -> SharedProcess {
        BdsProcess::new(
            crate::seeding::seed(session, crate::seeding::keys::BUILDING_DEVELOPMENT),
            seeds,
        )
    }

    /// Re-seeds the generator (the host injects the seed; the step count
    /// is kept).
    pub fn reseed(&self, seed: u32, seeds: &dyn SeedSource) {
        self.rng.borrow_mut().seed(seed, seeds);
    }

    pub fn catalogue(&self) -> std::cell::Ref<'_, BuildingCatalogue> {
        self.catalogue.borrow()
    }

    pub fn catalogue_mut(&self) -> std::cell::RefMut<'_, BuildingCatalogue> {
        self.catalogue.borrow_mut()
    }

    /// `RandomUint32Uniform(n)` on the file-local generator.
    pub fn uniform(&self, n: u32) -> u32 {
        let v = self.rng.borrow_mut().uniform_u32(n);
        if n != 0 {
            self.steps.set(self.steps.get().wrapping_add(1));
        }
        if let Some(t) = self.trace.borrow_mut().as_mut() {
            t.push(Draw {
                n,
                value: v,
                stepped: n != 0,
            });
        }
        v
    }

    /// Generator steps taken so far.
    pub fn steps(&self) -> u64 {
        self.steps.get()
    }

    /// The LCG state.
    pub fn rng_state(&self) -> u32 {
        self.rng.borrow().lcg_state()
    }

    /// Starts (or restarts) recording draws.
    pub fn enable_trace(&self) {
        *self.trace.borrow_mut() = Some(Vec::new());
    }

    /// The draws recorded since the last call.
    pub fn take_trace(&self) -> Vec<Draw> {
        self.trace.borrow_mut().as_mut().map(std::mem::take).unwrap_or_default()
    }

    /// The tried-tract marks of developer type `t`.
    pub fn marks_of(&self, t: u32) -> BTreeSet<(i32, i32)> {
        self.marks.borrow().get(&t).cloned().unwrap_or_default()
    }

    /// Every mark set (ascending type).
    pub fn marks(&self) -> BTreeMap<u32, BTreeSet<(i32, i32)>> {
        self.marks.borrow().clone()
    }

    pub fn is_marked(&self, t: u32, cell: (i32, i32)) -> bool {
        self.marks.borrow().get(&t).is_some_and(|s| s.contains(&cell))
    }

    /// `marks[t]` through `operator[]`: creates an empty set for `t` when
    /// absent *[inferred]* (an empty entry is saved with 0 cells).
    pub fn touch_marks(&self, t: u32) {
        self.marks.borrow_mut().entry(t).or_default();
    }

    pub fn mark(&self, t: u32, cell: (i32, i32)) {
        self.marks.borrow_mut().entry(t).or_default().insert(cell);
    }

    /// Clears `marks[t]` (the entry stays, empty).
    pub fn clear_marks_of(&self, t: u32) {
        if let Some(s) = self.marks.borrow_mut().get_mut(&t) {
            s.clear();
        }
    }

    /// Clears every mark (`Shutdown`).
    pub fn clear_marks(&self) {
        self.marks.borrow_mut().clear();
    }

    /// The static-data cleanup callback at process exit: drops the
    /// catalogue.
    pub fn app_shutdown(&self) {
        self.catalogue.borrow_mut().app_shutdown();
    }
}

// ---- pure helpers -----------------------------------------------------------------------------------

/// One 24-bit rounding (foundations §18), as an `f32`.
fn r24(x: f64) -> f32 {
    x as f32
}

/// The terrain-changed rectangle (§3.2): `x1 := (data1 >> 16) − 1` clamped
/// ≥ 0, `x2 := data1 & 0xFFFF` clamped ≤ cellCountX − 1, the same for z
/// with data2.
pub fn unpack_terrain_rect(data1: u32, data2: u32, cells: (i32, i32)) -> CellRect {
    let lo = |d: u32| ((d >> 16) as i32 - 1).max(0);
    let hi = |d: u32, n: i32| ((d & 0xFFFF) as i32).min(n.wrapping_sub(1));
    (lo(data1), lo(data2), hi(data1, cells.0), hi(data2, cells.1))
}

/// The monthly construction supply (§4.1):
/// `clamp_u(low32(trunc(r24(r24(r24(I + C) + R) × perResident))), min, max)`
/// with unsigned clamps, then `float(int32 u)`.
pub fn monthly_supply(r: f32, c: f32, i: f32, per_resident: f32, min: u32, max: u32) -> f32 {
    let ic = r24(f64::from(i) + f64::from(c));
    let icr = r24(f64::from(ic) + f64::from(r));
    let x = r24(f64::from(icr) * f64::from(per_resident));
    let u = crate::x87::ftol(f64::from(x)) as u32;
    let u = u.max(min).min(max);
    u as i32 as f32
}

/// The day of decline-plan entry `k` (§4.3): `trunc(r24(float(now) +
/// float(k)))`, both unsigned.
pub fn decline_day(now: u32, k: u32) -> u32 {
    crate::x87::ftol(f64::from(r24(f64::from(now) + f64::from(k)))) as u32
}

/// The little-endian bytes of a variant's values at their own widths
/// (floats as bits), for the raw `RefUint32` read of §4.13.
fn raw_bytes(v: &crate::variant::Variant) -> Vec<u8> {
    use crate::variant::Elem;
    let mut out = Vec::new();
    for e in v.items() {
        match e {
            Elem::Bool(b) => out.push(u8::from(b)),
            Elem::U8(x) | Elem::Char(x) | Elem::RzChar(x) => out.push(x),
            Elem::I8(x) => out.push(x as u8),
            Elem::U16(x) | Elem::UnicodeChar(x) => out.extend_from_slice(&x.to_le_bytes()),
            Elem::I16(x) => out.extend_from_slice(&x.to_le_bytes()),
            Elem::U32(x) => out.extend_from_slice(&x.to_le_bytes()),
            Elem::I32(x) => out.extend_from_slice(&x.to_le_bytes()),
            Elem::F32(x) => out.extend_from_slice(&x.to_bits().to_le_bytes()),
            Elem::U64(x) | Elem::VoidPtr(x) => out.extend_from_slice(&x.to_le_bytes()),
            Elem::I64(x) => out.extend_from_slice(&x.to_le_bytes()),
            Elem::F64(x) => out.extend_from_slice(&x.to_bits().to_le_bytes()),
        }
    }
    out
}

/// The first two `uint32` of property `0x47E2C380`'s data (§4.13, no type
/// or count check): ([choice]) each word that has 4 bytes behind it, else
/// `None`.
pub fn tuning_min_max(h: &PropertyHolder) -> (Option<u32>, Option<u32>) {
    let Some(v) = h.get(prop::CONSTRUCTION_MIN_MAX) else {
        return (None, None);
    };
    let b = raw_bytes(v);
    let word = |i: usize| {
        b.get(i * 4..i * 4 + 4)
            .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    };
    (word(0), word(1))
}
