// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The network lot manager (§2.2, §3.2, §4.2, §6.2, §8, §17).

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;

use crate::grid::CellRect;
use crate::rng::RzRandomSuper;
use crate::stream::{InStream, OutStream, RefValue, SaveObject};
use crate::traffic_network::{DX, DZ};

use super::config::{
    is_lot_compatible_pattern, object_position, pothole_offset, road_texture, road_variation, slope_fits,
    wealth_choice, zone_choice, NetworkLotConfig, OBJECT_FLORA, OBJECT_PROP,
};
use super::flags::{flag, has_all, has_any};
use super::ordered::OrderedMap;
use super::{
    key_cell, lot_key, msg, NetworkLotHost, NetworkLotMessage, NetworkLotRandom, Position, CLSID_NETWORK_LOT_MANAGER,
    IID_EFFECT_PROP, IID_NETWORK_OCCUPANT, LOT_RECORD_VERSION,
};

/// The edge-type kind avenue style asks for (§8.7 `GetEdgeTypesFlag(6)`).
pub const AVENUE_EDGE_KIND: u32 = 6;
/// The flag a created street prop gets on a flipped piece (§8.6).
pub const FLIPPED_PROP_FLAG: u32 = 0x80;
/// The pothole effect names (§8.8).
pub const POTHOLE_NAME: &str = "pothole";
pub const POTHOLE_RAIL_NAME: &str = "pothole_rail";
/// The texture of rail beside a zone (§8.7 step 5).
pub const RAIL_TEXTURE: u32 = 0x0840_0000;

/// A network lot (`cSC4NetworkLot`, §2.3).
#[derive(Clone, Debug, PartialEq)]
pub struct LotEntry {
    /// The network occupant of the cell, held (`None`: a reference that did
    /// not resolve on load).
    pub occupant: Option<u64>,
    /// The chosen configuration (null only between `Read` and `Init`).
    pub config: Option<Rc<NetworkLotConfig>>,
    /// The configuration ID read from the save.
    pub config_id: u32,
    /// Created from a forced configuration: never re-chosen.
    pub locked: bool,
}

/// Every field of §2.2.
#[derive(Clone, Debug, Default)]
pub struct NetworkLotState {
    pub initialized: bool,
    pub ref_count: u32,
    pub destroyed: bool,
    /// Cell key → network lot.
    pub lots: OrderedMap<u16, LotEntry>,
    /// Piece ID → configurations in load order (groups in first-load order).
    pub configs: Vec<(u32, Vec<Rc<NetworkLotConfig>>)>,
    /// The configuration for the next piece (slot 13).
    pub forced: Option<Rc<NetworkLotConfig>>,
    /// Cell key → pothole effect prop, held (`None`: a reference that did not
    /// resolve on load).
    pub potholes: OrderedMap<u16, Option<u64>>,
    /// Cells whose network lot must be re-examined (§8.5).
    pub pending: BTreeSet<u16>,
}

/// The network lot manager.
pub struct NetworkLotManager {
    host: Rc<dyn NetworkLotHost>,
    token: Cell<u64>,
    rng: RefCell<Box<dyn NetworkLotRandom>>,
    s: RefCell<NetworkLotState>,
}

impl NetworkLotManager {
    /// A constructed (not initialised) manager. The generator `0x00B44870`
    /// is seeded through `seeding::keys::NETWORK_LOT_MANAGER`: `derive(S,
    /// 0x8C09C379)` (foundations §6.3).
    pub fn new(host: Rc<dyn NetworkLotHost>, token: u64, session_seed: u32) -> Rc<NetworkLotManager> {
        let key = crate::seeding::keys::NETWORK_LOT_MANAGER;
        let rng = RzRandomSuper::new(
            crate::seeding::seed(session_seed, key),
            &crate::seeding::clocks(session_seed, key),
        );
        Rc::new(NetworkLotManager {
            host,
            token: Cell::new(token),
            rng: RefCell::new(Box::new(rng)),
            s: RefCell::new(NetworkLotState::default()),
        })
    }

    pub fn token(&self) -> u64 {
        self.token.get()
    }

    /// Replaces the generator (tests, replays).
    pub fn set_random(&self, rng: Box<dyn NetworkLotRandom>) {
        *self.rng.borrow_mut() = rng;
    }

    pub fn snapshot(&self) -> NetworkLotState {
        self.s.borrow().clone()
    }

    pub fn with_state<R>(&self, f: impl FnOnce(&mut NetworkLotState) -> R) -> R {
        f(&mut self.s.borrow_mut())
    }

    fn draw(&self, n: u32) -> u32 {
        self.rng.borrow_mut().uniform_u32(n)
    }

    // ---- identity (§1) ----

    pub fn add_ref(&self) -> u32 {
        let mut s = self.s.borrow_mut();
        s.ref_count = s.ref_count.wrapping_add(1);
        s.ref_count
    }

    /// `Release`: above 1 it decrements; at 1 or below the object is
    /// deleted (here: marked) and 0 is returned.
    pub fn release(&self) -> u32 {
        let mut s = self.s.borrow_mut();
        if s.ref_count > 1 {
            s.ref_count -= 1;
            return s.ref_count;
        }
        s.ref_count = 0;
        s.destroyed = true;
        0
    }

    // ---- lifecycle (§3.2) ----

    /// Slot 3 `Init` (§3.2). Returns true.
    pub fn init(&self) -> bool {
        let h = &*self.host;
        {
            let mut s = self.s.borrow_mut();
            if s.initialized {
                return true;
            }
            s.initialized = true;
        }
        for m in msg::LOT_SUBSCRIPTIONS {
            h.subscribe(m);
        }
        // 4. The configurations, grouped by piece in load order.
        for cfg in h.load_configurations() {
            let cfg = Rc::new(cfg);
            let mut s = self.s.borrow_mut();
            match s.configs.iter_mut().find(|(p, _)| *p == cfg.piece_id) {
                Some((_, v)) => v.push(cfg),
                None => s.configs.push((cfg.piece_id, vec![cfg])),
            }
        }
        // 5. Resolve the loaded lots; drop those whose configuration is
        // missing without releasing them *[quirk]*.
        let mut s = self.s.borrow_mut();
        let mut missing = Vec::new();
        for key in s.lots.keys() {
            let id = s.lots.get(&key).map_or(0, |l| l.config_id);
            let found = s
                .configs
                .iter()
                .flat_map(|(_, v)| v.iter())
                .find(|c| c.id == id)
                .cloned();
            match found {
                Some(c) => {
                    if let Some(l) = s.lots.get_mut(&key) {
                        l.config = Some(c);
                    }
                }
                None => missing.push(key),
            }
        }
        for key in missing {
            s.lots.remove(&key);
        }
        true
    }

    /// Slot 4 `Shutdown` (§3.2). Returns true.
    pub fn shutdown(&self) -> bool {
        let h = &*self.host;
        {
            let mut s = self.s.borrow_mut();
            if !s.initialized {
                return true;
            }
            s.initialized = false;
        }
        for m in msg::LOT_SUBSCRIPTIONS {
            h.unsubscribe(m);
        }
        let (lots, potholes) = {
            let mut s = self.s.borrow_mut();
            s.configs.clear();
            // [choice] Both maps are cleared after their entries are released.
            (s.lots.drain(), s.potholes.drain())
        };
        for (_, l) in lots {
            if let Some(o) = l.occupant {
                h.release(o);
            }
        }
        for (_, p) in potholes {
            if let Some(p) = p {
                h.effect_stop(p);
                h.release(p);
            }
        }
        true
    }

    // ---- messages (§6.2) ----

    /// `DoMessage` (`0x0060FC30`, §6.2). Returns true.
    pub fn do_message(&self, m: &NetworkLotMessage) -> bool {
        let h = &*self.host;
        match m.msg_type {
            msg::USABLE_CHANGED => {
                let Some(n) = m.unknown.and_then(|u| h.query_network_occupant(u)) else {
                    return true;
                };
                if self.lot_candidate(n) {
                    if m.data1 != 0 {
                        let (x, z) = h.position_cell(n);
                        self.update_roads_in_cell(x, z);
                    } else {
                        self.do_network_occupant_removed(n);
                    }
                }
                h.release(n);
            }
            msg::OCCUPANT_REMOVED => {
                let Some(n) = m.void1.and_then(|u| h.query_network_occupant(u)) else {
                    return true;
                };
                if self.lot_candidate(n) {
                    self.do_network_occupant_removed(n);
                }
                h.release(n);
            }
            msg::ZONING_CHANGED => {
                if let Some(r) = m.rect {
                    let size = h.city_size();
                    let mut s = self.s.borrow_mut();
                    for z in r.z1 - 1..=r.z2 + 1 {
                        for x in r.x1 - 1..=r.x2 + 1 {
                            if inside(x, z, size) {
                                s.pending.insert(lot_key(x, z));
                            }
                        }
                    }
                }
                self.process_update_list();
            }
            msg::LOT_STATE_CHANGED => {
                let Some(lot) = m.unknown.and_then(|u| h.query_lot(u)) else {
                    return true;
                };
                let r = h.lot_bounding_rect(lot);
                self.add_border_to_pending(CellRect {
                    x1: r.x1 - 1,
                    z1: r.z1 - 1,
                    x2: r.x2 + 1,
                    z2: r.z2 + 1,
                });
                self.process_update_list();
                h.release(lot);
            }
            msg::ROAD_DAMAGE_CHANGED => {
                self.do_road_damage_change(m.data1 as i32, m.data2 as i32, m.data3 & 0xFF != 0);
            }
            msg::CREATE_LOT_OBJECTS => {
                // [choice] void1 is read as a lot's cell key (never posted).
                let lot = m.void1.and_then(|k| self.s.borrow().lots.get(&(k as u16)).cloned());
                if let Some(l) = lot {
                    self.create_lot_objects(&l);
                }
            }
            _ => {}
        }
        true
    }

    /// The §6.2 filter: a surface network that is not a connection node.
    fn lot_candidate(&self, n: u64) -> bool {
        let f = self.host.network_flags(n);
        has_any(f, flag::SURFACE) && !has_all(f, flag::CONNECTION_NODE)
    }

    /// The cells of the §6.3 lot-state boundary walk of the grown rect `r`
    /// inside the city, added to `pending` (§6.2 `0xE8BDA2E7`). The walk
    /// starts at (0, 0), not at (x0, z0) *[quirk]* (answers NM10, NM11).
    pub fn add_border_to_pending(&self, r: CellRect) {
        let size = self.host.city_size();
        let mut s = self.s.borrow_mut();
        for (x, z) in lot_state_walk(r) {
            if inside(x, z, size) {
                s.pending.insert(lot_key(x, z));
            }
        }
    }

    /// Adds a cell to `pending` (test and glue hook).
    pub fn add_pending(&self, x: i32, z: i32) {
        self.s.borrow_mut().pending.insert(lot_key(x, z));
    }

    // ---- queries (§8.1–§8.4) ----

    /// Slot 5 `GetNetworkLot(x, z)` (§8.1).
    pub fn get_network_lot(&self, x: i32, z: i32) -> Option<LotEntry> {
        self.s.borrow().lots.get(&lot_key(x, z)).cloned()
    }

    /// Slot 9 `GetZoneTypeForNetwork(x, z)` (§8.2).
    pub fn zone_type_for_network(&self, x: i32, z: i32) -> i32 {
        let h = &*self.host;
        zone_choice(x, z, h.city_size(), |cx, cz| h.zone_type(cx, cz))
    }

    /// The wealth a lot counts with in §8.3 step 3 (before the range test).
    pub fn effective_lot_wealth(&self, lot: u64) -> i32 {
        let h = &*self.host;
        let mut w = h.lot_wealth(lot);
        if h.lot_is_livable(lot) && w == 0 {
            let bt = h.lot_building_type(lot);
            if bt != 0 {
                if let Some(pw) = h.building_profile_wealth(bt) {
                    w = pw;
                }
            }
        }
        w
    }

    /// Slot 10 `GetWealthForNetwork(x, z)` (§8.3).
    pub fn wealth_for_network(&self, x: i32, z: i32) -> i32 {
        let h = &*self.host;
        wealth_choice(x, z, h.city_size(), |cx, cz| {
            h.lot_at(cx, cz).map(|l| self.effective_lot_wealth(l))
        })
    }

    /// `IsLotCompatible` (`0x0060CD30`, §8.4); the slope is asked for last.
    #[allow(clippy::too_many_arguments)]
    pub fn is_lot_compatible(
        &self,
        cfg: &NetworkLotConfig,
        x: i32,
        z: i32,
        zone: i32,
        wealth: i32,
        rotation: i32,
        flip: i32,
    ) -> bool {
        is_lot_compatible_pattern(cfg, x, z, zone, wealth, rotation, flip)
            && slope_fits(cfg, self.host.area_slope(x, z))
    }

    /// Slot 6 `GetNetworkLotConfigurationSet`: every configuration.
    pub fn configuration_set(&self) -> Vec<Rc<NetworkLotConfig>> {
        self.s
            .borrow()
            .configs
            .iter()
            .flat_map(|(_, v)| v.iter().cloned())
            .collect()
    }

    /// Slot 7 `GetNetworkLotConfigurationById(id, out)`: true if some
    /// configuration has that ID (the original never writes `out`
    /// *[quirk]*).
    pub fn configuration_by_id(&self, id: u32) -> bool {
        self.s
            .borrow()
            .configs
            .iter()
            .any(|(_, v)| v.iter().any(|c| c.id == id))
    }

    /// Slot 13 `SetLotConfigurationForNextPiece(cfg)`.
    pub fn set_lot_configuration_for_next_piece(&self, cfg: Option<Rc<NetworkLotConfig>>) {
        self.s.borrow_mut().forced = cfg;
    }

    /// Slot 11 `IterateAllNetworkLots(cb)`: `cb(x, z, lot)` in `lots` order;
    /// false at the first false.
    pub fn iterate_all_network_lots(&self, mut cb: impl FnMut(i32, i32, &LotEntry) -> bool) -> bool {
        let lots: Vec<(u16, LotEntry)> = self.s.borrow().lots.iter().map(|(k, v)| (*k, v.clone())).collect();
        for (k, l) in lots {
            let (x, z) = key_cell(k);
            if !cb(x, z, &l) {
                return false;
            }
        }
        true
    }

    // ---- choosing, keeping and removing (§8.5) ----

    /// `UpdateRoadsInCell(x, z)` (`0x0060F980`).
    pub fn update_roads_in_cell(&self, x: i32, z: i32) {
        let h = &*self.host;
        let Some(n) = h.find_network_tile(x, z, flag::SURFACE) else {
            return;
        };
        if has_any(h.network_flags(n), flag::ROADS_IN_CELL_SKIP) {
            return;
        }
        self.update_road_style(x, z, n);
        if has_all(h.network_flags(n), flag::AVENUE) {
            return;
        }
        let w = self.wealth_for_network(x, z);
        let zt = self.zone_type_for_network(x, z);
        self.update_lot(n, x, z, zt, w);
    }

    /// `UpdateLot(n, x, z, zone, wealth)` (`0x0060E2D0`).
    pub fn update_lot(&self, n: u64, x: i32, z: i32, zone: i32, wealth: i32) {
        let h = &*self.host;
        let existing = self.s.borrow().lots.get(&lot_key(x, z)).cloned();
        match existing {
            Some(l) => {
                if l.locked {
                    return;
                }
                if let Some(cfg) = &l.config {
                    if self.is_lot_compatible(cfg, x, z, zone, wealth, h.rotation(n), h.flip(n)) {
                        return;
                    }
                }
                let Some(o) = l.occupant else {
                    return;
                };
                h.add_ref(o);
                self.do_network_occupant_removed(o);
                self.do_network_occupant_inserted(o, zone, wealth);
                h.release(o);
            }
            None => {
                if has_all(h.network_flags(n), flag::USABLE) {
                    self.do_network_occupant_inserted(n, zone, wealth);
                }
            }
        }
    }

    /// `DoNetworkOccupantInserted(n, zone, wealth)` (`0x0060DCE0`).
    pub fn do_network_occupant_inserted(&self, n: u64, zone: i32, wealth: i32) {
        let h = &*self.host;
        let piece = h.piece_id(n);
        let (group, forced) = {
            let s = self.s.borrow();
            (
                s.configs.iter().find(|(p, _)| *p == piece).map(|(_, v)| v.clone()),
                s.forced.clone(),
            )
        };
        if group.is_none() && forced.is_none() {
            return;
        }
        let (x, z) = h.position_cell(n);
        let cfg = match &forced {
            Some(f) => f.clone(),
            None => {
                let (rot, flip) = (h.rotation(n), h.flip(n));
                let candidates: Vec<Rc<NetworkLotConfig>> = group
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|c| self.is_lot_compatible(c, x, z, zone, wealth, rot, flip))
                    .collect();
                if candidates.is_empty() {
                    return;
                }
                let i = self.draw(candidates.len() as u32) as usize;
                // [choice] A generator answering past the count takes the last.
                candidates[i.min(candidates.len() - 1)].clone()
            }
        };
        h.add_ref(n);
        let lot = LotEntry {
            occupant: Some(n),
            config_id: cfg.id,
            config: Some(cfg),
            locked: false,
        };
        // An existing entry is overwritten without release.
        self.s.borrow_mut().lots.insert(lot_key(x, z), lot.clone());
        self.create_lot_objects(&lot);
        if forced.is_some() {
            let mut s = self.s.borrow_mut();
            if let Some(l) = s.lots.get_mut(&lot_key(x, z)) {
                l.locked = true;
            }
            s.forced = None;
        }
    }

    /// `DoNetworkOccupantRemoved(n)` (`0x0060D8B0`).
    pub fn do_network_occupant_removed(&self, n: u64) {
        let h = &*self.host;
        let (x, z) = h.position_cell(n);
        h.remove_street_props(x & 0xFF, z & 0xFF);
        h.demolish_flora(x, z);
        let old = self.s.borrow_mut().lots.remove(&lot_key(x, z));
        if let Some(Some(o)) = old.map(|l| l.occupant) {
            h.release(o);
        }
    }

    /// `ProcessUpdateList()` (`0x0060FA10`): keys in ascending order
    /// (the spec's *[choice]*).
    pub fn process_update_list(&self) {
        loop {
            let Some(&k) = self.s.borrow().pending.iter().next() else {
                break;
            };
            let (x, z) = key_cell(k);
            self.update_roads_in_cell(x, z);
            self.s.borrow_mut().pending.remove(&k);
        }
    }

    // ---- road style (§8.7) ----

    /// `UpdateRoadStyle(x, z, n)` (`0x0060F510`).
    pub fn update_road_style(&self, x: i32, z: i32, n: u64) {
        let h = &*self.host;
        let f = h.network_flags(n);
        if has_any(f, flag::ROAD_STYLE_SKIP) || !has_all(f, flag::USABLE) {
            return;
        }
        if has_all(f, flag::AVENUE) {
            self.update_avenue_style(x, z);
            return;
        }
        let zt = self.zone_type_for_network(x, z);
        let w = self.wealth_for_network(x, z);
        let tex = if has_any(f, flag::RAIL) && !has_any(f, flag::ROAD_HIGHWAY_STREET) {
            if zt != 0 {
                RAIL_TEXTURE
            } else {
                0
            }
        } else {
            road_texture(zt, w)
        };
        h.set_under_texture(n, tex);
        if tex != 0 {
            h.set_network_flag(n, flag::UNDER_TEXTURE);
        }
        h.set_variation(n, road_variation(zt, w));
    }

    /// `UpdateAvenueStyle(x, z)` (`0x0060EF40`): the avenue is styled as one
    /// block. The work set is taken in ascending key order and a cell
    /// already accepted or rejected is not added again [inferred] (NM4).
    pub fn update_avenue_style(&self, x: i32, z: i32) {
        let h = &*self.host;
        let Some(s0) = h.find_network_tile(x, z, flag::AVENUE) else {
            return;
        };
        let c = h.is_intersection(s0);
        let size = h.city_size();
        let mut work: BTreeSet<u16> = BTreeSet::new();
        let mut accepted_keys: BTreeSet<u16> = BTreeSet::new();
        let mut rejected: BTreeSet<u16> = BTreeSet::new();
        let mut accepted: Vec<(i32, i32, u64)> = Vec::new();
        let mut zone_count = [0u32; 16];
        let mut wealth_count = [0u32; 4];
        work.insert(lot_key(x, z));
        while let Some(k) = work.pop_first() {
            let (cx, cz) = key_cell(k);
            match h.find_network_tile(cx, cz, flag::AVENUE) {
                Some(t) if h.is_intersection(t) == c => {
                    accepted_keys.insert(k);
                    accepted.push((cx, cz, t));
                    let zt = self.zone_type_for_network(cx, cz);
                    if let Some(c) = usize::try_from(zt).ok().and_then(|i| zone_count.get_mut(i)) {
                        *c += 1;
                    }
                    let w = self.wealth_for_network(cx, cz);
                    if let Some(c) = usize::try_from(w).ok().and_then(|i| wealth_count.get_mut(i)) {
                        *c += 1;
                    }
                    let edges = h.edge_types_flag(t, AVENUE_EDGE_KIND);
                    for d in 0..4 {
                        if (edges >> (8 * d)) & 7 == 0 {
                            continue;
                        }
                        let (nx, nz) = (cx + DX[d], cz + DZ[d]);
                        let nk = lot_key(nx, nz);
                        if inside(nx, nz, size) && !accepted_keys.contains(&nk) && !rejected.contains(&nk) {
                            work.insert(nk);
                        }
                    }
                }
                _ => {
                    rejected.insert(k);
                }
            }
        }
        // The largest count wins, ties to the lower value, 0 when none counts.
        let (mut zt, mut best) = (0, 0);
        for (zone, &n) in zone_count.iter().enumerate().skip(1) {
            if n > best {
                (zt, best) = (zone, n);
            }
        }
        let (mut w, mut best) = (0, 0);
        for (wealth, &n) in wealth_count.iter().enumerate().skip(1) {
            if n > best {
                (w, best) = (wealth, n);
            }
        }
        let (zt, w) = (zt as i32, w as i32);
        let tex = road_texture(zt, w);
        let var = road_variation(zt, w);
        for (cx, cz, t) in accepted {
            if has_all(h.network_flags(t), flag::USABLE) {
                h.set_under_texture(t, tex);
                h.set_variation(t, var);
                self.update_lot(t, cx, cz, zt, w);
            }
            self.s.borrow_mut().pending.remove(&lot_key(cx, cz));
        }
    }

    // ---- lot objects (§8.6) ----

    /// `GetPositionForLotConfigurationObject(lot, object i)`: `None` when the
    /// lot has no occupant or configuration, or `i` is past the objects
    /// [choice].
    pub fn object_position(&self, lot: &LotEntry, i: usize) -> Option<(Position, u32)> {
        let h = &*self.host;
        let n = lot.occupant?;
        let obj = lot.config.as_ref()?.objects.get(i)?.clone();
        Some(object_position(
            h.occupied_cell(n),
            h.rotation(n),
            h.flip(n) != 0,
            obj.orientation,
            obj.offset,
            |x, z| h.occupant_altitude(n, x, z),
        ))
    }

    /// `CreateLotObjects(lot)` (`0x0060D410`).
    pub fn create_lot_objects(&self, lot: &LotEntry) {
        let h = &*self.host;
        let Some(cfg) = lot.config.clone() else {
            return;
        };
        for (i, obj) in cfg.objects.iter().enumerate() {
            match obj.kind {
                OBJECT_PROP if obj.flags & 1 == 0 => {
                    let ids = h.object_ids(cfg.token, i);
                    if !ids.is_empty() {
                        let pick = self.draw(ids.len() as u32) as usize;
                        self.create_prop(lot, i, ids[pick.min(ids.len() - 1)]);
                    }
                }
                OBJECT_FLORA => {
                    let ids = h.object_ids(cfg.token, i);
                    if !ids.is_empty() {
                        let pick = self.draw(ids.len() as u32) as usize;
                        if let Some((pos, _)) = self.object_position(lot, i) {
                            if let Some(f) = h.add_flora(ids[pick.min(ids.len() - 1)], pos) {
                                h.flora_slot28(f, i as u32);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        if let Some(o) = lot.occupant {
            if h.traffic_map_exists() {
                h.update_intersection(o);
            }
        }
    }

    /// Slot 8 `CreateProp(lot, i, id)` (`0x0060CF80`): the street prop
    /// (borrowed) or `None`.
    pub fn create_prop(&self, lot: &LotEntry, i: usize, id: u32) -> Option<u64> {
        let h = &*self.host;
        let (pos, orientation) = self.object_position(lot, i)?;
        if id == 0 {
            return None;
        }
        let p = h.add_street_prop(id, pos, orientation)?;
        let field = lot
            .config
            .as_ref()
            .and_then(|c| c.objects.get(i))
            .map_or(0, |o| o.field_24);
        h.street_prop_slot28(p, field);
        if lot.occupant.is_some_and(|o| h.flip(o) != 0) {
            h.street_prop_slot29(p, FLIPPED_PROP_FLAG);
        }
        Some(p)
    }

    /// Slot 12 `RedevelopLot(x, z, lot)` (`0x0060D5E0`): the cell's street
    /// props and flora go, and the lot's objects are created again (new
    /// draws, §17).
    pub fn redevelop_lot(&self, x: i32, z: i32, lot: &LotEntry) {
        let h = &*self.host;
        h.remove_street_props(x & 0xFF, z & 0xFF);
        h.demolish_flora(x, z);
        self.create_lot_objects(lot);
    }

    // ---- potholes (§8.8) ----

    /// `DoRoadDamageChange(x, z, damaged)` (`0x0060DF50`).
    pub fn do_road_damage_change(&self, x: i32, z: i32, damaged: bool) {
        let h = &*self.host;
        let key = lot_key(x, z);
        if !damaged {
            let old = self.s.borrow_mut().potholes.remove(&key);
            if let Some(Some(p)) = old {
                h.effect_stop(p);
                h.remove_occupant(p);
                h.release(p);
            }
            return;
        }
        if self.s.borrow().potholes.contains_key(&key) {
            return;
        }
        let Some(p) = h.create_effect_prop() else {
            return;
        };
        let (mut px, mut pz) = h.cell_center(x, z);
        let mut py = h.terrain_altitude(px, pz);
        let mut name = POTHOLE_NAME;
        if let Some(o) = h.first_network_occupant_at(x, z, flag::SURFACE) {
            py = h.occupant_altitude(o, px, pz);
            let (dx, dz) = pothole_offset(h.active_edges(o) & 0xFF);
            px += 4.0f32 * f32::from(dx);
            pz += 4.0f32 * f32::from(dz);
            if h.network_flags(o) & flag::ALL_TYPES == flag::RAIL {
                name = POTHOLE_RAIL_NAME;
            }
        }
        h.effect_set_name(p, name);
        h.set_position(p, [px, py, pz]);
        h.insert_occupant(p);
        h.effect_start(p);
        self.s.borrow_mut().potholes.insert(key, Some(p));
    }

    // ---- the record (§4.2) ----

    fn read_record(&self, inp: &mut dyn InStream) -> bool {
        if inp.get_u16() != Some(LOT_RECORD_VERSION) {
            return false;
        }
        let h = &*self.host;
        let resolve = |r: RefValue, iid: u32| if r.is_null() { None } else { h.resolve_ref(&r, iid) };
        if let Some(n) = inp.get_u32() {
            for _ in 0..n {
                // [choice] A stream error stops the loop.
                let Some(key) = inp.get_u16() else { break };
                let Some(r) = inp.get_ref_as(IID_NETWORK_OCCUPANT) else {
                    break;
                };
                let occupant = resolve(r, IID_NETWORK_OCCUPANT);
                let Some(config_id) = inp.get_u32() else {
                    break;
                };
                let lot = LotEntry {
                    occupant,
                    config: None,
                    config_id,
                    locked: false,
                };
                // An existing entry is overwritten without release.
                self.s.borrow_mut().lots.insert(key, lot);
            }
        }
        if let Some(m) = inp.get_u32() {
            for _ in 0..m {
                let Some(key) = inp.get_u16() else { break };
                let Some(r) = inp.get_ref_as(IID_EFFECT_PROP) else {
                    break;
                };
                let p = resolve(r, IID_EFFECT_PROP);
                self.s.borrow_mut().potholes.insert(key, p);
            }
        }
        inp.error() == 0
    }
}

fn inside(x: i32, z: i32, size: (i32, i32)) -> bool {
    x >= 0 && z >= 0 && x < size.0 && z < size.1
}

/// The §6.3 lot-state boundary walk over the grown rect `r` (inclusive
/// x1..x2, z1..z2), in visiting order, not clipped to the city. The
/// iterator starts at (0, 0) with step 1 and only its rectangle is set
/// (`x2 := max(x2, x1)`, `z2 := max(z2, z1)`) *[quirk]*:
/// fresh → (0, 0); running → `x += step`, `x ≤ x2` → (x, z); else done if
/// `z ≥ z2`, else `x := x1`, `z += 1`, `step := max(x2 − x1, 1)` if
/// `z < z2` else 1, → (x1, z).
pub fn lot_state_walk(r: CellRect) -> Vec<(i32, i32)> {
    let (x0, z0) = (r.x1, r.z1);
    let x1 = r.x2.max(x0);
    let z1 = r.z2.max(z0);
    let (mut x, mut z, mut step) = (0i32, 0i32, 1i32);
    let mut out = vec![(x, z)];
    loop {
        x += step;
        if x <= x1 {
            out.push((x, z));
            continue;
        }
        if z >= z1 {
            return out;
        }
        x = x0;
        z += 1;
        step = if z < z1 { (x1 - x0).max(1) } else { 1 };
        out.push((x, z));
    }
}

impl SaveObject for NetworkLotManager {
    fn class_id(&self) -> u32 {
        CLSID_NETWORK_LOT_MANAGER
    }

    /// §4.2 `Write` (`0x0060D6B0`): `lots` then `potholes`, in map order;
    /// the configuration ID is the live configuration's (the read ID before
    /// `Init` [choice]).
    fn write(&self, out: &mut dyn OutStream) -> bool {
        let h = &*self.host;
        let (lots, potholes) = {
            let s = self.s.borrow();
            (
                s.lots.iter().map(|(k, v)| (*k, v.clone())).collect::<Vec<_>>(),
                s.potholes.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>(),
            )
        };
        let r = |t: Option<u64>| t.map_or(RefValue::Null, |t| h.object_ref(t));
        out.put_u16(LOT_RECORD_VERSION);
        out.put_u32(lots.len() as u32);
        for (k, l) in lots {
            out.put_u16(k);
            out.put_ref(&r(l.occupant));
            out.put_u32(l.config.as_ref().map_or(l.config_id, |c| c.id));
        }
        out.put_u32(potholes.len() as u32);
        for (k, p) in potholes {
            out.put_u16(k);
            out.put_ref(&r(p));
        }
        out.error() == 0
    }

    /// §4.2 `Read` (`0x0060DAC0`): false unless the version is 2.
    fn read(&self, inp: &mut dyn InStream) -> bool {
        self.read_record(inp)
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }

    fn host_token(&self) -> u64 {
        self.token.get()
    }
}
