// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! `cSC4BuildingDevelopmentSimulator` (§1–§7).

use std::any::Any;
use std::cell::{Cell, RefCell, RefMut};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use crate::development::{building_fits_in_zone, building_map_key, developer_type, wealth_of};
use crate::pacing::{self, Pacing};
use crate::props;
use crate::stream::{InStream, OutStream, RefValue, ResKey, SaveObject};
use crate::x87::ftol32;

use super::{
    cheat, decline_day, exemplar_type, monthly_supply, msg, prop, tuning_min_max, unpack_terrain_rect, AdviceSubject,
    CellRect, CommandParam, ConstructionNode, ConstructionProfile, DeveloperId, DevelopmentHost, GridProvider,
    GrowthDevelopers, GrowthItem, GrowthProfile, LotId, Message, OccupantId, Outcome, SharedProcess, TractGrid, Vec3,
    AGENT_FLAGS, AGENT_LEVELS, AGENT_NAME, AGRICULTURE, BUILDING_MAP_INSTANCE, BUILDING_MAP_TRACT_SIZE, CENSUS_C,
    CENSUS_I, CENSUS_R, CITY_PLANNING_ADVISOR, CLSID_BDS, DECLINE_PLAN_THRESHOLD, GROWTH_THRESHOLD,
    HIGHEST_STAGE_EVENT, IID_DEMAND, IID_GROWTH_DEVELOPER, IID_OCCUPANT, IID_SIM_GRID, MAX_PLAN_ENTRIES,
    MAX_SLOPE_CHANGE, NEW_NODE_DEMAND, NEW_NODE_SUPPLY, PLACE_BUILDING_COMMAND, PLACE_BUILDING_NAME, SPECIAL_GROUPS,
};
use crate::development::BuildingProfile;

/// The record version (§6); every other version is rejected.
pub const RECORD_VERSION: u16 = 4;

/// [choice] The default daily work budget of `Grow` (§4.4): the most
/// tract-developer `Grow` calls one day makes. Effectively unlimited until
/// it is calibrated from in-game measurement; set it with
/// [`DevelopmentSim::set_grow_attempt_budget`].
pub const DEFAULT_GROW_ATTEMPT_BUDGET: u32 = u32::MAX;

/// [choice] The most loop passes one `PlanDecline` makes (wasted draws
/// included); far above any real plan.
const MAX_PLAN_PASSES: u32 = 1 << 24;

/// One 24-bit rounding (foundations §18).
fn r24(x: f64) -> f32 {
    x as f32
}

/// The tuning of §4.13.
#[derive(Clone, Debug, PartialEq)]
pub struct Tuning {
    pub cr_min: u32,
    pub cr_max: u32,
    pub cr_per_resident: f32,
    pub bias: i32,
    pub stage_multiplier: Vec<f32>,
}

impl Default for Tuning {
    /// The code defaults.
    fn default() -> Self {
        Tuning {
            cr_min: 100,
            cr_max: 2000,
            cr_per_resident: 0.5,
            bias: 10,
            stage_multiplier: Vec::new(),
        }
    }
}

/// One decline-plan entry (§4.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanEntry {
    pub day: u32,
    pub x: i32,
    pub z: i32,
    pub developer: DeveloperId,
    /// Always 0.
    pub stage: u8,
}

/// A key of the (dead) planned-growth map (§2, §6): saved, never filled by
/// the shipped game.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlannedGrowth {
    pub developer: DeveloperId,
    pub x: i32,
    pub z: i32,
    pub stage: u8,
}

/// The per-object state of §2 (the grid, the node and the process-wide
/// statics are held separately).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DevelopmentData {
    pub ref_count: u32,
    /// Set when `Release` destroyed the object.
    pub destroyed: bool,
    pub initialized: bool,
    pub loaded_from_save: bool,
    /// Developer type → developer (one reference each).
    pub developers: BTreeMap<u32, DeveloperId>,
    pub highest_stage: BTreeMap<u32, u8>,
    /// Landmarks and rewards, insertion order (one reference each).
    pub special_buildings: Vec<OccupantId>,
    /// The planning state machine (0–4, §4.2).
    pub state: u32,
    /// The next developer type state 2 asks (`None` = the map's end).
    pub cursor: Option<u32>,
    pub decline_plan: Vec<PlanEntry>,
    pub planned_growth: BTreeMap<PlannedGrowth, u32>,
    pub last_successful_build: BTreeMap<u32, u32>,
    pub tuning: Tuning,
    /// Never set (§2).
    pub use_growth_plan: bool,
    pub cheat_ignore_plumbing: bool,
    pub cheat_ignore_power: bool,
    pub cheat_ignore_caps: bool,
    pub cheat_debug_growth: bool,
    pub cheat_coffee_bar: bool,
    /// The terrain's cell counts cached at `Init`.
    pub cell_counts: (i32, i32),
}

type Grid = Rc<dyn TractGrid<u8>>;

/// The building development simulator.
pub struct DevelopmentSim {
    host: Rc<dyn DevelopmentHost>,
    devs: Rc<dyn GrowthDevelopers>,
    process: SharedProcess,
    token: Cell<u64>,
    d: RefCell<DevelopmentData>,
    grid: RefCell<Option<Grid>>,
    node: RefCell<Option<Node>>,
    grow_budget: Cell<u32>,
    /// simulator.md §5.7: captured from [`pacing::process`] at construction.
    pacing: Cell<Pacing>,
}

type Node = Rc<dyn ConstructionNode>;

impl DevelopmentSim {
    /// A new BDS with the constructor values of §2, sharing the process
    /// statics `process`; `devs` provides the growth developers.
    pub fn new(
        host: Rc<dyn DevelopmentHost>,
        devs: Rc<dyn GrowthDevelopers>,
        process: SharedProcess,
        token: u64,
    ) -> Rc<DevelopmentSim> {
        Rc::new(DevelopmentSim {
            host,
            devs,
            process,
            token: Cell::new(token),
            d: RefCell::new(DevelopmentData::default()),
            grid: RefCell::new(None),
            node: RefCell::new(None),
            grow_budget: Cell::new(DEFAULT_GROW_ATTEMPT_BUDGET),
            pacing: Cell::new(pacing::process()),
        })
    }

    /// The pacing this simulator runs (simulator.md §5.7).
    pub fn pacing(&self) -> Pacing {
        self.pacing.get()
    }

    /// Shadows and tests: [`Pacing::Original`] keeps one step per idle.
    pub fn set_pacing(&self, p: Pacing) {
        self.pacing.set(p);
    }

    /// simulator.md §5.7 [choice] (deterministic pacing): the day's settle
    /// delivery runs the monthly rebuild to completion (state 0). False while
    /// it waits for a growth developer's map update: the simulator delivers
    /// the settle idle again after the other idle agents (the developers
    /// among them) have run.
    pub fn settle_cycle(&self) -> bool {
        pacing::run_to_idle(
            || {
                let before = {
                    let d = self.d.borrow();
                    (d.state, d.cursor)
                };
                self.do_idle();
                let d = self.d.borrow();
                (d.state, d.cursor) != before
            },
            || self.d.borrow().state == 0,
        )
    }

    /// The daily work budget of `Grow` ([choice], §4.4): at most this many
    /// tract-developer `Grow` calls per day. Not saved.
    pub fn grow_attempt_budget(&self) -> u32 {
        self.grow_budget.get()
    }

    pub fn set_grow_attempt_budget(&self, n: u32) {
        self.grow_budget.set(n);
    }

    pub fn token(&self) -> u64 {
        self.token.get()
    }

    pub fn set_token(&self, token: u64) {
        self.token.set(token);
    }

    pub fn process(&self) -> SharedProcess {
        self.process.clone()
    }

    pub fn snapshot(&self) -> DevelopmentData {
        self.d.borrow().clone()
    }

    /// Direct state access, for hosts that restore state and for tests.
    pub fn data_mut(&self) -> RefMut<'_, DevelopmentData> {
        self.d.borrow_mut()
    }

    /// The construction node.
    pub fn construction_node(&self) -> Option<Rc<dyn ConstructionNode>> {
        self.node.borrow().clone()
    }

    pub fn set_construction_node(&self, node: Option<Rc<dyn ConstructionNode>>) {
        *self.node.borrow_mut() = node;
    }

    pub fn set_building_map(&self, grid: Option<Grid>) {
        *self.grid.borrow_mut() = grid;
    }

    fn today(&self) -> u32 {
        self.host.sim_day()
    }

    /// `ad(D)`: `trunc(GetSourceDemand().QueryActiveDemandValue())` as
    /// int32 ([choice] 0 without a node).
    fn ad(&self, dev: DeveloperId) -> i32 {
        match self.devs.developer_source_demand(dev) {
            Some(node) => ftol32(f64::from(self.host.demand_active_value(node))),
            None => 0,
        }
    }

    // ---- lifecycle (§1.3–§1.5) ---------------------------------------------------------------------

    /// `Init()` (§1.3). Always true.
    pub fn init(&self) -> bool {
        if self.d.borrow().initialized {
            return true;
        }
        self.d.borrow_mut().initialized = true;
        let h = self.host.clone();
        let first = !self.process.catalogue().is_loaded();
        if first {
            self.load_catalogue();
        }
        for (key, ex) in h.exemplars_of_types(&[exemplar_type::DEVELOPER]) {
            if props::u32_of(&ex, prop::EXEMPLAR_TYPE) == Some(exemplar_type::DEVELOPER) {
                self.create_developer(&key, &ex);
            }
        }
        self.read_tuning();
        if !self.d.borrow().loaded_from_save {
            let mut d = self.d.borrow_mut();
            let types: Vec<u32> = d.developers.keys().copied().collect();
            for t in types {
                d.highest_stage.insert(t, 0);
            }
        }
        h.acquire_services();
        self.d.borrow_mut().cell_counts = h.terrain_cell_counts();
        if self.grid.borrow().is_none() {
            let g = GridProvider::<u8>::create_grid(&*h, BUILDING_MAP_INSTANCE, BUILDING_MAP_TRACT_SIZE);
            *self.grid.borrow_mut() = g;
        }
        let node = self.node.borrow().clone();
        match node {
            Some(n) => {
                n.init();
            }
            None => {
                if let Some(n) = h.create_construction_node() {
                    n.init();
                    n.set_supply(NEW_NODE_SUPPLY);
                    n.set_demand(NEW_NODE_DEMAND);
                    *self.node.borrow_mut() = Some(n);
                }
            }
        }
        h.subscribe(&msg::SUBSCRIPTIONS);
        h.register_command(PLACE_BUILDING_COMMAND, PLACE_BUILDING_NAME);
        for level in AGENT_LEVELS {
            h.add_agent(level, AGENT_NAME, AGENT_FLAGS);
        }
        for (id, name) in cheat::ALL {
            h.register_cheat(id, name);
        }
        true
    }

    /// The static catalogue (§1.3 step 1, once per process).
    fn load_catalogue(&self) {
        let h = &self.host;
        self.process.catalogue_mut().set_loaded(true);
        h.register_static_cleanup();
        let types = [
            exemplar_type::BUILDING,
            exemplar_type::FOUNDATION,
            exemplar_type::RETAINING_WALL,
        ];
        for (key, ex) in h.exemplars_of_types(&types) {
            match props::u32_of(&ex, prop::EXEMPLAR_TYPE) {
                Some(exemplar_type::BUILDING) => {
                    let localized = h.exemplar_string(&ex, prop::LOCALIZED_NAME);
                    self.process.catalogue_mut().add_building(key, &ex, localized);
                }
                Some(exemplar_type::FOUNDATION) => self.process.catalogue_mut().add_foundation(key),
                Some(exemplar_type::RETAINING_WALL) => self.process.catalogue_mut().add_retaining_wall(key),
                _ => {}
            }
        }
        let n = self.process.catalogue().building_count() as u32;
        h.post_building_types_updated(n);
        let configs = h.lot_configurations();
        let widths = h.cell_widths();
        self.process.catalogue_mut().build_density_table(&configs, widths);
    }

    /// `CreateDeveloper(holder)` (§1.5).
    fn create_developer(&self, key: &ResKey, ex: &Rc<crate::property::PropertyHolder>) {
        let t = props::u32_of(ex, prop::DEVELOPER_TYPE).unwrap_or(0);
        if t == 0 {
            return;
        }
        let existing = self.d.borrow().developers.get(&t).copied();
        match existing {
            Some(dev) => {
                self.devs.developer_init(dev, key, ex);
            }
            None => {
                let Some(dev) = self.devs.create_developer(key, ex) else {
                    return;
                };
                self.devs.developer_init(dev, key, ex);
                self.d.borrow_mut().developers.insert(t, dev);
            }
        }
        let today = self.today();
        self.d.borrow_mut().last_successful_build.insert(t, today);
    }

    /// `Shutdown()` (§1.4). Always true.
    pub fn shutdown(&self) -> bool {
        if !self.d.borrow().initialized {
            return true;
        }
        self.d.borrow_mut().initialized = false;
        let h = self.host.clone();
        for (id, _) in cheat::ALL {
            h.unregister_cheat(id);
        }
        h.remove_agent();
        h.unsubscribe(&msg::SUBSCRIPTIONS);
        h.unregister_command(PLACE_BUILDING_COMMAND);
        let (devs, special) = {
            let mut d = self.d.borrow_mut();
            (
                std::mem::take(&mut d.developers),
                std::mem::take(&mut d.special_buildings),
            )
        };
        for dev in devs.values() {
            self.devs.developer_shutdown(*dev);
            self.devs.developer_release(*dev);
        }
        for occ in special {
            h.release(occ);
        }
        {
            let mut d = self.d.borrow_mut();
            d.decline_plan.clear();
            d.planned_growth.clear();
            d.highest_stage.clear();
        }
        *self.grid.borrow_mut() = None;
        if let Some(n) = self.node.borrow_mut().take() {
            n.shutdown();
        }
        h.release_services();
        self.process.clear_marks();
        true
    }

    pub fn is_initialized(&self) -> bool {
        self.d.borrow().initialized
    }

    /// `AddRef()`.
    pub fn add_ref(&self) -> u32 {
        let mut d = self.d.borrow_mut();
        d.ref_count = d.ref_count.wrapping_add(1);
        d.ref_count
    }

    /// `Release()` (§1.1): above 1, decrement; otherwise `Shutdown()` and
    /// destruction; returns 0.
    pub fn release(&self) -> u32 {
        {
            let mut d = self.d.borrow_mut();
            if d.ref_count > 1 {
                d.ref_count -= 1;
                return d.ref_count;
            }
        }
        self.shutdown();
        let mut d = self.d.borrow_mut();
        d.ref_count = 0;
        d.destroyed = true;
        0
    }

    /// `ReadSimulatorTunableValues` (§4.13): code defaults first, then the
    /// exemplar's values.
    pub fn read_tuning(&self) {
        let mut t = Tuning::default();
        if let Some(h) = self.host.tuning() {
            let (min, max) = tuning_min_max(&h);
            if let Some(v) = min {
                t.cr_min = v;
            }
            if let Some(v) = max {
                t.cr_max = v;
            }
            if let Some(v) = props::f32_of(&h, prop::PER_RESIDENT) {
                t.cr_per_resident = v;
            }
            if let Some(v) = props::u32_of(&h, prop::BIAS) {
                t.bias = v as i32;
            }
            if let Some(v) = props::f32s(&h, prop::STAGE_MULTIPLIER) {
                t.stage_multiplier = v;
            }
        }
        self.d.borrow_mut().tuning = t;
    }

    // ---- messages (§3) -----------------------------------------------------------------------------

    /// `DoMessage` (§3): always true, except a deterministic-pacing settle
    /// idle that waits for a developer ([`DevelopmentSim::settle_cycle`]).
    pub fn do_message(&self, m: &Message) -> bool {
        match m.msg_type {
            msg::SIM_BEGIN | msg::SIM_NEW_MONTH => self.do_monthly_simulation(),
            msg::SIM_NEW_DAY => {
                self.do_idle();
                self.do_daily_simulation();
            }
            msg::SIM_IDLE if self.pacing.get().is_deterministic() => return self.settle_cycle(),
            msg::SIM_IDLE => self.do_idle(),
            msg::CHEAT => self.do_cheat(m.data1),
            msg::PLACE_BUILDING => self.place_building_command(&m.params),
            msg::TERRAIN_CHANGED => {
                let cells = self.d.borrow().cell_counts;
                self.do_terrain_changed(unpack_terrain_rect(m.data1, m.data2, cells), m.data4);
            }
            msg::OCCUPANT_INSERTED | msg::OCCUPANT_REMOVED => {
                if let Some(occ) = m.occupant {
                    self.occupant_changed(occ, m.msg_type == msg::OCCUPANT_INSERTED);
                }
            }
            msg::STATE_CHANGED => {
                if let Some(lot) = m.lot {
                    self.state_changed(lot, m.data1, m.data2, m.data4);
                }
            }
            _ => {}
        }
        true
    }

    /// The cheat notification (§3.2).
    pub fn do_cheat(&self, id: u32) {
        let mut d = self.d.borrow_mut();
        match id {
            cheat::HOW_DRY_I_AM => d.cheat_ignore_plumbing = true,
            cheat::FIGHT_THE_POWER => d.cheat_ignore_power = true,
            cheat::BARSTUCK => d.cheat_coffee_bar = !d.cheat_coffee_bar,
            _ => {}
        }
    }

    /// Occupant inserted / removed (§3.2): landmarks and rewards.
    fn occupant_changed(&self, occ: OccupantId, inserted: bool) {
        let h = &self.host;
        if h.occupant_has_property(occ, prop::PREVIEW_OCCUPANT) {
            return;
        }
        if !SPECIAL_GROUPS.iter().any(|&g| h.occupant_in_group(occ, g)) {
            return;
        }
        if inserted {
            self.d.borrow_mut().special_buildings.push(occ);
            h.add_ref(occ);
        } else {
            let found = {
                let mut d = self.d.borrow_mut();
                match d.special_buildings.iter().position(|&o| o == occ) {
                    Some(i) => {
                        d.special_buildings.remove(i);
                        true
                    }
                    None => false,
                }
            };
            if found {
                h.release(occ);
            }
        }
    }

    /// StateChanged (§3.2).
    fn state_changed(&self, lot: LotId, old_state: u32, new_state: u32, wealth: u32) {
        if self.host.lot_building_type(lot) == 0 {
            return;
        }
        let occupied = |s: u32| s == 2 || s == 3;
        let (old, new) = (occupied(old_state), occupied(new_state));
        if old != new {
            self.do_building_changed(lot, wealth, new);
        }
    }

    // ---- monthly, idle, daily (§4.1–§4.5) -----------------------------------------------------------

    /// `DoMonthlySimulation` (§4.1): only in state 0.
    pub fn do_monthly_simulation(&self) {
        if self.d.borrow().state != 0 {
            return;
        }
        let h = &self.host;
        let r = h.census_supply(CENSUS_R).unwrap_or(0.0);
        let c = h.census_supply(CENSUS_C).unwrap_or(0.0);
        let i = h.census_supply(CENSUS_I).unwrap_or(0.0);
        let t = self.d.borrow().tuning.clone();
        let supply = monthly_supply(r, c, i, t.cr_per_resident, t.cr_min, t.cr_max);
        if let Some(n) = self.node.borrow().clone() {
            n.set_supply(supply);
        }
        let devs: Vec<DeveloperId> = self.d.borrow().developers.values().copied().collect();
        for dev in devs {
            self.devs.developer_start_map_update(dev);
        }
        self.d.borrow_mut().state = 1;
    }

    /// `DoIdle` (§4.2).
    pub fn do_idle(&self) {
        let state = self.d.borrow().state;
        match state {
            1 | 2 => {
                if state == 1 {
                    let mut d = self.d.borrow_mut();
                    d.cursor = d.developers.keys().next().copied();
                    d.state = 2;
                }
                loop {
                    let next = {
                        let d = self.d.borrow();
                        d.cursor
                            .and_then(|c| d.developers.range(c..).next().map(|(&t, &dev)| (t, dev)))
                    };
                    let Some((t, dev)) = next else { break };
                    if !self.devs.developer_map_update_finished(dev) {
                        self.d.borrow_mut().cursor = Some(t);
                        return;
                    }
                    self.d.borrow_mut().cursor = t.checked_add(1);
                }
                let mut d = self.d.borrow_mut();
                d.cursor = None;
                d.state = 3;
                // `PlanGrowth` would run here if `use_growth_plan` (never).
                d.state = 4;
            }
            4 => {
                self.plan_decline();
                self.d.borrow_mut().state = 0;
            }
            _ => {}
        }
    }

    /// `DoDailySimulation` (§4.4): `Grow`, then `ExecuteDeclinePlan`.
    pub fn do_daily_simulation(&self) {
        self.grow();
        self.execute_decline_plan();
    }

    /// `PlanDecline` (§4.3).
    pub fn plan_decline(&self) {
        let (devs, bias) = {
            let mut d = self.d.borrow_mut();
            d.decline_plan.clear();
            (d.developers.clone(), d.tuning.bias)
        };
        let mut total: i32 = 0;
        // (developer, its decline list, cursor)
        let mut qualifying: Vec<(DeveloperId, Vec<GrowthItem>, usize)> = Vec::new();
        for &dev in devs.values() {
            if self.devs.developer_purpose(dev) != AGRICULTURE {
                let ad = self.ad(dev);
                if ad < DECLINE_PLAN_THRESHOLD {
                    total = total.wrapping_add(bias.wrapping_sub(ad));
                    qualifying.push((dev, self.devs.developer_decline_list(dev), 0));
                }
            }
        }
        let now = self.today();
        let mut plan: Vec<PlanEntry> = Vec::new();
        let mut passes = 0u32;
        loop {
            let mut r = self.process.uniform(total as u32) as i32;
            let mut picked = None;
            for (i, (dev, _, _)) in qualifying.iter().enumerate() {
                let w = bias.wrapping_sub(self.ad(*dev));
                if r < w {
                    picked = Some(i);
                    break;
                }
                r = r.wrapping_sub(w);
            }
            if let Some((dev, list, cursor)) = picked.and_then(|i| qualifying.get_mut(i)) {
                if let Some(item) = list.get(*cursor) {
                    plan.push(PlanEntry {
                        day: plan.len() as u32,
                        x: item.x,
                        z: item.z,
                        developer: *dev,
                        stage: 0,
                    });
                    *cursor += 1;
                }
            }
            if qualifying.iter().all(|(_, list, cursor)| *cursor >= list.len()) {
                break;
            }
            passes += 1;
            if passes >= MAX_PLAN_PASSES || plan.len() >= MAX_PLAN_ENTRIES {
                break;
            }
        }
        for e in &mut plan {
            e.day = decline_day(now, e.day);
        }
        self.d.borrow_mut().decline_plan = plan;
    }

    /// `Grow` (§4.4) under a deterministic work budget ([choice]): the
    /// original's 3 ms timer, spin and step adaptation are not modelled;
    /// the day ends when [`DevelopmentSim::grow_attempt_budget`]
    /// tract-developer `Grow` calls have been made or every qualifying
    /// developer has made its one attempt, whichever comes first.
    pub fn grow(&self) {
        let h = self.host.clone();
        let (mut pending, bias) = {
            let d = self.d.borrow();
            (d.developers.clone(), d.tuning.bias)
        };
        let budget = self.grow_budget.get();
        let mut attempts: u32 = 0;
        // glue27: one scratch for the selection and one for a Grow's outcomes,
        // reused across the day's attempts. Each attempt used to allocate an
        // outcomes Vec, and - the expensive one - each developer's demand was
        // read twice per round (`ad` is two host calls), once to add up the
        // weights and once to walk them. The values cannot change inside a
        // round (both are reads), so one read per developer serves both.
        let mut weights: Vec<(DeveloperId, i32)> = Vec::new();
        let mut outcomes: Vec<Outcome> = Vec::new();
        while !pending.is_empty() && attempts < budget {
            weights.clear();
            let mut total: i32 = 0;
            for &dev in pending.values() {
                let ad = self.ad(dev);
                if ad > GROWTH_THRESHOLD {
                    let w = ad.wrapping_add(bias);
                    total = total.wrapping_add(w);
                    weights.push((dev, w));
                }
            }
            let mut r = self.process.uniform(total as u32) as i32;
            let mut picked = None;
            for &(dev, w) in &weights {
                if r < w {
                    picked = Some(dev);
                    break;
                }
                r = r.wrapping_sub(w);
            }
            // [choice] Nothing picked: the original spins until its timer
            // expires; nobody else can be picked.
            let Some(dev) = picked else { break };
            let t = self.devs.developer_sub_type(dev);
            if self.devs.developer_lists_valid(dev) {
                let list = self.devs.developer_growth_list(dev);
                self.process.touch_marks(t);
                let mut item = list.iter().find(|it| !self.process.is_marked(t, (it.x, it.z))).copied();
                if item.is_none() {
                    self.process.clear_marks_of(t);
                    item = list.first().copied();
                }
                if let Some(it) = item {
                    self.process.mark(t, (it.x, it.z));
                    let profile = GrowthProfile {
                        developer: dev,
                        x: it.x,
                        z: it.z,
                        stage: it.max_stage,
                    };
                    outcomes.clear();
                    let mut flag = false;
                    let built = h.tract_grow(&profile, &mut outcomes, &mut flag);
                    attempts += 1;
                    if built > 0 {
                        let today = self.today();
                        self.d.borrow_mut().last_successful_build.insert(t, today);
                    }
                }
            }
            // [choice] A sub-type that is not a pending key ends the day.
            if pending.remove(&t).is_none() {
                break;
            }
        }
    }

    /// `ExecuteDeclinePlan` (§4.5).
    pub fn execute_decline_plan(&self) {
        let today = self.today();
        let plan = std::mem::take(&mut self.d.borrow_mut().decline_plan);
        let mut keep = Vec::with_capacity(plan.len());
        let mut it = plan.into_iter();
        while let Some(e) = it.next() {
            if e.day > today {
                keep.push(e);
                continue;
            }
            if self.ad(e.developer) > DECLINE_PLAN_THRESHOLD {
                continue;
            }
            let result = self.host.tract_decline(&GrowthProfile {
                developer: e.developer,
                x: e.x,
                z: e.z,
                stage: 0,
            });
            if result > 0 {
                keep.extend(it);
                break;
            }
        }
        // Entries a host callback may have planned meanwhile come after.
        let mut d = self.d.borrow_mut();
        let added = std::mem::take(&mut d.decline_plan);
        keep.extend(added);
        d.decline_plan = keep;
    }

    // ---- construction resources (§4.6) -------------------------------------------------------------

    /// `GetTotalConstructionResources()` (49): `trunc(supply)`, low 32 bits.
    pub fn total_construction_resources(&self) -> i32 {
        self.node.borrow().as_ref().map_or(0, |n| ftol32(f64::from(n.supply())))
    }

    /// `GetFreeConstructionResources()` (48): `trunc(r24(supply − demand))`,
    /// negative → 0.
    pub fn free_construction_resources(&self) -> i32 {
        let Some(n) = self.node.borrow().clone() else { return 0 };
        let v = ftol32(f64::from(r24(f64::from(n.supply()) - f64::from(n.demand()))));
        v.max(0)
    }

    /// `ClaimConstructionResources(n)` (50).
    pub fn claim_construction_resources(&self, amount: u16) -> bool {
        let Some(n) = self.node.borrow().clone() else {
            return false;
        };
        let t = r24(f64::from(n.demand()) + f64::from(amount));
        let supply = n.supply();
        if supply < t || supply.is_nan() || t.is_nan() {
            return false;
        }
        n.add_to_demand(f32::from(amount));
        n.end_of_cycle();
        true
    }

    /// `ReleaseConstructionResources(n)` (51): returns `EndOfCycle`'s result.
    pub fn release_construction_resources(&self, amount: u16) -> bool {
        let Some(n) = self.node.borrow().clone() else {
            return false;
        };
        n.add_to_demand(-(i32::from(amount) as f32));
        n.end_of_cycle()
    }

    // ---- catalogue slots (§4.7, §7.1) ---------------------------------------------------------------

    /// `IsBuildingTypeValid` (15).
    pub fn is_building_type_valid(&self, t: u32) -> bool {
        self.process.catalogue().is_building_type_valid(t)
    }

    /// `IsBuildingFamilyTypeValid` (16).
    pub fn is_building_family_type_valid(&self, f: u32) -> bool {
        self.process.catalogue().is_building_family_type_valid(f)
    }

    /// `IsBuildingInFamily` (20).
    pub fn is_building_in_family(&self, b: u32, f: u32) -> bool {
        self.process.catalogue().is_building_in_family(b, f)
    }

    /// `GetBuildingTypesByFamily(vector&, f)` (18).
    pub fn building_types_by_family(&self, out: &mut Vec<u32>, f: u32) -> u32 {
        self.process.catalogue().building_types_by_family(out, f)
    }

    /// `GetBuildingTypesByFamily(hash_set&, f)` (19).
    pub fn building_types_by_family_set(&self, out: &mut BTreeSet<u32>, f: u32) -> u32 {
        self.process.catalogue().building_types_by_family_set(out, f)
    }

    /// `GetBuildingTypesByFilter(hash_set&, filter)` (17).
    pub fn building_types_by_filter(&self, out: &mut BTreeSet<u32>, filter: Option<&dyn Fn(u32) -> bool>) -> u32 {
        let types = self.process.catalogue().all_building_types();
        let mut n = 0u32;
        for t in types {
            if filter.map_or(true, |f| f(t)) {
                out.insert(t);
                n = n.wrapping_add(1);
            }
        }
        n
    }

    /// `GetBuildingProfile(t)` (39): inserts a zero profile when unknown.
    pub fn building_profile(&self, t: u32) -> BuildingProfile {
        self.process.catalogue_mut().profile_entry(t).clone()
    }

    /// `GetBuildingProfilePtr(t)` (40).
    pub fn building_profile_ptr(&self, t: u32) -> Option<BuildingProfile> {
        self.process.catalogue().profile(t).cloned()
    }

    /// `GetBuildingKeyFromType` (14).
    pub fn building_key_from_type(&self, t: u32) -> Option<ResKey> {
        self.process.catalogue().building_key(t)
    }

    /// `GetFoundationKeyFromType` (22).
    pub fn foundation_key_from_type(&self, t: u32) -> Option<ResKey> {
        self.process.catalogue().foundation_key(t)
    }

    /// `GetLotRetainingWallKeyFromType` (28).
    pub fn retaining_wall_key_from_type(&self, t: u32) -> Option<ResKey> {
        self.process.catalogue().retaining_wall_key(t)
    }

    /// `GetExemplarName(t)` (37).
    pub fn exemplar_name(&self, t: u32) -> Vec<u8> {
        self.process.catalogue_mut().exemplar_name(t)
    }

    /// `GetBuildingName(t)` (38).
    pub fn building_name(&self, t: u32) -> Vec<u8> {
        self.process.catalogue_mut().building_name(t)
    }

    /// Clears `out` and appends `items`; returns the new size.
    fn assign(out: &mut Vec<u32>, items: Vec<u32>) -> u32 {
        out.clear();
        out.extend(items);
        out.len() as u32
    }

    /// `GetAllBuildingTypes(vector&)` (11): the new size (§7.1 replacement).
    pub fn all_building_types(&self, out: &mut Vec<u32>) -> u32 {
        Self::assign(out, self.process.catalogue().all_building_types())
    }

    /// `GetAllBuildingTypes(array, n&)` (12): `n := min(n, count)`, fills
    /// the first n (as many as `array` holds; none without it), returns the
    /// count.
    pub fn all_building_types_array(&self, array: Option<&mut [u32]>, n: &mut u32) -> u32 {
        let types = self.process.catalogue().all_building_types();
        let count = types.len() as u32;
        *n = (*n).min(count);
        if let Some(a) = array {
            for (slot, t) in a.iter_mut().zip(types.iter().take(*n as usize)) {
                *slot = *t;
            }
        }
        count
    }

    /// `GetAllBuildingFamilyTypes(vector&)` (13).
    pub fn all_building_family_types(&self, out: &mut Vec<u32>) -> u32 {
        Self::assign(out, self.process.catalogue().all_family_types())
    }

    /// `GetAllFoundationTypes(vector&)` (21).
    pub fn all_foundation_types(&self, out: &mut Vec<u32>) -> u32 {
        Self::assign(out, self.process.catalogue().all_foundation_types())
    }

    /// `GetAllLotRetainingWallTypes(vector&)` (27).
    pub fn all_lot_retaining_wall_types(&self, out: &mut Vec<u32>) -> u32 {
        Self::assign(out, self.process.catalogue().all_retaining_wall_types())
    }

    /// `GetDeveloperType(p, w)` (30).
    pub fn developer_type(&self, purpose: u32, wealth: u32) -> u32 {
        developer_type(purpose, wealth)
    }

    /// `GetWealth(devType)` (31).
    pub fn wealth(&self, dev_type: u32) -> u32 {
        wealth_of(dev_type)
    }

    /// `GetDeveloper(devType)` (32): no reference added.
    pub fn developer(&self, dev_type: u32) -> Option<DeveloperId> {
        self.d.borrow().developers.get(&dev_type).copied()
    }

    /// `GetDeveloper(p, w)` (33).
    pub fn developer_by(&self, purpose: u32, wealth: u32) -> Option<DeveloperId> {
        self.developer(self.developer_type(purpose, wealth))
    }

    /// `GetDevelopers()` (34).
    pub fn developers(&self) -> BTreeMap<u32, DeveloperId> {
        self.d.borrow().developers.clone()
    }

    /// The developers as [DevelopmentSim::developers] lists them (ascending
    /// type), without a copy: 
/// (type, developer) for each.
 /// must not call
    /// back into this object (the state is borrowed meanwhile).
    pub fn for_each_developer(&self, mut f: impl FnMut(u32, DeveloperId)) {
        for (&t, &dev) in &self.d.borrow().developers {
            f(t, dev);
        }
    }

    /// `GetBuildingMap()` (41).
    pub fn building_map(&self) -> Option<Grid> {
        self.grid.borrow().clone()
    }

    /// `GetBuildingMapKey(p, w)` (42).
    pub fn building_map_key(&self, purpose: u32, wealth: u32) -> u8 {
        building_map_key(purpose, wealth)
    }

    /// `GetBuildingMapKey(t)` (43): inserts a zero profile when unknown.
    pub fn building_map_key_of_type(&self, t: u32) -> u8 {
        self.process.catalogue_mut().profile_entry(t).map_key()
    }

    /// `GetSpecialBuildings()` (44).
    pub fn special_buildings(&self) -> Vec<OccupantId> {
        self.d.borrow().special_buildings.clone()
    }

    /// The special buildings as [DevelopmentSim::special_buildings] lists
    /// them, without a copy. 
 /// must not call back into this object.
    pub fn for_each_special_building(&self, mut f: impl FnMut(OccupantId)) {
        for &o in &self.d.borrow().special_buildings {
            f(o);
        }
    }

    /// `GetMaxCapacity(zone, p, w)` (45).
    pub fn max_capacity(&self, zone: u32, purpose: u32, wealth: u32) -> f32 {
        self.process.catalogue().max_capacity(zone, purpose, wealth)
    }

    /// `GetDayOfLastSuccessfulBuild(devType)` (52): 0 if absent.
    pub fn day_of_last_successful_build(&self, dev_type: u32) -> i32 {
        self.d
            .borrow()
            .last_successful_build
            .get(&dev_type)
            .map_or(0, |&d| d as i32)
    }

    /// `GetStagePopulationMultiplier()` (53): [choice] 1.0 outside the
    /// vector.
    pub fn stage_population_multiplier(&self) -> f32 {
        let i = self.host.difficulty() as usize;
        self.d.borrow().tuning.stage_multiplier.get(i).copied().unwrap_or(1.0)
    }

    /// `CheatIgnorePlumbing()` (54).
    pub fn cheat_ignore_plumbing(&self) -> bool {
        self.d.borrow().cheat_ignore_plumbing
    }

    /// `CheatIgnorePower()` (55).
    pub fn cheat_ignore_power(&self) -> bool {
        self.d.borrow().cheat_ignore_power
    }

    /// `CheatIgnoreCaps()` (56).
    pub fn cheat_ignore_caps(&self) -> bool {
        self.d.borrow().cheat_ignore_caps
    }

    /// `CheatDebugGrowth()` (57).
    pub fn cheat_debug_growth(&self) -> bool {
        self.d.borrow().cheat_debug_growth
    }

    /// `CheatCoffeeBar()` (58).
    pub fn cheat_coffee_bar(&self) -> bool {
        self.d.borrow().cheat_coffee_bar
    }

    // ---- suitability (§4.8) ------------------------------------------------------------------------

    /// `IsSuitableForConstruction(profile, reason&)` (46): the first failing
    /// check sets `reason`.
    pub fn is_suitable_for_construction(&self, p: &ConstructionProfile, reason: &mut i32) -> bool {
        let h = &self.host;
        let (x, z) = (p.x, p.z);
        if !building_fits_in_zone(p.zone, i32::from(h.zone_type(x, z))) {
            *reason = 0;
            return false;
        }
        let Some(lot) = h.lot_at(x, z) else {
            *reason = 3;
            return false;
        };
        let ignore_power = self.d.borrow().cheat_ignore_power;
        if !(ignore_power || h.powered(x, z)) {
            *reason = 5;
            return false;
        }
        if p.land_value_min == 0 && p.land_value_max == 0 {
            if !matches!(p.zone, 7..=9) && h.land_value_type(x, z) != p.wealth {
                *reason = 2;
                return false;
            }
        } else {
            let lv = h.land_value(x, z);
            if !(p.land_value_min <= lv && lv <= p.land_value_max) {
                *reason = 1;
                return false;
            }
        }
        if h.lot_state(lot) != 1 {
            *reason = 4;
            return false;
        }
        true
    }

    // ---- placing buildings (§4.9) ------------------------------------------------------------------

    /// `LocationIsOKForNewBuilding(type, x, z)` (5).
    pub fn location_is_ok_for_new_building(&self, t: u32, x: f32, z: f32) -> bool {
        if !self.is_building_type_valid(t) {
            return false;
        }
        let (cx, cz) = self.host.position_to_cell(x, z);
        !self.host.building_occupant_at_cell(cx, cz)
    }

    /// `AddNewBuildingOccupant(type, pos, orientation)` (7).
    pub fn add_new_building_occupant(&self, t: u32, pos: Vec3, orientation: i32) -> Option<OccupantId> {
        if !self.is_building_type_valid(t) || !self.location_is_ok_for_new_building(t, pos[0], pos[2]) {
            return None;
        }
        let h = &self.host;
        // DG5 (§4.9.1): the host's occupant comes from `CreateInstance(0xA9BD882D)`
        // with the caller's reference already counted, so it is not AddRef'd
        // again; the loader reports success on every path, so a building whose
        // exemplar does not load is still inserted, with the constructor's empty
        // box.
        let occ = h.create_building_occupant()?;
        h.set_building_type(occ, t);
        self.load_building_occupant_properties(occ, t);
        let (mut min, mut max) = h.occupant_bounding_box(occ);
        if orientation == 1 || orientation == 3 {
            min.swap(0, 2);
            max.swap(0, 2);
        }
        h.set_occupant_bounding_box(occ, min, max);
        let mut p = pos;
        if p[1] < 0.0 {
            p[1] = h.area_altitude([min[0], min[2], max[0], max[2]]);
        }
        h.set_occupant_position(occ, p);
        h.set_occupant_orientation(occ, orientation);
        h.init_occupant(occ);
        let inserted = self.insert_building_occupant(occ);
        h.release(occ);
        inserted.then_some(occ)
    }

    /// `AddNewBuildingOccupant(type, cx, cz, orientation)` (6).
    pub fn add_new_building_occupant_at_cell(&self, t: u32, cx: i32, cz: i32, orientation: i32) -> Option<OccupantId> {
        let (x, z) = self.host.cell_center(cx, cz);
        let y = self.host.altitude(x, z);
        self.add_new_building_occupant(t, [x, y, z], orientation)
    }

    /// `PlopBuilding(profile)` (47).
    pub fn plop_building(&self, p: &ConstructionProfile) -> Option<OccupantId> {
        self.add_new_building_occupant_at_cell(p.building_type, p.x, p.z, p.orientation)
    }

    /// `AddNewBuildingOccupant(occupant)` (8) and
    /// `AddNewFoundationOccupant(occupant)` (24).
    pub fn insert_building_occupant(&self, occ: OccupantId) -> bool {
        self.host.has_occupant_manager() && self.host.insert_occupant(occ)
    }

    /// `DemolishFoundationOccupant(occupant, u)` (25).
    pub fn demolish_foundation_occupant(&self, occ: OccupantId) -> bool {
        self.host.remove_occupant(occ)
    }

    /// `GetCostOfOccupantDemolition(building, u)` (10).
    pub fn cost_of_building_demolition(&self, occ: OccupantId) -> i64 {
        self.host
            .occupant_property_i64(occ, super::prop::DEMOLITION_COST)
            .unwrap_or(0)
    }

    /// `GetCostOfOccupantDemolition(foundation, u)` (9): 0.
    pub fn cost_of_foundation_demolition(&self, _occ: OccupantId) -> i64 {
        0
    }

    /// `LoadBuildingOccupantProperties(occ, type)` (`0x00698490`, §4.9.1,
    /// DG5): the type's exemplar becomes the occupant's default exemplar and
    /// the occupant size `0x27812810` sets the box `(0, 0, 0)`–size. True on
    /// every path: an unknown type or an exemplar that does not load leaves
    /// the occupant as it was.
    pub fn load_building_occupant_properties(&self, occ: OccupantId, t: u32) -> bool {
        let h = &self.host;
        let t = if t == 0 { h.building_type(occ) } else { t };
        let key = self.process.catalogue().building_key(t);
        let Some(key) = key else { return true };
        // [choice] (DG37, re-answers11) An occupant with no exemplar property
        // holder crashes the original (no null test); here it is a success
        // with an empty box. Unreachable with the game's classes.
        if !h.set_default_exemplar(occ, key) {
            return true;
        }
        if let Some(v) = h.occupant_property_f32s(occ, prop::OCCUPANT_SIZE) {
            // [choice] (DG32, re-answers11) The original passes the value
            // field to SetBoundingBox as three floats with no type or count
            // check (garbage past a short array, a crash for a scalar, an
            // integer array's bits as floats); a property with fewer values
            // gives 0 for the missing ones here. Every shipped building
            // exemplar holds a Float32 array of 3.
            let mut size = [0.0f32; 3];
            for (slot, x) in size.iter_mut().zip(v) {
                *slot = x;
            }
            h.set_occupant_bounding_box(occ, [0.0; 3], size);
        }
        true
    }

    /// `LoadFoundationProperties(occ, type)` (26, §4.9.2, DG6): the type's
    /// exemplar (foundation key map) becomes the occupant's default
    /// exemplar, and property `0x88FCC3E3`, when present, its maximum height.
    /// False for a type that is not in the map or an exemplar that does not
    /// load.
    pub fn load_foundation_properties(&self, occ: OccupantId, t: u32) -> bool {
        let h = &self.host;
        if occ == 0 {
            return false; // [choice] a null occupant (the original does not test it)
        }
        let t = if t == 0 { h.foundation_type(occ) } else { t };
        let key = self.process.catalogue().foundation_key(t);
        let Some(key) = key else { return false };
        if !h.set_default_exemplar(occ, key) {
            return false;
        }
        if let Some(v) = h.occupant_property_f32s(occ, prop::FOUNDATION_MAX_HEIGHT) {
            // [spec question, choice] The original reads the value with the
            // variant's float32 getter; a present property without values
            // gives 0 here.
            h.set_foundation_max_height(occ, v.first().copied().unwrap_or(0.0));
        }
        true
    }

    /// `LoadLotRetainingWallProperties(occ, type)` (29, §4.9.2, DG6): as
    /// slot 26 with the retaining-wall key map and no property read.
    pub fn load_retaining_wall_properties(&self, occ: OccupantId, t: u32) -> bool {
        let h = &self.host;
        if occ == 0 {
            return false; // [choice] as slot 26
        }
        let t = if t == 0 { h.retaining_wall_type(occ) } else { t };
        let key = self.process.catalogue().retaining_wall_key(t);
        let Some(key) = key else { return false };
        h.set_default_exemplar(occ, key)
    }

    /// `AddNewFoundationOccupant(type, usage, pos, sizeX, sizeZ, orientation,
    /// base)` (23, `0x00695AF0`, §4.9.2, DG6; lot-developer.md §17.5.3). The
    /// foundation's position is its top centre; `pos[1] < 0` is replaced by
    /// the highest altitude over its cells, written into `pos`. Returns the
    /// insert's result; false only when the occupant cannot be created or
    /// the type's properties do not load.
    #[allow(clippy::too_many_arguments)]
    pub fn add_new_foundation_occupant(
        &self,
        t: u32,
        usage: i32,
        pos: &mut Vec3,
        size_x: f32,
        size_z: f32,
        orientation: i32,
        base: f32,
    ) -> bool {
        let h = self.host.clone();
        let Some(occ) = h.create_foundation_occupant() else {
            return false;
        };
        h.set_foundation_type(occ, t);
        if !self.load_foundation_properties(occ, t) {
            h.release(occ);
            return false;
        }
        let cells = self.foundation_cells(pos, size_x, size_z);
        // Step 4: strictly below 0 (NaN does not count).
        if pos[1] < 0.0 {
            pos[1] = h.area_altitude_cells(cells, false, true);
        }
        // Step 5: a NaN base counts as >= 0.
        let b = if base < 0.0 {
            h.area_altitude_cells(cells, true, true)
        } else {
            base
        };
        let mut height = r24(f64::from(pos[1]) - f64::from(b));
        if height < 0.5 {
            height = 0.5;
        }
        h.set_occupant_position(occ, *pos);
        h.set_foundation_size(occ, [size_x, height, size_z]);
        h.set_foundation_usage(occ, usage);
        h.set_foundation_orientation(occ, orientation);
        h.init_occupant(occ);
        let ok = self.insert_building_occupant(occ);
        h.release(occ);
        ok
    }

    /// The cells under a foundation (§4.9.2 step 3, `0x00695610`): the
    /// footprint inset by half a metre (float32 stores), each corner × 0.0625
    /// truncated toward zero, the low corner raised to 0 and the high corner
    /// lowered to the cell counts cached by `Init`; no other clamp.
    pub fn foundation_cells(&self, pos: &Vec3, size_x: f32, size_z: f32) -> CellRect {
        let hx = r24(f64::from(size_x) * 0.5);
        let hz = r24(f64::from(size_z) * 0.5);
        let x0 = r24(f64::from(r24(f64::from(pos[0]) - f64::from(hx))) + 0.5);
        let z0 = r24(f64::from(r24(f64::from(pos[2]) - f64::from(hz))) + 0.5);
        let x1 = r24(f64::from(r24(f64::from(pos[0]) + f64::from(hx))) - 0.5);
        let z1 = r24(f64::from(r24(f64::from(pos[2]) + f64::from(hz))) - 0.5);
        let cell = |v: f32| ftol32(f64::from(r24(f64::from(v) * 0.0625)));
        let (cx, cz) = self.d.borrow().cell_counts;
        (
            cell(x0).max(0),
            cell(z0).max(0),
            cell(x1).min(cx - 1),
            cell(z1).min(cz - 1),
        )
    }

    /// The PlaceBuilding command (§4.12): `AddNewBuildingOccupant(type,
    /// {a, b, −1}, 2)` *[quirk]*.
    fn place_building_command(&self, params: &[CommandParam]) {
        let [p0, p1, p2, ..] = params else { return };
        self.add_new_building_occupant(p0.as_u32, [p1.as_f32, p2.as_f32, -1.0], 2);
    }

    // ---- building map and highest stage (§4.10) -----------------------------------------------------

    /// `DoBuildingChanged(lot, wealth, built)` (§4.10).
    pub fn do_building_changed(&self, lot: LotId, wealth: u32, built: bool) {
        let h = self.host.clone();
        let t = h.lot_building_type(lot);
        let p = self.process.catalogue_mut().profile_entry(t).clone();
        let key = if built {
            building_map_key(u32::from(p.purpose), wealth)
        } else {
            0
        };
        let rect = h.lot_bounding_rect(lot);
        if let Some(g) = self.grid.borrow().clone() {
            let geo = g.geometry();
            let x1 = rect.0.max(0);
            let z1 = rect.1.max(0);
            let x2 = rect.2.min(geo.count_x.saturating_sub(1));
            let z2 = rect.3.min(geo.count_z.saturating_sub(1));
            if x1 <= x2 && z1 <= z2 {
                for x in x1..=x2 {
                    for z in z1..=z2 {
                        g.set(x, z, key);
                    }
                }
            }
        }
        if !(built && p.purpose != 0 && u32::from(p.wealth) == wealth) {
            return;
        }
        let (Some(stage), Some(b)) = (h.lot_growth_stage(lot), h.lot_building(lot)) else {
            return;
        };
        let dt = developer_type(u32::from(p.purpose), u32::from(p.wealth));
        let raised = {
            let mut d = self.d.borrow_mut();
            let e = d.highest_stage.entry(dt).or_insert(0);
            if stage > *e {
                *e = stage;
                true
            } else {
                false
            }
        };
        if raised {
            let pos = h.occupant_position(b);
            let (cw, cz) = h.cell_widths();
            let subject = AdviceSubject {
                x: r24(f64::from(pos[0]) / f64::from(cw)),
                z: r24(f64::from(pos[2]) / f64::from(cz)),
                lot,
                data1: (u32::from(stage) << 24) | dt,
            };
            // DG16 (§4.10): the original fills a subject and pushes the event
            // with a null one; the host pushes null and may skip the subject.
            h.push_advisor_event(CITY_PLANNING_ADVISOR, HIGHEST_STAGE_EVENT, &subject);
        }
    }

    // ---- terrain changes (§4.11) -------------------------------------------------------------------

    /// `DoTerrainChanged(rect, kind)` (§4.11).
    pub fn do_terrain_changed(&self, rect: CellRect, kind: u32) {
        if rect.0 > rect.2 || rect.1 > rect.3 {
            return;
        }
        let h = self.host.clone();
        let lots = h.lots_in_cell_rect(rect);
        if !(1..=3).contains(&kind) {
            for lot in lots {
                h.update_lot_elements(lot, kind);
            }
            return;
        }
        let (cw, _) = h.cell_widths();
        let inv = r24(1.0 / f64::from(cw));
        let mut queued = Vec::new();
        for lot in lots {
            let c = h.lot_center(lot);
            let d = r24(f64::from(h.altitude(c[0], c[2])) - f64::from(c[1]));
            let a = r24(f64::from(r24(f64::from(h.lot_developer_slope(lot)) * f64::from(inv))).atan());
            let b = r24(f64::from(r24(f64::from(h.lot_slope(lot)) * f64::from(inv))).atan());
            // NaN counts as inside.
            let inside = d.is_nan() || (-1.0..=2.0).contains(&d);
            if inside && r24(f64::from(a) - f64::from(b)) <= MAX_SLOPE_CHANGE {
                h.update_lot_elements(lot, kind);
            } else if h.lot_demolishable(lot) {
                queued.push(lot);
            } else {
                h.dezone(h.lot_bounding_rect(lot), kind == 1 || kind == 2);
            }
        }
        let cost = h.demolish_lots(&queued);
        if (kind == 1 || kind == 2) && cost > 0 {
            h.set_total_funds(h.total_funds().wrapping_sub(cost));
        }
    }
}

// ---- persistence (§6) --------------------------------------------------------------------------------

impl SaveObject for DevelopmentSim {
    fn class_id(&self) -> u32 {
        CLSID_BDS
    }

    fn write(&self, out: &mut dyn OutStream) -> bool {
        if out.error() != 0 {
            return false;
        }
        let h = &self.host;
        let d = self.d.borrow().clone();
        out.put_u16(RECORD_VERSION);
        let grid = self.grid.borrow().as_ref().map_or(RefValue::Null, |g| g.save_ref());
        out.put_ref(&grid);
        let node = self.node.borrow().clone().map_or(RefValue::Null, |n| n.save_ref());
        out.put_ref(&node);
        out.put_u32(d.developers.len() as u32);
        for (&t, &dev) in &d.developers {
            out.put_u32(t);
            out.put_ref(&self.devs.developer_ref(dev));
        }
        out.put_u32(d.special_buildings.len() as u32);
        for &o in &d.special_buildings {
            out.put_ref(&h.object_ref(o));
        }
        out.put_u32(d.highest_stage.len() as u32);
        for (&t, &s) in &d.highest_stage {
            out.put_u32(t);
            out.put_u8(s);
        }
        let marks = self.process.marks();
        out.put_u32(marks.len() as u32);
        for (t, cells) in &marks {
            out.put_u32(*t);
            out.put_u32(cells.len() as u32);
            for &(x, z) in cells {
                out.put_i32(x);
                out.put_i32(z);
            }
        }
        out.put_u32(d.planned_growth.len() as u32);
        for (k, &day) in &d.planned_growth {
            out.put_ref(&self.devs.developer_ref(k.developer));
            out.put_i32(k.x);
            out.put_i32(k.z);
            out.put_u8(k.stage);
            out.put_u32(day);
        }
        out.put_bool(d.cheat_ignore_plumbing);
        out.put_bool(d.cheat_ignore_power);
        out.put_bool(d.cheat_ignore_caps);
        out.error() == 0
    }

    fn read(&self, inp: &mut dyn InStream) -> bool {
        if inp.error() != 0 {
            return false;
        }
        if inp.get_u16() != Some(RECORD_VERSION) {
            return false;
        }
        let h = self.host.clone();
        self.d.borrow_mut().loaded_from_save = true;
        if let Some(r) = inp.get_ref_as(IID_SIM_GRID) {
            *self.grid.borrow_mut() = GridProvider::<u8>::resolve_grid(&*h, &r);
        }
        if let Some(r) = inp.get_ref_as(IID_DEMAND) {
            *self.node.borrow_mut() = h.resolve_construction_node(&r);
        }
        let n = inp.get_u32().unwrap_or(0);
        for _ in 0..n {
            let Some(t) = inp.get_u32() else { break };
            let Some(r) = inp.get_ref_as(IID_GROWTH_DEVELOPER) else {
                break;
            };
            if let Some(dev) = self.devs.resolve_developer(&r) {
                let old = self.d.borrow_mut().developers.insert(t, dev);
                // The map holds one reference per slot: the one just read.
                if let Some(old) = old {
                    self.devs.developer_release(old);
                }
            }
        }
        let n = inp.get_u32().unwrap_or(0);
        for _ in 0..n {
            let Some(r) = inp.get_ref_as(IID_OCCUPANT) else { break };
            if let Some(o) = h.resolve_occupant(&r) {
                self.d.borrow_mut().special_buildings.push(o);
            }
        }
        let n = inp.get_u32().unwrap_or(0);
        for _ in 0..n {
            let (Some(t), Some(s)) = (inp.get_u32(), inp.get_u8()) else {
                break;
            };
            self.d.borrow_mut().highest_stage.insert(t, s);
        }
        let n = inp.get_u32().unwrap_or(0);
        'marks: for _ in 0..n {
            let (Some(t), Some(cells)) = (inp.get_u32(), inp.get_u32()) else {
                break;
            };
            self.process.touch_marks(t);
            for _ in 0..cells {
                let (Some(x), Some(z)) = (inp.get_i32(), inp.get_i32()) else {
                    break 'marks;
                };
                self.process.mark(t, (x, z));
            }
        }
        let n = inp.get_u32().unwrap_or(0);
        for _ in 0..n {
            let Some(r) = inp.get_ref_as(IID_GROWTH_DEVELOPER) else {
                break;
            };
            let (Some(x), Some(z), Some(stage), Some(day)) =
                (inp.get_i32(), inp.get_i32(), inp.get_u8(), inp.get_u32())
            else {
                break;
            };
            if let Some(dev) = self.devs.resolve_developer(&r) {
                let key = PlannedGrowth {
                    developer: dev,
                    x,
                    z,
                    stage,
                };
                self.d.borrow_mut().planned_growth.insert(key, day);
                self.devs.developer_release(dev);
            }
        }
        let mut d = self.d.borrow_mut();
        let d = &mut *d;
        for field in [
            &mut d.cheat_ignore_plumbing,
            &mut d.cheat_ignore_power,
            &mut d.cheat_ignore_caps,
        ] {
            if let Some(b) = inp.get_bool() {
                *field = b;
            }
        }
        inp.error() == 0
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }

    fn host_token(&self) -> u64 {
        self.token.get()
    }
}
