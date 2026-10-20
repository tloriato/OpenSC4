// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! `cISC4PollutionSimulator`: state (§3), lifecycle (§4.1), building
//! changes (§4.2), disaster radiation (§4.3), messages (§5) and the
//! interface slots (§8.1).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Mutex, MutexGuard};

use crate::curve::ResponseCurve;
use crate::grid::{add_radius_effect, CellIterator, TractGrid, MODE_TABLE};
use crate::rng::{CrtRand, RzRandomSuper, SeedSource};
use crate::seeding::SharedSuper;
use crate::variant::{Elem, Variant};
use crate::x87::{ftol, ftol32, rhe};

use super::tuning::{curve_of, Tuning};
use super::{budget, history, msg, prop, PollutionHost, AGENT_FLAGS, AGENT_LEVELS, AGENT_NAME, LANDFILL_FLAG};
use super::{VIEW_SURFACE, ZONE_LANDFILL};

/// The eight grids (§2), indexed by the `G_*` constants.
pub type Grids = [Option<Rc<dyn TractGrid<i16>>>; 8];

pub const G_AIR_SRC: usize = 0;
pub const G_WATER_SRC: usize = 1;
pub const G_GARBAGE_SRC: usize = 2;
pub const G_RAD_SRC: usize = 3;
pub const G_AIR_VAL: usize = 4;
pub const G_WATER_VAL: usize = 5;
pub const G_GARBAGE_VAL: usize = 6;
pub const G_RAD_VAL: usize = 7;

/// Instance IDs in save order: four sources, then four values (§2).
pub const GRID_INSTANCES: [u32; 8] = [
    0x89D5_BBF0,
    0x89D5_BBF1,
    0x89D5_BBF2,
    0x89D5_BBF3,
    0x49D5_BC40,
    0x49D5_BC41,
    0x49D5_BC42,
    0x49D5_BC43,
];
/// Tract sizes: 2, except the garbage value grid (1).
pub const GRID_TRACT_SIZES: [i32; 8] = [2, 2, 2, 2, 2, 2, 1, 2];

/// Occupant lists (§3.2), in save order.
pub const L_WTP: usize = 0;
pub const L_INC: usize = 1;
pub const L_WTE: usize = 2;
pub const L_REC: usize = 3;

/// Effect kinds by array index (water, air, radiation, garbage).
pub const EFFECT_KINDS: [u32; 4] = [1, 2, 3, 4];

/// [choice] Cell counts used for loops over cells are clamped to this.
pub(crate) const MAX_CELLS: i32 = 8192;
/// [choice] Effect block counts per axis are clamped to this.
pub(crate) const MAX_BLOCKS: i32 = 1024;

/// One month's garbage counters (§3.1).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Counters {
    pub produced: i64,
    pub imported: i64,
    pub exported: i64,
    pub recycled: f64,
    pub incinerated: i64,
    pub wte: i64,
    pub landfilled: i64,
}

/// `(cellX, cellZ, value)` of a worst 8×8 block (§4.10).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorstTract {
    pub x: u32,
    pub z: u32,
    pub value: i32,
}

/// The day's remaining disposal capacities (§4.4.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Remaining {
    pub wtp: u32,
    pub inc: u32,
    pub wte: u32,
}

/// Every field of §3 (persisted or not), the grids, lists and effects.
#[derive(Clone)]
pub struct PollutionState {
    pub initialized: bool,
    pub loaded: bool,
    pub grids: Grids,
    pub cell_x: u32,
    pub cell_z: u32,
    pub tracts_per_day: u32,
    pub iterator: CellIterator,
    pub effect_iterator: CellIterator,
    pub recycling_count: u32,
    pub avg_air: i32,
    pub avg_water: i32,
    pub avg_garbage: u16,
    pub air_sum: i32,
    pub water_sum_treated: i32,
    pub water_sum: i32,
    pub treated_sum: i32,
    pub last_water_sum: i32,
    pub last_treated_sum: i32,
    pub developed_tracts: u32,
    pub radiation_sum: u32,
    pub incinerator_capacity: i32,
    pub wte_capacity: i32,
    pub recycling_effect: f32,
    pub garbage_uncollected: bool,
    pub total_uncollected: u32,
    /// This month's counters.
    pub month: Counters,
    pub last_month: Counters,
    pub export_capacity: i32,
    pub decomposed: u32,
    pub decomposed_last_month: u32,
    pub active_landfill_cells: u32,
    pub unused64: i64,
    pub last_month_cost: i64,
    pub dirty_radiation: bool,
    pub dirty_air: bool,
    pub dirty_water: bool,
    pub dirty_garbage: bool,
    pub effects_visible: bool,
    pub worst_air: WorstTract,
    pub worst_water: WorstTract,
    pub worst_garbage: WorstTract,
    pub efficiency: f32,
    pub garbage_supply: i32,
    pub garbage_capacity: i32,
    /// The sanitation department (host token).
    pub department: Option<u64>,
    /// Water treatment, incinerators, waste-to-energy, recycling (tokens).
    pub lists: [Vec<u64>; 4],
    /// Water, air, radiation, garbage effects per 8×8 block.
    pub effects: [Vec<Option<u64>>; 4],
    pub blocks_x: i32,
    pub blocks_z: i32,
}

impl Default for PollutionState {
    /// A new object (§3.1 "Initial").
    fn default() -> Self {
        PollutionState {
            initialized: false,
            loaded: false,
            grids: Default::default(),
            cell_x: 0,
            cell_z: 0,
            tracts_per_day: 0,
            iterator: CellIterator::constructed(),
            effect_iterator: CellIterator::new(0, 0, -1, -1),
            recycling_count: 0,
            avg_air: 0,
            avg_water: 0,
            avg_garbage: 0,
            air_sum: 0,
            water_sum_treated: 0,
            water_sum: 0,
            treated_sum: 0,
            last_water_sum: 0,
            last_treated_sum: 0,
            developed_tracts: 0,
            radiation_sum: 0,
            incinerator_capacity: 0,
            wte_capacity: 0,
            recycling_effect: 0.0,
            garbage_uncollected: false,
            total_uncollected: 0,
            month: Counters::default(),
            last_month: Counters::default(),
            export_capacity: 0,
            decomposed: 0,
            decomposed_last_month: 0,
            active_landfill_cells: 0,
            unused64: 0,
            last_month_cost: 0,
            dirty_radiation: false,
            dirty_air: false,
            dirty_water: false,
            dirty_garbage: false,
            effects_visible: true,
            worst_air: WorstTract::default(),
            worst_water: WorstTract::default(),
            worst_garbage: WorstTract::default(),
            efficiency: 0.0,
            garbage_supply: 0,
            garbage_capacity: 0,
            department: None,
            lists: Default::default(),
            effects: Default::default(),
            blocks_x: 0,
            blocks_z: 0,
        }
    }
}

impl PollutionState {
    /// Value of grid `g` at a cell (0 without the grid or out of range).
    pub fn cell(&self, g: usize, x: i32, z: i32) -> i16 {
        self.grids
            .get(g)
            .and_then(|o| o.as_ref())
            .map_or(0, |grid| grid.get_cell_value(x, z))
    }

    /// Writes the tract containing a cell (ignored without the grid).
    pub fn set_cell(&self, g: usize, x: i32, z: i32, v: i16) {
        if let Some(grid) = self.grids.get(g).and_then(|o| o.as_ref()) {
            let (tx, tz) = grid.geometry().cell_to_tract(x, z);
            grid.set(tx, tz, v);
        }
    }

    /// Tract counts of grid `g` (0, 0 without it).
    pub fn tract_counts(&self, g: usize) -> (i32, i32) {
        self.grids.get(g).and_then(|o| o.as_ref()).map_or((0, 0), |grid| {
            let geo = grid.geometry();
            (geo.count_x.max(0), geo.count_z.max(0))
        })
    }

    /// Slot 7.
    pub fn air_value(&self, x: i32, z: i32) -> i32 {
        i32::from(self.cell(G_AIR_VAL, x, z))
    }
    /// Slot 8.
    pub fn water_value(&self, x: i32, z: i32) -> i32 {
        i32::from(self.cell(G_WATER_VAL, x, z))
    }
    /// Slot 9: `signExtend16(raw) & 0xFFFF7FFF` (landfill cells negative).
    pub fn garbage_value(&self, x: i32, z: i32) -> i32 {
        i32::from(self.cell(G_GARBAGE_VAL, x, z)) & !0x8000
    }
    /// Slot 26.
    pub fn set_air_value(&mut self, x: i32, z: i32, v: i16) {
        self.set_cell(G_AIR_VAL, x, z, v);
        self.dirty_air = true;
    }
    /// Slot 27.
    pub fn set_water_value(&mut self, x: i32, z: i32, v: i16) {
        self.set_cell(G_WATER_VAL, x, z, v);
        self.dirty_water = true;
    }
    /// Slot 28: keeps the landfill bit; `v` is not masked.
    pub fn set_garbage_value(&mut self, x: i32, z: i32, v: i16) {
        let raw = self.cell(G_GARBAGE_VAL, x, z) as u16;
        self.set_cell(G_GARBAGE_VAL, x, z, ((raw & LANDFILL_FLAG) | v as u16) as i16);
        self.dirty_garbage = true;
    }
    /// Slot 25: the radiation source tract := 1 or 0.
    pub fn set_is_radioactive(&mut self, x: i32, z: i32, on: bool) {
        self.set_cell(G_RAD_SRC, x, z, i16::from(on));
        self.dirty_radiation = true;
    }
    /// Slot 29 (§8.1, PG11) [compat]: with `s` the supply on entry, the delta
    /// is `min(n, 0x7FFFFFFF − s)` for n > 0, 0 for n = 0, and `n` if `s + n ≥
    /// 0` else `−s` for n < 0; the supply becomes `s + delta`, then 0 if
    /// negative (only an already negative `s`, from a damaged save). Returns
    /// the delta taken **before** that last clamp (EAX; no caller reads it).
    /// `0x7FFFFFFF − s` and `s + n` are exact (a negative `s` with n > 0
    /// gives delta `n`). PG15 (answered; the bracket matches): the original
    /// forms `s + n` in int32 and takes `0x7FFFFFFF − s` only when that sum
    /// wrapped, which needs s > 0, where the subtraction is exact.
    /// [choice] (pollution.md §8.1 quirk, PG16 answered: acceptable): with a
    /// negative `s` (a damaged save only) and n < 0, the original's `s + n`
    /// wraps to a positive value only when `s` lies within `|n|` of `−2^31`;
    /// FreeCity4 clamps (delta `−s`, supply 0), as the original does for every
    /// other negative supply.
    pub fn add_to_garbage_supply(&mut self, n: i32) -> i32 {
        let s = i64::from(self.garbage_supply);
        let n = i64::from(n);
        let delta = match n.cmp(&0) {
            std::cmp::Ordering::Greater => n.min(i64::from(i32::MAX) - s),
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Less => {
                if s + n >= 0 {
                    n
                } else {
                    -s
                }
            }
        };
        // s + delta always fits in an i32 (0 when a negative n overshoots).
        let new = s + delta;
        self.garbage_supply = if new < 0 { 0 } else { new as i32 };
        delta as i32
    }

    /// Zeroes every statistic and counter of §3.1 (new-city `Init`).
    fn zero_statistics(&mut self) {
        let keep = PollutionState {
            initialized: self.initialized,
            loaded: self.loaded,
            grids: std::mem::take(&mut self.grids),
            cell_x: self.cell_x,
            cell_z: self.cell_z,
            tracts_per_day: self.tracts_per_day,
            iterator: std::mem::take(&mut self.iterator),
            effect_iterator: std::mem::take(&mut self.effect_iterator),
            effects_visible: self.effects_visible,
            worst_air: self.worst_air,
            worst_water: self.worst_water,
            worst_garbage: self.worst_garbage,
            dirty_radiation: self.dirty_radiation,
            dirty_air: self.dirty_air,
            dirty_water: self.dirty_water,
            dirty_garbage: self.dirty_garbage,
            department: self.department,
            lists: std::mem::take(&mut self.lists),
            effects: std::mem::take(&mut self.effects),
            blocks_x: self.blocks_x,
            blocks_z: self.blocks_z,
            ..PollutionState::default()
        };
        *self = keep;
    }
}

// ---- occupant property readers ----------------------------------------------------------------------

fn numbers(v: &Variant) -> Vec<Elem> {
    match v {
        Variant::Array { elem, .. } if *elem == crate::variant::mem::RZ_CHAR || *elem == 0 => Vec::new(),
        _ => v.items(),
    }
}

fn elem_f32(e: &Elem) -> f32 {
    match *e {
        Elem::F32(x) => x,
        other => other.as_f64() as f32,
    }
}

pub(crate) fn p_u32(h: &dyn PollutionHost, occ: u64, id: u32) -> Option<u32> {
    let v = h.occupant_property(occ, id)?;
    numbers(&v).first().map(|e| e.as_i64() as u32)
}

pub(crate) fn p_f32(h: &dyn PollutionHost, occ: u64, id: u32) -> Option<f32> {
    let v = h.occupant_property(occ, id)?;
    numbers(&v).first().map(elem_f32)
}

pub(crate) fn p_i64(h: &dyn PollutionHost, occ: u64, id: u32) -> Option<i64> {
    let v = h.occupant_property(occ, id)?;
    numbers(&v).first().map(Elem::as_i64)
}

fn p_i32s(h: &dyn PollutionHost, occ: u64, id: u32) -> Option<Vec<i32>> {
    let v = h.occupant_property(occ, id)?;
    Some(numbers(&v).iter().map(|e| e.as_i64() as i32).collect())
}

fn p_f32s(h: &dyn PollutionHost, occ: u64, id: u32) -> Option<Vec<f32>> {
    let v = h.occupant_property(occ, id)?;
    Some(numbers(&v).iter().map(elem_f32).collect())
}

/// A per-occupant curve property (foundations §4.1, [`crate::props::curve_of_variant`]):
/// absent for a missing, non-numeric or single-value property; a present
/// property without points is an empty curve, which evaluates to 0.
pub(crate) fn p_curve(h: &dyn PollutionHost, occ: u64, id: u32) -> Option<ResponseCurve> {
    crate::props::curve_of_variant(&h.occupant_property(occ, id)?)
}

/// `float × int`, `int × float`: the integer enters unrounded and the
/// product is rounded to `f32` once (foundations §18.3).
pub(crate) fn fmul_i(i: f64, f: f32) -> f32 {
    (i * f64::from(f)) as f32
}

/// `int / int` evaluated in floating point: one `f32` rounding (§18.3).
pub(crate) fn fdiv_ii(a: f64, b: f64) -> f32 {
    (a / b) as f32
}

/// `float + int`: one `f32` rounding (§18.3).
pub(crate) fn fadd_i(f: f32, i: f64) -> f32 {
    (f64::from(f) + i) as f32
}

pub(crate) fn u32_prop(v: u32) -> Variant {
    Variant::Scalar(Elem::U32(v))
}

/// The pollution simulator.
///
/// # Threads
///
/// The game calls the interface's getters from a second thread while the
/// main thread is inside `OnTick` (FC4's simulator notes). Everything those
/// getters read lives behind a `Mutex`: the state ([`PollutionState`]) and
/// the tuning. The lock is held only for plain data work — never across a
/// host callback that could call back in — so a second-thread getter waits
/// at most for one such section. Long main-thread work (the day, the month,
/// the effects) runs on a copy of the state that is stored back at the end,
/// so getters keep seeing the previous consistent state meanwhile; the grids
/// themselves are shared objects written in place, as in the original.
///
/// Getters never clone the grid handles (an `Rc` count is not atomic); they
/// borrow them under the lock. The `rand()` generators are main-thread only.
pub struct PollutionSim {
    pub(super) host: Rc<dyn PollutionHost>,
    pub(super) token: Cell<u64>,
    pub(super) s: Mutex<PollutionState>,
    pub(super) tuning: Mutex<Tuning>,
    pub(super) crt: RefCell<CrtRand>,
    /// The visual-effect `cRZRandomSuper`, shared process-wide when built
    /// by [`PollutionSim::process_wide`] (RN3).
    pub(super) fx: SharedSuper,
    /// The shadow's replacement for the next tract-iterator shuffle (main
    /// thread only; see [`PollutionSim::set_shuffle_override`]).
    pub(super) shuffle_override: RefCell<Option<Vec<u16>>>,
    /// Tract-iterator resets at the end of a cycle since creation.
    pub(super) cycle_resets: Cell<u32>,
}

impl PollutionSim {
    /// A new simulator with constructor values (§3); `fx_seed` seeds the
    /// visual-effect `cRZRandomSuper` (foundations §6.2, §6.3).
    pub fn new(host: Rc<dyn PollutionHost>, token: u64, fx_seed: u32, seeds: &dyn SeedSource) -> Rc<PollutionSim> {
        PollutionSim::with_fx(host, token, RzRandomSuper::new(fx_seed, seeds))
    }

    /// A new simulator whose visual-effect generator, of its own, is seeded
    /// from the session seed (foundations §6.3 *[choice]*): the file-local
    /// `cRZRandomSuper` takes `derive(0x4C09C4D3)`.
    pub fn from_session(host: Rc<dyn PollutionHost>, token: u64, session: u32) -> Rc<PollutionSim> {
        let key = crate::seeding::keys::POLLUTION_EFFECTS;
        let fx = RzRandomSuper::new(
            crate::seeding::seed(session, key),
            &crate::seeding::clocks(session, key),
        );
        PollutionSim::with_fx(host, token, fx)
    }

    /// A new simulator drawing its visual effects from the process-wide
    /// generator (RN3): the file-local `cRZRandomSuper` is seeded once per
    /// process with `derive(S, 0x4C09C4D3)` of the first session and keeps
    /// its stream across city loads.
    pub fn process_wide(host: Rc<dyn PollutionHost>, token: u64, session: u32) -> Rc<PollutionSim> {
        let fx = crate::seeding::process_super(session, crate::seeding::keys::POLLUTION_EFFECTS);
        PollutionSim::with_shared_fx(host, token, fx)
    }

    fn with_fx(host: Rc<dyn PollutionHost>, token: u64, fx: RzRandomSuper) -> Rc<PollutionSim> {
        PollutionSim::with_shared_fx(host, token, Rc::new(RefCell::new(fx)))
    }

    /// A new simulator drawing its visual effects from `fx`.
    pub fn with_shared_fx(host: Rc<dyn PollutionHost>, token: u64, fx: SharedSuper) -> Rc<PollutionSim> {
        Rc::new(PollutionSim {
            host,
            token: Cell::new(token),
            s: Mutex::new(PollutionState::default()),
            tuning: Mutex::new(Tuning::default()),
            crt: RefCell::new(CrtRand::default()),
            fx,
            shuffle_override: RefCell::new(None),
            cycle_resets: Cell::new(0),
        })
    }

    pub fn token(&self) -> u64 {
        self.token.get()
    }

    pub fn set_token(&self, token: u64) {
        self.token.set(token);
    }

    pub fn host(&self) -> &Rc<dyn PollutionHost> {
        &self.host
    }

    /// Locks the state for one section. A poisoned lock is used anyway: the
    /// state is still readable, and a release build aborts on the panic that
    /// poisoned it.
    pub(super) fn gate(&self) -> MutexGuard<'_, PollutionState> {
        self.s.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(super) fn tuning_gate(&self) -> MutexGuard<'_, Tuning> {
        self.tuning.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A copy of the state (main thread: it clones the grid handles).
    pub fn snapshot(&self) -> PollutionState {
        self.gate().clone()
    }

    /// Runs `f` on the state in place, under the lock. `f` must not call the
    /// host except for grid reads (any thread).
    pub fn peek<R>(&self, f: impl FnOnce(&PollutionState) -> R) -> R {
        f(&self.gate())
    }

    /// Replaces the shuffle of the next tract-iterator reset at the end of a
    /// cycle (§4.4.4 step 11) with this table, used once (the shadow replays
    /// the original's order; it must have the iterator's length or it is
    /// ignored). `None` clears it.
    pub fn set_shuffle_override(&self, table: Option<Vec<u16>>) {
        *self.shuffle_override.borrow_mut() = table;
    }

    /// Tract-iterator resets at the end of a cycle since creation.
    pub fn cycle_resets(&self) -> u32 {
        self.cycle_resets.get()
    }

    /// Reads every grid's geometry once on this thread, so a host grid that
    /// caches it lazily is never filled in from a getter on another thread.
    pub(super) fn warm_grids(&self) {
        let grids = self.gate().grids.clone();
        for g in grids.iter().flatten() {
            let _ = g.geometry();
        }
    }

    /// Changes the state directly (hosts restoring state, tests).
    pub fn with_state<R>(&self, f: impl FnOnce(&mut PollutionState) -> R) -> R {
        self.edit(f)
    }

    pub fn tuning(&self) -> Tuning {
        self.tuning_gate().clone()
    }

    pub fn set_tuning(&self, t: Tuning) {
        *self.tuning_gate() = t;
    }

    /// The `rand()` generator of the iterators (foundations §12.6).
    pub fn crt(&self) -> CrtRand {
        *self.crt.borrow()
    }

    pub fn set_crt(&self, crt: CrtRand) {
        *self.crt.borrow_mut() = crt;
    }

    /// Replaces the visual-effect generator's state (shared: every
    /// simulator drawing from the same generator sees it).
    pub fn set_effect_rng(&self, rng: RzRandomSuper) {
        *self.fx.borrow_mut() = rng;
    }

    pub fn effect_rng(&self) -> RzRandomSuper {
        self.fx.borrow().clone()
    }

    /// The visual-effect generator itself (shared with every simulator
    /// drawing from the same one, RN3).
    pub fn effect_generator(&self) -> &SharedSuper {
        &self.fx
    }

    /// Runs `f` on a copy of the state and stores it back, so the lock is not
    /// held while `f` calls the host (main thread).
    pub(super) fn edit<R>(&self, f: impl FnOnce(&mut PollutionState) -> R) -> R {
        let mut st = self.gate().clone();
        let r = f(&mut st);
        *self.gate() = st;
        r
    }

    /// Runs `f` on the state in place, under the lock: plain data work only.
    pub(super) fn update<R>(&self, f: impl FnOnce(&mut PollutionState) -> R) -> R {
        f(&mut self.gate())
    }

    /// `rand()` users run on a copy written back afterwards.
    pub(super) fn with_crt<R>(&self, f: impl FnOnce(&mut CrtRand) -> R) -> R {
        let mut c = *self.crt.borrow();
        let r = f(&mut c);
        *self.crt.borrow_mut() = c;
        r
    }

    /// City cell counts, clamped to `0 ..= MAX_CELLS` ([choice]).
    pub(super) fn cell_counts(&self) -> (i32, i32) {
        let (x, z) = self.host.cell_counts();
        (x.clamp(0, MAX_CELLS), z.clamp(0, MAX_CELLS))
    }

    /// The department's funding fraction, 1.0 without one ([choice]).
    pub(super) fn funding(&self, s: &PollutionState) -> f32 {
        s.department.map_or(1.0, |d| self.host.department_funding(d))
    }

    // ---- lifecycle (§4.1) --------------------------------------------------------------------------------

    /// `Init()`.
    pub fn init(&self) -> bool {
        let h = self.host.clone();
        let (loaded, initialized) = self.peek(|s| (s.loaded, s.initialized));
        if !loaded {
            self.edit(|s| {
                for (i, slot) in s.grids.iter_mut().enumerate() {
                    if slot.is_none() {
                        *slot = h.create_grid(GRID_INSTANCES[i], GRID_TRACT_SIZES[i]);
                    }
                }
                s.zero_statistics();
                s.recycling_effect = 0.0;
                s.recycling_count = s.lists[L_REC].len() as u32;
                let (tx, tz) = s.tract_counts(G_AIR_SRC);
                s.tracts_per_day = ((i64::from(tx) * i64::from(tz)) / 15) as u32;
                s.iterator.mode = MODE_TABLE;
                s.iterator.set_rect(0, 0, tx - 1, tz - 1);
                self.with_crt(|crt| s.iterator.reset(true, crt));
                if let Some((x, z)) = s.iterator.next_cell() {
                    s.cell_x = x.wrapping_mul(2) as u32;
                    s.cell_z = z.wrapping_mul(2) as u32;
                }
                s.dirty_radiation = false;
                s.dirty_air = false;
                s.dirty_water = false;
                s.dirty_garbage = false;
            });
            // New city only (PG5): a load gets it back from the prop developer's save.
            h.add_prop_requester();
        }
        if !initialized {
            self.update(|s| s.initialized = true);
            self.reload_tuning();
            for level in AGENT_LEVELS {
                h.add_agent(level, AGENT_NAME, AGENT_FLAGS);
            }
            h.subscribe(&msg::SUBSCRIPTIONS);
            let (cx, cz) = self.cell_counts();
            let (bx, bz) = ((cx / 8).clamp(0, MAX_BLOCKS), (cz / 8).clamp(0, MAX_BLOCKS));
            let leftovers: Vec<u64> = self.edit(|s| {
                let old: Vec<u64> = s.effects.iter().flatten().flatten().copied().collect();
                for e in s.effects.iter_mut() {
                    *e = vec![None; (bx * bz) as usize];
                }
                s.blocks_x = bx;
                s.blocks_z = bz;
                s.effect_iterator = CellIterator::new(0, 0, bx - 1, bz - 1);
                old
            });
            for e in leftovers {
                h.release(e);
            }
            if self.peek(|s| s.department.is_none()) {
                if let Some(d) = h.create_department(budget::DEPARTMENT, budget::DEPARTMENT_GROUP) {
                    h.add_ref(d);
                    h.set_department_name(d, budget::NAME_GROUP, budget::DEPARTMENT_NAME);
                    h.set_department_max_funding(d, budget::MAX_FUNDING);
                    for (line, name) in budget::LINE_ITEMS {
                        h.create_line_item(d, line, false, budget::NAME_GROUP, name);
                    }
                    self.update(|s| s.department = Some(d));
                }
            }
            h.set_history_queue_length(history::AIR_PLUS_RADIATION, 2);
        }
        self.warm_grids();
        true
    }

    /// `Shutdown()`.
    pub fn shutdown(&self) -> bool {
        let h = self.host.clone();
        let (dept, lists, effects) = self.edit(|s| {
            s.initialized = false;
            let dept = s.department.take();
            let lists = std::mem::take(&mut s.lists);
            s.recycling_count = 0;
            let effects = std::mem::take(&mut s.effects);
            s.grids = Default::default();
            (dept, lists, effects)
        });
        if let Some(d) = dept {
            h.release(d);
        }
        for occ in lists.iter().flatten() {
            h.release(*occ);
        }
        for e in effects.iter().flatten().flatten() {
            if h.is_effect_started(*e) {
                h.stop_effect(*e, 1);
            }
            h.release(*e);
        }
        h.unsubscribe(&msg::SUBSCRIPTIONS);
        h.remove_agent();
        h.remove_prop_requester();
        true
    }

    /// Re-reads the tuning exemplar (§7.1; `Init` and `0xCA1B151C`).
    pub fn reload_tuning(&self) {
        let ex = self.host.tuning();
        let mut t = self.tuning_gate().clone();
        t.reload(ex.as_deref());
        *self.tuning_gate() = t;
    }

    // ---- messages (§5) -----------------------------------------------------------------------------------

    /// `DoMessage`: `occupant` is the message's occupant (insert / remove),
    /// `new_view_level` the u32 void2 points at (PreViewLevelChange).
    /// Always true.
    pub fn do_message(&self, msg_type: u32, occupant: u64, new_view_level: u32) -> bool {
        match msg_type {
            msg::SIM_NEW_DAY => self.simulate_day(),
            msg::SIM_NEW_MONTH => self.simulate_month(),
            msg::SIM_BEGIN => self.edit(|s| self.update_efficiency(s)),
            msg::OCCUPANT_INSERTED => self.occupant_changed(occupant, true),
            msg::OCCUPANT_REMOVED => self.occupant_changed(occupant, false),
            msg::POST_CITY_INIT => self.update_visual_effects(),
            msg::PRE_VIEW_LEVEL_CHANGE => {
                let b = new_view_level == VIEW_SURFACE;
                let changed = self.update(|s| {
                    let c = s.effects_visible != b;
                    s.effects_visible = b;
                    c
                });
                if changed {
                    self.update_visual_effects();
                }
            }
            msg::TUNING_CHANGED => self.reload_tuning(),
            _ => {}
        }
        true
    }

    // ---- building changes (§4.2) -------------------------------------------------------------------------

    /// Occupant inserted (`insert`) or removed. The state is changed in
    /// short sections between the host calls (no copy of the state: a city
    /// load inserts every building).
    pub fn occupant_changed(&self, occ: u64, insert: bool) {
        let h = self.host.clone();
        let h = &*h;
        if p_u32(h, occ, prop::PREVIEW).is_some_and(|v| v != 0) {
            return;
        }
        let sign: i32 = if insert { 1 } else { -1 };
        let (ncx, ncz) = self.cell_counts();
        if let Some(centre) = p_i32s(h, occ, prop::CENTRE) {
            let at = |a: &[i32], i: usize| a.get(i).copied().unwrap_or(0);
            let outer = p_i32s(h, occ, prop::OUTER).unwrap_or_default();
            // The source grids (shared objects, written in place).
            let sources: [Option<Rc<dyn TractGrid<i16>>>; 4] = self.update(|s| {
                s.dirty_garbage = true;
                [
                    s.grids[G_AIR_SRC].clone(),
                    s.grids[G_WATER_SRC].clone(),
                    s.grids[G_GARBAGE_SRC].clone(),
                    s.grids[G_RAD_SRC].clone(),
                ]
            });
            // The garbage rectangle.
            let (lo, hi) = h.occupant_bounding_box(occ);
            let (x0, z0) = h.position_to_cell(lo[0], lo[2]);
            let (x1, z1) = h.position_to_cell(hi[0], hi[2]);
            let add = sign.wrapping_mul(at(&centre, 2));
            if let (Some(g), true) = (&sources[G_GARBAGE_SRC], ncx > 0 && ncz > 0) {
                let geo = g.geometry();
                let (x0, x1) = (x0.clamp(0, ncx - 1), x1.clamp(0, ncx - 1));
                let (z0, z1) = (z0.clamp(0, ncz - 1), z1.clamp(0, ncz - 1));
                for x in x0..=x1 {
                    for z in z0..=z1 {
                        let (tx, tz) = geo.cell_to_tract(x, z);
                        let v = g.get(tx, tz);
                        g.set(tx, tz, i32::from(v).wrapping_add(add) as i16);
                    }
                }
            }
            // The cones.
            let radii = p_f32s(h, occ, prop::RADII).unwrap_or_else(|| vec![2.5, 1.5, 0.5, 4.0]);
            let pos = h.occupant_position(occ);
            for (i, grid) in sources.iter().enumerate() {
                let r = radii.get(i).copied().unwrap_or(0.0);
                if r.is_nan() || r <= 0.0 {
                    continue;
                }
                if let Some(g) = grid {
                    let geo = g.geometry();
                    let (tx, tz) = geo.position_to_tract(pos[0], pos[2]);
                    let c = sign.wrapping_mul(at(&centre, i)) as f32;
                    let o = sign.wrapping_mul(at(&outer, i)) as f32;
                    add_radius_effect(&**g, geo.count_x, geo.count_z, tx, tz, r, c, o);
                }
            }
        }
        // The lists (§4.2.3).
        let lists = [
            (L_WTP, prop::WATER_TREATMENT),
            (L_INC, prop::INCINERATOR),
            (L_WTE, prop::WASTE_TO_ENERGY),
            (L_REC, prop::RECYCLING),
        ];
        for (list, capacity_prop) in lists {
            if h.occupant_property(occ, capacity_prop).is_none() {
                continue;
            }
            if insert {
                h.add_ref(occ);
                self.update(|s| s.lists[list].push(occ));
            } else {
                let removed = self.update(|s| match s.lists[list].iter().position(|&o| o == occ) {
                    Some(i) => {
                        s.lists[list].remove(i);
                        true
                    }
                    None => false,
                });
                if removed {
                    h.release(occ);
                }
            }
            match list {
                L_WTP if insert => {
                    h.set_occupant_property(occ, prop::WTP_USED, u32_prop(0));
                    h.set_occupant_property(occ, prop::WTP_USED_LAST, u32_prop(0));
                }
                L_INC if insert => {
                    h.set_occupant_property(occ, prop::INC_USED, u32_prop(0));
                    h.set_occupant_property(occ, prop::INC_USED_LAST, u32_prop(0));
                }
                L_WTE => {
                    let cap = i64::from(p_u32(h, occ, prop::WASTE_TO_ENERGY).unwrap_or(0));
                    if insert {
                        h.set_occupant_property(occ, prop::WTE_USED, u32_prop(0));
                        h.set_occupant_property(occ, prop::WTE_USED_LAST, u32_prop(0));
                        if let Some(out) = p_u32(h, occ, prop::WTE_OUTPUT) {
                            h.set_occupant_property(occ, prop::WTE_MAX_OUTPUT, u32_prop(out));
                            h.remove_occupant_property(occ, prop::WTE_OUTPUT);
                        }
                    }
                    self.update(|s| {
                        let v = i64::from(s.wte_capacity) + i64::from(sign) * cap;
                        s.wte_capacity = v.clamp(0, i64::from(i32::MAX)) as i32;
                    });
                }
                L_REC => self.update(|s| s.recycling_count = s.lists[L_REC].len() as u32),
                _ => {}
            }
        }
        // The budget (§4.2.4).
        if let Some(item) = h.budget_item_for_purpose(occ, budget::PURPOSE) {
            if item.line_item == budget::LINE_MAINTENANCE {
                if insert {
                    h.add_budget_item_to_budget(occ, item.index);
                } else {
                    h.remove_budget_item_from_budget(occ, item.index);
                }
            }
            h.set_occupant_property(occ, prop::BASE_COST, Variant::Scalar(Elem::I64(item.cost)));
        }
    }

    // ---- disaster radiation (§4.3) -----------------------------------------------------------------------

    /// `AddRadiationToCity(position, strength, radius)` (slot 66).
    pub fn add_radiation_to_city(&self, position: [f32; 3], strength: f32, radius: f32) {
        if radius.is_nan() || radius <= 0.0 || strength == 0.0 {
            return;
        }
        let grid = self.peek(|s| s.grids[G_RAD_SRC].clone());
        if let Some(g) = grid {
            let geo = g.geometry();
            let (tx, tz) = geo.position_to_tract(position[0], position[2]);
            add_radius_effect(&*g, geo.count_x, geo.count_z, tx, tz, radius, strength, 0.0);
        }
        self.update_visual_effects();
        self.host.post(msg::RADIATION_CHANGED, 0);
    }

    // ---- interface slots (§8.1) --------------------------------------------------------------------------

    /// The grid index slot 5 answers for a type: 0 air value, 1 water value,
    /// 2 garbage value, 3 radiation **source**; others none ([choice], PG13).
    pub fn pollution_grid_index(kind: u32) -> Option<usize> {
        match kind {
            0 => Some(G_AIR_VAL),
            1 => Some(G_WATER_VAL),
            2 => Some(G_GARBAGE_VAL),
            3 => Some(G_RAD_SRC),
            _ => None,
        }
    }

    /// Slot 5 (main thread: clones the handle; other threads use
    /// [`PollutionSim::with_pollution_grid`]).
    pub fn pollution_grid(&self, kind: u32) -> Option<Rc<dyn TractGrid<i16>>> {
        let g = Self::pollution_grid_index(kind)?;
        self.peek(|s| s.grids[g].clone())
    }

    /// Slot 5 on any thread: `f` sees the grid under the lock.
    pub fn with_pollution_grid<R>(&self, kind: u32, f: impl FnOnce(Option<&dyn TractGrid<i16>>) -> R) -> R {
        match Self::pollution_grid_index(kind) {
            Some(g) => self.peek(|s| f(s.grids[g].as_deref())),
            None => f(None),
        }
    }

    /// Slot 6: `max(0, source[type])`; types 4–7 read the value grids
    /// [quirk]; ≥ 8 give 0 ([choice]).
    pub fn pollution_value(&self, kind: u32, x: i32, z: i32) -> u16 {
        if kind >= 8 {
            return 0;
        }
        self.peek(|s| s.cell(kind as usize, x, z).max(0) as u16)
    }

    pub fn air_value(&self, x: i32, z: i32) -> i32 {
        self.peek(|s| s.air_value(x, z))
    }
    pub fn water_value(&self, x: i32, z: i32) -> i32 {
        self.peek(|s| s.water_value(x, z))
    }
    pub fn garbage_value(&self, x: i32, z: i32) -> i32 {
        self.peek(|s| s.garbage_value(x, z))
    }
    pub fn average_air_value(&self) -> i32 {
        self.peek(|s| s.avg_air)
    }
    pub fn average_water_value(&self) -> i32 {
        self.peek(|s| s.avg_water)
    }
    pub fn average_garbage_value(&self) -> i32 {
        self.peek(|s| i32::from(s.avg_garbage))
    }
    /// Slots 13–15: unsigned `v ≥ threshold` [quirk].
    pub fn is_air_polluted(&self, x: i32, z: i32) -> bool {
        self.air_value(x, z) as u32 >= self.tuning_gate().air_threshold
    }
    pub fn is_water_polluted(&self, x: i32, z: i32) -> bool {
        self.water_value(x, z) as u32 >= self.tuning_gate().water_threshold
    }
    pub fn is_garbage_accumulated(&self, x: i32, z: i32) -> bool {
        self.garbage_value(x, z) as u32 >= self.tuning_gate().garbage_threshold
    }
    /// Slot 16: the radiation source value ≠ 0.
    pub fn is_radioactive(&self, x: i32, z: i32) -> bool {
        self.peek(|s| s.cell(G_RAD_SRC, x, z) != 0)
    }
    pub fn worst_air_tract(&self) -> WorstTract {
        self.peek(|s| s.worst_air)
    }
    pub fn worst_water_tract(&self) -> WorstTract {
        self.peek(|s| s.worst_water)
    }
    pub fn worst_garbage_tract(&self) -> WorstTract {
        self.peek(|s| s.worst_garbage)
    }
    /// Slot 20: `max(0, supply)`, storing the clamp.
    pub fn garbage_supply(&self) -> u32 {
        self.update(|s| {
            s.garbage_supply = s.garbage_supply.max(0);
            s.garbage_supply as u32
        })
    }
    pub fn landfill_tile_capacity(&self) -> i32 {
        self.tuning_gate().landfill_tile_capacity as i32
    }
    pub fn garbage_capacity(&self) -> i32 {
        self.peek(|s| s.garbage_capacity)
    }
    pub fn is_garbage_uncollected(&self) -> bool {
        self.peek(|s| s.garbage_uncollected)
    }
    pub fn total_garbage_uncollected(&self) -> i32 {
        self.peek(|s| s.total_uncollected as i32)
    }
    /// Slots 25–28 write the grids in place (no host callback but the grid).
    pub fn set_is_radioactive(&self, x: i32, z: i32, on: bool) {
        self.update(|s| s.set_is_radioactive(x, z, on));
    }
    pub fn set_air_value(&self, x: i32, z: i32, v: i16) {
        self.update(|s| s.set_air_value(x, z, v));
    }
    pub fn set_water_value(&self, x: i32, z: i32, v: i16) {
        self.update(|s| s.set_water_value(x, z, v));
    }
    pub fn set_garbage_value(&self, x: i32, z: i32, v: i16) {
        self.update(|s| s.set_garbage_value(x, z, v));
    }
    /// Slot 29: returns the applied delta.
    pub fn add_to_garbage_supply(&self, n: i32) -> i32 {
        self.update(|s| s.add_to_garbage_supply(n))
    }
    /// Slot 30: returns the argument.
    pub fn set_garbage_capacity(&self, n: i32) -> i32 {
        self.update(|s| s.garbage_capacity = n);
        n
    }
    pub fn monthly_incinerator_capacity(&self) -> i32 {
        self.peek(|s| s.incinerator_capacity)
    }
    pub fn monthly_wte_capacity(&self) -> i32 {
        self.peek(|s| s.wte_capacity)
    }
    /// Slot 33 returns a pointer to this value.
    pub fn recycling_center_effect(&self) -> f32 {
        self.peek(|s| s.recycling_effect)
    }
    /// Slot 34: `decomposedLastMonth`, zero-extended.
    pub fn garbage_abducted_prev_month(&self) -> i64 {
        self.peek(|s| i64::from(s.decomposed_last_month))
    }
    /// Slots 35–41: last month's counters (recycled truncated).
    pub fn last_month(&self) -> Counters {
        self.peek(|s| s.last_month)
    }
    pub fn garbage_recycled_last_month(&self) -> i64 {
        ftol(self.peek(|s| s.last_month.recycled))
    }
    /// Slots 42–45: list lengths (duplicates count).
    pub fn incinerator_count(&self) -> u32 {
        self.peek(|s| s.lists[L_INC].len() as u32)
    }
    pub fn wte_building_count(&self) -> u32 {
        self.peek(|s| s.lists[L_WTE].len() as u32)
    }
    pub fn recycling_center_count(&self) -> u32 {
        self.peek(|s| s.lists[L_REC].len() as u32)
    }
    pub fn water_treatment_plant_count(&self) -> u32 {
        self.peek(|s| s.lists[L_WTP].len() as u32)
    }
    /// Slot 46: incinerators, waste-to-energy, recycling (each `AddRef`ed
    /// for the caller's list).
    pub fn garbage_buildings(&self) -> Vec<u64> {
        let v: Vec<u64> = self.peek(|s| {
            [L_INC, L_WTE, L_REC]
                .iter()
                .flat_map(|&l| s.lists[l].iter().copied())
                .collect()
        });
        for &o in &v {
            self.host.add_ref(o);
        }
        v
    }
    /// Slot 47.
    pub fn building_garbage_capacity(&self, occ: u64) -> u32 {
        p_u32(&*self.host, occ, prop::GARBAGE_CAPACITY).unwrap_or(0)
    }
    /// Slot 48.
    pub fn garbage_scaling_factor(&self) -> f32 {
        self.tuning_gate().garbage_scaling
    }
    /// Slot 49: bit 15 of the garbage cell.
    pub fn is_active_landfill(&self, x: i32, z: i32) -> bool {
        self.peek(|s| s.cell(G_GARBAGE_VAL, x, z) as u16 & LANDFILL_FLAG != 0)
    }
    /// Slot 50: only on zone-14 cells whose flag differs.
    pub fn set_is_active_landfill(&self, x: i32, z: i32, on: bool) {
        if self.host.zone_type(x, z) != ZONE_LANDFILL {
            return;
        }
        self.update(|s| {
            let raw = s.cell(G_GARBAGE_VAL, x, z) as u16;
            if (raw & LANDFILL_FLAG != 0) == on {
                return;
            }
            if on {
                s.set_cell(G_GARBAGE_VAL, x, z, (raw | LANDFILL_FLAG) as i16);
                s.active_landfill_cells = s.active_landfill_cells.wrapping_add(1);
            } else {
                s.set_cell(G_GARBAGE_VAL, x, z, (raw & !LANDFILL_FLAG) as i16);
                if s.active_landfill_cells > 0 {
                    s.active_landfill_cells -= 1;
                }
            }
            s.dirty_garbage = true;
        });
    }
    /// Slot 51: `landfilled += n`; returns the new total.
    pub fn garbage_put_in_landfill(&self, n: i32) -> i64 {
        self.update(|s| {
            s.month.landfilled = s.month.landfilled.saturating_add(i64::from(n));
            s.month.landfilled
        })
    }

    /// Slot 52: `r := 0`, then the u32 property; returns (found, r).
    pub fn max_capacity(&self, property: u32, occ: u64) -> (bool, u32) {
        match p_u32(&*self.host, occ, property) {
            Some(v) => (true, v),
            None => (false, 0),
        }
    }

    /// Slot 53 (§8.1) on a given state.
    pub(super) fn current_capacity_in(&self, s: &PollutionState, property: u32, occ: u64) -> (bool, u32) {
        self.capacity_at(s.efficiency, property, occ)
    }

    /// Slot 53 with a given efficiency (the host is called without the lock).
    fn capacity_at(&self, efficiency: f32, property: u32, occ: u64) -> (bool, u32) {
        let h = &*self.host;
        let Some(base) = p_u32(h, occ, property) else {
            return (false, 0);
        };
        let mut r = if h.is_occupant_powered(occ) { base } else { 0 };
        r = ftol32(f64::from(fmul_i(f64::from(r), efficiency))) as u32;
        if let Some(curve) = p_curve(h, occ, prop::CONDITION_CURVE) {
            let cond = p_f32(h, occ, prop::CONDITION).unwrap_or(0.0);
            r = rhe(fmul_i(f64::from(r), curve.eval(cond))) as u32;
        }
        (true, r)
    }

    /// Slot 53 `GetCurrentCapacity(prop, occ)`.
    pub fn current_capacity(&self, property: u32, occ: u64) -> (bool, u32) {
        let efficiency = self.peek(|s| s.efficiency);
        self.capacity_at(efficiency, property, occ)
    }

    /// Slot 54: the raw percentage property (default 0).
    pub fn recycling_center_max_capacity(&self, occ: u64) -> (bool, f32) {
        match p_f32(&*self.host, occ, prop::RECYCLING) {
            Some(v) => (true, v),
            None => (false, 0.0),
        }
    }

    pub(super) fn recycling_capacity_in(&self, s: &PollutionState, occ: u64) -> (bool, f32) {
        self.recycling_capacity_at(s.efficiency, occ)
    }

    fn recycling_capacity_at(&self, efficiency: f32, occ: u64) -> (bool, f32) {
        match self.capacity_at(efficiency, prop::RECYCLING, occ) {
            // `f32(float(uint32 r) × 0.01f)`: the f32 literal 0.01f (§8.1).
            (true, r) => (true, fmul_i(f64::from(r), 0.01f32)),
            _ => (false, 0.0),
        }
    }

    /// Slot 55: slot 53 with `0x88ED1059`, × 0.01.
    pub fn recycling_center_current_capacity(&self, occ: u64) -> (bool, f32) {
        let efficiency = self.peek(|s| s.efficiency);
        self.recycling_capacity_at(efficiency, occ)
    }

    /// Slot 56.
    pub fn current_occupant_condition(&self, occ: u64) -> (bool, f32) {
        let h = &*self.host;
        match p_curve(h, occ, prop::CONDITION_CURVE) {
            Some(c) => (true, c.eval(p_f32(h, occ, prop::CONDITION).unwrap_or(0.0)) * 1.0),
            None => (false, 1.0),
        }
    }

    /// Slot 57.
    pub fn current_occupant_efficiency(&self, occ: u64) -> (bool, f32) {
        if self.host.is_occupant_powered(occ) {
            if let (true, c) = self.current_occupant_condition(occ) {
                return (true, self.peek(|s| s.efficiency) * c);
            }
        }
        (false, 0.0)
    }

    /// Slots 58–64.
    pub fn incinerator_used_capacity(&self, occ: u64) -> (bool, u32) {
        self.max_capacity(prop::INC_USED, occ)
    }
    pub fn wte_used_capacity(&self, occ: u64) -> (bool, u32) {
        self.max_capacity(prop::WTE_USED, occ)
    }
    pub fn incinerator_current_capacity(&self, occ: u64) -> (bool, u32) {
        self.current_capacity(prop::INCINERATOR, occ)
    }
    pub fn wte_current_capacity(&self, occ: u64) -> (bool, u32) {
        self.current_capacity(prop::WASTE_TO_ENERGY, occ)
    }
    pub fn water_treatment_max_capacity(&self, occ: u64) -> (bool, u32) {
        self.max_capacity(prop::WATER_TREATMENT, occ)
    }
    pub fn water_treatment_current_capacity(&self, occ: u64) -> (bool, u32) {
        self.current_capacity(prop::WATER_TREATMENT, occ)
    }
    pub fn water_treatment_used_capacity(&self, occ: u64) -> (bool, u32) {
        self.max_capacity(prop::WTP_USED, occ)
    }

    /// Slot 65: `trunc(base × curve(condition))`.
    pub fn calc_occupant_maintenance_cost(&self, occ: u64) -> (bool, i64) {
        let h = &*self.host;
        let Some(base) = p_i64(h, occ, prop::BASE_COST) else {
            return (false, 0);
        };
        let (Some(cond), Some(curve)) = (p_f32(h, occ, prop::CONDITION), p_curve(h, occ, prop::MAINTENANCE_CURVE))
        else {
            return (false, base);
        };
        (true, ftol(f64::from(fmul_i(base as f64, curve.eval(cond)))))
    }

    /// Slots 67–70.
    pub fn max_level_for_ui(&self, kind: u32) -> i32 {
        let t = self.tuning_gate();
        (match kind {
            0 => t.max_air_ui,
            1 => t.max_water_ui,
            2 => t.max_garbage_ui,
            _ => t.max_radiation_ui,
        }) as i32
    }

    // ---- prop requester (§4.11, §8.4) --------------------------------------------------------------------

    /// `GetRequestedShare(rect)` (inclusive cell rectangle).
    pub fn requested_share(&self, x0: i32, z0: i32, x1: i32, z1: i32) -> i32 {
        let (w, h) = (i64::from(x1) - i64::from(x0) + 1, i64::from(z1) - i64::from(z0) + 1);
        if w <= 0 || h <= 0 {
            return 0;
        }
        let count = (w as u32).wrapping_mul(h as u32);
        if count == 0 {
            return 0;
        }
        let sum = self.peek(|s| {
            let (cx, cz) = s.tract_counts(G_GARBAGE_VAL);
            let shift = s.grids[G_GARBAGE_VAL].as_ref().map_or(0, |g| g.geometry().safe_shift());
            let (ex, ez) = (i64::from(cx) << shift, i64::from(cz) << shift);
            let mut sum = 0i32;
            for x in i64::from(x0).max(0)..=i64::from(x1).min(ex - 1) {
                for z in i64::from(z0).max(0)..=i64::from(z1).min(ez - 1) {
                    sum = sum.wrapping_add(s.garbage_value(x as i32, z as i32));
                }
            }
            sum
        });
        let avg = (sum as u32) / count;
        let v = fmul_i(f64::from(avg), 1.0f32 / 32767.0);
        rhe(self.tuning_gate().prop_curve.eval(v))
    }

    pub fn requester_satisfaction_id(&self) -> u32 {
        super::PROP_SATISFACTION_ID
    }

    pub fn uses_custom_prop_selector(&self) -> bool {
        false
    }

    pub fn select_prop(&self, _props: &[u32], _a: i32, _b: i32) -> u32 {
        0
    }

    /// The efficiency curve read fresh from the exemplar (§4.7).
    pub(super) fn update_efficiency(&self, s: &mut PollutionState) {
        let curve = self
            .host
            .tuning()
            .and_then(|h| curve_of(&h, super::tuning::id::EFFICIENCY_CURVE))
            .unwrap_or_default();
        let f = self.funding(s);
        s.efficiency = curve.eval(f * 100.0) * 0.01;
    }
}
