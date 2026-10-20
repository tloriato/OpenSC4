// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The city detail manager over the C ABI (fc4.h "city detail manager",
//! docs/specs/city-detail.md).
//!
//! The host implements [`fc4_city_detail_host`]; the plugin's
//! `cISC4CityDetailManager` (with its serializable sub-object) forwards its
//! slots here. Tokens are opaque host identities: lots, occupants and
//! network lots. Lists come back through the sinks of `sinks.rs`.
//!
//! The increase pass's temporary generator is seeded per session (§3.5
//! *[choice]*): `derive_seed(S, 0xEAD4BA36 XOR n)`, `n` the increase
//! passes run by this manager object, from 0 at its creation (CD5; the game
//! makes a new manager at every city load).
//!
//! Threads: the core is single-threaded. The plugin serialises every call
//! on one object (fc4.h).

use std::ffi::c_void;
use std::rc::Rc;

use fc4_core::city_detail::{query_interface, CityDetailHost, CityDetailManager, DetailSession, ObjectPlacement};
use fc4_core::lot_developer::CfgObject;
use fc4_core::stream::SaveObject;

use crate::guard;
use crate::sinks::{cfg_objects, fc4_lot_cfg_objects_sink, fc4_u32s_sink, fc4_u64s_sink, tokens, u32s};
use crate::stream::{fc4_istream, fc4_ostream, istream_ok, ostream_ok, CIn, COut};

type U = *mut c_void;

#[repr(C)]
pub struct fc4_city_detail {
    _private: [u8; 0],
}

/// A position and orientation (lot developer slot 39).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct fc4_object_placement {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub orientation: u32,
}

/// One network lot of `IterateAllNetworkLots`.
pub type fc4_network_lot_sink = Option<unsafe extern "C" fn(ctx: *mut c_void, x: i32, z: i32, network_lot: u64)>;

/// What the city detail manager needs from the game (fc4.h
/// `fc4_city_detail_host`). A missing callback reads as 0 / false / none.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct fc4_city_detail_host {
    pub struct_size: u32,
    pub user: *mut c_void,
    pub detail_setting: Option<unsafe extern "C" fn(U) -> u32>,
    pub lots: Option<unsafe extern "C" fn(U, fc4_u64s_sink, U)>,
    pub lot_config_objects: Option<unsafe extern "C" fn(U, u64, fc4_lot_cfg_objects_sink, U) -> bool>,
    pub lot_occupants: Option<unsafe extern "C" fn(U, u64, fc4_u64s_sink, U)>,
    pub occupants_in_lot_rect: Option<unsafe extern "C" fn(U, u64, fc4_u64s_sink, U)>,
    pub occupant_type: Option<unsafe extern "C" fn(U, u64) -> u32>,
    pub occupant_lot_tag: Option<unsafe extern "C" fn(U, u64, *mut u32) -> bool>,
    pub demolish_flora: Option<unsafe extern "C" fn(U, u64)>,
    pub remove_prop: Option<unsafe extern "C" fn(U, u64)>,
    pub object_placement: Option<unsafe extern "C" fn(U, u64, u32, *mut fc4_object_placement)>,
    pub object_id_array: Option<unsafe extern "C" fn(U, u64, u32, fc4_u32s_sink, U)>,
    pub add_lot_prop: Option<unsafe extern "C" fn(U, u32, *const fc4_object_placement) -> u64>,
    pub add_flora: Option<unsafe extern "C" fn(U, u32, *const fc4_object_placement) -> u64>,
    pub set_lot_tag: Option<unsafe extern "C" fn(U, u64, u32)>,
    pub network_lots: Option<unsafe extern "C" fn(U, fc4_network_lot_sink, U)>,
    pub redevelop_network_lot: Option<unsafe extern "C" fn(U, i32, i32, u64)>,
}

/// Diagnostics (`fc4_city_detail_get_state`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_city_detail_state {
    pub struct_size: u32,
    /// The stored level (`0xFFFFFFFF` = none).
    pub level: u32,
    pub ref_count: u32,
    pub destroyed: u8,
    pub reserved: [u8; 3],
    /// The last `Init`: 0 nothing, 1 reduce, 2 increase; the occupants
    /// removed and created, the draws, the network lots redeveloped.
    pub last_pass: u32,
    pub last_removed: u32,
    pub last_created: u32,
    pub last_draws: u32,
    pub last_network_lots: u32,
    /// Increase passes by this manager object so far (`n` of §3.5, CD5).
    pub session_passes: u32,
}

const _: () = {
    assert!(std::mem::size_of::<fc4_object_placement>() == 16);
    assert!(std::mem::size_of::<fc4_city_detail_state>() == 40);
    assert!(std::mem::size_of::<fc4_city_detail_host>() == if cfg!(target_pointer_width = "32") { 72 } else { 144 });
};

// ---- host adapter -----------------------------------------------------------------------------------

struct CHost {
    h: fc4_city_detail_host,
}

macro_rules! cb {
    ($s:expr, $f:ident, $default:expr $(, $arg:expr)*) => {
        // SAFETY: whoever created the object promised valid callbacks.
        match $s.h.$f { Some(f) => unsafe { f($s.h.user $(, $arg)*) }, None => $default }
    };
}

fn opt(t: u64) -> Option<u64> {
    (t != 0).then_some(t)
}

fn placement_to_c(p: ObjectPlacement) -> fc4_object_placement {
    fc4_object_placement {
        x: p.x,
        y: p.y,
        z: p.z,
        orientation: p.orientation,
    }
}

unsafe extern "C" fn collect_network_lot(ctx: *mut c_void, x: i32, z: i32, lot: u64) {
    (*(ctx as *mut Vec<(i32, i32, u64)>)).push((x, z, lot));
}

impl CityDetailHost for CHost {
    fn detail_setting(&self) -> u32 {
        cb!(self, detail_setting, 0)
    }
    fn lots(&self) -> Vec<u64> {
        tokens(|s, c| cb!(self, lots, (), s, c))
    }
    fn lot_config_objects(&self, lot: u64) -> Option<Rc<Vec<CfgObject>>> {
        cfg_objects(|s, c| cb!(self, lot_config_objects, false, lot, s, c))
    }
    fn lot_occupants(&self, lot: u64) -> Vec<u64> {
        tokens(|s, c| cb!(self, lot_occupants, (), lot, s, c))
    }
    fn occupants_in_lot_rect(&self, lot: u64) -> Vec<u64> {
        tokens(|s, c| cb!(self, occupants_in_lot_rect, (), lot, s, c))
    }
    fn occupant_type(&self, occupant: u64) -> u32 {
        cb!(self, occupant_type, 0, occupant)
    }
    fn occupant_lot_tag(&self, occupant: u64) -> Option<u32> {
        let mut t = 0u32;
        cb!(self, occupant_lot_tag, false, occupant, &mut t).then_some(t)
    }
    fn demolish_flora(&self, occupant: u64) {
        cb!(self, demolish_flora, (), occupant)
    }
    fn remove_prop(&self, occupant: u64) {
        cb!(self, remove_prop, (), occupant)
    }
    fn object_placement(&self, lot: u64, index: usize) -> ObjectPlacement {
        let mut p = fc4_object_placement::default();
        cb!(self, object_placement, (), lot, index as u32, &mut p);
        ObjectPlacement {
            x: p.x,
            y: p.y,
            z: p.z,
            orientation: p.orientation,
        }
    }
    fn object_id_array(&self, lot: u64, index: usize) -> Vec<u32> {
        u32s(|s, c| cb!(self, object_id_array, (), lot, index as u32, s, c))
    }
    fn add_lot_prop(&self, id: u32, placement: ObjectPlacement) -> Option<u64> {
        let p = placement_to_c(placement);
        opt(cb!(self, add_lot_prop, 0, id, &p))
    }
    fn add_flora(&self, id: u32, placement: ObjectPlacement) -> Option<u64> {
        let p = placement_to_c(placement);
        opt(cb!(self, add_flora, 0, id, &p))
    }
    fn set_lot_tag(&self, occupant: u64, tag: u32) {
        cb!(self, set_lot_tag, (), occupant, tag)
    }
    fn network_lots(&self) -> Vec<(i32, i32, u64)> {
        let mut v: Vec<(i32, i32, u64)> = Vec::new();
        let ctx = &mut v as *mut Vec<(i32, i32, u64)> as *mut c_void;
        cb!(self, network_lots, (), Some(collect_network_lot), ctx);
        v
    }
    fn redevelop_network_lot(&self, x: i32, z: i32, network_lot: u64) {
        cb!(self, redevelop_network_lot, (), x, z, network_lot)
    }
}

// ---- handles ----------------------------------------------------------------------------------------

struct Handle {
    sim: Rc<CityDetailManager>,
}

fn with_p<R>(p: *const fc4_city_detail, fallback: R, f: impl FnOnce(&Handle) -> R) -> R {
    // SAFETY: documented precondition: NULL or a live fc4_city_detail*.
    match unsafe { (p as *const Handle).as_ref() } {
        Some(h) => guard(fallback, || f(h)),
        None => fallback,
    }
}

/// Creates the core object (level none, reference count 0). The host table
/// is copied. `session_seed` is the session seed `S` of §3.5; `token` is the
/// manager's own host identity (written for references to it).
#[no_mangle]
pub unsafe extern "C" fn fc4_city_detail_create(
    host: *const fc4_city_detail_host,
    session_seed: u32,
    token: u64,
) -> *mut fc4_city_detail {
    guard(std::ptr::null_mut(), || match host.as_ref() {
        Some(h) if (h.struct_size as usize) >= std::mem::size_of::<fc4_city_detail_host>() => {
            let c = Rc::new(CHost { h: *h });
            // The pass count is per manager object (CD5): each create
            // starts again at n = 0.
            let sim = CityDetailManager::new(c, DetailSession::new(session_seed), token);
            Box::into_raw(Box::new(Handle { sim })) as *mut fc4_city_detail
        }
        _ => std::ptr::null_mut(),
    })
}

#[no_mangle]
pub unsafe extern "C" fn fc4_city_detail_destroy(p: *mut fc4_city_detail) {
    if !p.is_null() {
        guard((), || drop(Box::from_raw(p as *mut Handle)));
    }
}

#[no_mangle]
pub extern "C" fn fc4_city_detail_query_interface(iid: u32) -> bool {
    query_interface(iid)
}

#[no_mangle]
pub unsafe extern "C" fn fc4_city_detail_add_ref(p: *mut fc4_city_detail) -> u32 {
    with_p(p, 0, |h| h.sim.add_ref())
}

/// `Release`: 0 when the object was destroyed; the handle stays valid until
/// [`fc4_city_detail_destroy`].
#[no_mangle]
pub unsafe extern "C" fn fc4_city_detail_release(p: *mut fc4_city_detail) -> u32 {
    with_p(p, 0, |h| h.sim.release())
}

/// `Init` (§3.1): reduce or increase, then redevelop the network lots.
/// Always true.
#[no_mangle]
pub unsafe extern "C" fn fc4_city_detail_init(p: *mut fc4_city_detail) -> bool {
    with_p(p, false, |h| h.sim.init())
}

#[no_mangle]
pub unsafe extern "C" fn fc4_city_detail_shutdown(p: *mut fc4_city_detail) -> bool {
    with_p(p, false, |h| h.sim.shutdown())
}

/// Slot 5 (*[inferred]* `GetCityDetail`): the stored level.
#[no_mangle]
pub unsafe extern "C" fn fc4_city_detail_get_level(p: *const fc4_city_detail) -> u32 {
    with_p(p, 0, |h| h.sim.city_detail())
}

#[no_mangle]
pub unsafe extern "C" fn fc4_city_detail_get_state(p: *const fc4_city_detail, out: *mut fc4_city_detail_state) -> bool {
    let Some(o) = out.as_mut() else { return false };
    with_p(p, false, |h| {
        let s = h.sim.snapshot();
        let l = h.sim.last_outcome();
        *o = fc4_city_detail_state {
            struct_size: std::mem::size_of::<fc4_city_detail_state>() as u32,
            level: s.level,
            ref_count: s.ref_count,
            destroyed: s.destroyed.into(),
            reserved: [0; 3],
            last_pass: l.pass,
            last_removed: l.removed,
            last_created: l.created,
            last_draws: l.draws,
            last_network_lots: l.network_lots,
            session_passes: h.sim.passes(),
        };
        true
    })
}

/// `Write` (§4): version 0, the level.
#[no_mangle]
pub unsafe extern "C" fn fc4_city_detail_write(p: *const fc4_city_detail, out: *const fc4_ostream) -> bool {
    let Some(o) = out.as_ref() else { return false };
    if !ostream_ok(o) {
        return false;
    }
    with_p(p, false, |h| h.sim.write(&mut COut(o)))
}

fn no_core_objects(_: u32, _: *mut c_void) -> Option<Rc<dyn SaveObject>> {
    None
}

/// `Read` (§4): false unless the version is 0 (the level is then unchanged).
#[no_mangle]
pub unsafe extern "C" fn fc4_city_detail_read(p: *mut fc4_city_detail, inp: *const fc4_istream) -> bool {
    let Some(i) = inp.as_ref() else { return false };
    if !istream_ok(i) {
        return false;
    }
    with_p(p, false, |h| {
        h.sim.read(&mut CIn {
            stream: i,
            decode: no_core_objects,
        })
    })
}
