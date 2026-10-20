// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The airport and seaport developers over the C ABI (fc4.h "port
//! developers", docs/specs/port-developers.md).
//!
//! The host implements [`fc4_port_host`]; the plugin's
//! `cISC4AirportDeveloper` / `cISC4SeaportDeveloper` (with the port,
//! message-target and serializable sub-objects) forwards its slots here.
//! `kind` is 0 for the airport, 1 for the seaport. Tokens are opaque host
//! identities: lots, occupants, lot configurations, the budget department.
//!
//! Threads: the core is single-threaded. The plugin serialises every call
//! on one object (fc4.h).

use std::ffi::{c_char, c_void, CString};
use std::rc::Rc;

use fc4_core::port_developer::{query_interface, CellRect, PortDeveloper, PortHost, PortKind};
use fc4_core::property::PropertyHolder;
use fc4_core::stream::SaveObject;

use crate::guard;
use crate::hostgrid::{fc4_cell_rect, take_props};
use crate::props::fc4_props;
use crate::sinks::{fc4_u32s_sink, u32s};
use crate::stream::{fc4_istream, fc4_ostream, istream_ok, ostream_ok, CIn, COut};

type U = *mut c_void;

#[repr(C)]
pub struct fc4_port_developer {
    _private: [u8; 0],
}

/// What a port developer needs from the game (fc4.h `fc4_port_host`). A
/// missing callback reads as 0 / false / none.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct fc4_port_host {
    pub struct_size: u32,
    pub user: *mut c_void,
    /// A NEW fc4_props with exemplar (0x6534284A, 0x67CD5FA1, instance), NULL
    /// if missing.
    pub exemplar: Option<unsafe extern "C" fn(U, u32) -> *mut fc4_props>,
    pub add_agent: Option<unsafe extern "C" fn(U, u32, *const c_char, u32)>,
    pub remove_agent: Option<unsafe extern "C" fn(U, *const c_char)>,
    pub department: Option<unsafe extern "C" fn(U, u32) -> u64>,
    pub create_department: Option<unsafe extern "C" fn(U, u32, u32, u32, u32, f32, u32) -> u64>,
    pub line_expenses: Option<unsafe extern "C" fn(U, u64, u32, *mut i64, *mut i64) -> bool>,
    pub set_full_expenses: Option<unsafe extern "C" fn(U, u64, u32, i64)>,
    pub withdraw_funds: Option<unsafe extern "C" fn(U, i64) -> bool>,
    pub occupant_is_preview: Option<unsafe extern "C" fn(U, u64) -> bool>,
    pub occupant_type: Option<unsafe extern "C" fn(U, u64) -> u32>,
    pub occupant_in_group: Option<unsafe extern "C" fn(U, u64, u32) -> bool>,
    pub occupant_lot: Option<unsafe extern "C" fn(U, u64) -> u64>,
    pub lot_config: Option<unsafe extern "C" fn(U, u64, *mut u32, *mut u32) -> bool>,
    pub lot_location: Option<unsafe extern "C" fn(U, u64, *mut i16, *mut i16)>,
    pub lot_at: Option<unsafe extern "C" fn(U, i16, i16) -> u64>,
    pub lot_state: Option<unsafe extern "C" fn(U, u64) -> u32>,
    pub lot_condition: Option<unsafe extern "C" fn(U, u64) -> u32>,
    pub set_lot_condition: Option<unsafe extern "C" fn(U, u64, u32)>,
    pub lot_days_since_state_change: Option<unsafe extern "C" fn(U, u64) -> u32>,
    pub lot_rect: Option<unsafe extern "C" fn(U, u64, *mut fc4_cell_rect)>,
    pub lot_facing: Option<unsafe extern "C" fn(U, u64) -> u32>,
    pub lot_zone_compatible: Option<unsafe extern "C" fn(U, u64, u16) -> bool>,
    pub lot_configuration: Option<unsafe extern "C" fn(U, u32) -> u64>,
    pub start_demolish_lot: Option<unsafe extern "C" fn(U, u64)>,
    pub end_demolish_lot: Option<unsafe extern "C" fn(U, u64)>,
    pub create_lot: Option<unsafe extern "C" fn(U, i32, i32, i32, i32, u32) -> u64>,
    pub set_lot_configuration: Option<unsafe extern "C" fn(U, u64, u64)>,
    pub construct_default_lot: Option<unsafe extern "C" fn(U, u64, u64) -> bool>,
    pub dezone: Option<unsafe extern "C" fn(U, *const fc4_cell_rect)>,
    pub update_occupant_effects: Option<unsafe extern "C" fn(U, u64, f32, f32)>,
    pub record_history: Option<unsafe extern "C" fn(U, u32, i32)>,
    pub push_event: Option<unsafe extern "C" fn(U, u32)>,
    pub office_counts: Option<unsafe extern "C" fn(U, u32, fc4_u32s_sink, U)>,
    pub lot_subnetworks: Option<unsafe extern "C" fn(U, u64, fc4_u32s_sink, U)>,
    pub traffic_arrived: Option<unsafe extern "C" fn(U, u64, u32) -> u32>,
    pub set_max_trip_capacity: Option<unsafe extern "C" fn(U, u64, u32, u32)>,
    pub terrain_size: Option<unsafe extern "C" fn(U, *mut u32, *mut u32)>,
    pub cell_has_water: Option<unsafe extern "C" fn(U, u32, u32) -> bool>,
    pub set_access_props_state: Option<unsafe extern "C" fn(U, u64, u32, u32) -> u32>,
    pub set_altitude: Option<unsafe extern "C" fn(U, i32, i32, f32)>,
}

/// One port lot (`fc4_port_developer_get_port`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct fc4_port_lot {
    pub struct_size: u32,
    pub stage: u32,
    pub type_index: u32,
    pub x: i16,
    pub z: i16,
    pub efficiency: f32,
    pub usage: f32,
    /// Airport only.
    pub trips: u32,
    pub needs_lookup: u8,
    pub swapping: u8,
    pub reserved: [u8; 2],
    /// 0 = none (or not looked up yet).
    pub lot: u64,
}

/// Diagnostics (`fc4_port_developer_get_state`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct fc4_port_state {
    pub struct_size: u32,
    pub kind: u32,
    pub tuning_instance: u32,
    pub type_count: u32,
    pub types_loaded: u32,
    pub port_count: u32,
    pub ref_count: u32,
    pub zone_type: u16,
    pub tuning_loaded: u8,
    pub initialized: u8,
    pub destroyed: u8,
    pub recheck_access: u8,
    pub city_has_sea: u8,
    pub reserved: u8,
    /// The last month: ran (with ports), history value, advisor event,
    /// ports skipped.
    pub month_ran: u32,
    pub month_history: i32,
    pub month_event: u32,
    pub month_skipped: u32,
    pub reserved2: u32,
    pub department: u64,
}

const _: () = {
    assert!(std::mem::size_of::<fc4_port_lot>() == 40);
    assert!(std::mem::size_of::<fc4_port_state>() == 64);
    assert!(std::mem::size_of::<fc4_port_host>() == if cfg!(target_pointer_width = "32") { 168 } else { 336 });
};

// ---- host adapter -----------------------------------------------------------------------------------

struct CHost {
    h: fc4_port_host,
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

fn rect_c(r: CellRect) -> fc4_cell_rect {
    fc4_cell_rect {
        x1: r.x0,
        z1: r.z0,
        x2: r.x1,
        z2: r.z1,
    }
}

impl PortHost for CHost {
    fn exemplar(&self, instance: u32) -> Option<Rc<PropertyHolder>> {
        let p = cb!(self, exemplar, std::ptr::null_mut(), instance);
        // SAFETY: the host hands over a NEW fc4_props (or NULL).
        unsafe { take_props(p) }
    }
    fn add_agent(&self, name: &str, level: u32, flags: u32) {
        let n = CString::new(name).unwrap_or_default();
        cb!(self, add_agent, (), level, n.as_ptr(), flags)
    }
    fn remove_agent(&self, name: &str) {
        let n = CString::new(name).unwrap_or_default();
        cb!(self, remove_agent, (), n.as_ptr())
    }
    fn department(&self, id: u32) -> Option<u64> {
        opt(cb!(self, department, 0, id))
    }
    fn create_department(&self, id: u32, group: u32, name: (u32, u32), max_funding: f32, line: u32) -> Option<u64> {
        opt(cb!(
            self,
            create_department,
            0,
            id,
            group,
            name.0,
            name.1,
            max_funding,
            line
        ))
    }
    fn line_expenses(&self, department: u64, line: u32) -> Option<(i64, i64)> {
        let (mut full, mut cur) = (0i64, 0i64);
        cb!(self, line_expenses, false, department, line, &mut full, &mut cur).then_some((full, cur))
    }
    fn set_full_expenses(&self, department: u64, line: u32, v: i64) {
        cb!(self, set_full_expenses, (), department, line, v)
    }
    fn withdraw_funds(&self, amount: i64) -> bool {
        cb!(self, withdraw_funds, false, amount)
    }
    fn occupant_is_preview(&self, occupant: u64) -> bool {
        cb!(self, occupant_is_preview, false, occupant)
    }
    fn occupant_type(&self, occupant: u64) -> u32 {
        cb!(self, occupant_type, 0, occupant)
    }
    fn occupant_in_group(&self, occupant: u64, group: u32) -> bool {
        cb!(self, occupant_in_group, false, occupant, group)
    }
    fn occupant_lot(&self, occupant: u64) -> Option<u64> {
        opt(cb!(self, occupant_lot, 0, occupant))
    }
    fn lot_config(&self, lot: u64) -> Option<(u32, u32)> {
        let (mut id, mut stage) = (0u32, 0u32);
        cb!(self, lot_config, false, lot, &mut id, &mut stage).then_some((id, stage))
    }
    fn lot_location(&self, lot: u64) -> (i16, i16) {
        let (mut x, mut z) = (0i16, 0i16);
        cb!(self, lot_location, (), lot, &mut x, &mut z);
        (x, z)
    }
    fn lot_at(&self, x: i16, z: i16) -> Option<u64> {
        opt(cb!(self, lot_at, 0, x, z))
    }
    fn lot_state(&self, lot: u64) -> u32 {
        cb!(self, lot_state, 0, lot)
    }
    fn lot_condition(&self, lot: u64) -> u32 {
        cb!(self, lot_condition, 0, lot)
    }
    fn set_lot_condition(&self, lot: u64, condition: u32) {
        cb!(self, set_lot_condition, (), lot, condition)
    }
    fn lot_days_since_state_change(&self, lot: u64) -> u32 {
        cb!(self, lot_days_since_state_change, 0, lot)
    }
    fn lot_rect(&self, lot: u64) -> CellRect {
        let mut r = fc4_cell_rect::default();
        cb!(self, lot_rect, (), lot, &mut r);
        CellRect {
            x0: r.x1,
            z0: r.z1,
            x1: r.x2,
            z1: r.z2,
        }
    }
    fn lot_facing(&self, lot: u64) -> u32 {
        cb!(self, lot_facing, 0, lot)
    }
    fn lot_zone_compatible(&self, lot: u64, zone_type: u16) -> bool {
        cb!(self, lot_zone_compatible, false, lot, zone_type)
    }
    fn lot_configuration(&self, id: u32) -> Option<u64> {
        opt(cb!(self, lot_configuration, 0, id))
    }
    fn start_demolish_lot(&self, lot: u64) {
        cb!(self, start_demolish_lot, (), lot)
    }
    fn end_demolish_lot(&self, lot: u64) {
        cb!(self, end_demolish_lot, (), lot)
    }
    fn create_lot(&self, x: i32, z: i32, width: i32, depth: i32, facing: u32) -> Option<u64> {
        opt(cb!(self, create_lot, 0, x, z, width, depth, facing))
    }
    // A null configuration (PD5) goes to the host as 0.
    fn set_lot_configuration(&self, lot: u64, config: Option<u64>) {
        cb!(self, set_lot_configuration, (), lot, config.unwrap_or(0))
    }
    fn construct_default_lot(&self, lot: u64, config: Option<u64>) -> bool {
        cb!(self, construct_default_lot, false, lot, config.unwrap_or(0))
    }
    fn dezone(&self, rect: CellRect) {
        let r = rect_c(rect);
        cb!(self, dezone, (), &r)
    }
    fn update_occupant_effects(&self, lot: u64, old: f32, new: f32) {
        cb!(self, update_occupant_effects, (), lot, old, new)
    }
    fn record_history(&self, key: u32, value: i32) {
        cb!(self, record_history, (), key, value)
    }
    fn push_event(&self, event: u32) {
        cb!(self, push_event, (), event)
    }
    fn office_counts(&self, developer_type: u32) -> Vec<u32> {
        u32s(|s, c| cb!(self, office_counts, (), developer_type, s, c))
    }
    fn lot_subnetworks(&self, lot: u64) -> Vec<u32> {
        u32s(|s, c| cb!(self, lot_subnetworks, (), lot, s, c))
    }
    fn traffic_arrived(&self, lot: u64, trip_type: u32) -> u32 {
        cb!(self, traffic_arrived, 0, lot, trip_type)
    }
    fn set_max_trip_capacity(&self, lot: u64, trip_type: u32, capacity: u32) {
        cb!(self, set_max_trip_capacity, (), lot, trip_type, capacity)
    }
    fn terrain_size(&self) -> (u32, u32) {
        let (mut w, mut d) = (0u32, 0u32);
        cb!(self, terrain_size, (), &mut w, &mut d);
        (w, d)
    }
    fn cell_has_water(&self, x: u32, z: u32) -> bool {
        cb!(self, cell_has_water, false, x, z)
    }
    fn set_access_props_state(&self, lot: u64, property: u32, state: u32) -> u32 {
        cb!(self, set_access_props_state, 0, lot, property, state)
    }
    fn set_altitude(&self, x: i32, z: i32, altitude: f32) {
        cb!(self, set_altitude, (), x, z, altitude)
    }
}

// ---- handles ----------------------------------------------------------------------------------------

struct Handle {
    sim: Rc<PortDeveloper>,
}

fn with_p<R>(p: *const fc4_port_developer, fallback: R, f: impl FnOnce(&Handle) -> R) -> R {
    // SAFETY: documented precondition: NULL or a live fc4_port_developer*.
    match unsafe { (p as *const Handle).as_ref() } {
        Some(h) => guard(fallback, || f(h)),
        None => fallback,
    }
}

fn kind_of(kind: u32) -> Option<PortKind> {
    match kind {
        0 => Some(PortKind::Airport),
        1 => Some(PortKind::Seaport),
        _ => None,
    }
}

/// Creates a developer: `kind` 0 airport, 1 seaport (NULL for another
/// value). The host table is copied. `token` is the developer's own host
/// identity.
#[no_mangle]
pub unsafe extern "C" fn fc4_port_developer_create(
    host: *const fc4_port_host,
    kind: u32,
    token: u64,
) -> *mut fc4_port_developer {
    guard(std::ptr::null_mut(), || match (host.as_ref(), kind_of(kind)) {
        (Some(h), Some(k)) if (h.struct_size as usize) >= std::mem::size_of::<fc4_port_host>() => {
            let c = Rc::new(CHost { h: *h });
            let sim = PortDeveloper::new(k, c, token);
            Box::into_raw(Box::new(Handle { sim })) as *mut fc4_port_developer
        }
        _ => std::ptr::null_mut(),
    })
}

#[no_mangle]
pub unsafe extern "C" fn fc4_port_developer_destroy(p: *mut fc4_port_developer) {
    if !p.is_null() {
        guard((), || drop(Box::from_raw(p as *mut Handle)));
    }
}

/// `QueryInterface` (§1): the sub-object offset (0, 4, 8 or 0xC) in
/// `*offset`; false for an IID the kind does not answer.
#[no_mangle]
pub unsafe extern "C" fn fc4_port_developer_query_interface(kind: u32, iid: u32, offset: *mut u32) -> bool {
    match kind_of(kind).and_then(|k| query_interface(k, iid)) {
        Some(o) => {
            if let Some(out) = offset.as_mut() {
                *out = o;
            }
            true
        }
        None => false,
    }
}

#[no_mangle]
pub unsafe extern "C" fn fc4_port_developer_add_ref(p: *mut fc4_port_developer) -> u32 {
    with_p(p, 0, |h| h.sim.add_ref())
}

/// `Release`: the last release calls `Shutdown` and returns 0; the handle
/// stays valid until [`fc4_port_developer_destroy`].
#[no_mangle]
pub unsafe extern "C" fn fc4_port_developer_release(p: *mut fc4_port_developer) -> u32 {
    with_p(p, 0, |h| h.sim.release())
}

/// `Init`: tuning, department, agent; the seaport's map.
#[no_mangle]
pub unsafe extern "C" fn fc4_port_developer_init(p: *mut fc4_port_developer) -> bool {
    with_p(p, false, |h| h.sim.init())
}

#[no_mangle]
pub unsafe extern "C" fn fc4_port_developer_shutdown(p: *mut fc4_port_developer) -> bool {
    with_p(p, false, |h| h.sim.shutdown())
}

/// `DoMessage` (§3): `data` is the occupant (`0x99EF1142`) or the lot
/// (`0xE8BDA2E7`, with the old and new states); the month `0x66956816`,
/// PostCityInit, the terrain message and the growth messages take no data.
/// Always true.
#[no_mangle]
pub unsafe extern "C" fn fc4_port_developer_do_message(
    p: *mut fc4_port_developer,
    msg: u32,
    data: u64,
    old_state: u32,
    new_state: u32,
) -> bool {
    with_p(p, false, |h| h.sim.do_message(msg, data, old_state, new_state))
}

/// `GetAvgEfficiency` (1.0 with no ports).
#[no_mangle]
pub unsafe extern "C" fn fc4_port_developer_avg_efficiency(p: *const fc4_port_developer) -> f32 {
    with_p(p, 0.0, |h| h.sim.avg_efficiency())
}

/// `GetLeastEfficientPort`: false with no ports. [choice] With two or more
/// the minimum (first on ties).
#[no_mangle]
pub unsafe extern "C" fn fc4_port_developer_least_efficient(
    p: *const fc4_port_developer,
    efficiency: *mut f32,
    x: *mut i16,
    z: *mut i16,
) -> bool {
    with_p(p, false, |h| match h.sim.least_efficient_port() {
        Some((e, px, pz)) => {
            if let Some(o) = efficiency.as_mut() {
                *o = e;
            }
            if let Some(o) = x.as_mut() {
                *o = px;
            }
            if let Some(o) = z.as_mut() {
                *o = pz;
            }
            true
        }
        None => false,
    })
}

/// `GetAllPortLots`: up to `cap` lot tokens into `out`; returns the count
/// (call with `cap` 0 to size).
#[no_mangle]
pub unsafe extern "C" fn fc4_port_developer_all_port_lots(
    p: *const fc4_port_developer,
    out: *mut u64,
    cap: usize,
) -> usize {
    with_p(p, 0, |h| {
        let lots = h.sim.all_port_lots();
        if !out.is_null() {
            for (i, &l) in lots.iter().take(cap).enumerate() {
                *out.add(i) = l;
            }
        }
        lots.len()
    })
}

/// `GetNumPorts(sizeFlags, stageFlags)` (size bit `1 << sizeClass`).
#[no_mangle]
pub unsafe extern "C" fn fc4_port_developer_num_ports(
    p: *const fc4_port_developer,
    size_flags: u32,
    stage_flags: u32,
) -> u32 {
    with_p(p, 0, |h| h.sim.num_ports(size_flags, stage_flags))
}

/// `GetNumAirports(size)` (airport slot 11).
#[no_mangle]
pub unsafe extern "C" fn fc4_port_developer_num_airports(p: *const fc4_port_developer, size: u32) -> u32 {
    with_p(p, 0, |h| h.sim.num_airports(size))
}

/// `GetNumSeaports()` (seaport slot 11, no argument).
#[no_mangle]
pub unsafe extern "C" fn fc4_port_developer_num_seaports(p: *const fc4_port_developer) -> u32 {
    with_p(p, 0, |h| h.sim.num_seaports())
}

/// `Get…UsageValues(lot, &usage, &cap)`: false (0, 0) if not a port.
#[no_mangle]
pub unsafe extern "C" fn fc4_port_developer_usage_values(
    p: *const fc4_port_developer,
    lot: u64,
    usage: *mut u32,
    cap: *mut u32,
) -> bool {
    with_p(p, false, |h| {
        let r = h.sim.usage_values(lot);
        let (u, c) = r.unwrap_or((0, 0));
        if let Some(o) = usage.as_mut() {
            *o = u;
        }
        if let Some(o) = cap.as_mut() {
            *o = c;
        }
        r.is_some()
    })
}

/// `GetNum…Trips`.
#[no_mangle]
pub unsafe extern "C" fn fc4_port_developer_num_trips(p: *const fc4_port_developer) -> u32 {
    with_p(p, 0, |h| h.sim.num_trips())
}

/// `GetMaintenanceCost(lot)` (0 if not a port).
#[no_mangle]
pub unsafe extern "C" fn fc4_port_developer_maintenance_cost(p: *const fc4_port_developer, lot: u64) -> i64 {
    with_p(p, 0, |h| h.sim.maintenance_cost(lot))
}

/// `CityHasSea` (seaport).
#[no_mangle]
pub unsafe extern "C" fn fc4_port_developer_city_has_sea(p: *const fc4_port_developer) -> bool {
    with_p(p, false, |h| h.sim.city_has_sea())
}

/// `GetCellSeaAccess(x, z)` (seaport): the map byte, 0 outside the map.
#[no_mangle]
pub unsafe extern "C" fn fc4_port_developer_cell_sea_access(p: *const fc4_port_developer, x: i32, z: i32) -> u8 {
    with_p(p, 0, |h| h.sim.cell_sea_access(x, z))
}

/// Port lot `index` (list order); false past the end.
#[no_mangle]
pub unsafe extern "C" fn fc4_port_developer_get_port(
    p: *const fc4_port_developer,
    index: u32,
    out: *mut fc4_port_lot,
) -> bool {
    let Some(o) = out.as_mut() else { return false };
    with_p(p, false, |h| match h.sim.ports().get(index as usize) {
        Some(l) => {
            *o = fc4_port_lot {
                struct_size: std::mem::size_of::<fc4_port_lot>() as u32,
                stage: l.stage,
                type_index: l.type_index as u32,
                x: l.x,
                z: l.z,
                efficiency: l.efficiency,
                usage: l.usage,
                trips: l.trips,
                needs_lookup: l.needs_lookup.into(),
                swapping: l.swapping.into(),
                reserved: [0; 2],
                lot: l.lot.unwrap_or(0),
            };
            true
        }
        None => false,
    })
}

#[no_mangle]
pub unsafe extern "C" fn fc4_port_developer_get_state(p: *const fc4_port_developer, out: *mut fc4_port_state) -> bool {
    let Some(o) = out.as_mut() else { return false };
    with_p(p, false, |h| {
        let s = h.sim.snapshot();
        let m = h.sim.last_month();
        *o = fc4_port_state {
            struct_size: std::mem::size_of::<fc4_port_state>() as u32,
            kind: u32::from(h.sim.kind() == PortKind::Seaport),
            tuning_instance: s.tuning_instance,
            type_count: s.type_count,
            types_loaded: h.sim.types().len() as u32,
            port_count: h.sim.ports().len() as u32,
            ref_count: s.ref_count,
            zone_type: s.zone_type,
            tuning_loaded: s.tuning_loaded.into(),
            initialized: s.initialized.into(),
            destroyed: s.destroyed.into(),
            recheck_access: s.recheck_access.into(),
            city_has_sea: s.city_has_sea.into(),
            reserved: 0,
            month_ran: m.ran.into(),
            month_history: m.history,
            month_event: m.event,
            month_skipped: m.skipped,
            reserved2: 0,
            department: s.department.unwrap_or(0),
        };
        true
    })
}

/// `Write` (§7).
#[no_mangle]
pub unsafe extern "C" fn fc4_port_developer_write(p: *const fc4_port_developer, out: *const fc4_ostream) -> bool {
    let Some(o) = out.as_ref() else { return false };
    if !ostream_ok(o) {
        return false;
    }
    with_p(p, false, |h| h.sim.write(&mut COut(o)))
}

fn no_core_objects(_: u32, _: *mut c_void) -> Option<Rc<dyn SaveObject>> {
    None
}

/// `Read` (§7): false for another version or a type index out of range
/// (the port list is then unchanged); appends the ports.
#[no_mangle]
pub unsafe extern "C" fn fc4_port_developer_read(p: *mut fc4_port_developer, inp: *const fc4_istream) -> bool {
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
