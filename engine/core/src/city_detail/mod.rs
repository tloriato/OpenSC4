// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The city detail manager (`cSC4CityDetailManager`, class `0xEAD4BA36`),
//! `docs/specs/city-detail.md`. Section references (`§n`) point at that
//! spec.
//!
//! The manager remembers the city detail preference the city was last
//! initialised with (§2). At `Init` (§3.1) a lower current setting removes
//! lot props and lot flora whose tag the lot's configuration no longer
//! lists (§3.2); a higher one creates the missing required props and flora
//! (§3.3). Either way every network lot is then redeveloped (§3.4). The
//! record (§4) is 8 bytes.
//!
//! Everything outside the class goes through [`CityDetailHost`]: the
//! preference, the lots in the lot manager's order, their configuration
//! objects and occupants, the prop manager, the flora simulator, the lot
//! developer's object positions, the lot configuration manager's ID arrays
//! and the network lot manager. Objects are host tokens.
//!
//! # Choices (`[choice]`) and literal readings (`[question]`)
//!
//! - **The temporary generator** of §3.3 is an [`RzRandom`] seeded with
//!   `derive_seed(S, 0xEAD4BA36 XOR n)`, `n` the number of earlier increase
//!   passes **by this manager object**, from 0 at its construction (§3.5,
//!   the spec's *[choice]*; CD5). The city makes a new manager for every
//!   city, so `n` starts again at 0 at each city load. `S` comes from a
//!   [`DetailSession`]; the generator is behind the injectable
//!   [`DetailRandom`].
//! - **`Init` compares at every call** (there is no "initialised" flag,
//!   CD3): a second `Init` sees the level the first one stored and does
//!   nothing.
//! - **`Read`** is atomic: on a stream error or a version other than 0 the
//!   stored level is unchanged [choice].
//! - **A flora or prop occupant** is recognised by its occupant type
//!   (`0x74758926` flora, `0xA823821E` prop, §3.2); an occupant "has a lot
//!   tag" when the host's `GetLotTag` answers (occupant slot 27).
//! - **The presence test of §3.3 step 3** (CD4) counts only flora and prop
//!   occupants, by their lot tag alone: a prop tagged with a flora object's
//!   ID satisfies it and the reverse; any other occupant type is ignored
//!   whatever its tag.
//! - **The objects of §3.3 step 4** are visited in array order; every
//!   object of any type whose ID is still in `need` is placed and, with a
//!   non-empty ID list, drawn for (CD4); type 1 creates a prop, type 4 a
//!   flora occupant, any other type nothing [quirk].
//! - Threading (foundations §19.3): the state is a `Cell`; the manager is
//!   main-thread only and no state is borrowed across a host call.

#[cfg(test)]
mod tests;

use std::any::Any;
use std::cell::Cell;
use std::collections::BTreeSet;
use std::rc::Rc;

use crate::lot_developer::CfgObject;
use crate::rng::{FixedSeeds, RzRandom};
use crate::stream::{InStream, OutStream, SaveObject};

/// Class ID (§1).
pub const CLSID_CITY_DETAIL: u32 = 0xEAD4_BA36;
/// Interface ID (§1).
pub const IID_CITY_DETAIL: u32 = 0x6AD4_B9F1;
pub const IID_UNKNOWN: u32 = 0x0000_0001;
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;

/// City component (§1).
pub const CITY_COMPONENT: u32 = 51;

/// The stored level before any `Init` (§2): "none".
pub const LEVEL_NONE: u32 = 0xFFFF_FFFF;
/// The preference's values (§2) and its default.
pub const DETAIL_LOW: u32 = 1;
pub const DETAIL_MEDIUM: u32 = 2;
pub const DETAIL_HIGH: u32 = 3;

/// The record version (§4).
pub const RECORD_VERSION: u32 = 0;

/// Standalone record state (§4), independent of the live placement host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetailRecord {
    pub level: u32,
}

impl DetailRecord {
    pub fn read(r: &mut dyn InStream) -> Option<Self> {
        if r.get_u32()? != RECORD_VERSION {
            return None;
        }
        Some(Self { level: r.get_u32()? })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u32(RECORD_VERSION) && w.put_u32(self.level)
    }
}

/// Occupant types (§3.2).
pub const OCCUPANT_TYPE_FLORA: u32 = 0x7475_8926;
pub const OCCUPANT_TYPE_PROP: u32 = 0xA823_821E;

/// Configuration object types (§3.2, §3.3).
pub const OBJECT_PROP: u32 = 1;
pub const OBJECT_FLORA: u32 = 4;
/// Flag bit 0: an optional object (§3.3 step 2).
pub const FLAG_OPTIONAL: u8 = 0x01;

/// `QueryInterface` (§1): `1`, `0x6AD4B9F1` (+0) and `cIGZSerializable`
/// (+4).
pub fn query_interface(iid: u32) -> bool {
    matches!(iid, IID_UNKNOWN | IID_CITY_DETAIL | IID_SERIALIZABLE)
}

/// A position and orientation from the lot developer's slot 39
/// (§3.3 step 4).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ObjectPlacement {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub orientation: u32,
}

/// The draw of §3.5 on the temporary generator.
pub trait DetailRandom {
    /// `RandomUint32Uniform(n)`.
    fn uniform_u32(&mut self, n: u32) -> u32;
}

impl DetailRandom for RzRandom {
    fn uniform_u32(&mut self, n: u32) -> u32 {
        RzRandom::uniform_u32(self, n)
    }
}

/// The session seed `S` of §3.5 (foundations §6.3). The pass count `n` is
/// kept by each manager (CD5).
#[derive(Debug)]
pub struct DetailSession {
    pub seed: u32,
}

impl DetailSession {
    pub fn new(seed: u32) -> Rc<DetailSession> {
        Rc::new(DetailSession { seed })
    }

    /// The seed of the `n`-th increase pass (§3.5 *[choice]*):
    /// `derive_seed(S, 0xEAD4BA36 XOR n)`.
    pub fn pass_seed(&self, n: u32) -> u32 {
        crate::seeding::seed(self.seed, crate::seeding::keys::city_detail_increase(n))
    }

    /// The generator of the `n`-th increase pass.
    pub fn generator(&self, n: u32) -> RzRandom {
        let seed = self.pass_seed(n);
        RzRandom::new(
            seed,
            &FixedSeeds {
                wall_clock: seed,
                process_clock: seed,
            },
        )
    }
}

/// Everything the city detail manager reads from, or does to, the rest of
/// the game. Objects are host tokens. Re-entrancy: callbacks must not call
/// back into the manager.
pub trait CityDetailHost {
    /// The current detail preference (§2: preferences byte `+0xF16`).
    fn detail_setting(&self) -> u32;
    /// Every lot, in the lot manager's list order (slot 15, the safe lot
    /// iterator).
    fn lots(&self) -> Vec<u64>;
    /// `lot.GetLotConfiguration()`'s object array (cfg slot 23); `None` when
    /// the lot has no configuration.
    fn lot_config_objects(&self, lot: u64) -> Option<Rc<Vec<CfgObject>>>;
    /// `lot.GetLotOccupants` (lot slot 49), in list order.
    fn lot_occupants(&self, lot: u64) -> Vec<u64>;
    /// Every occupant in the lot's cell rectangle (lot slot 14, occupant
    /// manager slot 46 with no filter), in the iteration order.
    fn occupants_in_lot_rect(&self, lot: u64) -> Vec<u64>;
    /// `occ.GetType()`.
    fn occupant_type(&self, occupant: u64) -> u32;
    /// `occ.GetLotTag(&tag)` (occupant slot 27): `None` without a tag.
    fn occupant_lot_tag(&self, occupant: u64) -> Option<u32>;
    /// `flora.DemolishFloraOccupant(occ, 0)` (flora slot 19).
    fn demolish_flora(&self, occupant: u64);
    /// `propManager.RemovePropA(occ)` (prop manager slot 29).
    fn remove_prop(&self, occupant: u64);
    /// `lotDeveloper.GetPositionForLotConfigurationObject(lot, cfg,
    /// object)` (lot developer slot 39) for object `index` of the lot's
    /// configuration.
    fn object_placement(&self, lot: u64, index: usize) -> ObjectPlacement;
    /// `lotConfigurationManager.GetObjectIDArray(object)` (slot 27,
    /// families expanded) for object `index` of the lot's configuration.
    fn object_id_array(&self, lot: u64, index: usize) -> Vec<u32>;
    /// `propManager.AddLotProp(id, pos, orientation)` (slot 26).
    fn add_lot_prop(&self, id: u32, placement: ObjectPlacement) -> Option<u64>;
    /// `flora.AddNewFloraOccupant(id, pos, false)` (flora slot 17).
    fn add_flora(&self, id: u32, placement: ObjectPlacement) -> Option<u64>;
    /// `occ.SetLotTag(tag)` (occupant slot 28).
    fn set_lot_tag(&self, occupant: u64, tag: u32);
    /// The network lots of `IterateAllNetworkLots` (slot 11), in the
    /// manager's order: `(x, z, networkLot)`.
    fn network_lots(&self) -> Vec<(i32, i32, u64)>;
    /// `networkLotManager.RedevelopLot(x, z, networkLot)` (slot 12).
    fn redevelop_network_lot(&self, x: i32, z: i32, network_lot: u64);
}

/// The fields of §2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CityDetailState {
    /// The detail level the city was last initialised with.
    pub level: u32,
    pub ref_count: u32,
    /// `Release` destroyed the object.
    pub destroyed: bool,
}

impl Default for CityDetailState {
    fn default() -> CityDetailState {
        CityDetailState {
            level: LEVEL_NONE,
            ref_count: 0,
            destroyed: false,
        }
    }
}

/// What one `Init` did (diagnostics).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InitOutcome {
    /// 0 nothing, 1 reduce, 2 increase.
    pub pass: u32,
    pub removed: u32,
    pub created: u32,
    pub draws: u32,
    pub network_lots: u32,
}

/// The city detail manager.
pub struct CityDetailManager {
    host: Rc<dyn CityDetailHost>,
    session: Rc<DetailSession>,
    /// Increase passes by this object so far (`n` of §3.5, CD5; not saved).
    passes: Cell<u32>,
    token: Cell<u64>,
    s: Cell<CityDetailState>,
    last: Cell<InitOutcome>,
    /// Test hook: the generator of the next increase pass instead of the
    /// session's.
    next_random: std::cell::RefCell<Option<Box<dyn DetailRandom>>>,
}

impl CityDetailManager {
    /// A constructed manager: level none, reference count 0.
    pub fn new(host: Rc<dyn CityDetailHost>, session: Rc<DetailSession>, token: u64) -> Rc<CityDetailManager> {
        Rc::new(CityDetailManager {
            host,
            session,
            passes: Cell::new(0),
            token: Cell::new(token),
            s: Cell::new(CityDetailState::default()),
            last: Cell::new(InitOutcome::default()),
            next_random: std::cell::RefCell::new(None),
        })
    }

    pub fn token(&self) -> u64 {
        self.token.get()
    }

    pub fn set_token(&self, token: u64) {
        self.token.set(token);
    }

    pub fn snapshot(&self) -> CityDetailState {
        self.s.get()
    }

    /// Sets the stored level (tests and tools).
    pub fn set_level(&self, level: u32) {
        let mut s = self.s.get();
        s.level = level;
        self.s.set(s);
    }

    /// What the last `Init` did.
    pub fn last_outcome(&self) -> InitOutcome {
        self.last.get()
    }

    /// The increase passes this object has run (`n` of the next one, §3.5).
    pub fn passes(&self) -> u32 {
        self.passes.get()
    }

    /// Replaces the generator of the next increase pass (tests, replays).
    /// The pass count still advances.
    pub fn set_next_random(&self, rng: Box<dyn DetailRandom>) {
        *self.next_random.borrow_mut() = Some(rng);
    }

    // ---- identity (§1, §6) ----

    pub fn add_ref(&self) -> u32 {
        let mut s = self.s.get();
        s.ref_count = s.ref_count.wrapping_add(1);
        self.s.set(s);
        s.ref_count
    }

    /// Slot 2 `Release`: the last release destroys the object (here: marks
    /// it) and returns 0. `Shutdown` does nothing, so it is not called.
    pub fn release(&self) -> u32 {
        let mut s = self.s.get();
        if s.ref_count > 1 {
            s.ref_count -= 1;
            self.s.set(s);
            return s.ref_count;
        }
        s.ref_count = 0;
        s.destroyed = true;
        self.s.set(s);
        0
    }

    // ---- §3 ----

    /// Slot 3 `Init` (§3.1). Returns true.
    pub fn init(&self) -> bool {
        let cur = self.host.detail_setting();
        let saved = self.s.get().level;
        let mut out = InitOutcome::default();
        if saved != LEVEL_NONE {
            if cur < saved {
                out.pass = 1;
                self.reduce(&mut out);
                self.network_lots(&mut out);
            } else if cur > saved {
                out.pass = 2;
                self.increase(&mut out);
                self.network_lots(&mut out);
            }
        }
        self.set_level(cur);
        self.last.set(out);
        true
    }

    /// Slot 4 `Shutdown`: returns true and does nothing (§3.1).
    pub fn shutdown(&self) -> bool {
        true
    }

    /// Slot 5 (*[inferred]* `GetCityDetail`): the stored level.
    pub fn city_detail(&self) -> u32 {
        self.s.get().level
    }

    /// The object IDs (record +0x24) of the prop and flora objects that
    /// pass `keep`.
    fn object_ids(objs: &[CfgObject], keep: impl Fn(&CfgObject) -> bool) -> BTreeSet<u32> {
        objs.iter()
            .filter(|o| (o.object_type == OBJECT_PROP || o.object_type == OBJECT_FLORA) && keep(o))
            .map(|o| o.tag)
            .collect()
    }

    /// Reduce (§3.2).
    fn reduce(&self, out: &mut InitOutcome) {
        let h = &*self.host;
        for lot in h.lots() {
            // 1.
            let Some(objs) = h.lot_config_objects(lot) else {
                continue;
            };
            // 2. Whatever their flags.
            let keep = Self::object_ids(&objs, |_| true);
            // 3.
            for occ in h.lot_occupants(lot) {
                let Some(tag) = h.occupant_lot_tag(occ) else {
                    continue;
                };
                if keep.contains(&tag) {
                    continue;
                }
                match h.occupant_type(occ) {
                    OCCUPANT_TYPE_FLORA => {
                        h.demolish_flora(occ);
                        out.removed += 1;
                    }
                    OCCUPANT_TYPE_PROP => {
                        h.remove_prop(occ);
                        out.removed += 1;
                    }
                    _ => {}
                }
            }
        }
    }

    /// Increase (§3.3).
    fn increase(&self, out: &mut InitOutcome) {
        let h = &*self.host;
        // One temporary generator for the whole pass (§3.5), the n-th of
        // this object (CD5).
        let n = self.passes.get();
        self.passes.set(n.wrapping_add(1));
        let session_rng = self.session.generator(n);
        let mut rng: Box<dyn DetailRandom> = match self.next_random.borrow_mut().take() {
            Some(r) => r,
            None => Box::new(session_rng),
        };
        for lot in h.lots() {
            // 1.
            let Some(objs) = h.lot_config_objects(lot) else {
                continue;
            };
            // 2. Required objects only.
            let mut need = Self::object_ids(&objs, |o| o.flags & FLAG_OPTIONAL == 0);
            // 3. Flora and prop occupants only, by tag alone (CD4).
            for occ in h.occupants_in_lot_rect(lot) {
                let t = h.occupant_type(occ);
                if t != OCCUPANT_TYPE_FLORA && t != OCCUPANT_TYPE_PROP {
                    continue;
                }
                if let Some(tag) = h.occupant_lot_tag(occ) {
                    need.remove(&tag);
                }
            }
            // 4. An ID stays in `need` after it is used [quirk]. Every
            // object whose ID is in `need` is placed and drawn for, whatever
            // its type and flags; only types 1 and 4 create anything (CD4).
            for (i, o) in objs.iter().enumerate() {
                if !need.contains(&o.tag) {
                    continue;
                }
                let placement = h.object_placement(lot, i);
                let ids = h.object_id_array(lot, i);
                if ids.is_empty() {
                    continue;
                }
                let k = rng.uniform_u32(ids.len() as u32) as usize;
                out.draws += 1;
                let id = ids.get(k).copied().unwrap_or(0);
                if id == 0 {
                    continue;
                }
                let occ = match o.object_type {
                    OBJECT_PROP => h.add_lot_prop(id, placement),
                    OBJECT_FLORA => h.add_flora(id, placement),
                    _ => None,
                };
                if let Some(occ) = occ {
                    h.set_lot_tag(occ, o.tag);
                    out.created += 1;
                }
            }
        }
    }

    /// Network lots (§3.4).
    fn network_lots(&self, out: &mut InitOutcome) {
        let h = &*self.host;
        for (x, z, nl) in h.network_lots() {
            h.redevelop_network_lot(x, z, nl);
            out.network_lots += 1;
        }
    }

    fn parse(inp: &mut dyn InStream) -> Option<u32> {
        if inp.get_u32()? != RECORD_VERSION {
            return None;
        }
        let level = inp.get_u32()?;
        (inp.error() == 0).then_some(level)
    }
}

impl SaveObject for CityDetailManager {
    fn class_id(&self) -> u32 {
        CLSID_CITY_DETAIL
    }

    /// §4: version 0, the level. True unless the stream reports an error.
    fn write(&self, out: &mut dyn OutStream) -> bool {
        out.put_u32(RECORD_VERSION);
        out.put_u32(self.s.get().level);
        out.error() == 0
    }

    /// §4: false unless the version is 0. [choice] Atomic.
    fn read(&self, inp: &mut dyn InStream) -> bool {
        match Self::parse(inp) {
            Some(level) => {
                self.set_level(level);
                true
            }
            None => false,
        }
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }

    fn host_token(&self) -> u64 {
        self.token.get()
    }
}
