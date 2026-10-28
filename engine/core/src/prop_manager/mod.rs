// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The prop manager (`cSC4PropManager`, class `0xA990BD10`, city component
//! 5), `docs/specs/prop-lotconfig.md` Part A. Section references (`§An`)
//! point at that spec.
//!
//! # Structure
//!
//! - `catalogue.rs`: the process-wide prop catalogue of §A2 / §A3 (type,
//!   family, profile and name maps, the "built" flag), the 0x1C-byte
//!   [`PropProfile`] and the catalogue queries of §A5 (slots 5–14).
//! - `manager.rs`: the component (§A1, §A2, §A4, §A6–§A8): `Init` /
//!   `Shutdown`, `DoMessage`, `CreateProp` / `CreateGenericProp`, the city /
//!   lot / street / building queries and removals, and the record.
//! - `tests.rs`: a fake game and the §T tests of Part A.
//!
//! Everything outside the class goes through [`PropHost`]: the resource
//! manager (exemplar keys and exemplars, as [`PropertyHolder`]s), GZCOM
//! creation, the prop occupants, the occupant manager, the lot manager, the
//! city's cell conversions, the building-prop property (§A7.1), the text
//! decal of the debug command, references and save references. Objects are
//! host tokens (`u64`, 0 is never a token).
//!
//! # Choices (`[choice]`) and literal readings
//!
//! - **The catalogue is process-wide** (§A2): it lives in a
//!   [`SharedPropCatalogue`] that every manager of a process shares, built
//!   by the first `Init` that finds it unbuilt.
//! - **Profile reading order** (§A3.2) *[inferred]*: the table order (class,
//!   size, satisfaction, wealth, crime), each over a profile that starts with
//!   the defaults. OccupantSize values past the third write the dwords from
//!   +0x10 on (the quirk); values past the profile's end (the seventh value
//!   on) are dropped [choice] (the original writes past the local copy).
//!   Every property is read with the core's vector / converting readers
//!   (`crate::props`), so a scalar family or size property reads as empty
//!   (EX39). Question PL4.
//! - **The bounding box** (§A6.1 step 5): from the loaded exemplar's
//!   OccupantSize; absent (or empty) gives (1, 1, 1); one or two values give
//!   the missing ones 0 [choice] (the building loader's bracket, DG32).
//!   Question PL5.
//! - **A missing occupant manager** in slots 20, 26 and 32 (which do not
//!   test it, quirk 5): the insertion goes to the host, which answers false
//!   when there is none [choice] (the original would crash).
//! - **Slot 39** removes the occupant only when the occupant manager exists,
//!   as slot 38 says [inferred].
//! - **Slot 36 without a property holder**: the property step is skipped and
//!   the usage is still set; true [inferred]. Question PL6.
//! - **`Read`** (§A8) is atomic [choice]: on a stream error or a wrong
//!   version nothing changes and the references read so far are released. A
//!   field that already holds a service is released when replaced [choice]
//!   (`Read` runs before `Init`, so the fields are null in the game).
//! - **Threading** (foundations §19.3): the state is a `RefCell`; the core is
//!   main-thread only. No borrow is held across a host call that could call
//!   back into the manager.

mod catalogue;
pub mod archive;
mod manager;

#[cfg(test)]
mod tests;

pub use catalogue::{PropCatalogue, PropProfile, SharedPropCatalogue};
pub use manager::{query_interface, PropManager, PropState};

use std::rc::Rc;

use crate::property::PropertyHolder;
use crate::stream::{RefValue, ResKey};

/// Class ID (§A1).
pub const CLSID_PROP_MANAGER: u32 = 0xA990_BD10;
/// `cISC4PropManager` (§A1).
pub const IID_PROP_MANAGER: u32 = 0x2832_6F3E;
pub const IID_UNKNOWN: u32 = 0x0000_0001;
pub const IID_MESSAGE_TARGET2: u32 = 0x4522_94AA;
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;
/// `cISC4PropOccupant` (§A6.1 step 3, §A7.1).
pub const IID_PROP_OCCUPANT: u32 = 0xE979_3A65;
/// The interface a prop's property holder offers for its exemplar (§A6.1
/// step 5).
pub const IID_EXEMPLAR_SINK: u32 = 0x0AC2_B5F7;
/// Exemplars are loaded with this IID (§A3.1).
pub const IID_EXEMPLAR: u32 = 0xA521_60F5;
/// The record's references are read with these IIDs (§A8).
pub const IID_OCCUPANT_MANAGER: u32 = 0xC6BB_8334;
pub const IID_LOT_MANAGER: u32 = 0x6819_05BD;

/// City component (§A1).
pub const CITY_COMPONENT: u32 = 5;

/// `ExemplarType` of a prop exemplar (§A3.1).
pub const EXEMPLAR_TYPE_PROP: u32 = 0x1E;

/// Exemplar properties (§A3).
pub mod prop {
    /// The occupant class `CreateProp` asks GZCOM for.
    pub const OCCUPANT_CLASS: u32 = 0xA977_A86B;
    /// OccupantSize (floats).
    pub const OCCUPANT_SIZE: u32 = 0x2781_2810;
    pub const REQUESTER_SATISFACTION: u32 = 0x49A1_E05A;
    pub const WEALTH: u32 = 0xE9A3_16EB;
    pub const CRIME_EFFECT: u32 = 0x8AA8_64D5;
    /// The prop families.
    pub const FAMILIES: u32 = 0x2781_2870;
    /// The exemplar name (read and thrown away).
    pub const NAME: u32 = 0x0000_0020;
}

/// Occupant classes (§A3.2, §A6.1 step 3).
pub mod class {
    /// The default prop class (profile +0x00 when the property is absent).
    pub const DEFAULT_PROP: u32 = 0x2977_AA47;
    pub const TERRAIN_DECAL_PROP: u32 = 0x2977_AA48;
    pub const VISUAL_EFFECT_PROP: u32 = 0x2977_AA49;
}

/// Prop usages (prop occupant slots 22 / 23, §A2).
pub mod usage {
    pub const CITY: u32 = 1;
    pub const LOT: u32 = 2;
    pub const BUILDING: u32 = 3;
    pub const STREET: u32 = 4;
}

/// Occupant flags (§A7).
pub const FLAG_INSERTED: u32 = 1;
pub const FLAG_SHOWN: u32 = 2;

/// The building-prop property: a property object holding a vector of prop
/// pointers (§A7.1 step 3).
pub const PROP_BUILDING_PROPS: u32 = 0xA832_2B7F;
/// The occupant type of a building (slots 34, 37).
pub const OCCUPANT_TYPE_BUILDING: u32 = 0x2781_28A0;

/// Messages (§A3.4, §A4).
pub mod msg {
    /// `kSC4MessagePostViewFilterChange`.
    pub const POST_VIEW_FILTER_CHANGE: u32 = 0x06CD_B65B;
    /// `kCommandID_CreateTextDecal` (a debug command).
    pub const CREATE_TEXT_DECAL: u32 = 0x0BEA_F1A2;
    /// `kSC4MessageViewZRChange3`.
    pub const VIEW_ZR_CHANGE3: u32 = 0xA6B7_9602;
    /// `kSC4MessagePostCityInit`.
    pub const POST_CITY_INIT: u32 = 0x26D3_1EC1;
    /// The subscriptions of `Init`, in order.
    pub const SUBSCRIPTIONS: [u32; 4] = [
        VIEW_ZR_CHANGE3,
        POST_VIEW_FILTER_CHANGE,
        POST_CITY_INIT,
        CREATE_TEXT_DECAL,
    ];
    /// `kSC4MessagePropExemplarDataUpdated`, posted once by `StaticInit`.
    pub const PROP_EXEMPLAR_DATA_UPDATED: u32 = 0x896F_DC70;
}

/// The command registered by `Init` (§A4.1).
pub const COMMAND_CREATE_TEXT_DECAL: u32 = msg::CREATE_TEXT_DECAL;

/// The record version (§A8).
pub const RECORD_VERSION: u16 = 1;

/// The three built-in prop classes of §A6.1 step 3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuiltinProp {
    /// `0x2977AA48`, 0xA0 bytes.
    TerrainDecal,
    /// `0x2977AA49`, 0x94 bytes.
    VisualEffect,
    /// Any other class: a plain prop occupant, 0x68 bytes.
    Plain,
}

impl BuiltinProp {
    /// The built-in class used when GZCOM has no instance of `class`.
    pub fn for_class(class_id: u32) -> BuiltinProp {
        match class_id {
            class::TERRAIN_DECAL_PROP => BuiltinProp::TerrainDecal,
            class::VISUAL_EFFECT_PROP => BuiltinProp::VisualEffect,
            _ => BuiltinProp::Plain,
        }
    }
}

/// A service the manager holds (§A2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropService {
    /// Global `0x00B43D0C` (saved).
    OccupantManager,
    /// Global `0x00B43CA8` (not saved).
    ResourceManager,
    /// Global `0x00B43D08` (saved).
    LotManager,
}

/// A cell rectangle `(x1, z1, x2, z2)` (`SC4Rect<int32>`).
pub type CellRect = (i32, i32, i32, i32);
/// A world rectangle `(x1, z1, x2, z2)` (`SC4Rect<float>`).
pub type WorldRect = (f32, f32, f32, f32);

/// The building-prop property of a building (§A7.1 step 3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildingProps {
    /// The building has no property holder.
    NoHolder,
    /// The holder has no property `0xA8322B7F`.
    Absent,
    /// The property's vector of prop pointers, in order.
    Present(Vec<u64>),
}

/// One argument of the `CreateTextDecal` command: its text and its value as
/// a float (the host converts).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommandArg {
    pub text: String,
    pub value: f32,
}

/// A message as `DoMessage` sees it (§A4.3).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PropMessage {
    pub msg_type: u32,
    /// `*void2` of `PostViewFilterChange`: the new view filter (`None` =
    /// null).
    pub view_filter: Option<u64>,
    /// The command's arguments (`CreateTextDecal`).
    pub args: Vec<CommandArg>,
}

impl PropMessage {
    pub fn of_type(msg_type: u32) -> PropMessage {
        PropMessage {
            msg_type,
            ..PropMessage::default()
        }
    }
}

/// Everything the prop manager reads from, or does to, the rest of the game.
/// Objects are host tokens. Re-entrancy: callbacks must not call back into
/// the manager.
pub trait PropHost {
    // ---- the catalogue (§A3) ----
    /// The resource manager's keys of type `0x6534284A` (slot 18), in its
    /// order; `None` when there is no resource manager.
    fn exemplar_keys(&self) -> Option<Vec<ResKey>>;
    /// Loads an exemplar (resource manager slot 3, IID `0xA52160F5`) through
    /// the global resource manager.
    fn load_exemplar(&self, key: ResKey) -> Option<Rc<PropertyHolder>>;
    /// Registers the catalogue's shutdown callback with the application
    /// (slot 69).
    fn register_shutdown_callback(&self);
    /// Posts `kSC4MessagePropExemplarDataUpdated` (§A3.4) when the message
    /// server exists: void1 and the `cIGZUnknown` field = the manager
    /// (`manager`, its host token), data2 = `data2`.
    fn post_exemplar_data_updated(&self, manager: u64, data2: u32);

    // ---- lifecycle (§A4) ----
    /// `0x00B43CB0` slot 6 / slot 9.
    fn register_command(&self, id: u32);
    fn unregister_command(&self, id: u32);
    fn subscribe(&self, msg_type: u32);
    fn unsubscribe(&self, msg_type: u32);
    /// The service's global, with a new reference; `None` when null.
    fn acquire_service(&self, service: PropService) -> Option<u64>;
    /// The 3D view's view filter (slot 18), with a reference; `None` when
    /// null.
    fn view_filter(&self) -> Option<u64>;
    fn add_ref(&self, token: u64);
    fn release(&self, token: u64);

    // ---- creating props (§A6) ----
    /// GZCOM: an instance of `class_id` with IID `0xE9793A65`, one reference;
    /// `None` when no such class exists.
    fn create_prop_instance(&self, class_id: u32) -> Option<u64>;
    /// One of the three built-in classes, one reference.
    fn create_builtin_prop(&self, kind: BuiltinProp) -> u64;
    /// Prop slot 8 `SetPropType`.
    fn set_prop_type(&self, prop: u64, id: u32);
    /// §A6.1 step 5 up to the bounding box: through the prop's property
    /// holder (occupant slot 6) ask for interface `0x0AC2B5F7`, load the
    /// exemplar at `key` (through the manager's resource manager, or the
    /// global one when `global`) and hand it to that interface (slot 3).
    /// Returns the exemplar's properties; `None` when a piece is missing (no
    /// interface, a failed load).
    fn attach_exemplar(&self, prop: u64, key: ResKey, global: bool) -> Option<Rc<PropertyHolder>>;
    /// Prop slot 4 `SetBoundingBox(min, max)`.
    fn set_bounding_box(&self, prop: u64, min: [f32; 3], max: [f32; 3]);

    // ---- occupants (§A7) ----
    /// Occupant slot `IsFlagSet(flag)`.
    fn occupant_flag(&self, occ: u64, flag: u32) -> bool;
    /// Occupant slot 3 `Init`.
    fn occupant_init(&self, occ: u64) -> bool;
    /// Occupant slot 4 `Shutdown`.
    fn occupant_shutdown(&self, occ: u64);
    /// `QueryInterface(0xE9793A65)` and prop slot 23 `SetPropUsage(usage)`.
    fn set_prop_usage(&self, occ: u64, usage: u32);
    /// Occupant slot 9 `SetPosition`.
    fn set_position(&self, occ: u64, pos: [f32; 3]);
    /// Prop slot 10 `SetOrientation`.
    fn set_orientation(&self, occ: u64, orientation: i32);
    /// Occupant slot 8 `GetPosition`.
    fn position(&self, occ: u64) -> [f32; 3];
    /// The flag-2 part of "removing one prop" (§A7.3): the placeable object
    /// (occupant slot 19), its slot 11, clear flag 2 (occupant slot 31).
    fn hide(&self, occ: u64);

    // ---- the occupant manager ----
    /// Slot 20 `InsertOccupant(occ, false)`; false without an occupant
    /// manager.
    fn insert_occupant(&self, occ: u64) -> bool;
    /// Slot 21 `RemoveOccupant(occ, 0, 0)`.
    fn remove_occupant(&self, occ: u64);
    /// Slot 36 with the box `min`–`max`, the filter at `usage` and mask
    /// `0xFFFFFFFF`: the props found, one reference each for the caller;
    /// `None` when the call returns false.
    fn occupants_in_box(&self, min: [f32; 3], max: [f32; 3], usage: u32) -> Option<Vec<u64>>;
    /// Slot 40 on the cell rectangle with the filter at `usage` and mask
    /// `0xFFFFFFFF`, as [`occupants_in_box`](Self::occupants_in_box).
    fn occupants_in_cell_rect(&self, rect: CellRect, usage: u32) -> Option<Vec<u64>>;
    /// Slot 33 with type `0x278128A0`: the building at the cell, with a
    /// reference; `None` when there is none.
    fn building_at_cell(&self, x: i32, z: i32) -> Option<u64>;

    // ---- lots and the city ----
    /// Lot manager slot 6 `GetLot(x, z, false)` (no reference).
    fn lot_at_cell(&self, x: i32, z: i32) -> Option<u64>;
    /// Lot slot 14: the lot's cell rectangle.
    fn lot_cell_rect(&self, lot: u64) -> CellRect;
    /// City slot 96: a cell corner to world (x, z).
    fn cell_to_world(&self, x: i32, z: i32) -> (f32, f32);
    /// City slot 95: a world position to its cell.
    fn world_to_cell(&self, x: f32, z: f32) -> (i32, i32);

    // ---- the building-prop property (§A7.1 step 3) ----
    /// The building's property `0xA8322B7F` (through occupant slot 6 and
    /// holder slot 6).
    fn building_props(&self, building: u64) -> BuildingProps;
    /// Replaces the vector of the existing property object.
    fn set_building_props(&self, building: u64, props: &[u64]);
    /// Creates the property object (0x14 bytes, vtable `0x00A8F9CC`) holding
    /// `props` and adds it (holder slot 11 `AddProperty(0xA8322B7F, obj,
    /// false)`).
    fn add_building_props_property(&self, building: u64, props: &[u64]);
    /// Holder slot 17 `RemoveProperty(0xA8322B7F, false)`.
    fn remove_building_props_property(&self, building: u64);

    // ---- the CreateTextDecal command (§A4.3) ----
    /// A text decal occupant (`0x005F8300`, 0xBC bytes), one reference.
    fn create_text_decal(&self) -> Option<u64>;
    /// `cISTETerrain` slot 13: the altitude at (x, z).
    fn terrain_altitude(&self, x: f32, z: f32) -> f32;
    /// Decal slot 4 (the text).
    fn decal_set_text(&self, decal: u64, text: &str);
    /// Decal slot 8 (three floats), slot 10 and slot 6 (PL1).
    fn decal_slot8(&self, decal: u64, values: [f32; 3]);
    fn decal_slot10(&self, decal: u64, value: f32);
    fn decal_slot6(&self, decal: u64, value: f32);

    // ---- save references (§A8) ----
    fn object_ref(&self, token: u64) -> RefValue;
    /// Resolves a reference read with `iid`; the token comes with one
    /// reference.
    fn resolve_ref(&self, r: &RefValue, iid: u32) -> Option<u64>;
}
