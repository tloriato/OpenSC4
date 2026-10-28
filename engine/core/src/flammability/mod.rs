// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The flammability simulator (`docs/specs/flammability.md`,
//! `cISC4FlammabilitySimulator`, class `0xE990C045`). Section references
//! (`§n`) point at that spec.
//!
//! # Structure
//!
//! - [`FlammabilitySim`]: the state of §3, the lifecycle of §4, the messages
//!   of §5, the algorithms of §6, the tuning of §8, the posted message of §9,
//!   the save record of §10 and the portable part of the interface slots of
//!   §11 (`QueryInterface` table, reference count, the four query slots).
//! - [`FlammabilityHost`]: everything the simulator reads from, or does to,
//!   the game: the city, occupants and their properties, lots, the zone
//!   manager, the ordinance, weather and plumbing simulators, the history
//!   warehouse, the fire manager, message posting and the grids
//!   ([`GridProvider<u8>`] is a supertrait: in game the plugin creates the
//!   game's own `cSC4SimGrid<uint8>` objects, simgrid §6.4).
//! - [`FlammabilityRandom`]: the fire roll's draw from the simulator's own
//!   `cRZRandom` (§7), behind a trait so tests can script it
//!   ([`FlammabilityRng`] is the real one). The `rand()` users (iterator
//!   start, hotspot shuffle) draw from the simulator's own [`CrtRand`]
//!   (foundations §12.6).
//! - Pure helpers for the arithmetic of §6: [`occupant_raw_value`],
//!   [`daily_value`], [`adjusted_value`], [`global_multiplier`],
//!   [`compute_hotspots`].
//!
//! The GZCOM side (agent registration, notifications, `QueryInterface`
//! pointers, the class factory) is the plugin's: [`AGENT_LEVEL`],
//! [`AGENT_FLAGS`], [`AGENT_NAME`], [`NOTIFICATIONS`] and [`answers_iid`]
//! give it the facts of §4.1, §5 and §1.
//!
//! Host handles ([`OccupantId`], [`LotId`]) are opaque tokens the plugin
//! hands in; they only need to stay valid for the duration of the call that
//! receives them. Host callbacks must not call back into the simulator.
//!
//! # Choices made where the spec is open (`[choice]`)
//!
//! - Occupant types (§1, §6.1, §6.2): buildings `0x278128A0` and props
//!   `0xA823821E` get the abandonment multiplier, flora `0x74758926` never
//!   does (no lot lookup); the tract priority is building > prop > flora.
//! - Arithmetic follows foundations §18 (24-bit x87 precision, [compat]):
//!   a product of an integer and a `float32` multiplier is the exact product
//!   rounded once to `float32`, then `roundHE` rounds that to an integer,
//!   ties to even (§6, §15.5): e.g. `roundHE(10 × 0.95f)` is 10 and
//!   `roundHE(50 × 0.95f)` is 48 (the products round to 9.5 and 47.5).
//!   Results are clamped
//!   to 0..255 with NaN reading as 0, also in the daily refresh, which has no
//!   lower clamp in the original (§14.6).
//! - The occupant property `0x29244DB5` is read with the `GetValSint32`
//!   table of §6.1 ([`sint32_of_elem`]): e.g. `Uint32` 0xFFFFFFFF is −1,
//!   floats truncate through `_ftol` (low 32 bits). The spec's [choice]: an
//!   array reads its first element with the scalar rule (an empty array is
//!   0), any other value (empty, object) is 0, and so is a missing property.
//! - City cell counts are clamped to `0 ..= MAX_TRACTS` (4096).
//! - Missing services (§4.1 step 5, §6.4, §6.5; the original crashes): a
//!   missing ordinance simulator reads as factor 1.0, a missing weather
//!   simulator as "not summer", a missing plumbing simulator as "not
//!   watered" for every cell (the no-water multiplier applies), a missing
//!   zone manager as "unzoned" for every tract (zoned sum and count stay 0,
//!   the average is 0); the lot manager and tutorial answers are the
//!   host's. A missing fire manager
//!   (disaster layer, manager or interface) skips the ignition and
//!   `SetCause` (§6.8, the spec's own [choice]).
//! - Occupant messages whose void1 is null are ignored (the original would
//!   dereference it).
//! - Grids: a host that cannot create a grid leaves the slot empty; reads of
//!   a missing grid return 0 and writes are ignored, `tractsPerDay` and the
//!   iterator use counts 0. A grid loaded with a tract size other than 1 is
//!   dropped at `Init` (`Shutdown` through the host, then released) and a new
//!   one is created (§14.4: FreeCity4 supports only S = 1). A reference that
//!   fails to read or does not resolve to a `u8` grid leaves its slot empty.
//! - The hotspot fire curve `0x29B4013D` ("delete" pattern, §8): a property
//!   with fewer than two values (a scalar, a one-element or an empty array)
//!   or a non-numeric one counts as absent (§6.8, §8: the original builds a
//!   curve with no points and crashes when it evaluates it); two values make
//!   a one-point, constant curve.
//! - The hotspot settings `0x29B4013C` are read as `u32` and stored as `s32`
//!   (wrapping), only when the property has exactly three values.
//! - The daily loop runs while `n > 0` (a negative `tractsPerDay` cannot
//!   occur) and stops after a second iterator wrap without a visit in
//!   between, so a degenerate iterator cannot loop forever.
//! - With no adjusted grid, a hotspot fire uses tract size 1 (§6.8).
//! - `Read` appends the saved hotspots to the current list (§10 [compat];
//!   the list is empty on a fresh object), sets each field as it is read,
//!   and initialises the count to 0 (the spec's own [choice]). A grid
//!   reference is null-first (foundations §2.7): a reference that fails, or
//!   does not resolve to a `u8` grid, leaves its slot empty, as does every
//!   later one once the stream is in error; the previous grid is dropped
//!   without its `Shutdown`.
//! - Removal (§6.3): the tract recompute asks the host for the occupants
//!   now listed; the host must already have unlinked the removed occupant
//!   (the occupant manager does so before it posts RemoveOccupant).
//! - Reference count: `Release` from a count ≤ 1 calls `Shutdown`, returns 0
//!   and marks the object destroyed; the plugin then drops it (§4.3).
//! - `GetFlammabilityHotspot` multiplies the tract coordinates by the tract
//!   size with wrapping 32-bit arithmetic.

mod simulator;
pub mod archive;
#[cfg(test)]
mod tests;

pub use simulator::{
    adjusted_value, compute_hotspots, daily_value, global_multiplier, occupant_raw_value, property_sint32,
    sint32_of_elem, FlammabilityData, FlammabilitySim, Hotspot, Tuning, FLAMMABILITY_RECORD_VERSION,
};

use std::rc::Rc;

use crate::grid::{GridProvider, TractGrid};
use crate::property::PropertyHolder;
use crate::rng::{RzRandom, SeedSource};
use crate::stream::ResKey;
use crate::variant::Variant;

pub use crate::curve::ResponseCurve;
pub use crate::grid::CellIterator;
pub use crate::rng::CrtRand;

/// The class ID and the interfaces `QueryInterface` answers (§1).
pub const CLSID_FLAMMABILITY_SIMULATOR: u32 = 0xE990_C045;
pub const IID_FLAMMABILITY_SIMULATOR: u32 = 0x48F4_1617;
pub const IID_MESSAGE_TARGET2: u32 = 0x4522_94AA;
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;
pub const IID_UNKNOWN: u32 = 1;
/// `cIGZMessageTarget`: **not** answered (§1).
pub const IID_MESSAGE_TARGET: u32 = 0xE98B_2F57;
/// The city's save-record reference slot (§1).
pub const CITY_REFERENCE_SLOT: u32 = 26;

/// Whether `QueryInterface(iid)` succeeds (§1). For any other IID the
/// plugin returns false and leaves `*ppv` unchanged.
pub fn answers_iid(iid: u32) -> bool {
    matches!(
        iid,
        IID_FLAMMABILITY_SIMULATOR | IID_MESSAGE_TARGET2 | IID_SERIALIZABLE | IID_UNKNOWN
    )
}

/// Agent registration (§5): level 4 (SimNewDay) only, flags 2.
pub const AGENT_NAME: &str = "cSC4FlammabilitySimulator";
pub const AGENT_LEVEL: u32 = 4;
pub const AGENT_FLAGS: u32 = 2;

/// The grid instance IDs (§2).
pub const ADJUSTED_GRID_INSTANCE: u32 = 0x49D5_B953;
pub const RAW_GRID_INSTANCE: u32 = 0x49D5_B964;

/// The tuning exemplar (§8).
pub const TUNING_KEY: ResKey = ResKey {
    type_id: 0x6534_284A,
    group: 0xE7E2_C2DB,
    instance: 0xA8F4_1EE3,
};

/// Messages (§5, §9).
pub mod msg {
    pub const SIM_BEGIN: u32 = 0x6695_6810;
    pub const SIM_NEW_DAY: u32 = 0x6695_6814;
    pub const INSERT_OCCUPANT: u32 = 0x99EF_1142;
    pub const REMOVE_OCCUPANT: u32 = 0x99EF_1143;
    pub const CHANGE_OCCUPANT: u32 = 0x99EF_1145;
    pub const SEASON_CHANGE: u32 = 0x6716_A4E5;
    pub const ORDINANCE_UPDATED: u32 = 0xC2C0_9554;
    pub const CONDITION_CHANGED: u32 = 0xE99B_3D4D;
    /// Posted at every `EndOfCycle`: void1 = IGZUnknown = the simulator.
    pub const SIM_CYCLE_COMPLETE: u32 = 0x08F4_15FE;
    /// Exemplar changed (foundations §5.2): neither subscribed nor handled
    /// by this simulator (§5, [compat]); listed for hosts and tests only.
    pub const EXEMPLAR_CHANGED: u32 = 0xC53D_10AA;
}

/// The notifications `Init` subscribes, in order (§4.1 step 3): exactly
/// these six ([compat]; in particular not [`msg::EXEMPLAR_CHANGED`]).
pub const NOTIFICATIONS: [u32; 6] = [
    msg::INSERT_OCCUPANT,
    msg::REMOVE_OCCUPANT,
    msg::CHANGE_OCCUPANT,
    msg::SEASON_CHANGE,
    msg::ORDINANCE_UPDATED,
    msg::CONDITION_CHANGED,
];

/// Occupant types (§6.1).
pub mod occupant_type {
    pub const BUILDING: u32 = 0x2781_28A0;
    pub const PROP: u32 = 0xA823_821E;
    pub const FLORA: u32 = 0x7475_8926;
}

/// Properties (§5, §6.1, §6.4, §8).
pub mod prop {
    /// Occupant flammability (integer).
    pub const FLAMMABILITY: u32 = 0x2924_4DB5;
    /// The preview-occupant marker (foundations §5.2): tested for presence.
    pub const PREVIEW_OCCUPANT: u32 = 0x0292_44DB;
    /// The ordinance effect asked through `GetCumulativeFactor` (§6.4).
    pub const ORDINANCE_FACTOR: u32 = 0x28F4_2AA0;
    pub const SUMMER_MULTIPLIER: u32 = 0xC9B4_0128;
    pub const ABANDONMENT_MULTIPLIER: u32 = 0x29B4_0134;
    pub const NO_WATER_MULTIPLIER: u32 = 0x29B4_013B;
    /// `u32[3]`: threshold, radius, minimum weight.
    pub const HOTSPOT_SETTINGS: u32 = 0x29B4_013C;
    /// A response curve: hotspot weight → % per cycle.
    pub const HOTSPOT_FIRE_CURVE: u32 = 0x29B4_013D;
    pub const RANDOM_FIRE_PROBABILITY: u32 = 0x29B4_013E;
}

/// The history series of the average (§6.6).
pub const HISTORY_AVERAGE: u32 = 0x2A11_CB48;
/// The lot condition of an abandoned lot at its last decay stage
/// (foundations §5.2).
pub const ABANDONED_CONDITION: u8 = 4;
/// The weather season "summer" (§6.4).
pub const SUMMER: i32 = 2;
/// The fire cause set after every ignition (§6.8).
pub const FIRE_CAUSE: u32 = 5;
/// The fire disaster manager type and its interface (§6.8).
pub const FIRE_DISASTER_MANAGER: u32 = 0x621C_DA33;
pub const IID_DISASTER_MANAGER_FIRE: u32 = 0x49B4_3F57;

/// An occupant handle (a `cISC4Occupant*` in the plugin).
pub type OccupantId = u64;
/// A lot handle (a `cISC4Lot*` in the plugin).
pub type LotId = u64;

/// An inclusive cell rectangle `(x0, z0, x1, z1)` (`SC4Rect<long>`).
pub type CellRect = (i32, i32, i32, i32);

/// A message as the simulator's `DoMessage` sees it (§5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Message {
    pub msg_type: u32,
    /// `GetVoid1()` as an occupant (Insert / Remove / ChangeOccupant).
    pub occupant: Option<OccupantId>,
    /// ConditionChanged: `GetIGZUnknown()` `QueryInterface`d to `cISC4Lot`
    /// (`0xE818FCED`); `None` when it is null or not a lot.
    pub lot: Option<LotId>,
}

impl Message {
    /// A message with only a type.
    pub fn of_type(msg_type: u32) -> Message {
        Message {
            msg_type,
            ..Message::default()
        }
    }
}

/// Everything the flammability simulator reads from, or does to, the rest
/// of the game (§12). Coordinates are city cells (= tracts, S = 1).
pub trait FlammabilityHost: GridProvider<u8> {
    // ---- city ----
    /// `cISC4City::CellCountX()` / `CellCountZ()`.
    fn city_cell_counts(&self) -> (i32, i32);
    /// `cISC4City::PositionToCell(x, z, &cx, &cz)`.
    fn position_to_cell(&self, x: f32, z: f32) -> (i32, i32);
    /// `GetTutorialSystem()` is non-null and its `IsTutorial()` is true.
    fn is_tutorial(&self) -> bool;
    /// The "Flammability Simulator" exemplar ([`TUNING_KEY`]), if loaded.
    fn tuning(&self) -> Option<Rc<PropertyHolder>>;

    // ---- occupants ----
    /// `cISC4Occupant::GetType()`.
    fn occupant_type(&self, occ: OccupantId) -> u32;
    /// A property of `occ.AsPropertyHolder()`, `None` if absent
    /// (`HasProperty` is `is_some()`).
    fn occupant_property(&self, occ: OccupantId, id: u32) -> Option<Variant>;
    /// `GetBoundingCityCells(rect)` (inclusive).
    fn occupant_bounds(&self, occ: OccupantId) -> CellRect;
    /// `GetPosition()` (world x, y, z).
    fn occupant_position(&self, occ: OccupantId) -> [f32; 3];
    /// `GetOccupantsByStandardCityCells(list, {x0, z0}, {x1, z1}, filter,
    /// 0xFFFFFFFF)` with the type filter of §6.2 (building, flora, prop).
    fn occupants_in_cells(&self, rect: CellRect) -> Vec<OccupantId>;

    // ---- lots ----
    /// `cISC4LotManager::GetOccupantLot(occ)`.
    fn occupant_lot(&self, occ: OccupantId) -> Option<LotId>;
    /// `cISC4Lot::GetCondition()`.
    fn lot_condition(&self, lot: LotId) -> u8;
    /// `cISC4Lot::GetBuilding()->AsOccupant()`, `None` without a building.
    fn lot_building(&self, lot: LotId) -> Option<OccupantId>;
    /// `cISC4LotManager::GetRandomLot()` (the lot manager's own generator).
    fn random_lot(&self) -> Option<LotId>;

    // ---- other subsystems ----
    /// `cISC4OrdinanceSimulator::GetCumulativeFactor(prop, index)`; `None`
    /// without an ordinance simulator.
    fn cumulative_factor(&self, prop: u32, index: u32) -> Option<f32>;
    /// `cISC4WeatherSimulator::GetCurrentSeason()`; `None` without one.
    fn current_season(&self) -> Option<i32>;
    /// `cISC4PlumbingSimulator::IsCellWatered(x, z)`; `None` without a
    /// plumbing simulator (the simulator then reads "not watered", §4.1).
    fn is_cell_watered(&self, x: i32, z: i32) -> Option<bool>;
    /// `cISC4ZoneManager::GetZoneType(x, z)`; `None` without a zone manager
    /// (the simulator then reads "unzoned", §4.1).
    fn zone_type(&self, x: i32, z: i32) -> Option<u32>;
    /// `cISC4HistoryWarehouse::RecordHistoryData(type, value)`.
    fn record_history(&self, history_type: u32, value: i32);

    // ---- fire (§6.8) ----
    /// `GetDisasterLayer()->GetDisasterManager(0x621CDA33)` queried for
    /// `cISC4DisasterManagerFire`, then `IgniteCell(x, z, useProbability,
    /// teleport, fireIID)`. Returns false (and does nothing) when the layer,
    /// the manager or the interface is missing; the simulator then skips
    /// [`FlammabilityHost::set_fire_cause`].
    fn ignite_cell(&self, x: u32, z: u32, use_probability: bool, teleport: bool, fire_iid: u32) -> bool;
    /// `SetCause(cause)` on the same fire manager, then its release.
    fn set_fire_cause(&self, cause: u32);

    // ---- messages ----
    /// `MessagePost(msg, false)` of a message with void1 = IGZUnknown = the
    /// simulator (§9).
    fn post(&self, msg_type: u32);

    // ---- grids ----
    /// `Shutdown()` on a grid the simulator is about to release (§4.2). The
    /// simulator then drops its `Rc`, which releases the reference.
    fn grid_shutdown(&self, grid: &Rc<dyn TractGrid<u8>>);
}

/// The fire roll's draw (§7).
pub trait FlammabilityRandom {
    /// `RandomDoubleRangeUniform(a, b)`.
    fn range_f64(&mut self, a: f64, b: f64) -> f64;
}

/// The real generator: a `cRZRandom` seeded with 0 at construction (§7).
/// Its double path seeds itself from the process clock of `seeds` on first
/// use ([choice]: the host supplies it, foundations §6.3).
pub struct FlammabilityRng {
    rz: RzRandom,
}

impl FlammabilityRng {
    pub fn new(seeds: &dyn SeedSource) -> FlammabilityRng {
        FlammabilityRng {
            rz: RzRandom::new(0, seeds),
        }
    }

    /// Seeded with 0 like the original; the double path's clock reseed uses
    /// `derive(GZCLSID)` (`0xE990C045`) of the session seed (foundations
    /// §6.3 *[choice]*: a constant-seeded member keeps the constant).
    pub fn from_session(session: u32) -> FlammabilityRng {
        FlammabilityRng::new(&crate::seeding::clocks(
            session,
            crate::seeding::keys::FLAMMABILITY_RESEED,
        ))
    }

    /// The double path's state (for tests).
    pub fn mul_state(&self) -> u32 {
        self.rz.mul_state()
    }
}

impl FlammabilityRandom for FlammabilityRng {
    fn range_f64(&mut self, a: f64, b: f64) -> f64 {
        self.rz.range_f64(a, b)
    }
}
