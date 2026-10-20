// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! `cSC4GrowthDeveloper`: lifecycle (§1), state (§2), the `DoIdle` state
//! machine and messages (§3), desirability (§4), lists and population (§5),
//! capacity bookkeeping (§6), the shared generator (§7), the save record
//! (§9) and the interface slots (§10).
//!
//! No `RefCell` borrow is held across a host callback.

use std::any::Any;
use std::cell::{Cell, Ref, RefCell, RefMut};
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::curve::ResponseCurve;
use crate::development::{building_fits_in_zone, ConstructionProfile, GrowthItem, GrowthProfile};
use crate::grid::{
    add_radius_effect, process_two_grids_batched, GridElem, GridProvider, GridSnapshot, ScanIterator, TractGrid,
    IID_SIM_GRID,
};
use crate::pacing::{self, Pacing};
use crate::property::PropertyHolder;
use crate::rng::CLOCK_SEED;
use crate::stream::{InStream, OutStream, RefValue, SaveObject};
use crate::variant::{Elem, Variant};
use crate::x87::{ftol32, r24};

use super::algo::{
    add_curve, add_to_capacity, assign_stages, city_size, commute_add, commute_minimum, decline_key, decline_key_seed,
    low_byte, phantom_term, population_of, round_capacity, split_capacity, CitySizeInputs,
};
use super::{
    msg, prop, Curve, DevGrid, DeveloperTuning, GridHandle, GrowthDeveloperHost, GrowthMessage, GrowthStatics, Service,
    SourceMap, AGENT_FLAGS, AGENT_LEVELS, AGENT_NAME, CLSID_GROWTH_DEVELOPER, SOURCE_DEMAND_INDEX,
};
use crate::stl::stl_sort;

/// The save record version (§9).
pub const RECORD_VERSION: u16 = 1;

/// The largest tract shift the cell loops use ([choice]: the tuned tract
/// size is a `u8`, so the developer's own grids never exceed shift 7; a
/// loaded grid with a larger shift cannot make a loop run for hours).
const MAX_LOOP_SHIFT: u32 = 7;

/// The developer's state (§2).
#[derive(Clone, Debug)]
pub struct GrowthData {
    pub initialized: bool,
    pub ref_count: u32,
    /// Set by the final `Release`.
    pub destroyed: bool,
    pub tuning: DeveloperTuning,
    /// The `Init` holder (the developer exemplar).
    pub holder: Option<Rc<PropertyHolder>>,
    /// Services held (§1.2 step 2), in acquisition order.
    pub services: Vec<Service>,
    pub message_server: bool,
    /// 0 idle, 1..20 (§3.2).
    pub update_state: u32,
    pub start_day: u32,
    /// The tract scanning iterator over W (§5.2); `None` before `Init`.
    pub iterator: Option<ScanIterator>,
    /// The private copy of the BDS special-building list and its cursor.
    pub specials: Vec<u64>,
    pub special_cursor: usize,
    pub growth: Vec<GrowthItem>,
    pub decline: Vec<GrowthItem>,
    pub last_phantom_sum: i32,
    pub total_capacity: i32,
    /// The source demand node (`0x87CD6301`, held with one reference).
    pub source_demand: Option<u64>,
    /// The salt of the last industrial state 20 ([choice] diagnostics).
    pub last_salt: Option<u32>,
    /// The city size of the last state 20 ([choice] diagnostics).
    pub last_city_size: f32,
}

impl GrowthData {
    fn new() -> GrowthData {
        GrowthData {
            initialized: false,
            ref_count: 0,
            destroyed: false,
            tuning: DeveloperTuning::constructed(),
            holder: None,
            services: Vec::new(),
            message_server: false,
            update_state: 0,
            start_day: 0,
            iterator: None,
            specials: Vec::new(),
            special_cursor: 0,
            growth: Vec::new(),
            decline: Vec::new(),
            last_phantom_sum: 0,
            // DG1 (growth-developer §2, §9): the original's constructor never
            // initialises total capacity (+0x108); [choice] 0 for a new city.
            total_capacity: 0,
            source_demand: None,
            last_salt: None,
            last_city_size: 0.0,
        }
    }
}

/// The six grids (§1.2 step 4).
#[derive(Clone, Default)]
struct Grids {
    zoned: Option<Rc<dyn TractGrid<u8>>>,
    working: Option<Rc<dyn TractGrid<i16>>>,
    desirability: Option<Rc<dyn TractGrid<i16>>>,
    population: Option<Rc<dyn TractGrid<i16>>>,
    capacity: Option<Rc<dyn TractGrid<u16>>>,
    max_capacity: Option<Rc<dyn TractGrid<u16>>>,
}

impl Grids {
    /// Calls `f` for every present grid in creation order.
    fn for_each(&self, mut f: impl FnMut(DevGrid, GridHandle<'_>)) {
        for which in DevGrid::ALL {
            match which {
                DevGrid::ZonedCount => self.zoned.as_ref().map(|g| f(which, GridHandle::U8(g))),
                DevGrid::Working => self.working.as_ref().map(|g| f(which, GridHandle::I16(g))),
                DevGrid::Desirability => self.desirability.as_ref().map(|g| f(which, GridHandle::I16(g))),
                DevGrid::Population => self.population.as_ref().map(|g| f(which, GridHandle::I16(g))),
                DevGrid::Capacity => self.capacity.as_ref().map(|g| f(which, GridHandle::U16(g))),
                DevGrid::MaxCapacity => self.max_capacity.as_ref().map(|g| f(which, GridHandle::U16(g))),
            };
        }
    }
}

fn save_ref<T: GridElem>(g: &Option<Rc<dyn TractGrid<T>>>) -> RefValue {
    g.as_ref().map_or(RefValue::Null, |g| g.save_ref())
}

/// The counts of a grid clamped to the allocatable range.
fn counts<T: GridElem>(g: &dyn TractGrid<T>) -> (i32, i32) {
    let geo = g.geometry();
    (
        geo.count_x.clamp(0, crate::grid::MAX_TRACTS),
        geo.count_z.clamp(0, crate::grid::MAX_TRACTS),
    )
}

fn loop_shift<T: GridElem>(g: &dyn TractGrid<T>) -> u32 {
    g.geometry().safe_shift().min(MAX_LOOP_SHIFT)
}

/// The numeric elements of a variant (`u32` / `f32` occupant properties).
fn elems(v: &Variant) -> Vec<Elem> {
    v.items()
}

/// One growth developer object.
pub struct GrowthDeveloper {
    host: Rc<dyn GrowthDeveloperHost>,
    statics: Rc<GrowthStatics>,
    token: Cell<u64>,
    d: RefCell<GrowthData>,
    g: RefCell<Grids>,
    /// Decline-key temporaries whose coordinate seed read the clock since
    /// construction: the `n` of their `K` (foundations §6.3, RN2).
    decline_clock_reads: Cell<u32>,
    /// simulator.md §5.7: captured from [`pacing::process`] at construction.
    pacing: Cell<Pacing>,
}

impl GrowthDeveloper {
    /// A constructed (not initialised) developer (§1.1). `statics` is the
    /// process-wide state every developer shares; `token` is the host's
    /// handle for this developer (the `GrowthProfile`'s developer).
    pub fn new(host: Rc<dyn GrowthDeveloperHost>, statics: Rc<GrowthStatics>, token: u64) -> Rc<GrowthDeveloper> {
        Rc::new(GrowthDeveloper {
            host,
            statics,
            token: Cell::new(token),
            d: RefCell::new(GrowthData::new()),
            g: RefCell::new(Grids::default()),
            decline_clock_reads: Cell::new(0),
            pacing: Cell::new(pacing::process()),
        })
    }

    /// The pacing this developer runs (simulator.md §5.7).
    pub fn pacing(&self) -> Pacing {
        self.pacing.get()
    }

    /// Shadows and tests: [`Pacing::Original`] keeps the clock-bound steps.
    pub fn set_pacing(&self, p: Pacing) {
        self.pacing.set(p);
    }

    /// simulator.md §5.7 [choice] (deterministic pacing): the day's settle
    /// delivery runs the map update to completion (state 0), or until it
    /// waits (state 1 before `Init`). Returns whether it is idle.
    pub fn settle_cycle(&self) -> bool {
        pacing::run_to_idle(
            || {
                let before = self.d.borrow().update_state;
                self.do_idle(5);
                // States 15 and 18 progress through a cursor.
                before == 15 || before == 18 || self.d.borrow().update_state != before
            },
            || self.d.borrow().update_state == 0,
        )
    }

    /// `CreateDeveloper`'s construction (building-development.md §1.5, §1.1
    /// here): the constructor, then `AddRef` for the BDS (count 1).
    pub fn create(host: Rc<dyn GrowthDeveloperHost>, statics: Rc<GrowthStatics>, token: u64) -> Rc<GrowthDeveloper> {
        let d = GrowthDeveloper::new(host, statics, token);
        d.add_ref();
        d
    }

    /// [`GrowthDeveloper::create`], then `Init(exemplar)` with the developer
    /// exemplar (§1.2); returns the developer and `Init`'s result.
    pub fn create_from_exemplar(
        host: Rc<dyn GrowthDeveloperHost>,
        statics: Rc<GrowthStatics>,
        token: u64,
        exemplar: Rc<PropertyHolder>,
    ) -> (Rc<GrowthDeveloper>, bool) {
        let d = GrowthDeveloper::create(host, statics, token);
        let ok = d.init(Some(exemplar));
        (d, ok)
    }

    /// How a save refers to this developer (foundations §2.7; written by
    /// the class store with class [`CLSID_GROWTH_DEVELOPER`]).
    pub fn save_ref(self: &Rc<Self>) -> RefValue {
        RefValue::Object(self.clone() as Rc<dyn SaveObject>)
    }

    /// The developer a save reference points at (`None` for null or another
    /// class).
    pub fn from_save_ref(r: &RefValue) -> Option<Rc<GrowthDeveloper>> {
        r.object::<GrowthDeveloper>()
    }

    pub fn token(&self) -> u64 {
        self.token.get()
    }

    pub fn set_token(&self, token: u64) {
        self.token.set(token);
    }

    pub fn statics(&self) -> &Rc<GrowthStatics> {
        &self.statics
    }

    pub fn data(&self) -> Ref<'_, GrowthData> {
        self.d.borrow()
    }

    /// Mutable state, for hosts and tests that set up a scenario.
    pub fn data_mut(&self) -> RefMut<'_, GrowthData> {
        self.d.borrow_mut()
    }

    fn grids(&self) -> Grids {
        self.g.borrow().clone()
    }

    /// The working desirability grid W (internal; not published).
    pub fn working_map(&self) -> Option<Rc<dyn TractGrid<i16>>> {
        self.g.borrow().working.clone()
    }

    /// The zoned-cell count grid (internal).
    pub fn zoned_count_map(&self) -> Option<Rc<dyn TractGrid<u8>>> {
        self.g.borrow().zoned.clone()
    }

    // ---- reference count (§1) ------------------------------------------------------------------------

    /// `AddRef`: the new count.
    pub fn add_ref(&self) -> u32 {
        let mut d = self.d.borrow_mut();
        d.ref_count = d.ref_count.wrapping_add(1);
        d.ref_count
    }

    /// `Release`: count > 1 → decrement and return it; otherwise `Shutdown`,
    /// the destructor (releases the special-building copy) and 0.
    pub fn release(&self) -> u32 {
        let count = self.d.borrow().ref_count;
        if count > 1 {
            let mut d = self.d.borrow_mut();
            d.ref_count = count - 1;
            return d.ref_count;
        }
        self.shutdown();
        let specials = {
            let mut d = self.d.borrow_mut();
            d.ref_count = 0;
            d.destroyed = true;
            d.special_cursor = 0;
            d.growth.clear();
            d.decline.clear();
            std::mem::take(&mut d.specials)
        };
        for o in specials {
            self.host.release(o);
        }
        0
    }

    // ---- lifecycle (§1.2, §1.3) ----------------------------------------------------------------------

    /// `Init(holder)` (§1.2).
    pub fn init(&self, holder: Option<Rc<PropertyHolder>>) -> bool {
        if self.d.borrow().initialized {
            return true;
        }
        // Step 2: the services still missing, in order; those taken are kept.
        for s in Service::ACQUIRE {
            if self.d.borrow().services.contains(&s) {
                continue;
            }
            if !self.host.acquire_service(s) {
                return false;
            }
            self.d.borrow_mut().services.push(s);
        }
        // Step 3 ([choice]: a null holder fails; DG24: the original AddRefs
        // the holder without a null test, so it would crash).
        let Some(holder) = holder else { return false };
        {
            let mut d = self.d.borrow_mut();
            if let Some(t) = holder
                .get(prop::DEVELOPER_TYPE)
                .and_then(|v| v.items().first().map(Elem::as_i64))
            {
                d.tuning.developer_type = t as u32;
            }
            d.holder = Some(holder.clone());
        }
        // Step 4: the grids still null.
        self.create_missing_grids();
        // Steps 5 and 6.
        {
            let mut d = self.d.borrow_mut();
            d.update_state = 0;
            if d.iterator.is_none() {
                d.iterator = Some(ScanIterator::default());
            }
        }
        // Step 7.
        self.read_tuning(&holder);
        // Step 8.
        if !self.d.borrow().message_server {
            if !self.host.acquire_message_server() {
                return false;
            }
            self.d.borrow_mut().message_server = true;
            self.host.subscribe(&msg::SUBSCRIPTIONS);
        }
        // Step 9.
        if !self.host.simulator_present() {
            return false;
        }
        for level in AGENT_LEVELS {
            self.host.add_agent(level, AGENT_NAME, AGENT_FLAGS);
        }
        self.d.borrow_mut().initialized = true;
        true
    }

    fn create_missing_grids(&self) {
        let (dev, size) = {
            let d = self.d.borrow();
            (d.tuning.developer_type, i32::from(d.tuning.tract_size))
        };
        let inst = |w: DevGrid| dev.wrapping_add(w.instance_offset());
        let host = &*self.host;
        for which in DevGrid::ALL {
            let missing = {
                let g = self.g.borrow();
                match which {
                    DevGrid::ZonedCount => g.zoned.is_none(),
                    DevGrid::Working => g.working.is_none(),
                    DevGrid::Desirability => g.desirability.is_none(),
                    DevGrid::Population => g.population.is_none(),
                    DevGrid::Capacity => g.capacity.is_none(),
                    DevGrid::MaxCapacity => g.max_capacity.is_none(),
                }
            };
            if !missing {
                continue;
            }
            match which {
                DevGrid::ZonedCount => {
                    let n = GridProvider::<u8>::create_grid(host, inst(which), size);
                    self.g.borrow_mut().zoned = n;
                }
                DevGrid::Working | DevGrid::Desirability | DevGrid::Population => {
                    let n = GridProvider::<i16>::create_grid(host, inst(which), size);
                    let mut g = self.g.borrow_mut();
                    match which {
                        DevGrid::Working => g.working = n,
                        DevGrid::Desirability => g.desirability = n,
                        _ => g.population = n,
                    }
                }
                DevGrid::Capacity | DevGrid::MaxCapacity => {
                    let n = GridProvider::<u16>::create_grid(host, inst(which), size);
                    let mut g = self.g.borrow_mut();
                    if which == DevGrid::Capacity {
                        g.capacity = n;
                    } else {
                        g.max_capacity = n;
                    }
                }
            }
        }
    }

    /// `ReadSimulatorTunableValues` (§4.8) with its host effects: the
    /// source demand node, the tract size of the six grids, and ([choice])
    /// always the iterator rectangle.
    pub fn read_tuning(&self, holder: &PropertyHolder) {
        let effects = self.d.borrow_mut().tuning.read(holder);
        if let Some(id) = effects.source_demand {
            let old = self.d.borrow_mut().source_demand.take();
            if let Some(o) = old {
                self.host.release(o);
            }
            let node = self.host.get_demand(id, SOURCE_DEMAND_INDEX);
            if let Some(n) = node {
                self.host.add_ref(n);
            }
            self.d.borrow_mut().source_demand = node;
        }
        if let Some(s) = effects.new_tract_size {
            let grids = self.grids();
            grids.for_each(|which, h| self.host.grid_set_tract_size(which, h, i32::from(s)));
        }
        self.set_iterator_rect();
    }

    fn set_iterator_rect(&self) {
        let (cx, cz) = self.working_map().map_or((0, 0), |w| counts(&*w));
        let mut d = self.d.borrow_mut();
        if let Some(it) = d.iterator.as_mut() {
            let r = ScanIterator::for_counts(cx, cz);
            it.x0 = r.x0;
            it.z0 = r.z0;
            it.x1 = r.x1;
            it.z1 = r.z1;
        }
    }

    /// `Shutdown` (§1.3); always true.
    pub fn shutdown(&self) -> bool {
        if !self.d.borrow().initialized {
            return true;
        }
        self.d.borrow_mut().initialized = false;
        self.host.remove_agent();
        let had_server = std::mem::take(&mut self.d.borrow_mut().message_server);
        if had_server {
            self.host.unsubscribe(&msg::SUBSCRIPTIONS);
            self.host.release_message_server();
        }
        self.d.borrow_mut().iterator = None;
        let grids = std::mem::take(&mut *self.g.borrow_mut());
        grids.for_each(|which, h| self.host.grid_shutdown(which, h));
        let (node, held) = {
            let mut d = self.d.borrow_mut();
            d.holder = None;
            (d.source_demand.take(), std::mem::take(&mut d.services))
        };
        if let Some(n) = node {
            self.host.release(n);
        }
        for s in Service::RELEASE {
            if held.contains(&s) {
                self.host.release_service(s);
            }
        }
        self.d.borrow_mut().tuning.curves = Default::default();
        true
    }

    pub fn is_initialized(&self) -> bool {
        self.d.borrow().initialized
    }

    // ---- messages (§3.1) -----------------------------------------------------------------------------

    /// `DoMessage` (message-target slot 3).
    pub fn handle_message(&self, m: &GrowthMessage) -> bool {
        match m.msg_type {
            msg::SIM_IDLE => {
                if self.pacing.get().is_deterministic() {
                    self.settle_cycle();
                } else {
                    self.do_idle(m.data3);
                }
                true
            }
            msg::SIM_NEW_DAY => {
                self.do_idle(5);
                self.new_day_finished()
            }
            msg::ZONING_CHANGE => {
                self.update_max_capacity(m.rect);
                true
            }
            msg::STATE_CHANGED => {
                self.state_changed(m);
                true
            }
            _ => true,
        }
    }

    /// The NewDay "not finished" rule (§3.1): false when `state ≠ 0` and
    /// `state < today − startDay` (unsigned).
    pub fn new_day_finished(&self) -> bool {
        let today = self.host.sim_date_number();
        let d = self.d.borrow();
        let elapsed = today.wrapping_sub(d.start_day);
        !(d.update_state != 0 && d.update_state < elapsed)
    }

    /// `StartMapUpdate` (slot 27): state 0 → 1.
    pub fn start_map_update(&self) {
        let mut d = self.d.borrow_mut();
        if d.update_state == 0 {
            d.update_state = 1;
        }
    }

    /// `IsMapUpdateFinished` (slot 28).
    pub fn is_map_update_finished(&self) -> bool {
        self.d.borrow().update_state == 0
    }

    /// `GrowthAndDeclineListsAreValid` (slot 10): `state < 17` (signed).
    pub fn lists_valid(&self) -> bool {
        (self.d.borrow().update_state as i32) < 17
    }

    fn set_state(&self, s: u32) {
        self.d.borrow_mut().update_state = s;
    }

    // ---- the state machine (§3.2) --------------------------------------------------------------------

    /// `DoIdle(ms)` (§3.2): one state per call (15 and 18 may stay).
    pub fn do_idle(&self, ms: u32) {
        let state = self.d.borrow().update_state;
        match state {
            1 => {
                if self.d.borrow().initialized {
                    let day = self.host.sim_date_number();
                    let mut d = self.d.borrow_mut();
                    d.start_day = day;
                    d.update_state = 2;
                }
            }
            2 => {
                let baseline = self.d.borrow().tuning.baseline as i16;
                if let Some(w) = self.working_map() {
                    w.fill(baseline);
                }
                self.set_state(3);
            }
            3 => {
                let map = if self.d.borrow().tuning.use_intrinsic_land_value {
                    SourceMap::IntrinsicLandValue
                } else {
                    SourceMap::TotalLandValue
                };
                self.layer_u8(Curve::LandValue, map);
                self.set_state(4);
            }
            4 => {
                self.layer_f32(Curve::Slope, SourceMap::Slope);
                self.set_state(5);
            }
            5 => {
                for (c, w) in [
                    (Curve::ProximityR1, 1),
                    (Curve::ProximityR2, 2),
                    (Curve::ProximityR3, 3),
                ] {
                    self.layer_u8(c, SourceMap::ResidentialProximity(w));
                }
                self.set_state(6);
            }
            6 => {
                self.layer_u8(Curve::ProximityCO2, SourceMap::CommercialProximity(0x3320));
                self.layer_u8(Curve::ProximityCO3, SourceMap::CommercialProximity(0x3330));
                self.set_state(7);
            }
            7 => {
                self.layer_i16(Curve::Park, SourceMap::Park);
                self.set_state(8);
            }
            8 => {
                for (i, c) in [Curve::Air, Curve::Water, Curve::Garbage, Curve::Radiation]
                    .into_iter()
                    .enumerate()
                {
                    self.layer_i16(c, SourceMap::Pollution(i as u32));
                }
                self.set_state(9);
            }
            9 => {
                self.layer_f32(Curve::School, SourceMap::SchoolGrade);
                self.set_state(10);
            }
            10 => {
                self.layer_f32(Curve::Hospital, SourceMap::HospitalGrade);
                self.set_state(11);
            }
            11 => {
                self.layer_u8(Curve::Crime, SourceMap::Crime);
                self.set_state(12);
            }
            12 => {
                self.layer_commute();
                self.set_state(13);
            }
            13 => {
                self.layer_u8(Curve::Traffic, SourceMap::CommercialTraffic);
                self.set_state(14);
            }
            14 => {
                self.layer_i16(Curve::Landmark, SourceMap::Landmark);
                self.copy_special_buildings();
                self.set_state(15);
            }
            15 => self.special_buildings_step(),
            16 => {
                self.layer_i8(Curve::Transient, SourceMap::TransientAura);
                self.set_state(17);
            }
            17 => self.publish(),
            18 => self.population_step(ms),
            19 => {
                self.phantom_demand();
                self.set_state(20);
            }
            20 => {
                self.finish_update();
                self.set_state(0);
            }
            _ => {}
        }
    }

    fn curve(&self, c: Curve) -> Option<ResponseCurve> {
        self.d.borrow().tuning.curve(c).cloned()
    }

    fn layer_u8(&self, c: Curve, map: SourceMap) {
        let Some(curve) = self.curve(c) else { return };
        let Some(w) = self.working_map() else { return };
        let Some(src) = self.host.source_u8(map) else { return };
        process_two_grids_batched(&*w, &*src, |_, _, v, b| *v = add_curve(*v, curve.eval(f32::from(b))));
    }

    fn layer_i8(&self, c: Curve, map: SourceMap) {
        let Some(curve) = self.curve(c) else { return };
        let Some(w) = self.working_map() else { return };
        let Some(src) = self.host.source_i8(map) else { return };
        process_two_grids_batched(&*w, &*src, |_, _, v, b| *v = add_curve(*v, curve.eval(f32::from(b))));
    }

    fn layer_i16(&self, c: Curve, map: SourceMap) {
        let Some(curve) = self.curve(c) else { return };
        let Some(w) = self.working_map() else { return };
        let Some(src) = self.host.source_i16(map) else { return };
        process_two_grids_batched(&*w, &*src, |_, _, v, b| *v = add_curve(*v, curve.eval(f32::from(b))));
    }

    /// A float source (§4.3): equal sizes pass the value unchanged; W
    /// coarser accumulates `acc := ftol(r24(float(acc) + v))` and takes the
    /// floor mean; W finer truncates the value toward zero first.
    fn layer_f32(&self, c: Curve, map: SourceMap) {
        let Some(curve) = self.curve(c) else { return };
        let Some(w) = self.working_map() else { return };
        let Some(src) = self.host.source_f32(map) else { return };
        let (ga, gb) = (w.geometry(), src.geometry());
        let (sa, sb) = (ga.safe_shift(), gb.safe_shift());
        let (ax, az) = counts(&*w);
        let (bx, bz) = counts(&*src);
        // Each grid read in one host call when it offers it (devperf); the
        // same values as tract by tract.
        let src = GridSnapshot::new(&*src);
        let mut w = GridSnapshot::batched(&*w);
        let mut add = |x: i32, z: i32, input: f32| {
            let old = w.get(x, z);
            let new = add_curve(old, curve.eval(input));
            if new != old {
                w.set(x, z, new);
            }
        };
        if sa == sb {
            for x in 0..ax {
                for z in 0..az {
                    add(x, z, src.get(x, z));
                }
            }
        } else if sa > sb {
            let d = sa - sb;
            let side = 1i64 << d;
            for x in 0..ax {
                for z in 0..az {
                    let mut acc = 0i32;
                    let (x0, z0) = (i64::from(x) << d, i64::from(z) << d);
                    for i in x0..(x0 + side).min(i64::from(bx)) {
                        for j in z0..(z0 + side).min(i64::from(bz)) {
                            let v = src.get(i as i32, j as i32);
                            acc = ftol32(r24(f64::from(acc) + f64::from(v)));
                        }
                    }
                    add(x, z, ((acc >> d) >> d) as f32);
                }
            }
        } else {
            let d = sb - sa;
            let side = 1i64 << d;
            for x in 0..bx {
                for z in 0..bz {
                    let v = ftol32(f64::from(src.get(x, z))) as f32;
                    let (x0, z0) = (i64::from(x) << d, i64::from(z) << d);
                    for i in x0..(x0 + side).min(i64::from(ax)) {
                        for j in z0..(z0 + side).min(i64::from(az)) {
                            add(i as i32, j as i32, v);
                        }
                    }
                }
            }
        }
    }

    /// State 12, commute (§4.3): the smallest non-zero trip length over the
    /// tract's cells (255 when none); the sum with the curve output is
    /// truncated.
    fn layer_commute(&self) {
        let Some(curve) = self.curve(Curve::Commute) else {
            return;
        };
        let Some(w) = self.working_map() else { return };
        let Some(trip) = self.host.source_u8(SourceMap::TripLength) else {
            return;
        };
        let s = i64::from(w.geometry().tract_size.clamp(0, 1 << MAX_LOOP_SHIFT));
        let (cx, cz) = counts(&*w);
        // Each grid read in one host call when it offers it, W's writes in
        // one more (devperf).
        let trip = GridSnapshot::new(&*trip);
        let mut w = GridSnapshot::batched(&*w);
        for x in 0..cx {
            for z in 0..cz {
                let cells = (0..s).flat_map(|i| (0..s).map(move |j| (i, j)));
                let m = commute_minimum(cells.map(|(i, j)| {
                    let px = (i64::from(x) * s + i) as i32;
                    let pz = (i64::from(z) * s + j) as i32;
                    trip.get_cell_value(px, pz)
                }));
                let old = w.get(x, z);
                w.set(x, z, commute_add(old, curve.eval(f32::from(m))));
            }
        }
    }

    /// The end of state 14 (§4.4): the private list becomes a copy of the
    /// BDS's special buildings (each `AddRef`ed, the old ones released).
    fn copy_special_buildings(&self) {
        let new = self.host.special_buildings();
        for &o in &new {
            self.host.add_ref(o);
        }
        let old = {
            let mut d = self.d.borrow_mut();
            d.special_cursor = 0;
            std::mem::replace(&mut d.specials, new)
        };
        for o in old {
            self.host.release(o);
        }
    }

    /// State 15 (§4.4): at most one effect per call.
    fn special_buildings_step(&self) {
        loop {
            let occupant = {
                let mut d = self.d.borrow_mut();
                match d.specials.get(d.special_cursor).copied() {
                    None => {
                        d.update_state = 16;
                        return;
                    }
                    Some(o) => {
                        d.special_cursor += 1;
                        o
                    }
                }
            };
            if self.apply_special_building(occupant) {
                let mut d = self.d.borrow_mut();
                if d.special_cursor >= d.specials.len() {
                    d.update_state = 16;
                }
                return;
            }
        }
    }

    /// One special building's effect (§4.4 steps 1–4); false when skipped.
    fn apply_special_building(&self, occupant: u64) -> bool {
        let dev = self.d.borrow().tuning.developer_type;
        let Some(types) = self.host.occupant_property(occupant, prop::SPECIAL_TYPES) else {
            return false;
        };
        let Some(values) = self.host.occupant_property(occupant, prop::SPECIAL_EFFECTS) else {
            return false;
        };
        let Some(i) = elems(&types).iter().position(|e| e.as_i64() as u32 == dev) else {
            return false;
        };
        let values: Vec<f32> = elems(&values)
            .iter()
            .map(|e| match *e {
                Elem::F32(x) => x,
                other => other.as_f64() as f32,
            })
            .collect();
        // [choice] A triple beyond the array skips the occupant (quirk 9).
        let (Some(&radius), Some(&centre), Some(&outer)) =
            (values.get(3 * i), values.get(3 * i + 1), values.get(3 * i + 2))
        else {
            return false;
        };
        let Some(w) = self.working_map() else { return false };
        let geo = w.geometry();
        let r = r24(f64::from(radius) / f64::from(geo.tract_size)) as f32;
        let pos = self.host.occupant_position(occupant);
        let (cx, cz) = self.host.position_to_cell(pos[0], pos[2]);
        let shift = geo.safe_shift();
        let (nx, nz) = counts(&*w);
        add_radius_effect(&*w, nx, nz, cx >> shift, cz >> shift, r, centre, outer);
        true
    }

    /// State 17 (§5.1): publish D and build the growth list.
    fn publish(&self) {
        let g = self.grids();
        let mut growth = std::mem::take(&mut self.d.borrow_mut().growth);
        growth.clear();
        if let Some(w) = &g.working {
            let (cx, cz) = counts(&**w);
            // W read in one host call when it offers it (devperf).
            let w = GridSnapshot::new(&**w);
            if let Some(dm) = &g.desirability {
                // D's writes in one host call when it offers it (devperf).
                let mut dm = GridSnapshot::batched(&**dm);
                for x in 0..cx {
                    for z in 0..cz {
                        dm.set(x, z, w.get(x, z));
                    }
                }
            }
            if let Some(zoned) = &g.zoned {
                let zoned = GridSnapshot::new(&**zoned);
                for x in 0..cx {
                    for z in 0..cz {
                        if zoned.get(x, z) > 0 {
                            growth.push(GrowthItem { x, z, max_stage: 1 });
                        }
                    }
                }
            }
        }
        let mut d = self.d.borrow_mut();
        d.growth = growth;
        d.update_state = 18;
        if let Some(it) = d.iterator.as_mut() {
            it.reset(0, 0);
        }
    }

    /// State 18 (§5.2): the population pass, time-sliced.
    fn population_step(&self, ms: u32) {
        let g = self.grids();
        let (Some(w), Some(cap), Some(pop)) = (g.working, g.capacity, g.population) else {
            self.set_state(19);
            return;
        };
        let Some(mut it) = self.d.borrow_mut().iterator.take() else {
            self.set_state(19);
            return;
        };
        let mut slice = self.statics.slice_step();
        let step = slice.step.clamp(1, 1024);
        let deterministic = self.pacing.get().is_deterministic();
        self.host.limit_timer_start(ms);
        let td = self.host.tract_developer();
        let me = self.token();
        let mut count = 0u32;
        // glue27: each grid read and write in one host call when the grid
        // offers it (the batching `layer_f32` already uses, devperf). This
        // walk is per cell of the desirability map and used to make three
        // crossings per cell; the UpdatePopulation call below is the one
        // crossing per cell that has to stay.
        let w = GridSnapshot::new(&*w);
        let cap = GridSnapshot::new(&*cap);
        let mut pop = GridSnapshot::batched(&*pop);
        let done = loop {
            count = count.wrapping_add(step);
            // Deterministic pacing (simulator.md §5.7): no clock.
            if count % 1024 == 0 && !deterministic && self.host.limit_timer_expired() {
                break false;
            }
            let Some((x, z)) = it.next_cell() else { break true };
            pop.set(x, z, population_of(w.get(x, z), cap.get(x, z)));
            if let Some(td) = td {
                let profile = GrowthProfile {
                    developer: me,
                    x,
                    z,
                    stage: 0,
                };
                self.host.update_population(td, &profile);
            }
        };
        slice.adapt(count >> 10);
        self.statics.set_slice_step(slice);
        let mut d = self.d.borrow_mut();
        d.iterator = Some(it);
        if done {
            d.update_state = 19;
        }
    }

    /// State 19 (§5.3): phantom demand.
    fn phantom_demand(&self) {
        let g = self.grids();
        let (flag, threshold, items, last, node) = {
            let d = self.d.borrow();
            (
                d.tuning.phantom_demand,
                d.tuning.growth_threshold,
                if d.tuning.phantom_demand {
                    d.growth.clone()
                } else {
                    Vec::new()
                },
                d.last_phantom_sum,
                d.source_demand,
            )
        };
        let mut sum = 0i32;
        if flag {
            for it in &items {
                let w = g.working.as_ref().map_or(0, |w| w.get(it.x, it.z));
                let p = g.population.as_ref().map_or(0, |p| p.get(it.x, it.z));
                if let Some(t) = phantom_term(threshold, w, p) {
                    sum = sum.wrapping_add(t);
                }
            }
        }
        let delta = sum.wrapping_sub(last);
        if delta != 0 {
            if let Some(n) = node {
                self.host.add_to_demand_value(n, delta as f32);
            }
            self.d.borrow_mut().last_phantom_sum = sum;
        }
    }

    /// State 20 (§5.4, §5.5): sort, decline list, industrial order, filter,
    /// city size and stages.
    fn finish_update(&self) {
        let w = self.working_map();
        let wv = |it: &GrowthItem| w.as_ref().map_or(0, |w| w.get(it.x, it.z));
        let (mut growth, mut decline, purpose, growth_t, decline_t) = {
            let mut d = self.d.borrow_mut();
            (
                std::mem::take(&mut d.growth),
                std::mem::take(&mut d.decline),
                d.tuning.purpose,
                d.tuning.growth_threshold,
                d.tuning.decline_threshold,
            )
        };
        stl_sort(&mut growth, &mut |a, b| wv(a) > wv(b));
        decline.clear();
        decline.extend(growth.iter().rev().filter(|it| i32::from(wv(it)) < decline_t).copied());
        if (5..=8).contains(&purpose) {
            let salt = self.statics.draw_salt();
            self.d.borrow_mut().last_salt = Some(salt);
            let key = |it: &GrowthItem| {
                // A key seed of 0xFFFFFFFF reads the clock (§7 quirk); ours is
                // derive(GZCLSID XOR n) of the session seed, n counting those
                // reads (foundations §6.3 [choice], RN2).
                let wall = if decline_key_seed(it.x, it.z, salt) == CLOCK_SEED {
                    let n = self.decline_clock_reads.get();
                    self.decline_clock_reads.set(n.wrapping_add(1));
                    crate::seeding::seed(self.host.session_seed(), crate::seeding::keys::growth_decline_clock(n))
                } else {
                    0
                };
                decline_key(it.x, it.z, salt, wall)
            };
            stl_sort(&mut decline, &mut |a, b| key(b) < key(a));
        }
        growth.retain(|it| !(i32::from(wv(it)) < growth_t));
        let size = self.city_size();
        let stages = self.d.borrow().tuning.stage_curves.clone();
        assign_stages(&mut growth, &stages, size);
        let mut d = self.d.borrow_mut();
        d.growth = growth;
        d.decline = decline;
        d.last_city_size = size;
    }

    fn supply(&self, id: u32, index: u32) -> f32 {
        self.host
            .get_demand(id, index)
            .map_or(0.0, |n| self.host.query_supply(n))
    }

    /// The city size of §5.5 with its host queries, in the original's order.
    pub fn city_size(&self) -> f32 {
        let zone = self.d.borrow().tuning.target_zone;
        let mut v = CitySizeInputs::default();
        match zone {
            1 | 2 => {
                v.connected = self.host.connected_cities();
                v.all_connected = self.host.all_connected_cities();
                v.c_census = self.supply(0x3001, 0);
                v.c_region = self.supply(0x3301, 0x10000);
                if zone == 1 {
                    v.r_census = self.supply(0x1001, 0);
                }
            }
            3 => v.i_total = self.supply(0x4001, 0x20000),
            _ => {}
        }
        v.multiplier = self.host.stage_population_multiplier();
        city_size(zone, &v)
    }

    // ---- capacity bookkeeping (§6) -------------------------------------------------------------------

    /// `UpdateMaxCapacity(rect)` on ZoningChange (§6.1).
    pub fn update_max_capacity(&self, rect: [i32; 4]) {
        let g = self.grids();
        let (Some(zoned), Some(maxc)) = (g.zoned, g.max_capacity) else {
            return;
        };
        let s = loop_shift(&*maxc);
        let (cx, cz) = counts(&*maxc);
        let [x0, z0, x1, z1] = rect;
        let tx0 = (x0 >> s).max(0);
        let tz0 = (z0 >> s).max(0);
        let tx1 = (x1 >> s).min(cx - 1);
        let tz1 = (z1 >> s).min(cz - 1);
        let mut profile = ConstructionProfile::default();
        self.construction_info(&mut profile);
        let (purpose, wealth) = {
            let d = self.d.borrow();
            (u32::from(d.tuning.purpose), u32::from(d.tuning.wealth))
        };
        for tz in tz0..=tz1 {
            for tx in tx0..=tx1 {
                let mut n = 0u32;
                let mut sum = 0.0f32;
                for cz_ in (tz << s)..((tz + 1) << s) {
                    for cx_ in (tx << s)..((tx + 1) << s) {
                        let zt = i32::from(self.host.zone_type(cx_, cz_));
                        if building_fits_in_zone(profile.zone, zt) {
                            n += 1;
                            let c = self.host.max_capacity(zt, purpose, wealth);
                            sum = r24(f64::from(sum) + f64::from(c)) as f32;
                        }
                    }
                }
                zoned.set(tx, tz, n as u8);
                maxc.set(tx, tz, round_capacity(sum));
            }
        }
    }

    /// The StateChanged message (§6.3).
    pub fn state_changed(&self, m: &GrowthMessage) {
        let wealth = u32::from(self.d.borrow().tuning.wealth);
        if m.data4 != wealth {
            return;
        }
        let occupied = |v: u32| (2..4).contains(&(v as u8));
        let (old, new) = (occupied(m.data1), occupied(m.data2));
        if old == new {
            return;
        }
        let Some(lot) = m.unknown else { return };
        if m.data3 == 0 {
            // The looked-up type is discarded (quirk 6).
            let _ = self.host.lot_building_type(lot, true);
            return;
        }
        self.update_population_count(lot, m.data3, new);
    }

    /// `UpdatePopulationCount(lot, buildingType, add)` (§6.3): built
    /// capacity.
    pub fn update_population_count(&self, lot: u64, building_type: u32, add: bool) {
        let (purpose, dev) = {
            let d = self.d.borrow();
            (u32::from(d.tuning.purpose), d.tuning.developer_type)
        };
        if self.host.building_purpose(building_type) != purpose {
            return;
        }
        let cap = i32::from(self.host.lot_capacity(lot, dev, true));
        let delta = if add { cap } else { -cap };
        let (w, d) = self.host.lot_size(lot);
        let Some((q, r)) = split_capacity(delta, i32::from(w) * i32::from(d)) else {
            return;
        };
        let [ax, az, bx, bz] = self.host.lot_bounding_rect(lot);
        let (x0, x1) = (ax.min(bx), ax.max(bx));
        let (z0, z1) = (az.min(bz), az.max(bz));
        let grid = self.g.borrow().capacity.clone();
        if let Some(g) = &grid {
            let s = loop_shift(&**g);
            let (cx, cz) = counts(&**g);
            let max_x = (i64::from(cx) << s) - 1;
            let max_z = (i64::from(cz) << s) - 1;
            for cz_ in i64::from(z0).max(0)..=i64::from(z1).min(max_z) {
                for cx_ in i64::from(x0).max(0)..=i64::from(x1).min(max_x) {
                    let (tx, tz) = ((cx_ >> s) as i32, (cz_ >> s) as i32);
                    g.set(tx, tz, add_to_capacity(g.get(tx, tz), q));
                }
            }
        }
        let (lx, lz) = self.host.lot_location(lot);
        if let Some(g) = &grid {
            let s = loop_shift(&**g);
            let (tx, tz) = (lx >> s, lz >> s);
            g.set(tx, tz, add_to_capacity(g.get(tx, tz), r));
        }
        let mut dd = self.d.borrow_mut();
        dd.total_capacity = dd.total_capacity.wrapping_add(delta);
    }

    // ---- slots (§10.1) -------------------------------------------------------------------------------

    /// `GetType` (slot 5): the target zone.
    pub fn target_zone(&self) -> i32 {
        i32::from(self.d.borrow().tuning.target_zone)
    }

    /// `GetSubType` (slot 6): the developer type.
    pub fn developer_type(&self) -> u32 {
        self.d.borrow().tuning.developer_type
    }

    /// `GetSubType` (slot 6), under its slot name: [`GrowthDeveloper::developer_type`].
    pub fn sub_type(&self) -> u32 {
        self.developer_type()
    }

    /// `GetPurpose` (slot 7).
    pub fn purpose(&self) -> u32 {
        u32::from(self.d.borrow().tuning.purpose)
    }

    pub fn wealth(&self) -> u32 {
        u32::from(self.d.borrow().tuning.wealth)
    }

    /// `GetConstructionInfo` (slot 9, §6.2): writes the zone code (not for
    /// an industrial developer with a purpose outside 5..8), `unknown2`,
    /// wealth, purpose and the land value range; the rest of the caller's
    /// profile is left as it was.
    pub fn construction_info(&self, p: &mut ConstructionProfile) {
        let d = self.d.borrow();
        let t = &d.tuning;
        match t.target_zone {
            1 => p.zone = 1,
            2 => p.zone = 4,
            3 => match t.purpose {
                5 => p.zone = 7,
                6..=8 => p.zone = 8,
                _ => {}
            },
            _ => p.zone = 0,
        }
        p.unknown2 = 0;
        p.wealth = u32::from(t.wealth);
        p.purpose = u32::from(t.purpose);
        p.land_value_min = t.land_value_min;
        p.land_value_max = t.land_value_max;
    }

    /// `GetGrowthList` (slot 11): read in place.
    pub fn growth_list(&self) -> Ref<'_, Vec<GrowthItem>> {
        Ref::map(self.d.borrow(), |d| &d.growth)
    }

    /// `GetDeclineList` (slot 12).
    pub fn decline_list(&self) -> Ref<'_, Vec<GrowthItem>> {
        Ref::map(self.d.borrow(), |d| &d.decline)
    }

    /// An owned copy of the growth list (slot 11), for a caller that must
    /// not hold a borrow of the developer.
    pub fn growth_items(&self) -> Vec<GrowthItem> {
        self.d.borrow().growth.clone()
    }

    /// An owned copy of the decline list (slot 12).
    pub fn decline_items(&self) -> Vec<GrowthItem> {
        self.d.borrow().decline.clone()
    }

    /// `GetDesirabilityMap` (slot 13): D (not W).
    pub fn desirability_map(&self) -> Option<Rc<dyn TractGrid<i16>>> {
        self.g.borrow().desirability.clone()
    }

    /// `GetCapacityMap` (slot 14): built capacity.
    pub fn capacity_map(&self) -> Option<Rc<dyn TractGrid<u16>>> {
        self.g.borrow().capacity.clone()
    }

    /// `GetPopulationMap` (slot 15).
    pub fn population_map(&self) -> Option<Rc<dyn TractGrid<i16>>> {
        self.g.borrow().population.clone()
    }

    /// `GetMaxCapacityMap` (slot 16).
    pub fn max_capacity_map(&self) -> Option<Rc<dyn TractGrid<u16>>> {
        self.g.borrow().max_capacity.clone()
    }

    /// `GetTotalCapacity` (slot 17).
    pub fn total_capacity(&self) -> i32 {
        self.d.borrow().total_capacity
    }

    /// `GetSourceDemand` (slot 18).
    pub fn source_demand(&self) -> Option<u64> {
        self.d.borrow().source_demand
    }

    /// `GetSourceDemand().QueryActiveDemandValue()` (demand slot 12; the
    /// BDS's `ad(D)`, building-development.md §4.3, §4.4); `None` without a
    /// source node.
    pub fn source_active_demand(&self) -> Option<f32> {
        let node = self.source_demand()?;
        Some(self.host.active_demand(node))
    }

    /// Slots 19–22: power, water, fire, park caps.
    pub fn caps(&self) -> [u8; 4] {
        let t = &self.d.borrow().tuning;
        [t.power_cap, t.water_cap, t.fire_cap, t.park_cap]
    }

    /// `GetDesirabilityThreshold` (slot 23): the growth threshold.
    pub fn desirability_threshold(&self) -> i32 {
        self.d.borrow().tuning.growth_threshold
    }

    /// `GetAbandonmentThreshold(key)` (slot 24): inserts 0.0 for an unknown
    /// key.
    pub fn abandonment_threshold(&self, key: i32) -> f32 {
        self.d.borrow_mut().tuning.abandonment_threshold(key)
    }

    /// `GetMonopolyUse` (slot 25).
    pub fn monopoly_use(&self) -> u32 {
        u32::from(self.d.borrow().tuning.monopoly_use)
    }

    /// `GetStylePercentages` (slot 26): the member map.
    pub fn style_percentages(&self) -> Ref<'_, BTreeMap<u32, f32>> {
        Ref::map(self.d.borrow(), |d| &d.tuning.styles)
    }

    /// `GetSubjectiveThresholds(i)` (slot 30).
    pub fn subjective_thresholds(&self, i: i32) -> (i32, i32) {
        self.d.borrow().tuning.subjective_thresholds(i)
    }

    /// `EstimateDesirability(x, z)` (slot 29, §4.6): single-cell values, no
    /// commute and no special buildings; returned sign-extended.
    pub fn estimate_desirability(&self, x: i32, z: i32) -> i32 {
        let t = self.d.borrow().tuning.clone();
        let host = &*self.host;
        let u8_at = |m: SourceMap| host.source_u8(m).map(|g| f32::from(g.get_cell_value(x, z)));
        let f32_at = |m: SourceMap| host.source_f32(m).map(|g| g.get_cell_value(x, z));
        let i16_low = |m: SourceMap| host.source_i16(m).map(|g| low_byte(i32::from(g.get_cell_value(x, z))));
        let i16_unsigned = |m: SourceMap| host.source_i16(m).map(|g| f32::from(g.get_cell_value(x, z) as u16));
        let mut acc = t.baseline;
        for c in Curve::ALL {
            if c == Curve::Commute {
                continue;
            }
            let Some(curve) = t.curve(c) else { continue };
            let input = match c {
                Curve::LandValue => u8_at(if t.use_intrinsic_land_value {
                    SourceMap::IntrinsicLandValue
                } else {
                    SourceMap::TotalLandValue
                }),
                Curve::Slope => f32_at(SourceMap::Slope),
                Curve::ProximityR1 => u8_at(SourceMap::ResidentialProximity(1)),
                Curve::ProximityR2 => u8_at(SourceMap::ResidentialProximity(2)),
                Curve::ProximityR3 => u8_at(SourceMap::ResidentialProximity(3)),
                Curve::ProximityCO2 => u8_at(SourceMap::CommercialProximity(0x3320)),
                Curve::ProximityCO3 => u8_at(SourceMap::CommercialProximity(0x3330)),
                Curve::Park => i16_low(SourceMap::Park),
                Curve::Air => i16_unsigned(SourceMap::Pollution(0)),
                Curve::Water => i16_unsigned(SourceMap::Pollution(1)),
                Curve::Garbage => i16_unsigned(SourceMap::Pollution(2)),
                Curve::Radiation => i16_unsigned(SourceMap::Pollution(3)),
                Curve::School => f32_at(SourceMap::SchoolGrade),
                Curve::Hospital => f32_at(SourceMap::HospitalGrade),
                Curve::Crime => u8_at(SourceMap::Crime),
                Curve::Traffic => u8_at(SourceMap::CommercialTraffic),
                Curve::Landmark => i16_low(SourceMap::Landmark),
                Curve::Transient => host
                    .source_i8(SourceMap::TransientAura)
                    .map(|g| low_byte(i32::from(g.get_cell_value(x, z)))),
                Curve::Commute => None,
            };
            if let Some(v) = input {
                acc = acc.wrapping_add(ftol32(f64::from(curve.eval(v))));
            }
        }
        i32::from(acc as i16)
    }
}

impl SaveObject for GrowthDeveloper {
    fn class_id(&self) -> u32 {
        CLSID_GROWTH_DEVELOPER
    }

    /// The §9 record; returns `GetError() == 0`.
    fn write(&self, out: &mut dyn OutStream) -> bool {
        let (last, total) = {
            let d = self.d.borrow();
            (d.last_phantom_sum, d.total_capacity)
        };
        let g = self.grids();
        out.put_u16(RECORD_VERSION);
        out.put_i32(last);
        out.put_u32(total as u32);
        out.put_ref(&save_ref(&g.zoned));
        out.put_ref(&save_ref(&g.desirability));
        out.put_ref(&save_ref(&g.population));
        out.put_ref(&save_ref(&g.capacity));
        out.put_ref(&save_ref(&g.max_capacity));
        out.error() == 0
    }

    /// The §9 record: version ≠ 1 → false with nothing else read; otherwise
    /// every field (a failed read leaves the member as it was; each
    /// reference nulls its member first) and `GetError() == 0`.
    fn read(&self, inp: &mut dyn InStream) -> bool {
        match inp.get_u16() {
            Some(RECORD_VERSION) => {}
            _ => return false,
        }
        if let Some(v) = inp.get_i32() {
            self.d.borrow_mut().last_phantom_sum = v;
        }
        if let Some(v) = inp.get_u32() {
            self.d.borrow_mut().total_capacity = v as i32;
        }
        let host = &*self.host;
        let mut read_ref = || inp.get_ref_as(IID_SIM_GRID).filter(|r| !r.is_null());
        self.g.borrow_mut().zoned = None;
        let r = read_ref();
        self.g.borrow_mut().zoned = r.and_then(|r| GridProvider::<u8>::resolve_grid(host, &r));
        self.g.borrow_mut().desirability = None;
        let r = read_ref();
        self.g.borrow_mut().desirability = r.and_then(|r| GridProvider::<i16>::resolve_grid(host, &r));
        self.g.borrow_mut().population = None;
        let r = read_ref();
        self.g.borrow_mut().population = r.and_then(|r| GridProvider::<i16>::resolve_grid(host, &r));
        self.g.borrow_mut().capacity = None;
        let r = read_ref();
        self.g.borrow_mut().capacity = r.and_then(|r| GridProvider::<u16>::resolve_grid(host, &r));
        self.g.borrow_mut().max_capacity = None;
        let r = read_ref();
        self.g.borrow_mut().max_capacity = r.and_then(|r| GridProvider::<u16>::resolve_grid(host, &r));
        inp.error() == 0
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }

    fn host_token(&self) -> u64 {
        self.token.get()
    }
}
