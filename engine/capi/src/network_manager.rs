// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The network manager and the network lot manager over the C ABI (fc4.h
//! "network manager", docs/specs/network-manager.md).
//!
//! The host implements [`fc4_netmgr_host`] (cSC4NetworkManager, component
//! 53) and [`fc4_netlot_host`] (cSC4NetworkLotManager, component 11); the
//! plugin's objects forward their slots here. The riot's removal lock (§10)
//! is not here: in game it belongs to the riot manager, which the disaster
//! layer carries (`fc4_disasters`); `fc4_core::network_manager::RiotLock` is
//! the same contract for the §16 tests. The network lot configurations are host inputs
//! handed over before `Init` ([`fc4_netlot_add_configuration`] /
//! [`fc4_netlot_add_configuration_props`]).
//!
//! Tokens are opaque host identities (0 = none): network occupants, tools,
//! construction crews, lots, street props, flora and effect props. The
//! traffic graph built from the occupants is traffic-network.md's
//! (`fc4_traffic`); the lot manager only asks the host for the map's
//! `FindNetworkTile` / `UpdateIntersection`.
//!
//! Threads: the cores are single-threaded (`RefCell`). The plugin serialises
//! every call on one object (fc4.h). A construction crew's slot 5
//! (`crew_shutdown`) may call [`fc4_netmgr_unregister_construction_crew`] on
//! the same object; no other callback may call back into the object.

use std::cell::RefCell;
use std::ffi::{c_char, c_void, CString};
use std::rc::Rc;

use fc4_core::grid::CellRect;
use fc4_core::network_manager::{
    clear_network_flag, corner_mismatch, lot_manager_query_interface, manager_query_interface, pipe_model,
    set_network_flag, terrain_rect, FlagChange, LotEntry, NetworkLotConfig, NetworkLotHost, NetworkLotManager,
    NetworkLotMessage, NetworkLotObject, NetworkManager, NetworkManagerHost, NetworkMessage, PipeModel, Position,
};
use fc4_core::stream::{RefValue, SaveObject};

use crate::budget::fc4_token_callback;
use crate::demand::fc4_u32_callback;
use crate::guard;
use crate::hostgrid::{collect_tokens, fc4_cell_rect};
use crate::props::{fc4_props, props_ref};
use crate::stream::{fc4_istream, fc4_ostream, istream_ok, ostream_ok, CIn, COut};

type U = *mut c_void;

#[repr(C)]
pub struct fc4_netmgr {
    _private: [u8; 0],
}

#[repr(C)]
pub struct fc4_netlot {
    _private: [u8; 0],
}

// ---- the network manager's host --------------------------------------------------------------------

/// What the network manager needs from the game (fc4.h `fc4_netmgr_host`).
/// A missing callback reads as 0 / false / none.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct fc4_netmgr_host {
    pub struct_size: u32,
    pub user: *mut c_void,
    // messages and the city
    pub subscribe: Option<unsafe extern "C" fn(U, u32)>,
    pub unsubscribe: Option<unsafe extern "C" fn(U, u32)>,
    pub uk_paths: Option<unsafe extern "C" fn(U) -> bool>,
    pub city_size: Option<unsafe extern "C" fn(U, *mut i32, *mut i32)>,
    pub is_night: Option<unsafe extern "C" fn(U) -> bool>,
    pub view_rotation_changed: Option<unsafe extern "C" fn(U)>,
    // references
    pub add_ref: Option<unsafe extern "C" fn(U, u64)>,
    pub release: Option<unsafe extern "C" fn(U, u64)>,
    pub resolve_ref: Option<unsafe extern "C" fn(U, u64, u32) -> u64>,
    // tools
    pub create_tool: Option<unsafe extern "C" fn(U, u32) -> u64>,
    pub tool_init: Option<unsafe extern "C" fn(U, u64)>,
    pub tool_shutdown: Option<unsafe extern "C" fn(U, u64)>,
    pub tool_destroy_occupants: Option<unsafe extern "C" fn(U, u64, *const u64, usize)>,
    pub tool_update_underground_heights: Option<unsafe extern "C" fn(U, u64, *const i32, *const u64, usize, u32)>,
    pub tool_cell_info: Option<unsafe extern "C" fn(U, u64, u32) -> u64>,
    pub tool_reinsert_piece: Option<unsafe extern "C" fn(U, u64, u64)>,
    // network occupants
    pub query_network_occupant: Option<unsafe extern "C" fn(U, u64) -> u64>,
    pub is_network_occupant: Option<unsafe extern "C" fn(U, u64) -> bool>,
    pub network_flags: Option<unsafe extern "C" fn(U, u64) -> u32>,
    pub is_of_type: Option<unsafe extern "C" fn(U, u64, u32) -> bool>,
    pub occupant_flags: Option<unsafe extern "C" fn(U, u64) -> u32>,
    pub occupant_flag_set: Option<unsafe extern "C" fn(U, u64, u32) -> bool>,
    pub occupied_cell: Option<unsafe extern "C" fn(U, u64, *mut i32, *mut i32)>,
    pub occupant_altitude: Option<unsafe extern "C" fn(U, u64, f32, f32) -> f32>,
    pub is_managed_globally: Option<unsafe extern "C" fn(U, u64) -> bool>,
    pub can_be_lit: Option<unsafe extern "C" fn(U, u64) -> bool>,
    pub set_lit: Option<unsafe extern "C" fn(U, u64, bool)>,
    pub check_for_retrofit: Option<unsafe extern "C" fn(U, u64) -> bool>,
    pub retrofit: Option<unsafe extern "C" fn(U, u64)>,
    pub creation_number: Option<unsafe extern "C" fn(U, u64) -> u64>,
    // the occupant manager and the terrain
    pub remove_occupant: Option<unsafe extern "C" fn(U, u64) -> bool>,
    pub occupants_in_rect: Option<unsafe extern "C" fn(U, *const fc4_cell_rect, u32, fc4_token_callback, *mut c_void)>,
    pub network_occupants: Option<unsafe extern "C" fn(U, fc4_token_callback, *mut c_void)>,
    pub post_refresh: Option<unsafe extern "C" fn(U, u64)>,
    pub terrain_altitude: Option<unsafe extern "C" fn(U, f32, f32) -> f32>,
    // construction crews
    pub crew_shutdown: Option<unsafe extern "C" fn(U, u64)>,
}

/// Diagnostics (`fc4_netmgr_get_state`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct fc4_netmgr_state {
    pub struct_size: u32,
    pub initialized: u8,
    pub destroyed: u8,
    pub uk_paths: u8,
    pub long_terrain_op: u8,
    pub queueing: u8,
    pub reserved: [u8; 3],
    pub ref_count: u32,
    pub managed: u32,
    pub pending: u32,
    pub retrofit: u32,
    pub crews: u32,
    /// Cached tools that are not null.
    pub tools: u32,
    /// The accumulated terrain-change area (`0x7FFFFFFF, 0x7FFFFFFF, -1, -1`
    /// when empty).
    pub dirty: fc4_cell_rect,
}

/// A network flag change (`fc4_network_set_flag` / `_clear_flag`, §5).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct fc4_network_flag_change {
    pub flags: u32,
    /// The data1 of the `0x89D3E9F3` to post, or -1 for none.
    pub usable_post: i32,
    pub translucency: u8,
    pub reserved: [u8; 3],
}

// ---- the network lot manager's host ----------------------------------------------------------------

/// What the network lot manager needs from the game (fc4.h
/// `fc4_netlot_host`). A missing callback reads as 0 / false / none.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct fc4_netlot_host {
    pub struct_size: u32,
    pub user: *mut c_void,
    // messages and references
    pub subscribe: Option<unsafe extern "C" fn(U, u32)>,
    pub unsubscribe: Option<unsafe extern "C" fn(U, u32)>,
    pub add_ref: Option<unsafe extern "C" fn(U, u64)>,
    pub release: Option<unsafe extern "C" fn(U, u64)>,
    pub resolve_ref: Option<unsafe extern "C" fn(U, u64, u32) -> u64>,
    // network occupants
    pub query_network_occupant: Option<unsafe extern "C" fn(U, u64) -> u64>,
    pub network_flags: Option<unsafe extern "C" fn(U, u64) -> u32>,
    pub piece_id: Option<unsafe extern "C" fn(U, u64) -> u32>,
    pub rotation: Option<unsafe extern "C" fn(U, u64) -> i32>,
    pub flip: Option<unsafe extern "C" fn(U, u64) -> i32>,
    pub position_cell: Option<unsafe extern "C" fn(U, u64, *mut i32, *mut i32)>,
    pub occupied_cell: Option<unsafe extern "C" fn(U, u64, *mut i32, *mut i32)>,
    pub occupant_altitude: Option<unsafe extern "C" fn(U, u64, f32, f32) -> f32>,
    pub is_intersection: Option<unsafe extern "C" fn(U, u64) -> bool>,
    pub edge_types_flag: Option<unsafe extern "C" fn(U, u64, u32) -> u32>,
    pub active_edges: Option<unsafe extern "C" fn(U, u64) -> u32>,
    pub set_network_flag: Option<unsafe extern "C" fn(U, u64, u32)>,
    pub set_under_texture: Option<unsafe extern "C" fn(U, u64, u32)>,
    pub set_variation: Option<unsafe extern "C" fn(U, u64, u32)>,
    // the traffic network map
    pub traffic_map_exists: Option<unsafe extern "C" fn(U) -> bool>,
    pub find_network_tile: Option<unsafe extern "C" fn(U, i32, i32, u32) -> u64>,
    pub update_intersection: Option<unsafe extern "C" fn(U, u64)>,
    // the city, the terrain, zones and lots
    pub city_size: Option<unsafe extern "C" fn(U, *mut i32, *mut i32)>,
    pub cell_center: Option<unsafe extern "C" fn(U, i32, i32, *mut f32, *mut f32)>,
    pub terrain_altitude: Option<unsafe extern "C" fn(U, f32, f32) -> f32>,
    pub zone_type: Option<unsafe extern "C" fn(U, i32, i32) -> i32>,
    pub lot_at: Option<unsafe extern "C" fn(U, i32, i32) -> u64>,
    pub lot_wealth: Option<unsafe extern "C" fn(U, u64) -> i32>,
    pub lot_is_livable: Option<unsafe extern "C" fn(U, u64) -> bool>,
    pub lot_building_type: Option<unsafe extern "C" fn(U, u64) -> u32>,
    pub building_profile_wealth: Option<unsafe extern "C" fn(U, u32, *mut i32) -> bool>,
    pub query_lot: Option<unsafe extern "C" fn(U, u64) -> u64>,
    pub lot_bounding_rect: Option<unsafe extern "C" fn(U, u64, *mut fc4_cell_rect)>,
    pub area_slope: Option<unsafe extern "C" fn(U, i32, i32) -> f32>,
    // street props and flora
    pub remove_street_props: Option<unsafe extern "C" fn(U, i32, i32)>,
    pub add_street_prop: Option<unsafe extern "C" fn(U, u32, *const f32, u32) -> u64>,
    pub street_prop_slot28: Option<unsafe extern "C" fn(U, u64, u32)>,
    pub street_prop_slot29: Option<unsafe extern "C" fn(U, u64, u32)>,
    pub demolish_flora: Option<unsafe extern "C" fn(U, i32, i32)>,
    pub add_flora: Option<unsafe extern "C" fn(U, u32, *const f32) -> u64>,
    pub flora_slot28: Option<unsafe extern "C" fn(U, u64, u32)>,
    pub object_ids: Option<unsafe extern "C" fn(U, u64, u32, fc4_u32_callback, *mut c_void)>,
    // potholes
    pub create_effect_prop: Option<unsafe extern "C" fn(U) -> u64>,
    pub effect_set_name: Option<unsafe extern "C" fn(U, u64, *const c_char)>,
    pub effect_start: Option<unsafe extern "C" fn(U, u64)>,
    pub effect_stop: Option<unsafe extern "C" fn(U, u64)>,
    pub set_position: Option<unsafe extern "C" fn(U, u64, *const f32)>,
    pub insert_occupant: Option<unsafe extern "C" fn(U, u64)>,
    pub remove_occupant: Option<unsafe extern "C" fn(U, u64)>,
    pub first_network_occupant_at: Option<unsafe extern "C" fn(U, i32, i32, u32) -> u64>,
}

/// One network lot configuration object (§8.6), as the host built it.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct fc4_netlot_object {
    /// 1 prop, 4 flora.
    pub kind: u8,
    /// Bit 0 set: no prop is created.
    pub flags: u8,
    pub orientation: u8,
    pub reserved: u8,
    pub offset: [f32; 3],
    /// Passed to a created prop's slot 28.
    pub field_24: u32,
}

/// A network lot configuration (§2.4).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct fc4_netlot_config {
    pub struct_size: u32,
    pub piece_id: u32,
    /// The host's identity of the configuration (for `object_ids`).
    pub token: u64,
    pub id: u32,
    /// Bit z: zone type z allowed.
    pub zones: u16,
    /// Bit w: wealth w allowed.
    pub wealths: u8,
    pub period: u8,
    pub mask: [u8; 4],
    pub rotations: u8,
    pub flip_rule: u8,
    pub reserved: u16,
    pub min_slope: f32,
    pub max_slope: f32,
    pub object_count: u32,
    pub objects: *const fc4_netlot_object,
}

/// A network lot (`fc4_netlot_get_lot`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct fc4_netlot_lot {
    /// The network occupant (0 = a reference that did not resolve on load).
    pub occupant: u64,
    /// The configuration ID (the live configuration's, or the one read).
    pub config_id: u32,
    /// The configuration is resolved (false only between Read and Init).
    pub has_config: u8,
    pub locked: u8,
    pub reserved: u16,
}

/// Diagnostics (`fc4_netlot_get_state`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct fc4_netlot_state {
    pub struct_size: u32,
    pub initialized: u8,
    pub destroyed: u8,
    pub has_forced: u8,
    pub reserved: u8,
    pub ref_count: u32,
    pub lots: u32,
    pub potholes: u32,
    pub pending: u32,
    pub configs: u32,
    pub forced_id: u32,
}

pub type fc4_netlot_visitor =
    Option<unsafe extern "C" fn(ctx: *mut c_void, x: i32, z: i32, lot: *const fc4_netlot_lot) -> bool>;

const P4: bool = cfg!(target_pointer_width = "32");

const _: () = {
    assert!(std::mem::size_of::<fc4_netmgr_state>() == 52);
    assert!(std::mem::size_of::<fc4_network_flag_change>() == 12);
    assert!(std::mem::size_of::<fc4_netlot_object>() == 20);
    assert!(std::mem::size_of::<fc4_netlot_config>() == if P4 { 48 } else { 56 });
    assert!(std::mem::size_of::<fc4_netlot_lot>() == 16);
    assert!(std::mem::size_of::<fc4_netlot_state>() == 32);
    assert!(std::mem::size_of::<fc4_netmgr_host>() == if P4 { 8 + 36 * 4 } else { 16 + 36 * 8 });
    assert!(std::mem::size_of::<fc4_netlot_host>() == if P4 { 8 + 50 * 4 } else { 16 + 50 * 8 });
};

macro_rules! cb {
    ($s:expr, $f:ident, $default:expr $(, $arg:expr)*) => {
        // SAFETY: whoever created the object promised valid callbacks.
        match $s.h.$f { Some(f) => unsafe { f($s.h.user $(, $arg)*) }, None => $default }
    };
}

fn opt(t: u64) -> Option<u64> {
    (t != 0).then_some(t)
}

fn object_ref(token: u64) -> RefValue {
    if token == 0 {
        RefValue::Null
    } else {
        RefValue::Host { token, class_id: 0 }
    }
}

fn pair(f: impl FnOnce(*mut i32, *mut i32)) -> (i32, i32) {
    let (mut a, mut b) = (0i32, 0i32);
    f(&mut a, &mut b);
    (a, b)
}

// ---- network manager adapter -------------------------------------------------------------------------

struct MHost {
    h: fc4_netmgr_host,
}

impl NetworkManagerHost for MHost {
    fn subscribe(&self, msg_type: u32) {
        cb!(self, subscribe, (), msg_type)
    }
    fn unsubscribe(&self, msg_type: u32) {
        cb!(self, unsubscribe, (), msg_type)
    }
    fn uk_paths(&self) -> bool {
        cb!(self, uk_paths, false)
    }
    fn city_size(&self) -> (i32, i32) {
        pair(|a, b| cb!(self, city_size, (), a, b))
    }
    fn is_night(&self) -> bool {
        cb!(self, is_night, false)
    }
    fn view_rotation_changed(&self) {
        cb!(self, view_rotation_changed, ())
    }
    fn add_ref(&self, token: u64) {
        cb!(self, add_ref, (), token)
    }
    fn release(&self, token: u64) {
        cb!(self, release, (), token)
    }
    fn create_tool(&self, network_type: u32) -> Option<u64> {
        opt(cb!(self, create_tool, 0, network_type))
    }
    fn tool_init(&self, tool: u64) {
        cb!(self, tool_init, (), tool)
    }
    fn tool_shutdown(&self, tool: u64) {
        cb!(self, tool_shutdown, (), tool)
    }
    fn tool_destroy_occupants(&self, tool: u64, occupants: &[u64]) {
        cb!(
            self,
            tool_destroy_occupants,
            (),
            tool,
            occupants.as_ptr(),
            occupants.len()
        )
    }
    fn tool_update_underground_heights(&self, tool: u64, cells: &[(i32, i32)], occupants: &[u64], network_type: u32) {
        let flat: Vec<i32> = cells.iter().flat_map(|&(x, z)| [x, z]).collect();
        cb!(
            self,
            tool_update_underground_heights,
            (),
            tool,
            flat.as_ptr(),
            occupants.as_ptr(),
            occupants.len(),
            network_type
        )
    }
    fn tool_cell_info(&self, tool: u64, packed: u32) -> u64 {
        cb!(self, tool_cell_info, 0, tool, packed)
    }
    fn tool_reinsert_piece(&self, tool: u64, info: u64) {
        cb!(self, tool_reinsert_piece, (), tool, info)
    }
    fn query_network_occupant(&self, occupant: u64) -> Option<u64> {
        if occupant == 0 {
            return None;
        }
        opt(cb!(self, query_network_occupant, 0, occupant))
    }
    fn is_network_occupant(&self, occupant: u64) -> bool {
        cb!(self, is_network_occupant, false, occupant)
    }
    fn network_flags(&self, n: u64) -> u32 {
        cb!(self, network_flags, 0, n)
    }
    fn is_of_type(&self, n: u64, network_type: u32) -> bool {
        cb!(self, is_of_type, false, n, network_type)
    }
    fn occupant_flags(&self, occupant: u64) -> u32 {
        cb!(self, occupant_flags, 0, occupant)
    }
    fn occupant_flag_set(&self, occupant: u64, flag: u32) -> bool {
        cb!(self, occupant_flag_set, false, occupant, flag)
    }
    fn occupied_cell(&self, n: u64) -> (i32, i32) {
        pair(|a, b| cb!(self, occupied_cell, (), n, a, b))
    }
    fn occupant_altitude(&self, n: u64, x: f32, z: f32) -> f32 {
        cb!(self, occupant_altitude, 0.0, n, x, z)
    }
    fn is_managed_globally(&self, n: u64) -> bool {
        cb!(self, is_managed_globally, false, n)
    }
    fn can_be_lit(&self, n: u64) -> bool {
        cb!(self, can_be_lit, false, n)
    }
    fn set_lit(&self, n: u64, lit: bool) {
        cb!(self, set_lit, (), n, lit)
    }
    fn check_for_retrofit(&self, n: u64) -> bool {
        cb!(self, check_for_retrofit, false, n)
    }
    fn retrofit(&self, n: u64) {
        cb!(self, retrofit, (), n)
    }
    fn creation_number(&self, occupant: u64) -> u64 {
        cb!(self, creation_number, occupant, occupant)
    }
    fn remove_occupant(&self, occupant: u64) -> bool {
        cb!(self, remove_occupant, false, occupant)
    }
    fn occupants_in_rect(&self, rect: CellRect, mask: u32) -> Vec<u64> {
        let r = fc4_cell_rect::from(rect);
        collect_tokens(|f, ctx| cb!(self, occupants_in_rect, (), &r, mask, f, ctx))
    }
    fn network_occupants(&self) -> Vec<u64> {
        collect_tokens(|f, ctx| cb!(self, network_occupants, (), f, ctx))
    }
    fn post_refresh(&self, n: u64) {
        cb!(self, post_refresh, (), n)
    }
    fn terrain_altitude(&self, x: f32, z: f32) -> f32 {
        cb!(self, terrain_altitude, 0.0, x, z)
    }
    fn crew_shutdown(&self, crew: u64) {
        cb!(self, crew_shutdown, (), crew)
    }
    fn object_ref(&self, token: u64) -> RefValue {
        object_ref(token)
    }
    fn resolve_ref(&self, r: &RefValue, iid: u32) -> Option<u64> {
        match *r {
            RefValue::Host { token, .. } if token != 0 => opt(cb!(self, resolve_ref, 0, token, iid)),
            _ => None,
        }
    }
}

// ---- network lot manager adapter ---------------------------------------------------------------------

struct LHost {
    h: fc4_netlot_host,
    configs: RefCell<Vec<NetworkLotConfig>>,
}

unsafe extern "C" fn push_u32(ctx: *mut c_void, v: u32) {
    if let Some(out) = (ctx as *mut Vec<u32>).as_mut() {
        out.push(v);
    }
}

impl NetworkLotHost for LHost {
    fn subscribe(&self, msg_type: u32) {
        cb!(self, subscribe, (), msg_type)
    }
    fn unsubscribe(&self, msg_type: u32) {
        cb!(self, unsubscribe, (), msg_type)
    }
    fn load_configurations(&self) -> Vec<NetworkLotConfig> {
        self.configs.borrow().clone()
    }
    fn add_ref(&self, token: u64) {
        cb!(self, add_ref, (), token)
    }
    fn release(&self, token: u64) {
        cb!(self, release, (), token)
    }
    fn query_network_occupant(&self, unknown: u64) -> Option<u64> {
        if unknown == 0 {
            return None;
        }
        opt(cb!(self, query_network_occupant, 0, unknown))
    }
    fn network_flags(&self, n: u64) -> u32 {
        cb!(self, network_flags, 0, n)
    }
    fn piece_id(&self, n: u64) -> u32 {
        cb!(self, piece_id, 0, n)
    }
    fn rotation(&self, n: u64) -> i32 {
        cb!(self, rotation, 0, n)
    }
    fn flip(&self, n: u64) -> i32 {
        cb!(self, flip, 0, n)
    }
    fn position_cell(&self, n: u64) -> (i32, i32) {
        pair(|a, b| cb!(self, position_cell, (), n, a, b))
    }
    fn occupied_cell(&self, n: u64) -> (i32, i32) {
        pair(|a, b| cb!(self, occupied_cell, (), n, a, b))
    }
    fn occupant_altitude(&self, n: u64, x: f32, z: f32) -> f32 {
        cb!(self, occupant_altitude, 0.0, n, x, z)
    }
    fn is_intersection(&self, n: u64) -> bool {
        cb!(self, is_intersection, false, n)
    }
    fn edge_types_flag(&self, n: u64, kind: u32) -> u32 {
        cb!(self, edge_types_flag, 0, n, kind)
    }
    fn active_edges(&self, n: u64) -> u32 {
        cb!(self, active_edges, 0, n)
    }
    fn set_network_flag(&self, n: u64, bits: u32) {
        cb!(self, set_network_flag, (), n, bits)
    }
    fn set_under_texture(&self, n: u64, texture: u32) {
        cb!(self, set_under_texture, (), n, texture)
    }
    fn set_variation(&self, n: u64, variation: u32) {
        cb!(self, set_variation, (), n, variation)
    }
    fn traffic_map_exists(&self) -> bool {
        cb!(self, traffic_map_exists, false)
    }
    fn find_network_tile(&self, x: i32, z: i32, mask: u32) -> Option<u64> {
        opt(cb!(self, find_network_tile, 0, x, z, mask))
    }
    fn update_intersection(&self, n: u64) {
        cb!(self, update_intersection, (), n)
    }
    fn city_size(&self) -> (i32, i32) {
        pair(|a, b| cb!(self, city_size, (), a, b))
    }
    fn cell_center(&self, x: i32, z: i32) -> (f32, f32) {
        let (mut a, mut b) = (0.0f32, 0.0f32);
        cb!(self, cell_center, (), x, z, &mut a, &mut b);
        (a, b)
    }
    fn terrain_altitude(&self, x: f32, z: f32) -> f32 {
        cb!(self, terrain_altitude, 0.0, x, z)
    }
    fn zone_type(&self, x: i32, z: i32) -> i32 {
        cb!(self, zone_type, 0, x, z)
    }
    fn lot_at(&self, x: i32, z: i32) -> Option<u64> {
        opt(cb!(self, lot_at, 0, x, z))
    }
    fn lot_wealth(&self, lot: u64) -> i32 {
        cb!(self, lot_wealth, 0, lot)
    }
    fn lot_is_livable(&self, lot: u64) -> bool {
        cb!(self, lot_is_livable, false, lot)
    }
    fn lot_building_type(&self, lot: u64) -> u32 {
        cb!(self, lot_building_type, 0, lot)
    }
    fn building_profile_wealth(&self, building_type: u32) -> Option<i32> {
        let mut w = 0i32;
        cb!(self, building_profile_wealth, false, building_type, &mut w).then_some(w)
    }
    fn query_lot(&self, unknown: u64) -> Option<u64> {
        if unknown == 0 {
            return None;
        }
        opt(cb!(self, query_lot, 0, unknown))
    }
    fn lot_bounding_rect(&self, lot: u64) -> CellRect {
        let mut r = fc4_cell_rect {
            x1: 0,
            z1: 0,
            x2: -1,
            z2: -1,
        };
        cb!(self, lot_bounding_rect, (), lot, &mut r);
        r.into()
    }
    fn area_slope(&self, x: i32, z: i32) -> f32 {
        cb!(self, area_slope, 0.0, x, z)
    }
    fn remove_street_props(&self, x: i32, z: i32) {
        cb!(self, remove_street_props, (), x, z)
    }
    fn add_street_prop(&self, id: u32, pos: Position, orientation: u32) -> Option<u64> {
        opt(cb!(self, add_street_prop, 0, id, pos.as_ptr(), orientation))
    }
    fn street_prop_slot28(&self, prop: u64, value: u32) {
        cb!(self, street_prop_slot28, (), prop, value)
    }
    fn street_prop_slot29(&self, prop: u64, value: u32) {
        cb!(self, street_prop_slot29, (), prop, value)
    }
    fn demolish_flora(&self, x: i32, z: i32) {
        cb!(self, demolish_flora, (), x, z)
    }
    fn add_flora(&self, id: u32, pos: Position) -> Option<u64> {
        opt(cb!(self, add_flora, 0, id, pos.as_ptr()))
    }
    fn flora_slot28(&self, flora: u64, index: u32) {
        cb!(self, flora_slot28, (), flora, index)
    }
    fn object_ids(&self, config: u64, index: usize) -> Vec<u32> {
        let mut v: Vec<u32> = Vec::new();
        let ctx = &mut v as *mut Vec<u32> as *mut c_void;
        cb!(self, object_ids, (), config, index as u32, Some(push_u32), ctx);
        v
    }
    fn create_effect_prop(&self) -> Option<u64> {
        opt(cb!(self, create_effect_prop, 0))
    }
    fn effect_set_name(&self, prop: u64, name: &str) {
        let n = CString::new(name).unwrap_or_default();
        cb!(self, effect_set_name, (), prop, n.as_ptr())
    }
    fn effect_start(&self, prop: u64) {
        cb!(self, effect_start, (), prop)
    }
    fn effect_stop(&self, prop: u64) {
        cb!(self, effect_stop, (), prop)
    }
    fn set_position(&self, prop: u64, pos: Position) {
        cb!(self, set_position, (), prop, pos.as_ptr())
    }
    fn insert_occupant(&self, occupant: u64) {
        cb!(self, insert_occupant, (), occupant)
    }
    fn remove_occupant(&self, occupant: u64) {
        cb!(self, remove_occupant, (), occupant)
    }
    fn first_network_occupant_at(&self, x: i32, z: i32, mask: u32) -> Option<u64> {
        opt(cb!(self, first_network_occupant_at, 0, x, z, mask))
    }
    fn object_ref(&self, token: u64) -> RefValue {
        object_ref(token)
    }
    fn resolve_ref(&self, r: &RefValue, iid: u32) -> Option<u64> {
        match *r {
            RefValue::Host { token, .. } if token != 0 => opt(cb!(self, resolve_ref, 0, token, iid)),
            _ => None,
        }
    }
}

// ---- handles -------------------------------------------------------------------------------------------

struct MHandle {
    sim: Rc<NetworkManager>,
}

struct LHandle {
    sim: Rc<NetworkLotManager>,
    host: Rc<LHost>,
}

fn with_m<R>(p: *const fc4_netmgr, fallback: R, f: impl FnOnce(&MHandle) -> R) -> R {
    // SAFETY: documented precondition: NULL or a live fc4_netmgr*.
    match unsafe { (p as *const MHandle).as_ref() } {
        Some(h) => guard(fallback, || f(h)),
        None => fallback,
    }
}

fn with_l<R>(p: *const fc4_netlot, fallback: R, f: impl FnOnce(&LHandle) -> R) -> R {
    // SAFETY: documented precondition: NULL or a live fc4_netlot*.
    match unsafe { (p as *const LHandle).as_ref() } {
        Some(h) => guard(fallback, || f(h)),
        None => fallback,
    }
}

unsafe fn copy_out<T: Copy>(v: &[T], out: *mut T, capacity: usize) -> usize {
    if !out.is_null() {
        for (i, x) in v.iter().take(capacity).enumerate() {
            *out.add(i) = *x;
        }
    }
    v.len()
}

unsafe fn slice<'a, T>(p: *const T, n: usize) -> &'a [T] {
    if p.is_null() || n == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(p, n)
    }
}

fn no_core_objects(_: u32, _: *mut c_void) -> Option<Rc<dyn SaveObject>> {
    None
}

// ---- pure helpers --------------------------------------------------------------------------------------

fn change_to_c(c: FlagChange) -> fc4_network_flag_change {
    fc4_network_flag_change {
        flags: c.flags,
        usable_post: c.usable_post.map_or(-1, |v| v as i32),
        translucency: c.translucency.into(),
        reserved: [0; 3],
    }
}

/// `SetNetworkFlag(bits)` on an occupant with `flags` (§5): the new flags,
/// the usable message to post (data1 1 when USABLE is newly set on a piece
/// in the city), and whether the translucency changes.
#[no_mangle]
pub unsafe extern "C" fn fc4_network_set_flag(
    flags: u32,
    bits: u32,
    in_city: bool,
    out: *mut fc4_network_flag_change,
) -> bool {
    let Some(o) = out.as_mut() else { return false };
    guard(false, || {
        *o = change_to_c(set_network_flag(flags, bits, in_city));
        true
    })
}

/// `ClearNetworkFlag(bits)` (§5): data1 0 when USABLE was set.
#[no_mangle]
pub unsafe extern "C" fn fc4_network_clear_flag(flags: u32, bits: u32, out: *mut fc4_network_flag_change) -> bool {
    let Some(o) = out.as_mut() else { return false };
    guard(false, || {
        *o = change_to_c(clear_network_flag(flags, bits));
        true
    })
}

/// The pipe model for a pipe's flags (§5): 0 normal, 1 distressed, 2 burst.
#[no_mangle]
pub extern "C" fn fc4_network_pipe_model(flags: u32) -> u32 {
    match pipe_model(flags) {
        PipeModel::Normal => 0,
        PipeModel::Distressed => 1,
        PipeModel::Burst => 2,
    }
}

/// §7.6 step 1: the rectangle of a terrain message, clamped to a city of
/// `x_size` × `z_size` cells.
#[no_mangle]
pub unsafe extern "C" fn fc4_network_terrain_rect(
    data1: u32,
    data2: u32,
    x_size: i32,
    z_size: i32,
    out: *mut fc4_cell_rect,
) -> bool {
    let Some(o) = out.as_mut() else { return false };
    *o = terrain_rect(data1, data2, (x_size, z_size)).into();
    true
}

/// The corner rule of §7.6 for one corner's height difference.
#[no_mangle]
pub extern "C" fn fc4_network_corner_mismatch(d: f32) -> bool {
    corner_mismatch(d)
}

// ---- the network manager -------------------------------------------------------------------------------

/// Creates the network manager (constructed, reference count 0). The host
/// table is copied; `token` is the manager's own identity.
#[no_mangle]
pub unsafe extern "C" fn fc4_netmgr_create(host: *const fc4_netmgr_host, token: u64) -> *mut fc4_netmgr {
    guard(std::ptr::null_mut(), || match host.as_ref() {
        Some(h) if (h.struct_size as usize) >= std::mem::size_of::<fc4_netmgr_host>() => {
            let sim = NetworkManager::new(Rc::new(MHost { h: *h }), token);
            Box::into_raw(Box::new(MHandle { sim })) as *mut fc4_netmgr
        }
        _ => std::ptr::null_mut(),
    })
}

#[no_mangle]
pub unsafe extern "C" fn fc4_netmgr_destroy(p: *mut fc4_netmgr) {
    if !p.is_null() {
        guard((), || drop(Box::from_raw(p as *mut MHandle)));
    }
}

#[no_mangle]
pub extern "C" fn fc4_netmgr_query_interface(iid: u32) -> bool {
    manager_query_interface(iid)
}

#[no_mangle]
pub unsafe extern "C" fn fc4_netmgr_add_ref(p: *mut fc4_netmgr) -> u32 {
    with_m(p, 0, |h| h.sim.add_ref())
}

/// `Release`: 0 when the object was destroyed; the handle stays valid until
/// [`fc4_netmgr_destroy`].
#[no_mangle]
pub unsafe extern "C" fn fc4_netmgr_release(p: *mut fc4_netmgr) -> u32 {
    with_m(p, 0, |h| h.sim.release())
}

/// `Init` (§3.1): subscribes the 11 messages and creates the 13 tools.
#[no_mangle]
pub unsafe extern "C" fn fc4_netmgr_init(p: *mut fc4_netmgr) -> bool {
    with_m(p, false, |h| h.sim.init())
}

#[no_mangle]
pub unsafe extern "C" fn fc4_netmgr_shutdown(p: *mut fc4_netmgr) -> bool {
    with_m(p, false, |h| h.sim.shutdown())
}

/// `DoMessage` (§6.1). `void1` is the occupant of `0x99EF1142` /
/// `0x99EF1143` (0 = none); `view_old` / `view_new` are the two values of the
/// view message `0xA6B79602`. Always true.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn fc4_netmgr_do_message(
    p: *mut fc4_netmgr,
    msg_type: u32,
    data1: u32,
    data2: u32,
    data3: u32,
    data4: u32,
    void1: u64,
    view_old: u32,
    view_new: u32,
) -> bool {
    let m = NetworkMessage {
        msg_type,
        data1,
        data2,
        data3,
        data4,
        void1: opt(void1),
        view_old,
        view_new,
    };
    with_m(p, true, |h| h.sim.do_message(&m))
}

/// Slot 5 `GetNetworkTool(type, unique)` (§7.1): the cached tool (borrowed)
/// or, with `unique`, a new tool carrying the caller's reference; 0 = none.
#[no_mangle]
pub unsafe extern "C" fn fc4_netmgr_get_network_tool(p: *mut fc4_netmgr, network_type: u32, unique: bool) -> u64 {
    with_m(p, 0, |h| h.sim.get_network_tool(network_type, unique).unwrap_or(0))
}

/// Slot 6 `DestroyNetworkOccupants(list)` (§7.2).
#[no_mangle]
pub unsafe extern "C" fn fc4_netmgr_destroy_network_occupants(p: *mut fc4_netmgr, list: *const u64, count: usize) {
    let l = slice(list, count).to_vec();
    with_m(p, (), |h| h.sim.destroy_network_occupants(&l))
}

/// Slot 7 `QueueRemovals(on)` (§7.3).
#[no_mangle]
pub unsafe extern "C" fn fc4_netmgr_queue_removals(p: *mut fc4_netmgr, on: bool) {
    with_m(p, (), |h| h.sim.queue_removals(on))
}

/// Slot 8 `RemoveOrReplaceOccupant(tool, occ, &replaced)` (§7.4).
/// `*replaced` is written (true) only when an intersection was replaced.
#[no_mangle]
pub unsafe extern "C" fn fc4_netmgr_remove_or_replace_occupant(
    p: *mut fc4_netmgr,
    tool: u64,
    occupant: u64,
    replaced: *mut bool,
) -> bool {
    let (ok, rep) = with_m(p, (false, false), |h| {
        h.sim.remove_or_replace_occupant(tool, opt(occupant))
    });
    if rep {
        if let Some(r) = replaced.as_mut() {
            *r = true;
        }
    }
    ok
}

/// Slot 16 `RegisterConstructionCrew(c)` (held).
#[no_mangle]
pub unsafe extern "C" fn fc4_netmgr_register_construction_crew(p: *mut fc4_netmgr, crew: u64) {
    with_m(p, (), |h| h.sim.register_construction_crew(crew))
}

/// Slot 17 `UnregisterConstructionCrew(c)`; may be called from the host's
/// `crew_shutdown`.
#[no_mangle]
pub unsafe extern "C" fn fc4_netmgr_unregister_construction_crew(p: *mut fc4_netmgr, crew: u64) {
    with_m(p, (), |h| h.sim.unregister_construction_crew(crew))
}

/// Slot 23 `AddToRetrofitList(n)` (held).
#[no_mangle]
pub unsafe extern "C" fn fc4_netmgr_add_to_retrofit_list(p: *mut fc4_netmgr, n: u64) {
    with_m(p, (), |h| h.sim.add_to_retrofit_list(n))
}

/// The globally managed list, as tokens (0 = a null entry read from a save).
/// Returns the length; copies at most `capacity` (`out` may be NULL).
#[no_mangle]
pub unsafe extern "C" fn fc4_netmgr_managed(p: *const fc4_netmgr, out: *mut u64, capacity: usize) -> usize {
    with_m(p, 0, |h| {
        let v: Vec<u64> = h.sim.snapshot().managed.iter().map(|e| e.unwrap_or(0)).collect();
        copy_out(&v, out, capacity)
    })
}

#[no_mangle]
pub unsafe extern "C" fn fc4_netmgr_get_state(p: *const fc4_netmgr, out: *mut fc4_netmgr_state) -> bool {
    let Some(o) = out.as_mut() else { return false };
    with_m(p, false, |h| {
        let s = h.sim.snapshot();
        *o = fc4_netmgr_state {
            struct_size: std::mem::size_of::<fc4_netmgr_state>() as u32,
            initialized: s.initialized.into(),
            destroyed: s.destroyed.into(),
            uk_paths: s.uk_paths.into(),
            long_terrain_op: s.long_terrain_op.into(),
            queueing: s.queueing.into(),
            reserved: [0; 3],
            ref_count: s.ref_count,
            managed: s.managed.len() as u32,
            pending: s.pending.len() as u32,
            retrofit: s.retrofit.len() as u32,
            crews: s.crews.len() as u32,
            tools: s.tools.iter().filter(|t| t.is_some()).count() as u32,
            dirty: s.dirty.into(),
        };
        true
    })
}

/// `Write` (§4.1).
#[no_mangle]
pub unsafe extern "C" fn fc4_netmgr_write(p: *const fc4_netmgr, out: *const fc4_ostream) -> bool {
    let Some(o) = out.as_ref() else { return false };
    if !ostream_ok(o) {
        return false;
    }
    with_m(p, false, |h| h.sim.write(&mut COut(o)))
}

/// `Read` (§4.1): appends to the managed list; entries come back through the
/// host's `resolve_ref` (IID `0xA821EF94`).
#[no_mangle]
pub unsafe extern "C" fn fc4_netmgr_read(p: *mut fc4_netmgr, inp: *const fc4_istream) -> bool {
    let Some(i) = inp.as_ref() else { return false };
    if !istream_ok(i) {
        return false;
    }
    with_m(p, false, |h| {
        h.sim.read(&mut CIn {
            stream: i,
            decode: no_core_objects,
        })
    })
}

// ---- the network lot manager ---------------------------------------------------------------------------

/// Creates the network lot manager (constructed, reference count 0). The
/// host table is copied; `token` is the manager's identity; `session_seed`
/// seeds its generator (§17: `derive(0x8C09C379)`).
#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_create(
    host: *const fc4_netlot_host,
    token: u64,
    session_seed: u32,
) -> *mut fc4_netlot {
    guard(std::ptr::null_mut(), || match host.as_ref() {
        Some(h) if (h.struct_size as usize) >= std::mem::size_of::<fc4_netlot_host>() => {
            let c = Rc::new(LHost {
                h: *h,
                configs: RefCell::new(Vec::new()),
            });
            let sim = NetworkLotManager::new(c.clone(), token, session_seed);
            Box::into_raw(Box::new(LHandle { sim, host: c })) as *mut fc4_netlot
        }
        _ => std::ptr::null_mut(),
    })
}

#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_destroy(p: *mut fc4_netlot) {
    if !p.is_null() {
        guard((), || drop(Box::from_raw(p as *mut LHandle)));
    }
}

#[no_mangle]
pub extern "C" fn fc4_netlot_query_interface(iid: u32) -> bool {
    lot_manager_query_interface(iid)
}

#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_add_ref(p: *mut fc4_netlot) -> u32 {
    with_l(p, 0, |h| h.sim.add_ref())
}

#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_release(p: *mut fc4_netlot) -> u32 {
    with_l(p, 0, |h| h.sim.release())
}

unsafe fn objects_of(p: *const fc4_netlot_object, n: u32) -> Vec<NetworkLotObject> {
    slice(p, n as usize)
        .iter()
        .map(|o| NetworkLotObject {
            kind: o.kind,
            flags: o.flags,
            orientation: o.orientation,
            offset: o.offset,
            field_24: o.field_24,
        })
        .collect()
}

fn bits<const N: usize>(mask: u32) -> [bool; N] {
    let mut out = [false; N];
    for (i, b) in out.iter_mut().enumerate() {
        *b = mask & (1 << i) != 0;
    }
    out
}

/// Adds a configuration for the next `Init` (§3.2 step 4; load order is
/// call order). The objects are copied.
#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_add_configuration(p: *mut fc4_netlot, cfg: *const fc4_netlot_config) -> bool {
    let Some(c) = cfg.as_ref() else { return false };
    if (c.struct_size as usize) < std::mem::size_of::<fc4_netlot_config>() {
        return false;
    }
    let objects = objects_of(c.objects, c.object_count);
    with_l(p, false, |h| {
        h.host.configs.borrow_mut().push(NetworkLotConfig {
            token: c.token,
            piece_id: c.piece_id,
            id: c.id,
            name: Vec::new(),
            zones: bits(u32::from(c.zones)),
            wealths: bits(u32::from(c.wealths)),
            mask: c.mask,
            period: c.period,
            min_slope: c.min_slope,
            max_slope: c.max_slope,
            rotations: c.rotations,
            flip_rule: c.flip_rule,
            objects,
        });
        true
    })
}

/// Adds a configuration read from a lot configuration exemplar's
/// properties (`ReadLotConfiguration`, §2.4: false unless property `0x10`
/// is `0x21` and the piece ID is present), with the host's objects.
#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_add_configuration_props(
    p: *mut fc4_netlot,
    token: u64,
    props: *const fc4_props,
    objects: *const fc4_netlot_object,
    object_count: u32,
) -> bool {
    let Some(h) = props_ref(props) else { return false };
    let objects = objects_of(objects, object_count);
    with_l(p, false, |l| match NetworkLotConfig::from_exemplar(h, token) {
        Some(mut c) => {
            c.objects = objects;
            l.host.configs.borrow_mut().push(c);
            true
        }
        None => false,
    })
}

/// Forgets the configurations added so far (the loaded ones stay until
/// `Shutdown`).
#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_clear_configurations(p: *mut fc4_netlot) {
    with_l(p, (), |h| h.host.configs.borrow_mut().clear())
}

/// `Init` (§3.2): subscribes the 6 messages, loads the configurations and
/// resolves the loaded lots.
#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_init(p: *mut fc4_netlot) -> bool {
    with_l(p, false, |h| h.sim.init())
}

#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_shutdown(p: *mut fc4_netlot) -> bool {
    with_l(p, false, |h| h.sim.shutdown())
}

/// `DoMessage` (§6.2). `void1` / `unknown` as the spec (0 = none); `rect`
/// is `*data2` of `0x07699455` (NULL otherwise). Always true.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn fc4_netlot_do_message(
    p: *mut fc4_netlot,
    msg_type: u32,
    data1: u32,
    data2: u32,
    data3: u32,
    void1: u64,
    unknown: u64,
    rect: *const fc4_cell_rect,
) -> bool {
    let m = NetworkLotMessage {
        msg_type,
        data1,
        data2,
        data3,
        void1: opt(void1),
        unknown: opt(unknown),
        rect: rect.as_ref().map(|r| (*r).into()),
    };
    with_l(p, true, |h| h.sim.do_message(&m))
}

fn lot_to_c(l: &LotEntry) -> fc4_netlot_lot {
    fc4_netlot_lot {
        occupant: l.occupant.unwrap_or(0),
        config_id: l.config.as_ref().map_or(l.config_id, |c| c.id),
        has_config: l.config.is_some().into(),
        locked: l.locked.into(),
        reserved: 0,
    }
}

/// Slot 5 `GetNetworkLot(x, z)`: false when the cell has none.
#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_get_lot(p: *const fc4_netlot, x: i32, z: i32, out: *mut fc4_netlot_lot) -> bool {
    with_l(p, false, |h| match h.sim.get_network_lot(x, z) {
        Some(l) => {
            if let Some(o) = out.as_mut() {
                *o = lot_to_c(&l);
            }
            true
        }
        None => false,
    })
}

/// Slot 9 `GetZoneTypeForNetwork(x, z)` (§8.2).
#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_zone_type_for_network(p: *const fc4_netlot, x: i32, z: i32) -> i32 {
    with_l(p, 0, |h| h.sim.zone_type_for_network(x, z))
}

/// Slot 10 `GetWealthForNetwork(x, z)` (§8.3).
#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_wealth_for_network(p: *const fc4_netlot, x: i32, z: i32) -> i32 {
    with_l(p, 0, |h| h.sim.wealth_for_network(x, z))
}

fn loaded(h: &LHandle, id: u32) -> Option<Rc<NetworkLotConfig>> {
    h.sim.configuration_set().into_iter().find(|c| c.id == id)
}

/// `IsLotCompatible` (§8.4) for the loaded configuration `config_id` (false
/// when none is loaded).
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn fc4_netlot_is_lot_compatible(
    p: *const fc4_netlot,
    config_id: u32,
    x: i32,
    z: i32,
    zone: i32,
    wealth: i32,
    rotation: i32,
    flip: i32,
) -> bool {
    with_l(p, false, |h| {
        loaded(h, config_id).is_some_and(|c| h.sim.is_lot_compatible(&c, x, z, zone, wealth, rotation, flip))
    })
}

/// Slot 7 `GetNetworkLotConfigurationById(id)`: true if one is loaded.
#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_configuration_by_id(p: *const fc4_netlot, id: u32) -> bool {
    with_l(p, false, |h| h.sim.configuration_by_id(id))
}

/// Slot 13 `SetLotConfigurationForNextPiece`: the loaded configuration
/// `config_id` (false when none is loaded).
#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_set_next_configuration(p: *mut fc4_netlot, config_id: u32) -> bool {
    with_l(p, false, |h| match loaded(h, config_id) {
        Some(c) => {
            h.sim.set_lot_configuration_for_next_piece(Some(c));
            true
        }
        None => false,
    })
}

/// Slot 13 with null.
#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_clear_next_configuration(p: *mut fc4_netlot) {
    with_l(p, (), |h| h.sim.set_lot_configuration_for_next_piece(None))
}

/// Slot 11 `IterateAllNetworkLots(cb)`: `cb(ctx, x, z, lot)` in map order;
/// false at the first false.
#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_iterate_lots(
    p: *const fc4_netlot,
    cb: fc4_netlot_visitor,
    ctx: *mut c_void,
) -> bool {
    let Some(f) = cb else { return false };
    with_l(p, false, |h| {
        h.sim.iterate_all_network_lots(|x, z, l| {
            let c = lot_to_c(l);
            // SAFETY: the caller's visitor.
            unsafe { f(ctx, x, z, &c) }
        })
    })
}

/// `UpdateRoadsInCell(x, z)` (§8.5).
#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_update_roads_in_cell(p: *mut fc4_netlot, x: i32, z: i32) {
    with_l(p, (), |h| h.sim.update_roads_in_cell(x, z))
}

/// Adds a cell to `pending`.
#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_add_pending(p: *mut fc4_netlot, x: i32, z: i32) {
    with_l(p, (), |h| h.sim.add_pending(x, z))
}

/// `ProcessUpdateList()` (§8.5).
#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_process_update_list(p: *mut fc4_netlot) {
    with_l(p, (), |h| h.sim.process_update_list())
}

/// Slot 12 `RedevelopLot(x, z, lot)` for the cell's lot (false when none).
#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_redevelop_lot(p: *mut fc4_netlot, x: i32, z: i32) -> bool {
    with_l(p, false, |h| match h.sim.get_network_lot(x, z) {
        Some(l) => {
            h.sim.redevelop_lot(x, z, &l);
            true
        }
        None => false,
    })
}

/// Slot 8 `CreateProp(lot, i, id)` for the cell's lot: the street prop
/// (borrowed) or 0.
#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_create_prop(p: *mut fc4_netlot, x: i32, z: i32, index: u32, id: u32) -> u64 {
    with_l(p, 0, |h| {
        h.sim
            .get_network_lot(x, z)
            .and_then(|l| h.sim.create_prop(&l, index as usize, id))
            .unwrap_or(0)
    })
}

/// `DoRoadDamageChange(x, z, damaged)` (§8.8).
#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_road_damage_change(p: *mut fc4_netlot, x: i32, z: i32, damaged: bool) {
    with_l(p, (), |h| h.sim.do_road_damage_change(x, z, damaged))
}

#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_get_state(p: *const fc4_netlot, out: *mut fc4_netlot_state) -> bool {
    let Some(o) = out.as_mut() else { return false };
    with_l(p, false, |h| {
        let s = h.sim.snapshot();
        *o = fc4_netlot_state {
            struct_size: std::mem::size_of::<fc4_netlot_state>() as u32,
            initialized: s.initialized.into(),
            destroyed: s.destroyed.into(),
            has_forced: s.forced.is_some().into(),
            reserved: 0,
            ref_count: s.ref_count,
            lots: s.lots.len() as u32,
            potholes: s.potholes.len() as u32,
            pending: s.pending.len() as u32,
            configs: s.configs.iter().map(|(_, v)| v.len() as u32).sum(),
            forced_id: s.forced.as_ref().map_or(0, |c| c.id),
        };
        true
    })
}

/// `Write` (§4.2).
#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_write(p: *const fc4_netlot, out: *const fc4_ostream) -> bool {
    let Some(o) = out.as_ref() else { return false };
    if !ostream_ok(o) {
        return false;
    }
    with_l(p, false, |h| h.sim.write(&mut COut(o)))
}

/// `Read` (§4.2): false unless the version is 2; occupants (IID
/// `0xA821EF94`) and effect props (IID `0x09820792`) come back through the
/// host's `resolve_ref`.
#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_read(p: *mut fc4_netlot, inp: *const fc4_istream) -> bool {
    let Some(i) = inp.as_ref() else { return false };
    if !istream_ok(i) {
        return false;
    }
    with_l(p, false, |h| {
        h.sim.read(&mut CIn {
            stream: i,
            decode: no_core_objects,
        })
    })
}

/// §17.3 replay rule 1: loads the captured raw generator state `(words[624],
/// k)`, `k = 624 - left` (0..=624). False on a null pointer or `k > 624`.
#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_load_random_raw(p: *mut fc4_netlot, words: *const u32, k: u32) -> bool {
    if words.is_null() || k > 624 {
        return false;
    }
    let w: &[u32; 624] = &*(words as *const [u32; 624]);
    with_l(p, false, |h| {
        h.sim.load_random_raw(w, k);
        true
    })
}

/// The generator's raw state for the §17.3 rule 3 post-state check: fills
/// `words[624]` and `*k`; false when the generator has no raw state.
#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_random_raw(p: *const fc4_netlot, words: *mut u32, k: *mut u32) -> bool {
    if words.is_null() || k.is_null() {
        return false;
    }
    with_l(p, false, |h| match h.sim.random_raw() {
        Some(r) => {
            std::ptr::copy_nonoverlapping(r.words.as_ptr(), words, 624);
            *k = r.k;
            true
        }
        None => false,
    })
}

/// `pending`'s bucket count `B` (§8.5 replay contract).
#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_pending_bucket_count(p: *const fc4_netlot) -> u32 {
    with_l(p, 0, |h| h.sim.pending_bucket_count())
}

/// §8.5 replay rule 6: the next pass starts from an empty set with `b`
/// buckets. False while keys are pending (or on a null pointer).
#[no_mangle]
pub unsafe extern "C" fn fc4_netlot_set_pending_bucket_count(p: *mut fc4_netlot, b: u32) -> bool {
    with_l(p, false, |h| h.sim.set_pending_bucket_count(b))
}
