// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The aura shadow (fc4.h "aura shadow"): with the ORIGINAL aura simulator
//! running, FreeCity4's pipeline (docs/specs/aura.md §6) follows it step by
//! step and each of its cycles is compared with it (§14.3, AU7, AU13-AU15;
//! tooling, no spec behaviour).
//!
//! # Following the original (§14.3)
//!
//! A shadow that ran a whole cycle at the original's cycle-completed message
//! would read every input later than the original did (§5.3) and show timing
//! differences. This one reads them at the original's own moments:
//!
//! - The plugin reads the original's pipeline state (s32 at +0x70 of its
//!   `cISC4AuraSimulator` pointer, §14.3) before and after each SimIdle /
//!   SimNewDay / SimNewMonth delivery to its agent. When it moved from s to
//!   s′, [`fc4_aura_shadow_step`] runs our steps s, s+1, …, s′−1 at once,
//!   before any other agent runs; a return to 0 (from state 4 without lots,
//!   or from 17) runs our cycle to its end. Each of our states therefore
//!   reads its inputs (§5.3) right after the original's state read them.
//! - The original takes several calls for state 3 (one department each) and
//!   state 5 (time slices); ours runs whole in the call where the original
//!   leaves the state. A department grid changed, or a lot added or removed,
//!   between two of the original's calls can still differ; the work grid
//!   diagnostic shows such lots.
//! - Occupant inserted / removed (posted, §5.3) reach our raw park,
//!   department and building-effect grids when the original receives them
//!   ([`fc4_aura_shadow_occupant`]), not when the occupant is created.
//! - The transient grid takes effects the shadow cannot see (§8: fires, the
//!   limo, situations). States 15 and 17 read it, and 17 also writes it, so
//!   the plugin calls [`fc4_aura_shadow_before_step`] before a delivery that
//!   finds the original in state 15 or 17: the original's transient grid,
//!   the input that state is about to read, is copied into ours.
//! - After each of our steps 5..13 our work grid is compared with the
//!   original's (+0x44, §14.3) when the plugin passes it: it shows where in
//!   the pipeline a difference starts.
//! - Tax baseline (§6.5.2, §14.3): ours is created at city load from the
//!   original's record, so our first state 4 takes `previousTaxRate` from
//!   the same tax rates as the original's first state 4 after the load. It
//!   is kept when ours restarts from a record.
//!
//! # Comparing
//!
//! When the original's state returns to 0 the plugin captures its record
//! (its own `cIGZSerializable::Write` into memory) and calls
//! [`fc4_aura_shadow_cycle`]: the record's base, aura, park, landmark and
//! transient grids, mayor rating, best / worst and hotspots are compared with
//! ours. The park and landmark maps are exact too: ours follow the same
//! occupant messages, so they lag the same cycle (§6.3). Then ours restarts
//! from the record (Read, then Init, state 0), so each cycle starts from the
//! original's own grids. At city load [`fc4_aura_shadow_baseline`] creates
//! ours from the loaded record (Read, then Init: state 1, as the original's).
//!
//! Not compared: the riot roll (the original's generator is seeded from the
//! wall clock, §9); posts, history, disasters and strikes (writes, which the
//! shadow swallows). Nothing is written to the game: our grids are core
//! copies, and every write callback of the host table is unused.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::ffi::c_void;
use std::rc::Rc;

use fc4_core::aura::{msg, AuraHost, AuraMessage, AuraRng, AuraSim, LotInfo, LotIterator, Strike};
use fc4_core::grid::{CityDims, GridElem, GridProvider, SimGrid, TractGeometry, TractGrid, IID_SIM_GRID};
use fc4_core::julian::civil_from_julian;
use fc4_core::property::PropertyHolder;
use fc4_core::rng::FixedSeeds;
use fc4_core::stream::{ByteReader, InStream, RefValue, SaveObject};

use crate::aura::{fc4_aura_host, CHost};
use crate::guard;
use crate::landvalue::fc4_grid_geometry;
use crate::stream::fc4_bytes_sink;

/// One reference of a captured record (fc4.h `fc4_aura_capture_ref`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_aura_capture_ref {
    /// The object as a `cISC4SimGrid` (IID `0xC9BD939A`), borrowed; 0 = not a grid.
    pub grid: u64,
    /// `GetGZCLSID()`.
    pub class_id: u32,
    pub reserved: u32,
}

/// A captured record (fc4.h `fc4_aura_capture`): the bytes the original's
/// `Write` produced (primitives little-endian, references as `u32 objectID`
/// (index + 1, 0 = null) then `u32 classID`) and its references in write
/// order.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct fc4_aura_capture {
    pub struct_size: u32,
    /// Non-zero when `Write` returned true and the stream stayed good.
    pub ok: u32,
    pub bytes: *const u8,
    pub len: usize,
    pub refs: *const fc4_aura_capture_ref,
    pub ref_count: usize,
}

/// One move of the original's pipeline state (fc4.h `fc4_aura_shadow_step_in`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_aura_shadow_step_in {
    pub struct_size: u32,
    /// The original's state before and after the delivery (0..=17).
    pub from: i32,
    pub to: i32,
    /// The current date (the cycle's days on its line).
    pub julian: i32,
    /// The original's work grid (+0x44, §14.3) as a `cISC4SimGrid<int8>`,
    /// borrowed; 0 = not compared.
    pub work_grid: u64,
}

/// Totals since creation (fc4.h `fc4_aura_shadow_stats`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_aura_shadow_stats {
    pub struct_size: u32,
    /// Cycle ends seen (compared or not).
    pub cycles: u32,
    pub compared: u32,
    /// ... compared without any difference and without a desync.
    pub clean: u32,
    /// ... not compared (no record to start from, a capture failed, the
    /// cycle was joined in its middle, our cycle did not end).
    pub incomplete: u32,
    /// The last cycle's differences (fc4_aura_shadow_cycle; -1: not compared).
    pub last_mismatches: i32,
    /// Cycles whose mayor rating differed.
    pub rating_mismatches: u32,
    /// Compared cycles in which our state did not follow the original's.
    pub desynced: u32,
    /// Tracts compared per grid, and the differing base / aura tracts.
    pub tracts: u64,
    pub base_mismatches: u64,
    pub aura_mismatches: u64,
    /// ... aura differences in tracts whose final transient differs.
    pub aura_transient: u64,
}

const _: () = {
    assert!(std::mem::size_of::<fc4_aura_capture_ref>() == 16);
    assert!(std::mem::size_of::<fc4_aura_capture>() == if cfg!(target_pointer_width = "32") { 24 } else { 40 });
    assert!(std::mem::size_of::<fc4_aura_shadow_step_in>() == 24);
    assert!(std::mem::size_of::<fc4_aura_shadow_stats>() == 64);
};

/// The most pipeline calls one step runs (17 states, one call per
/// department in state 3, `MAX_LOTS_PER_CALL` lots per state-5 call).
const MAX_STEPS: u32 = 100_000;
/// The largest grid side copied (tracts).
const MAX_SIDE: i32 = 4096;
/// The work grid's states (§14.3: the per-step diagnostic).
const WORK_STATES: std::ops::RangeInclusive<i32> = 5..=13;

// ---- the shadow's host --------------------------------------------------------------------------------

/// The shadow's `AuraHost`: reads go to the read-only C table, grids are core
/// copies, writes stay here.
struct ShadowHost {
    c: CHost,
    dims: CityDims,
    /// Inputs that answered "none" in the current cycle, in first-ask order.
    missing: RefCell<Vec<&'static str>>,
    /// Calls swallowed (posts, history, disasters, strikes, registrations).
    swallowed: Cell<u32>,
}

impl ShadowHost {
    fn swallow(&self) {
        self.swallowed.set(self.swallowed.get().wrapping_add(1));
    }

    fn note<T>(&self, name: &'static str, v: Option<T>) -> Option<T> {
        if v.is_none() {
            let mut m = self.missing.borrow_mut();
            if !m.contains(&name) {
                m.push(name);
            }
        }
        v
    }

    fn geometry(&self, token: u64) -> TractGeometry {
        let api = &self.c.0.grids;
        let mut g = fc4_grid_geometry::default();
        if let Some(f) = api.geometry {
            // SAFETY: valid callback (fc4_aura_shadow_create's precondition).
            unsafe { f(api.user, token, &mut g) };
        }
        TractGeometry {
            tract_size: g.tract_size,
            shift: g.shift,
            count_x: g.count_x,
            count_z: g.count_z,
            width_x: g.width_x,
            width_z: g.width_z,
            inv_width_x: g.inv_width_x,
            inv_width_z: g.inv_width_z,
        }
    }

    /// A live game grid's values (x outer), or None when it cannot be read.
    fn game_values<T: GridElem>(&self, token: u64) -> Option<(TractGeometry, Vec<T>)> {
        if token == 0 {
            return None;
        }
        let api = &self.c.0.grids;
        let get = api.get?;
        let geom = self.geometry(token);
        if !(1..=MAX_SIDE).contains(&geom.count_x) || !(1..=MAX_SIDE).contains(&geom.count_z) {
            return None;
        }
        let mut values = Vec::with_capacity((geom.count_x * geom.count_z) as usize);
        for x in 0..geom.count_x {
            for z in 0..geom.count_z {
                // SAFETY: valid callback on a live game grid.
                values.push(T::from_f64(unsafe { get(api.user, token, T::CLSID, x, z) }));
            }
        }
        Some((geom, values))
    }

    /// A core copy of a live game grid.
    fn copy<T: GridElem>(&self, token: u64) -> Option<Rc<SimGrid<T>>> {
        let (geom, values) = self.game_values::<T>(token)?;
        Some(SimGrid::from_values(geom, &values))
    }

    /// Overwrites `ours` with a live game grid of the same tract counts.
    fn overwrite<T: GridElem>(&self, ours: &Rc<dyn TractGrid<T>>, token: u64) -> bool {
        let Some((geom, values)) = self.game_values::<T>(token) else {
            return false;
        };
        let g = ours.geometry();
        if (g.count_x, g.count_z) != (geom.count_x, geom.count_z) {
            return false;
        }
        let mut i = 0;
        for x in 0..geom.count_x {
            for z in 0..geom.count_z {
                ours.set(x, z, values[i]);
                i += 1;
            }
        }
        true
    }

    fn resolve<T: GridElem>(&self, r: &RefValue) -> Option<Rc<dyn TractGrid<T>>> {
        match *r {
            RefValue::Host { token, class_id } if token != 0 && class_id == T::CLSID => {
                self.copy::<T>(token).map(|g| g as Rc<dyn TractGrid<T>>)
            }
            _ => None,
        }
    }

    fn create<T: GridElem>(&self, instance: u32, tract_size: i32) -> Option<Rc<dyn TractGrid<T>>> {
        Some(SimGrid::<T>::create(&self.dims, instance, tract_size) as Rc<dyn TractGrid<T>>)
    }
}

impl GridProvider<i8> for ShadowHost {
    fn create_grid(&self, instance: u32, tract_size: i32) -> Option<Rc<dyn TractGrid<i8>>> {
        self.create(instance, tract_size)
    }
    fn resolve_grid(&self, r: &RefValue) -> Option<Rc<dyn TractGrid<i8>>> {
        self.resolve(r)
    }
}

impl GridProvider<i16> for ShadowHost {
    fn create_grid(&self, instance: u32, tract_size: i32) -> Option<Rc<dyn TractGrid<i16>>> {
        self.create(instance, tract_size)
    }
    fn resolve_grid(&self, r: &RefValue) -> Option<Rc<dyn TractGrid<i16>>> {
        self.resolve(r)
    }
}

const POLLUTION_NAMES: [&str; 4] = ["air pollution", "water pollution", "garbage", "radiation"];

impl AuraHost for ShadowHost {
    fn cell_width(&self) -> (f32, f32) {
        self.c.cell_width()
    }
    fn position_to_cell(&self, x: f32, z: f32) -> (i32, i32) {
        self.c.position_to_cell(x, z)
    }
    fn sim_date_number(&self) -> u32 {
        self.c.sim_date_number()
    }
    fn tuning(&self) -> Option<Rc<PropertyHolder>> {
        self.note("tuning", self.c.tuning())
    }
    fn publish_base_aura(&self, _: [i32; 3]) {
        self.swallow();
    }
    fn lot_count(&self) -> u32 {
        self.c.lot_count()
    }
    fn lot_iterator(&self) -> Box<dyn LotIterator> {
        self.c.lot_iterator()
    }
    fn lot_at(&self, x: i32, z: i32) -> Option<LotInfo> {
        self.c.lot_at(x, z)
    }
    fn land_value_type(&self, x: i32, z: i32) -> Option<i32> {
        self.c.land_value_type(x, z)
    }
    fn occupant_has_property(&self, occupant: u64, prop: u32) -> bool {
        self.c.occupant_has_property(occupant, prop)
    }
    fn occupant_sint32s(&self, occupant: u64, prop: u32) -> Option<Vec<i32>> {
        self.c.occupant_sint32s(occupant, prop)
    }
    fn occupant_type(&self, occupant: u64) -> u32 {
        self.c.occupant_type(occupant)
    }
    fn occupant_position(&self, occupant: u64) -> [f32; 3] {
        self.c.occupant_position(occupant)
    }
    fn budget_item_department(&self, occupant: u64, purpose: u32) -> Option<u32> {
        self.c.budget_item_department(occupant, purpose)
    }
    fn department_funding(&self, dept: u32) -> Option<f32> {
        self.c.department_funding(dept)
    }
    fn tax_rate(&self, index: i32) -> f32 {
        self.c.tax_rate(index)
    }
    fn cumulative_offset(&self, prop: u32, index: u32) -> i32 {
        self.c.cumulative_offset(prop, index)
    }
    fn pollution_grid(&self, kind: i32) -> Option<Rc<dyn TractGrid<i16>>> {
        let name = POLLUTION_NAMES.get(kind as usize).copied().unwrap_or("pollution");
        self.note(name, self.c.pollution_grid(kind))
    }
    fn school_grade_map(&self) -> Option<Rc<dyn TractGrid<f32>>> {
        self.note("school grades", self.c.school_grade_map())
    }
    fn hospital_grade_map(&self) -> Option<Rc<dyn TractGrid<f32>>> {
        self.note("hospital grades", self.c.hospital_grade_map())
    }
    fn crime_grid(&self) -> Option<Rc<dyn TractGrid<u8>>> {
        self.note("crime", self.c.crime_grid())
    }
    fn trip_length_map(&self) -> Option<Rc<dyn TractGrid<u8>>> {
        self.note("trip length", self.c.trip_length_map())
    }
    fn congestion_map(&self) -> Option<Rc<dyn TractGrid<u8>>> {
        self.note("congestion", self.c.congestion_map())
    }
    fn population_grids(&self) -> Option<[Rc<dyn TractGrid<u16>>; 3]> {
        self.note("population grids", self.c.population_grids())
    }
    fn record_history(&self, _: u32, _: i32) {
        self.swallow();
    }
    fn attempt_disaster_start(&self, _: u32, _: i32, _: i32) {
        self.swallow();
    }
    fn end_strike(&self, _: Strike) {
        self.swallow();
    }
    fn post(&self, _: u32) {
        self.swallow();
    }
    fn limit_timer_start(&self, _: u32) {}
    fn limit_timer_expired(&self) -> bool {
        false
    }
    fn subscribe(&self, _: &[u32]) {
        self.swallow();
    }
    fn unsubscribe(&self, _: &[u32]) {
        self.swallow();
    }
    fn add_agent(&self, _: u32, _: &str, _: u32) {
        self.swallow();
    }
    fn remove_agent(&self) {
        self.swallow();
    }
    fn registry_add(&self) -> bool {
        self.swallow();
        false
    }
    fn registry_remove(&self) {
        self.swallow();
    }
}

// ---- the captured record ---------------------------------------------------------------------------

/// A captured record as a core input stream: a reference is answered with
/// its grid token when the reader asks for a grid.
struct CaptureReader<'a> {
    r: ByteReader<'a>,
    refs: &'a [fc4_aura_capture_ref],
}

impl InStream for CaptureReader<'_> {
    fn get_u8(&mut self) -> Option<u8> {
        self.r.get_u8()
    }
    fn get_u16(&mut self) -> Option<u16> {
        self.r.get_u16()
    }
    fn get_u32(&mut self) -> Option<u32> {
        self.r.get_u32()
    }
    fn get_u64(&mut self) -> Option<u64> {
        self.r.get_u64()
    }
    fn get_f32(&mut self) -> Option<f32> {
        self.r.get_f32()
    }
    fn get_f64(&mut self) -> Option<f64> {
        self.r.get_f64()
    }
    fn get_bytes(&mut self, n: usize) -> Option<Vec<u8>> {
        self.r.get_bytes(n)
    }
    fn get_str(&mut self) -> Option<Vec<u8>> {
        self.r.get_str()
    }
    fn get_ref(&mut self) -> Option<RefValue> {
        self.get_ref_as(0)
    }
    fn get_ref_as(&mut self, iid: u32) -> Option<RefValue> {
        let id = self.r.get_u32()?;
        if id == 0 {
            return Some(RefValue::Null);
        }
        let class_id = self.r.get_u32()?;
        let Some(e) = self.refs.get(id as usize - 1) else {
            self.r.set_error();
            return None;
        };
        if iid == IID_SIM_GRID && e.grid != 0 {
            return Some(RefValue::Host {
                token: e.grid,
                class_id,
            });
        }
        // §2.7 helper semantics: an object without the interface reads as null.
        Some(RefValue::Null)
    }
    fn skip(&mut self, n: usize) -> bool {
        self.r.skip(n)
    }
    fn error(&self) -> i32 {
        self.r.error()
    }
    fn set_error(&mut self) {
        self.r.set_error();
    }
}

/// A capture copied out of the caller's buffers.
struct Capture {
    bytes: Vec<u8>,
    refs: Vec<fc4_aura_capture_ref>,
}

/// Copies a capture; `Err` with the reason when it is not usable.
unsafe fn capture_of(c: *const fc4_aura_capture) -> Result<Capture, &'static str> {
    let Some(c) = c.as_ref() else {
        return Err("no capture");
    };
    if (c.struct_size as usize) < std::mem::size_of::<fc4_aura_capture>() {
        return Err("capture struct too small");
    }
    if c.ok == 0 {
        return Err("the original's Write failed");
    }
    if c.bytes.is_null() || c.len == 0 {
        return Err("empty record");
    }
    let bytes = std::slice::from_raw_parts(c.bytes, c.len).to_vec();
    let refs = if c.refs.is_null() || c.ref_count == 0 {
        Vec::new()
    } else {
        std::slice::from_raw_parts(c.refs, c.ref_count).to_vec()
    };
    Ok(Capture { bytes, refs })
}

// ---- comparisons ------------------------------------------------------------------------------------

/// A differing tract: (x, z, orig, ours).
type Tract = (i32, i32, i64, i64);

/// A grid comparison: tracts, differences, max |d|, the first differing
/// tracts.
#[derive(Default)]
struct Diff {
    tracts: u64,
    mismatches: u64,
    max_abs: i64,
    first: Vec<Tract>,
    /// Differences in tracts of `mask`.
    masked: u64,
    geometry_differs: bool,
}

impl Diff {
    fn add(&mut self, x: i32, z: i32, a: i64, b: i64, keep: usize, masked: bool) {
        self.tracts += 1;
        if a == b {
            return;
        }
        self.mismatches += 1;
        self.max_abs = self.max_abs.max((a - b).abs());
        if masked {
            self.masked += 1;
        }
        if self.first.len() < keep {
            self.first.push((x, z, a, b));
        }
    }
}

fn compare<T: GridElem>(
    orig: &Option<Rc<dyn TractGrid<T>>>,
    ours: &Option<Rc<dyn TractGrid<T>>>,
    keep: usize,
    mask: Option<&BTreeSet<(i32, i32)>>,
) -> Option<Diff> {
    let (o, u) = (orig.as_ref()?, ours.as_ref()?);
    let (go, gu) = (o.geometry(), u.geometry());
    let mut d = Diff::default();
    if (go.count_x, go.count_z) != (gu.count_x, gu.count_z) {
        d.geometry_differs = true;
        return Some(d);
    }
    for x in 0..go.count_x.clamp(0, MAX_SIDE) {
        for z in 0..go.count_z.clamp(0, MAX_SIDE) {
            let (a, b) = (o.get(x, z).to_f64() as i64, u.get(x, z).to_f64() as i64);
            d.add(x, z, a, b, keep, mask.is_some_and(|m| m.contains(&(x, z))));
        }
    }
    Some(d)
}

fn mismatches(d: &Option<Diff>) -> u64 {
    d.as_ref().map_or(0, |d| d.mismatches)
}

fn show(name: &str, d: &Option<Diff>) -> String {
    match d {
        None => format!("{name} missing"),
        Some(d) if d.geometry_differs => format!("{name} geometry differs"),
        Some(d) => format!("{name} mismatches={} max|d|={}", d.mismatches, d.max_abs),
    }
}

fn cells(first: &[Tract]) -> String {
    let v: Vec<String> = first.iter().map(|(x, z, a, b)| format!("({x},{z}) {a}/{b}")).collect();
    v.join(" ")
}

fn first_cells(name: &str, d: &Option<Diff>) -> Option<String> {
    let d = d.as_ref()?;
    (!d.first.is_empty()).then(|| format!("{name} {}", cells(&d.first)))
}

fn date(julian: i32) -> String {
    let d = civil_from_julian(julian);
    format!("{:04}-{:02}-{:02}", d.year, d.month, d.day)
}

// ---- the shadow -------------------------------------------------------------------------------------

/// Opaque handle (fc4.h `fc4_aura_shadow`).
#[repr(C)]
pub struct fc4_aura_shadow {
    _private: [u8; 0],
}

/// What happened in the cycle being followed.
#[derive(Default)]
struct Track {
    /// The first state we ran (1: the cycle was followed from its start).
    began: Option<i32>,
    /// The last state we ran (17, or 4 for a city without lots).
    last: i32,
    start_julian: Option<i32>,
    moves: u32,
    /// Our pipeline calls (`DoIdle`).
    calls: u32,
    occupants: u32,
    /// Copies of the original's transient grid into ours.
    transient_copies: u32,
    /// Where our state did not follow the original's.
    desync: Vec<String>,
    /// After our states 5..=13: (cells differing, max |d|); None = not compared.
    work: [Option<(u64, i64)>; 9],
    /// The first state whose work grid differed, and its first cells.
    work_first: Option<(i32, Vec<Tract>)>,
}

/// When our tax baseline (§6.5.2) was taken.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TaxBaseline {
    NotYet,
    /// In the first cycle after the city load, as the original's.
    AtLoad,
    /// Later (ours was created after the load): tax-change effects can differ.
    Later(u32),
}

struct Shadow {
    host: Rc<ShadowHost>,
    max_cells: usize,
    /// Our simulator, rebuilt from the original's record at each cycle end.
    ours: Option<Rc<AuraSim>>,
    /// Why there is none.
    no_ours: &'static str,
    /// Kept when ours is rebuilt (§6.5.2: never saved, taken once).
    previous_tax_rate: [f32; 12],
    tax: TaxBaseline,
    /// Ours was created at city load and no cycle has ended since.
    first_after_load: bool,
    track: Track,
    stats: fc4_aura_shadow_stats,
}

fn fixed_rng() -> Box<AuraRng> {
    Box::new(AuraRng::new(&FixedSeeds {
        wall_clock: 1,
        process_clock: 1,
    }))
}

fn idle() -> AuraMessage {
    AuraMessage {
        msg_type: msg::SIM_IDLE,
        data3: 1000,
        ..AuraMessage::default()
    }
}

impl Shadow {
    /// The record in a never-initialised core object holding copies.
    fn parse(&self, c: &Capture) -> Result<Rc<AuraSim>, &'static str> {
        let sim = AuraSim::new(self.host.clone() as Rc<dyn AuraHost>, 0, fixed_rng());
        // Shadows replay the original's pacing (simulator.md §5.7).
        sim.set_pacing(fc4_core::pacing::Pacing::Original);
        let mut r = CaptureReader {
            r: ByteReader::new(&c.bytes),
            refs: &c.refs,
        };
        if !sim.read(&mut r) {
            return Err("the original's record did not parse");
        }
        if sim.base_grid().is_none() || sim.aura_grid().is_none() || sim.transient_grid().is_none() {
            return Err("the original's record names no base, aura or transient grid");
        }
        Ok(sim)
    }

    /// Ours from a parsed record: Init (Read before Init, as on a load:
    /// it creates the work and landmark-work grids, state 1), the kept tax
    /// baseline, and state 0 unless at city load.
    fn restart_from(&mut self, sim: Rc<AuraSim>, at_load: bool) {
        sim.init();
        {
            let mut d = sim.data_mut();
            d.previous_tax_rate = self.previous_tax_rate;
            if !at_load {
                d.state = 0;
            }
        }
        self.ours = Some(sim);
    }

    fn baseline(&mut self, c: Result<Capture, &'static str>, lines: &mut Vec<String>) -> bool {
        // A new city: a new original, so a new simulator and tax baseline.
        self.ours = None;
        self.track = Track::default();
        self.previous_tax_rate = [-1.0; 12];
        self.tax = TaxBaseline::NotYet;
        self.first_after_load = false;
        self.host.missing.borrow_mut().clear();
        match c.and_then(|c| self.parse(&c)) {
            Ok(sim) => {
                let (data, nonzero, tracts) = {
                    let base = sim.base_grid().map(|g| values_of(&g)).unwrap_or_default();
                    let transient = sim.transient_grid().map(|g| values_of(&g)).unwrap_or_default();
                    (
                        base.iter().filter(|&&v| v != -128).count(),
                        transient.iter().filter(|&&v| v != 0).count(),
                        base.len(),
                    )
                };
                let departments = sim.department_ids().len();
                let hotspots = sim.data().hotspots.len();
                lines.push(format!(
                    "aura shadow: city load: ours starts from the original's record (Read, then Init: state 1): base \
                     {data} of {tracts} tracts with data, transient {nonzero} nonzero, {departments} departments, \
                     {hotspots} hotspots"
                ));
                self.restart_from(sim, true);
                self.first_after_load = true;
                true
            }
            Err(why) => {
                lines.push(format!(
                    "aura shadow: no baseline at city load ({why}); ours starts from the record of the first cycle \
                     end, and the cycle after it is compared"
                ));
                self.no_ours = "no baseline at city load";
                false
            }
        }
    }

    /// One move of the original's state (§14.3). Returns our pipeline calls,
    /// -1 when nothing ran.
    fn step(&mut self, s: &fc4_aura_shadow_step_in) -> i32 {
        let Some(ours) = self.ours.clone() else {
            return -1;
        };
        let (from, to) = (s.from, s.to);
        if from == 3 && to == 3 {
            return self.step_department();
        }
        if !(0..=17).contains(&from) || !(0..=17).contains(&to) || from == to {
            return -1;
        }
        if from == 0 {
            // A new cycle (SimNewMonth, §5.1: 0 → 1). The occupant messages
            // since the last cycle end count for this one (ours received them
            // in state 0, as the original did).
            let was = ours.data().state;
            self.track = Track {
                began: Some(1),
                start_julian: Some(s.julian),
                occupants: self.track.occupants,
                ..Track::default()
            };
            if was != 0 {
                self.track
                    .desync
                    .push(format!("the original started a cycle while ours was in state {was}"));
            }
            ours.data_mut().state = 1;
        } else {
            let t = &mut self.track;
            t.start_julian.get_or_insert(s.julian);
            t.began.get_or_insert(from);
            let was = ours.data().state;
            if was != from {
                t.desync.push(format!("{from}->{to} with ours in {was}"));
                ours.data_mut().state = from;
            }
            if to != 0 && to < from {
                // Not a pipeline move (§6 only moves forward or to 0).
                t.desync.push(format!("{from}->{to} backwards"));
                t.began = None;
                ours.data_mut().state = to;
                t.moves += 1;
                return 0;
            }
        }
        self.track.moves += 1;
        let mut calls = 0u32;
        loop {
            let state = ours.data().state;
            if state == to || state == 0 || calls >= MAX_STEPS {
                break;
            }
            let untaxed = ours.data().previous_tax_rate.iter().all(|&r| r == -1.0);
            ours.do_message(&idle());
            calls += 1;
            self.track.last = state;
            if state == 4 && untaxed && ours.data().previous_tax_rate.iter().any(|&r| r != -1.0) {
                self.previous_tax_rate = ours.data().previous_tax_rate;
                self.tax = if self.first_after_load {
                    TaxBaseline::AtLoad
                } else {
                    TaxBaseline::Later(self.stats.cycles.wrapping_add(1))
                };
            }
        }
        self.track.calls = self.track.calls.wrapping_add(calls);
        let reached = ours.data().state;
        if reached != to {
            self.track.desync.push(format!("{from}->{to}: ours reached {reached}"));
        }
        // The work grid after the last step run (§14.3).
        let last = to - 1;
        if reached == to && to > from && WORK_STATES.contains(&last) && s.work_grid != 0 {
            if let Some(d) = self.compare_work(&ours, s.work_grid) {
                let t = &mut self.track;
                t.work[(last - 5) as usize] = Some((d.mismatches, d.max_abs));
                if d.mismatches > 0 && t.work_first.is_none() {
                    t.work_first = Some((last, d.first));
                }
            }
        }
        calls.min(i32::MAX as u32) as i32
    }

    /// State 3 spread over calls (§6.3: one department per call, N departments
    /// take N + 1 calls). `from == to == 3` means the original stayed in state 3
    /// but moved its department cursor: ours runs one state-3 call too, so each
    /// department grid is read at the original's moment (an occupant message
    /// between two of the original's department calls reaches both sides the
    /// same way). [choice] §14.3 names state 5 only; without this, ours summed
    /// every department at the 3 -> 4 move, after the original had read the
    /// earlier ones (verify-regress4, AU16). Returns 1, or 0 when ours is not
    /// in state 3 (a desync, recorded).
    fn step_department(&mut self) -> i32 {
        let Some(ours) = self.ours.clone() else {
            return -1;
        };
        let was = ours.data().state;
        if was != 3 {
            self.track.desync.push(format!("3->3 (department) with ours in {was}"));
            return 0;
        }
        ours.do_message(&idle());
        self.track.calls = self.track.calls.wrapping_add(1);
        self.track.last = 3;
        1
    }

    /// Before a delivery to the original in `state`: the transient grid that
    /// states 15 and 17 read (§5.3) is copied into ours, when ours waits in
    /// the same state.
    fn before_step(&mut self, state: i32, transient: u64) -> bool {
        let Some(ours) = self.ours.clone() else {
            return false;
        };
        if !(state == 15 || state == 17) || transient == 0 || ours.data().state != state {
            return false;
        }
        let Some(g) = ours.transient_grid() else {
            return false;
        };
        let copied = self.host.overwrite(&g, transient);
        if copied {
            self.track.transient_copies += 1;
        }
        copied
    }

    fn compare_work(&self, ours: &AuraSim, token: u64) -> Option<Diff> {
        let u = ours.work_grid()?;
        let (geom, values) = self.host.game_values::<i8>(token)?;
        let g = u.geometry();
        let mut d = Diff::default();
        if (g.count_x, g.count_z) != (geom.count_x, geom.count_z) {
            d.geometry_differs = true;
            d.mismatches = 1;
            return Some(d);
        }
        let mut i = 0;
        for x in 0..geom.count_x {
            for z in 0..geom.count_z {
                d.add(
                    x,
                    z,
                    i64::from(values[i]),
                    i64::from(u.get(x, z)),
                    self.max_cells,
                    false,
                );
                i += 1;
            }
        }
        Some(d)
    }

    fn occupant(&mut self, occupant: u64, inserted: bool) -> bool {
        let Some(ours) = self.ours.clone() else {
            return false;
        };
        ours.do_message(&AuraMessage {
            msg_type: if inserted {
                msg::OCCUPANT_INSERTED
            } else {
                msg::OCCUPANT_REMOVED
            },
            void1: occupant,
            ..AuraMessage::default()
        });
        self.track.occupants = self.track.occupants.wrapping_add(1);
        true
    }

    fn cycle(&mut self, c: Result<Capture, &'static str>, julian: i32, lines: &mut Vec<String>) -> i32 {
        self.stats.cycles = self.stats.cycles.wrapping_add(1);
        let n = self.stats.cycles;
        let track = std::mem::take(&mut self.track);
        let parsed = c.and_then(|c| self.parse(&c));
        let r = match &parsed {
            Ok(orig) => self.report(n, julian, &track, orig, lines),
            Err(why) => {
                lines.push(format!(
                    "aura shadow {} cycle {n}: INCOMPLETE ({why}); ours restarts from the next record",
                    date(julian)
                ));
                -1
            }
        };
        self.first_after_load = false;
        self.host.missing.borrow_mut().clear();
        match parsed {
            Ok(orig) => self.restart_from(orig, false),
            Err(_) => {
                self.ours = None;
                self.no_ours = "the last record could not be read";
            }
        }
        if r < 0 {
            self.stats.incomplete = self.stats.incomplete.wrapping_add(1);
        }
        self.stats.last_mismatches = r;
        r
    }

    fn report(&mut self, n: u32, julian: i32, t: &Track, orig: &AuraSim, lines: &mut Vec<String>) -> i32 {
        let head = format!("aura shadow {} cycle {n}", date(julian));
        let Some(ours) = self.ours.clone() else {
            lines.push(format!(
                "{head}: not compared (no record to start from: {}); ours starts from this record",
                self.no_ours
            ));
            return -1;
        };
        let days = julian.wrapping_sub(t.start_julian.unwrap_or(julian));
        let followed = match t.began {
            Some(b) => format!(
                "(followed states {b}-{} over {days} days: {} moves, {} calls, {} occupant messages)",
                t.last, t.moves, t.calls, t.occupants
            ),
            None => format!("(no state followed: {} moves)", t.moves),
        };
        if t.began != Some(1) {
            let why = match t.began {
                Some(b) => format!("joined the cycle in state {b}"),
                None => "no step of this cycle followed".to_string(),
            };
            lines.push(format!(
                "{head} {followed}: not compared ({why}); ours restarts from this record"
            ));
            return -1;
        }
        let state = ours.data().state;
        if state != 0 {
            lines.push(format!(
                "{head} {followed}: INCOMPLETE (our cycle did not end: state {state}); ours restarts from this record"
            ));
            return -1;
        }

        let keep = self.max_cells;
        let transient_d = compare(&orig.transient_grid(), &ours.transient_grid(), keep, None);
        let mut touched = BTreeSet::new();
        if let (Some(o), Some(u)) = (orig.transient_grid(), ours.transient_grid()) {
            let geo = o.geometry();
            if (geo.count_x, geo.count_z) == (u.geometry().count_x, u.geometry().count_z) {
                for x in 0..geo.count_x.clamp(0, MAX_SIDE) {
                    for z in 0..geo.count_z.clamp(0, MAX_SIDE) {
                        if o.get(x, z) != u.get(x, z) {
                            touched.insert((x, z));
                        }
                    }
                }
            }
        }
        let base_d = compare(&orig.base_grid(), &ours.base_grid(), keep, None);
        let aura_d = compare(&orig.aura_grid(), &ours.aura_grid(), keep, Some(&touched));
        let park_d = compare(&orig.park_map(), &ours.park_map(), keep, None);
        let landmark_d = compare(&orig.landmark_map(), &ours.landmark_map(), keep, None);
        let (od, ud) = (orig.data().clone(), ours.data().clone());

        let rating_differs = od.mayor_rating != ud.mayor_rating;
        let best_differs = (od.max_rating, od.max_location, od.min_rating, od.min_location)
            != (ud.max_rating, ud.max_location, ud.min_rating, ud.min_location);
        let same_spots = od.hotspots == ud.hotspots;
        let work_n: u64 = t.work.iter().flatten().map(|w| w.0).sum();
        let total = mismatches(&base_d)
            + mismatches(&aura_d)
            + mismatches(&park_d)
            + mismatches(&landmark_d)
            + mismatches(&transient_d)
            + work_n
            + u64::from(rating_differs)
            + u64::from(best_differs)
            + u64::from(!same_spots);
        let tracts = base_d.as_ref().map_or(0, |d| d.tracts);
        let masked = aura_d.as_ref().map_or(0, |d| d.masked);
        let st = &mut self.stats;
        st.compared = st.compared.wrapping_add(1);
        if total == 0 && t.desync.is_empty() {
            st.clean = st.clean.wrapping_add(1);
        }
        if !t.desync.is_empty() {
            st.desynced = st.desynced.wrapping_add(1);
        }
        if rating_differs {
            st.rating_mismatches = st.rating_mismatches.wrapping_add(1);
        }
        st.tracts = st.tracts.wrapping_add(tracts);
        st.base_mismatches = st.base_mismatches.wrapping_add(mismatches(&base_d));
        st.aura_mismatches = st.aura_mismatches.wrapping_add(mismatches(&aura_d));
        st.aura_transient = st.aura_transient.wrapping_add(masked);

        let work: Vec<String> = t
            .work
            .iter()
            .map(|w| w.map_or("-".to_string(), |(m, _)| m.to_string()))
            .collect();
        let mut line = format!(
            "{head} {followed}: tracts={tracts}; {}; {}{}; {}; {}; {} (copied before states 15 and 17: {} times); \
             work after states 5-13: {}; mayor rating orig/ours={}/{}; max orig/ours={}@({},{})/{}@({},{}); min \
             orig/ours={}@({},{})/{}@({},{}); hotspots orig/ours={}/{}{}; tax baseline: {}",
            show("base", &base_d),
            show("aura", &aura_d),
            if masked > 0 {
                format!(" ({masked} where the transient differs)")
            } else {
                String::new()
            },
            show("park map", &park_d),
            show("landmark map", &landmark_d),
            show("transient", &transient_d),
            t.transient_copies,
            work.join(","),
            od.mayor_rating,
            ud.mayor_rating,
            od.max_rating,
            od.max_location.0,
            od.max_location.1,
            ud.max_rating,
            ud.max_location.0,
            ud.max_location.1,
            od.min_rating,
            od.min_location.0,
            od.min_location.1,
            ud.min_rating,
            ud.min_location.0,
            ud.min_location.1,
            od.hotspots.len(),
            ud.hotspots.len(),
            if same_spots { " (same)" } else { " (differ)" },
            match self.tax {
                TaxBaseline::NotYet => "not taken (no tax-change curve)".to_string(),
                TaxBaseline::AtLoad => "the first cycle after the load (as the original's)".to_string(),
                TaxBaseline::Later(k) => format!(
                    "taken in cycle {k}, after the original's (the first cycle after the load): tax-change effects \
                     can differ"
                ),
            },
        );
        if t.last == 4 {
            line += "; ended in state 4 (no lots: no rating, no end message, §6.4)";
        }
        let mut firsts: Vec<String> = [
            first_cells("base", &base_d),
            first_cells("aura", &aura_d),
            first_cells("park map", &park_d),
            first_cells("landmark map", &landmark_d),
            first_cells("transient", &transient_d),
        ]
        .into_iter()
        .flatten()
        .collect();
        if let Some((state, first)) = &t.work_first {
            firsts.push(format!("work after state {state} {}", cells(first)));
        }
        if !firsts.is_empty() {
            line += &format!("; first differing (x,z) orig/ours: {}", firsts.join("; "));
        }
        if !t.desync.is_empty() {
            line += &format!("; desync: {}", t.desync.join(", "));
        }
        line += "; riot roll not compared (wall-clock RNG)";
        let missing = self.host.missing.borrow();
        if missing.is_empty() {
            line += "; inputs: all read";
        } else {
            line += &format!("; inputs not available (neutral): {}", missing.join(", "));
        }
        lines.push(line);
        total.min(i32::MAX as u64) as i32
    }
}

fn values_of<T: GridElem>(g: &Rc<dyn TractGrid<T>>) -> Vec<T> {
    let geom = g.geometry();
    let (nx, nz) = (geom.count_x.clamp(0, MAX_SIDE), geom.count_z.clamp(0, MAX_SIDE));
    let mut values = Vec::with_capacity((nx * nz) as usize);
    for x in 0..nx {
        for z in 0..nz {
            values.push(g.get(x, z));
        }
    }
    values
}

fn with_shadow<R>(s: *mut fc4_aura_shadow, fallback: R, f: impl FnOnce(&mut Shadow) -> R) -> R {
    // SAFETY: documented precondition: NULL or a live fc4_aura_shadow*.
    match unsafe { (s as *mut Shadow).as_mut() } {
        Some(sh) => guard(fallback, || f(sh)),
        None => fallback,
    }
}

unsafe fn emit(lines: &[String], sink: fc4_bytes_sink, ctx: *mut c_void) {
    if let Some(f) = sink {
        for l in lines {
            f(ctx, l.as_ptr(), l.len());
        }
    }
}

/// Creates a shadow over a read-only host table: only its read callbacks
/// (city, simulator date, tuning, lots, land value, occupant reads,
/// funding, tax rates, ordinances, input grids) and the grid table's
/// `geometry` / `get` are called; the write callbacks may be NULL. `cells_x`
/// / `cells_z`: the city's cell counts (our grids' geometry). `max_cells`:
/// the differing tracts listed per grid and line (0 → 4, at most 64). NULL
/// for a short table or a city outside 1..=4096 cells.
#[no_mangle]
pub unsafe extern "C" fn fc4_aura_shadow_create(
    host: *const fc4_aura_host,
    cells_x: i32,
    cells_z: i32,
    max_cells: u32,
) -> *mut fc4_aura_shadow {
    guard(std::ptr::null_mut(), || match host.as_ref() {
        Some(h)
            if (h.struct_size as usize) >= std::mem::size_of::<fc4_aura_host>()
                && h.grids.complete()
                && (1..=MAX_SIDE).contains(&cells_x)
                && (1..=MAX_SIDE).contains(&cells_z) =>
        {
            let c = CHost(*h);
            let (cell_width_x, cell_width_z) = c.cell_width();
            let host = Rc::new(ShadowHost {
                c,
                dims: CityDims {
                    cells_x,
                    cells_z,
                    cell_width_x,
                    cell_width_z,
                },
                missing: RefCell::new(Vec::new()),
                swallowed: Cell::new(0),
            });
            let max_cells = match max_cells {
                0 => 4,
                n => n.min(64) as usize,
            };
            Box::into_raw(Box::new(Shadow {
                host,
                max_cells,
                ours: None,
                no_ours: "no baseline at city load",
                previous_tax_rate: [-1.0; 12],
                tax: TaxBaseline::NotYet,
                first_after_load: false,
                track: Track::default(),
                stats: fc4_aura_shadow_stats {
                    struct_size: std::mem::size_of::<fc4_aura_shadow_stats>() as u32,
                    last_mismatches: -1,
                    ..Default::default()
                },
            })) as *mut fc4_aura_shadow
        }
        _ => std::ptr::null_mut(),
    })
}

/// Destroys a shadow (NULL is ignored). It holds no game reference.
#[no_mangle]
pub unsafe extern "C" fn fc4_aura_shadow_destroy(s: *mut fc4_aura_shadow) {
    if s.is_null() {
        return;
    }
    guard((), || drop(Box::from_raw(s as *mut Shadow)));
}

/// City load (PostCityInit): ours starts from the original's record (Read,
/// then Init: state 1, as the original's after a load), with a fresh tax
/// baseline. One line to `sink`. True when ours was created.
#[no_mangle]
pub unsafe extern "C" fn fc4_aura_shadow_baseline(
    s: *mut fc4_aura_shadow,
    capture: *const fc4_aura_capture,
    sink: fc4_bytes_sink,
    ctx: *mut c_void,
) -> bool {
    let c = capture_of(capture);
    let mut lines = Vec::new();
    let r = with_shadow(s, false, |sh| sh.baseline(c, &mut lines));
    emit(&lines, sink, ctx);
    r
}

/// Before a delivery to the original's agent that finds it in `state`: for
/// states 15 and 17, which read the transient grid (§5.3; 17 also decays
/// it), the original's transient grid (`GetTransientAuraGrid`, borrowed) is
/// copied into ours, so our step reads the same input. True when copied
/// (ours waits in the same state and the grids have the same tract counts).
#[no_mangle]
pub unsafe extern "C" fn fc4_aura_shadow_before_step(s: *mut fc4_aura_shadow, state: i32, transient_grid: u64) -> bool {
    with_shadow(s, false, |sh| sh.before_step(state, transient_grid))
}

/// The original's state moved from `in.from` to `in.to` in one delivery to
/// its agent (§14.3): our steps `from` .. `to − 1` run now (to 0: to the end
/// of our cycle), reading their inputs live, and our work grid is compared
/// with `in.work_grid` after a step 5..13. Returns our pipeline calls, -1
/// when nothing ran (NULL, no simulator of ours yet, a state outside 0..=17
/// or no move).
#[no_mangle]
pub unsafe extern "C" fn fc4_aura_shadow_step(s: *mut fc4_aura_shadow, step: *const fc4_aura_shadow_step_in) -> i32 {
    let Some(st) = step.as_ref() else { return -1 };
    if (st.struct_size as usize) < std::mem::size_of::<fc4_aura_shadow_step_in>() {
        return -1;
    }
    let st = *st;
    with_shadow(s, -1, |sh| sh.step(&st))
}

/// The original received occupant inserted (`inserted`) or removed for
/// `occupant` (a host occupant token): ours receives the same (§5.2: the
/// preview marker test, then §7's cones). False when there is no simulator
/// of ours yet.
#[no_mangle]
pub unsafe extern "C" fn fc4_aura_shadow_occupant(s: *mut fc4_aura_shadow, occupant: u64, inserted: bool) -> bool {
    with_shadow(s, false, |sh| sh.occupant(occupant, inserted))
}

/// The original's state returned to 0 (`capture`: its record now; `julian`:
/// the current date, for the line): the cycle followed is compared with the
/// record, one line to `sink`, then ours restarts from the record (state
/// 0). Returns the differences (base + aura + park map + landmark map +
/// transient tracts, work cells, and one each for the mayor rating, best /
/// worst and hotspots), -1 when nothing was compared.
#[no_mangle]
pub unsafe extern "C" fn fc4_aura_shadow_cycle(
    s: *mut fc4_aura_shadow,
    capture: *const fc4_aura_capture,
    julian: i32,
    sink: fc4_bytes_sink,
    ctx: *mut c_void,
) -> i32 {
    let c = capture_of(capture);
    let mut lines = Vec::new();
    let r = with_shadow(s, -1, |sh| sh.cycle(c, julian, &mut lines));
    emit(&lines, sink, ctx);
    r
}

/// Totals since creation.
#[no_mangle]
pub unsafe extern "C" fn fc4_aura_shadow_get_stats(s: *mut fc4_aura_shadow, out: *mut fc4_aura_shadow_stats) -> bool {
    let Some(o) = out.as_mut() else { return false };
    with_shadow(s, false, |sh| {
        *o = sh.stats;
        true
    })
}
