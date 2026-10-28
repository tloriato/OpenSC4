// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The lot configuration manager (`cSC4LotConfigurationManager`, class
//! `0x299062C0`, city component 9), `docs/specs/prop-lotconfig.md` Part B.
//! Section references (`§Bn`) point at that spec.
//!
//! # Structure
//!
//! - `objects.rs`: the 0x34-byte lot configuration object record
//!   (lot-developer.md §17.1) and the reader that builds a configuration's
//!   object array from its lot-object properties with the detail filter of
//!   §B3.3 (`ConvertPropertiesToObjectArray`). The reader belongs to the lot
//!   configuration class, which is a host input here; it is in the core so
//!   that every host builds the same arrays (and for the §T 7 test).
//! - `catalogue.rs`: the process-wide statics of §B2 (the configuration map,
//!   the family set, the loaded flag and the detail level).
//! - `manager.rs`: the component (§B1, §B2, §B3.1, §B3.2, §B3.4, §B4–§B6).
//! - `tests.rs`: a fake game and the §T tests of Part B.
//!
//! Lot configurations are **host objects** (tokens): the core creates them
//! through [`LotConfigHost::create_configuration`] and
//! [`LotConfigHost::load_configuration`] (configuration slot 31) and reads
//! them through the configuration slots the spec names (3, 5, 6, 7, 23, 24,
//! 28). The building development simulator, the flora simulator and the prop
//! manager are reached through the host too (the spec names which slots use
//! the held and which the global objects; the host answers for both).
//!
//! # Choices (`[choice]`) and literal readings
//!
//! - **Set queries** (§B5.2) return their matches in map bucket order (each
//!   configuration with a new reference for the caller); the caller adds them
//!   to its hash set. The count is the number of matches.
//! - **Building information** (§B4.3): a type whose exemplar does not load
//!   reads as "monopoly property absent" (sets the flag) and has no groups
//!   [inferred]. Question PL7.
//! - **`CreateGenericLotConfiguration`** (§B5.7, *[inferred]* in the spec):
//!   the lot is `1 + floor(w / 16) × 1 + floor(d / 16)` cells for the
//!   footprint `(w, d)` of slot 24 (`1 − trunc(m × −0.0625)`, PL12), the
//!   building object is centred on it [choice]. Questions PL8, PL12.
//! - **`FixLotConfiguration`** (§B5.7): see [`LotConfigManager::fix`]; the
//!   footprint, lot-bounds and centre steps are the literal readings of
//!   PL13 (questions PL9, PL10).
//! - **`Read`** (§B6) is atomic [choice], as for the prop manager.
//! - **Threading**: the state is a `RefCell`; main thread only.

mod catalogue;
pub mod archive;
mod configuration;
mod manager;
mod objects;

#[cfg(test)]
mod tests;

pub use catalogue::{LotConfigCatalogue, SharedLotConfigCatalogue};
pub use configuration::{configuration_fields, configuration_objects, ConfigurationFields, LotConfiguration};
pub use manager::{query_interface, LcmState, LotConfigManager};
pub use objects::{fixed_16_16, is_kept, lod_of, lot_object_array, object_from_values, LotConfigObject};

use crate::stl::StlHashSet;
use crate::stream::{RefValue, ResKey};

/// Class ID (§B1).
pub const CLSID_LOT_CONFIG_MANAGER: u32 = 0x2990_62C0;
/// `cISC4LotConfigurationManager` (§B1).
pub const IID_LOT_CONFIG_MANAGER: u32 = 0xC8ED_BEC0;
pub const IID_UNKNOWN: u32 = 0x0000_0001;
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;
/// The record's references are read with these IIDs (§B6).
pub const IID_BUILDING_DEVELOPMENT: u32 = 0x0781_3EAE;
pub const IID_FLORA: u32 = 0x06C6_6177;
pub const IID_PROP_MANAGER: u32 = 0x2832_6F3E;

/// City component (§B1).
pub const CITY_COMPONENT: u32 = 9;

/// The exemplar group of every lot configuration (§B3.1).
pub const LOT_CONFIG_GROUP: u32 = 0xA8FB_D372;

/// `kSC4MessagePostCityInit`, the one subscription (§B4).
pub const MSG_POST_CITY_INIT: u32 = 0x26D3_1EC1;

/// The record version (§B6).
pub const RECORD_VERSION: u16 = 1;

/// Lot configuration object types used here (lot-developer.md §17.1).
pub mod kind {
    pub const BUILDING: u32 = 0;
    pub const PROP: u32 = 1;
    pub const TEXTURE: u32 = 2;
    pub const FLORA: u32 = 4;
}

/// Exemplar properties (§B3.3, §B4.3).
pub mod prop {
    /// The configuration's version.
    pub const VERSION: u32 = 0x88ED_C789;
    /// The first lot-object property.
    pub const LOT_OBJECT_FIRST: u32 = 0x88ED_C900;
    /// The last lot-object property read (the scan stops at `0x88EDCE00`,
    /// lot-configuration.md §3.5).
    pub const LOT_OBJECT_END: u32 = 0x88ED_CDFF;
    /// A building exemplar's monopoly property (bool).
    pub const MONOPOLY: u32 = 0x2781_283F;
    /// A building exemplar's occupant groups (building-development.md §4.7).
    pub const OCCUPANT_GROUPS: u32 = 0xAA1D_D396;
}

/// The lot margin of `FixLotConfiguration` (§B5.7), metres. The three
/// constants of the lot-bounds test are given "as stored" in the game
/// (PL13): `0.049` is `0x3D48B43A` — one ULP above the `0.049` literal,
/// which rounds to `0x3D48B439` — `0.05` is `0x3D4CCCCD` and `0.001` is
/// `0x3A83126F`.
pub const FIX_MARGIN_BITS: u32 = 0x3D48_B43A;
/// `f32(lotW − 0.05)`'s subtrahend (PL13).
pub const FIX_EDGE_BITS: u32 = 0x3D4C_CCCD;
/// `ex + 0.001`'s addend (PL13).
pub const FIX_EPS_BITS: u32 = 0x3A83_126F;
/// Metres per cell.
pub const CELL_METRES: f32 = 16.0;

/// A service the manager holds (§B2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LcmService {
    /// Global `0x00B43D68`.
    BuildingDevelopment,
    /// Global `0x00B43D80`.
    Flora,
    /// Global `0x00B43D10`.
    PropManager,
}

/// What `0x0048AD40` stores in a configuration (§B4.3).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BuildingInfo {
    /// +0x3E: the smallest profile u16 (0 when no type was found).
    pub min_capacity: u16,
    /// +0x40: the largest.
    pub max_capacity: u16,
    /// +0x80: the occupant groups of the types, ascending.
    pub groups: Vec<u32>,
    /// +0x8C: at least one type lacks the monopoly property (or has it
    /// false).
    pub non_monopoly: bool,
}

/// A `cS3DRect2D` as the footprint slots write it: `(w, d, 0, 0)`.
pub type Rect2D = [f32; 4];

/// Everything the lot configuration manager reads from, or does to, the
/// rest of the game. Objects are host tokens. Re-entrancy: callbacks must
/// not call back into the manager.
pub trait LotConfigHost {
    // ---- the catalogue (§B3) ----
    /// The city detail preference (the byte at +0xF16 of the preferences
    /// object; 1 low, 2 medium, 3 high).
    fn detail_level(&self) -> u32;
    /// The resource manager's keys of type `0x6534284A` (slot 18), in its
    /// order; `None` when there is no resource manager.
    fn exemplar_keys(&self) -> Option<Vec<ResKey>>;
    /// Application slot 69: registers / unregisters the unload callback.
    fn register_shutdown_callback(&self);
    fn unregister_shutdown_callback(&self);
    /// A new lot configuration (0xC0 bytes), one reference.
    fn create_configuration(&self) -> u64;
    /// Configuration slot 31 `Load(key, detail)`.
    fn load_configuration(&self, cfg: u64, key: ResKey, detail: u32) -> bool;

    // ---- configuration slots ----
    /// Slot 3 `GetID`.
    fn config_id(&self, cfg: u64) -> u32;
    /// Slot 5 `BelongsToFamily`.
    fn config_belongs_to_family(&self, cfg: u64, family: u32) -> bool;
    /// Slot 6: the family list.
    fn config_families(&self, cfg: u64) -> Vec<u32>;
    /// Slot 7 `GetSize`.
    fn config_size(&self, cfg: u64) -> (u32, u32);
    /// Slot 23: the object array.
    fn config_objects(&self, cfg: u64) -> Vec<LotConfigObject>;
    /// Replaces the object array (`FixLotConfiguration`).
    fn set_config_objects(&self, cfg: u64, objects: Vec<LotConfigObject>);
    /// Slot 24: the primary building record.
    fn config_primary_building(&self, cfg: u64) -> Option<LotConfigObject>;
    /// Stores +0x3E, +0x40, +0x80, +0x8C (§B4.3).
    fn set_config_building_info(&self, cfg: u64, info: &BuildingInfo);
    /// Slot 28: the retaining wall ID list, and its replacement.
    fn config_retaining_walls(&self, cfg: u64) -> Vec<u32>;
    fn set_config_retaining_walls(&self, cfg: u64, ids: Vec<u32>);
    /// Whether a retaining wall ID of the slot 28 list is valid
    /// (`FixLotConfiguration`, PL10).
    fn is_retaining_wall_valid(&self, id: u32) -> bool;
    /// A configuration that is not in the map, of `size` cells with
    /// `objects`, for building type `ty` (`CreateGenericLotConfiguration`);
    /// one reference for the caller.
    fn create_generic_configuration(&self, ty: u32, size: (u32, u32), objects: Vec<LotConfigObject>) -> Option<u64>;

    // ---- lifecycle and references ----
    /// The service's global, with a new reference; `None` when null.
    fn acquire_service(&self, service: LcmService) -> Option<u64>;
    fn subscribe(&self, msg_type: u32);
    fn unsubscribe(&self, msg_type: u32);
    fn add_ref(&self, token: u64);
    fn release(&self, token: u64);

    // ---- filters ----
    /// Filter slot 3 on a configuration.
    fn filter_matches(&self, filter: u64, cfg: u64) -> bool;

    // ---- the building development simulator ----
    /// Slot 14 + the exemplar's load: the building type's exemplar.
    fn building_exemplar(&self, ty: u32) -> Option<std::rc::Rc<crate::property::PropertyHolder>>;
    /// Slot 15 `IsBuildingTypeValid`.
    fn is_building_type_valid(&self, id: u32) -> bool;
    /// Slot 16 `IsBuildingFamilyTypeValid` (a family or a type).
    fn is_building_family_type_valid(&self, id: u32) -> bool;
    /// Slot 18 `GetBuildingTypesByFamily(vector&, id)` (a family assigns,
    /// a type assigns `[id]` and returns 0, else untouched and 0).
    fn building_types_by_family(&self, vec: &mut Vec<u32>, id: u32) -> u32;
    /// Slot 19 `GetBuildingTypesByFamily(hash_set&, id)` (inserts).
    fn building_types_into_set(&self, set: &mut StlHashSet, id: u32) -> u32;
    /// Slot 20 `IsBuildingInFamily(type, family)`.
    fn is_building_in_family(&self, ty: u32, family: u32) -> bool;
    /// Slot 39 `GetBuildingProfile(type)`: the u16 at +8.
    fn building_capacity(&self, ty: u32) -> u16;
    /// Slot 40 `GetBuildingProfilePtr(type)`: +0x20 and +0x28, or `None`.
    fn building_footprint(&self, ty: u32) -> Option<(f32, f32)>;

    // ---- the flora simulator (global) ----
    /// Slot 7: the static data's +0x3C, or `None` for an unknown type.
    fn flora_radius(&self, id: u32) -> Option<f32>;
    /// Slot 11 `IsFloraTypeValid`.
    fn is_flora_type_valid(&self, id: u32) -> bool;
    /// Slot 14 `GetFloraTypesByFamily(hash_set&, family)` (inserts).
    fn flora_types_into_set(&self, set: &mut StlHashSet, id: u32) -> u32;

    // ---- the prop manager (global) ----
    /// Slot 8 `IsPropTypeValid`.
    fn is_prop_type_valid(&self, id: u32) -> bool;
    /// Slot 9 `GetPropTypesByFamily(vec&, id)`.
    fn prop_types_by_family(&self, vec: &mut Vec<u32>, id: u32) -> u32;
    /// Slot 11 `IsPropFamilyTypeValid`.
    fn is_prop_family_type_valid(&self, id: u32) -> bool;
    /// Slot 14: the profile's size x (+0x04) and size z (+0x0C), or `None`.
    fn prop_size(&self, id: u32) -> Option<(f32, f32)>;

    // ---- save references (§B6) ----
    fn object_ref(&self, token: u64) -> RefValue;
    /// Resolves a reference read with `iid`; the token comes with one
    /// reference.
    fn resolve_ref(&self, r: &RefValue, iid: u32) -> Option<u64>;
}
