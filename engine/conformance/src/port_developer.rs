// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Conformance ops for `docs/specs/port-developers.md`: a scripted city
//! (the shipped port exemplars, lots and building occupants, the budget
//! line, office counts per subnetwork, freight arrivals, the terrain's
//! water) and ops that drive an airport or seaport developer. See
//! README.md ("Port developer ops").

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::{Rc, Weak};

use fc4_core::port_developer::tuning::shipped_exemplars;
use fc4_core::port_developer::{query_interface, CellRect, PortDeveloper, PortHost, PortKind};
use fc4_core::property::PropertyHolder;
use fc4_core::stream::{ByteReader, ByteWriter, SaveObject};

use crate::foundations::{hex_decode, hex_encode};
use crate::json::Json;
use crate::runner::{boolean, field, HostRef};
use crate::scalar::{
    self, arr, bool_of, check_expect, check_state, holder_of, i64_of, jf, ji, opt, str_of, u32_of, EventLog, R,
};

#[derive(Clone, Debug)]
struct Lot {
    config: Option<(u32, u32)>,
    x: i16,
    z: i16,
    state: u32,
    condition: u32,
    days: u32,
    rect: CellRect,
    facing: u32,
    compatible: bool,
    subnetworks: Vec<u32>,
    arrived: u32,
}

#[derive(Clone, Copy)]
struct Occ {
    preview: bool,
    occ_type: u32,
    group: u32,
    lot: u64,
}

pub(crate) struct World {
    me: RefCell<Weak<World>>,
    events: EventLog,
    dev: RefCell<Option<Rc<PortDeveloper>>>,
    kind: Cell<PortKind>,
    exemplars: RefCell<BTreeMap<u32, Option<Rc<PropertyHolder>>>>,
    lots: RefCell<BTreeMap<u64, Lot>>,
    occupants: RefCell<BTreeMap<u64, Occ>>,
    department: Cell<Option<u64>>,
    line: Cell<Option<(i64, i64)>>,
    /// Current expenses follow the full expenses (fully funded).
    follow: Cell<bool>,
    offices: RefCell<(Vec<u32>, Vec<u32>)>,
    terrain: Cell<(u32, u32)>,
    water: RefCell<BTreeSet<(u32, u32)>>,
    create_lot: Cell<bool>,
    construct: Cell<bool>,
    /// Lot configuration IDs the configuration manager does not have (PD5).
    missing_configs: RefCell<BTreeSet<u32>>,
    next: Cell<u64>,
    records: RefCell<BTreeMap<String, Vec<u8>>>,
}

impl World {
    fn new() -> Rc<World> {
        let w = Rc::new(World {
            me: RefCell::new(Weak::new()),
            events: EventLog::default(),
            dev: RefCell::new(None),
            kind: Cell::new(PortKind::Airport),
            exemplars: RefCell::new(BTreeMap::new()),
            lots: RefCell::new(BTreeMap::new()),
            occupants: RefCell::new(BTreeMap::new()),
            department: Cell::new(None),
            line: Cell::new(None),
            follow: Cell::new(true),
            offices: RefCell::new((Vec::new(), Vec::new())),
            terrain: Cell::new((0, 0)),
            water: RefCell::new(BTreeSet::new()),
            create_lot: Cell::new(true),
            construct: Cell::new(true),
            missing_configs: RefCell::new(BTreeSet::new()),
            next: Cell::new(0x9000),
            records: RefCell::new(BTreeMap::new()),
        });
        *w.me.borrow_mut() = Rc::downgrade(&w);
        for airport in [true, false] {
            for (k, h) in shipped_exemplars(airport) {
                w.exemplars.borrow_mut().insert(k, Some(Rc::new(h)));
            }
        }
        w
    }

    fn rc(&self) -> Rc<World> {
        self.me.borrow().upgrade().expect("world alive")
    }

    fn dev(&self) -> R<Rc<PortDeveloper>> {
        self.dev
            .borrow()
            .clone()
            .ok_or_else(|| "no port developer (pd_create)".into())
    }

    fn new_dev(&self) -> Rc<PortDeveloper> {
        let d = PortDeveloper::new(self.kind.get(), self.rc() as Rc<dyn PortHost>, 0xC0);
        *self.dev.borrow_mut() = Some(d.clone());
        d
    }

    fn lot(&self, lot: u64) -> Lot {
        self.lots
            .borrow()
            .get(&lot)
            .cloned()
            .unwrap_or_else(|| default_lot(0, 0))
    }

    fn edit(&self, lot: u64, f: impl FnOnce(&mut Lot)) {
        let mut l = self.lot(lot);
        f(&mut l);
        self.lots.borrow_mut().insert(lot, l);
    }
}

fn default_lot(x: i16, z: i16) -> Lot {
    Lot {
        config: None,
        x,
        z,
        state: 3,
        condition: 0,
        days: 0,
        rect: CellRect {
            x0: i32::from(x),
            z0: i32::from(z),
            x1: i32::from(x),
            z1: i32::from(z),
        },
        facing: 0,
        compatible: true,
        subnetworks: vec![0],
        arrived: 0,
    }
}

fn t(v: u64) -> Json {
    ji(v as i64)
}

fn rect_json(r: CellRect) -> Json {
    Json::Array(vec![ji(r.x0), ji(r.z0), ji(r.x1), ji(r.z1)])
}

impl PortHost for World {
    fn exemplar(&self, instance: u32) -> Option<Rc<PropertyHolder>> {
        self.exemplars.borrow().get(&instance).cloned().flatten()
    }
    fn add_agent(&self, name: &str, level: u32, flags: u32) {
        self.events.push(
            "add_agent",
            vec![
                ("name", Json::Str(name.into())),
                ("level", ji(level)),
                ("flags", ji(flags)),
            ],
        );
    }
    fn remove_agent(&self, name: &str) {
        self.events.push("remove_agent", vec![("name", Json::Str(name.into()))]);
    }
    fn department(&self, _id: u32) -> Option<u64> {
        self.department.get()
    }
    fn create_department(&self, id: u32, group: u32, name: (u32, u32), max: f32, line: u32) -> Option<u64> {
        self.events.push(
            "create_department",
            vec![
                ("id", ji(id)),
                ("group", ji(group)),
                ("name_group", ji(name.0)),
                ("name", ji(name.1)),
                ("max", jf(max)),
                ("line", ji(line)),
            ],
        );
        self.department.set(Some(0xD));
        if self.line.get().is_none() {
            self.line.set(Some((0, 0)));
        }
        Some(0xD)
    }
    fn line_expenses(&self, _d: u64, _line: u32) -> Option<(i64, i64)> {
        self.line.get()
    }
    fn set_full_expenses(&self, _d: u64, _line: u32, v: i64) {
        let cur = if self.follow.get() {
            v
        } else {
            self.line.get().map_or(0, |l| l.1)
        };
        self.line.set(Some((v, cur)));
        self.events.push("full_expenses", vec![("value", ji(v))]);
    }
    fn withdraw_funds(&self, amount: i64) -> bool {
        self.events.push("withdraw", vec![("amount", ji(amount))]);
        false
    }
    fn occupant_is_preview(&self, o: u64) -> bool {
        self.occupants.borrow().get(&o).is_some_and(|o| o.preview)
    }
    fn occupant_type(&self, o: u64) -> u32 {
        self.occupants.borrow().get(&o).map_or(0, |o| o.occ_type)
    }
    fn occupant_in_group(&self, o: u64, group: u32) -> bool {
        self.occupants.borrow().get(&o).is_some_and(|o| o.group == group)
    }
    fn occupant_lot(&self, o: u64) -> Option<u64> {
        self.occupants.borrow().get(&o).map(|o| o.lot)
    }
    fn lot_config(&self, lot: u64) -> Option<(u32, u32)> {
        self.lot(lot).config
    }
    fn lot_location(&self, lot: u64) -> (i16, i16) {
        let l = self.lot(lot);
        (l.x, l.z)
    }
    fn lot_at(&self, x: i16, z: i16) -> Option<u64> {
        self.lots
            .borrow()
            .iter()
            .find(|(_, l)| l.x == x && l.z == z)
            .map(|(&k, _)| k)
    }
    fn lot_state(&self, lot: u64) -> u32 {
        self.lot(lot).state
    }
    fn lot_condition(&self, lot: u64) -> u32 {
        self.lot(lot).condition
    }
    fn set_lot_condition(&self, lot: u64, c: u32) {
        self.edit(lot, |l| l.condition = c);
        self.events
            .push("condition", vec![("lot", t(lot)), ("condition", ji(c))]);
    }
    fn lot_days_since_state_change(&self, lot: u64) -> u32 {
        self.lot(lot).days
    }
    fn lot_rect(&self, lot: u64) -> CellRect {
        self.lot(lot).rect
    }
    fn lot_facing(&self, lot: u64) -> u32 {
        self.lot(lot).facing
    }
    fn lot_zone_compatible(&self, lot: u64, _zone: u16) -> bool {
        self.lot(lot).compatible
    }
    fn lot_configuration(&self, id: u32) -> Option<u64> {
        (!self.missing_configs.borrow().contains(&id)).then_some(u64::from(id))
    }
    fn start_demolish_lot(&self, lot: u64) {
        self.events.push("start_demolish", vec![("lot", t(lot))]);
    }
    fn end_demolish_lot(&self, lot: u64) {
        self.events.push("end_demolish", vec![("lot", t(lot))]);
    }
    fn create_lot(&self, x: i32, z: i32, w: i32, d: i32, facing: u32) -> Option<u64> {
        let tok = self.create_lot.get().then(|| {
            let tok = self.next.get();
            self.next.set(tok + 1);
            let mut l = default_lot(x as i16, z as i16);
            l.state = 2;
            l.facing = facing;
            l.rect = CellRect {
                x0: x,
                z0: z,
                x1: x + w - 1,
                z1: z + d - 1,
            };
            self.lots.borrow_mut().insert(tok, l);
            tok
        });
        self.events.push(
            "create_lot",
            vec![
                ("x", ji(x)),
                ("z", ji(z)),
                ("width", ji(w)),
                ("depth", ji(d)),
                ("facing", ji(facing)),
                ("lot", tok.map_or(Json::Null, t)),
            ],
        );
        tok
    }
    fn set_lot_configuration(&self, lot: u64, config: Option<u64>) {
        self.edit(lot, |l| l.config = config.map(|c| (c as u32, 2)));
        self.events.push(
            "set_configuration",
            vec![("lot", t(lot)), ("config", config.map_or(Json::Null, t))],
        );
    }
    fn construct_default_lot(&self, lot: u64, config: Option<u64>) -> bool {
        self.events
            .push("construct", vec![("lot", t(lot)), ("config", config.map_or(Json::Null, t))]);
        if config.is_none() {
            // The lot developer with a null configuration (lot-developer.md
            // §6.4, PD5): an empty lot in state 2 with condition 1, true.
            self.edit(lot, |l| {
                l.state = 2;
                l.condition = 1;
            });
            return true;
        }
        self.construct.get()
    }
    fn dezone(&self, r: CellRect) {
        self.events.push("dezone", vec![("rect", rect_json(r))]);
    }
    fn update_occupant_effects(&self, lot: u64, old: f32, new: f32) {
        self.events
            .push("effects", vec![("lot", t(lot)), ("old", jf(old)), ("new", jf(new))]);
    }
    fn record_history(&self, key: u32, value: i32) {
        self.events
            .push("history", vec![("key", ji(key)), ("value", ji(value))]);
    }
    fn push_event(&self, event: u32) {
        self.events.push("advisor", vec![("id", ji(event))]);
    }
    fn office_counts(&self, developer_type: u32) -> Vec<u32> {
        let o = self.offices.borrow();
        if developer_type == 0x3320 {
            o.0.clone()
        } else {
            o.1.clone()
        }
    }
    fn lot_subnetworks(&self, lot: u64) -> Vec<u32> {
        self.lot(lot).subnetworks
    }
    fn traffic_arrived(&self, lot: u64, _t: u32) -> u32 {
        self.lot(lot).arrived
    }
    fn set_max_trip_capacity(&self, lot: u64, trip_type: u32, cap: u32) {
        self.events.push(
            "trip_capacity",
            vec![("lot", t(lot)), ("type", ji(trip_type)), ("capacity", ji(cap))],
        );
    }
    fn terrain_size(&self) -> (u32, u32) {
        self.terrain.get()
    }
    fn cell_has_water(&self, x: u32, z: u32) -> bool {
        self.water.borrow().contains(&(x, z))
    }
    fn set_access_props_state(&self, lot: u64, property: u32, state: u32) -> u32 {
        self.events.push(
            "props_state",
            vec![("lot", t(lot)), ("property", ji(property)), ("state", ji(state))],
        );
        1
    }
    fn set_altitude(&self, x: i32, z: i32, altitude: f32) {
        self.events
            .push("altitude", vec![("x", ji(x)), ("z", ji(z)), ("altitude", jf(altitude))]);
    }
}

fn world(host: &HostRef) -> Rc<World> {
    scalar::world(host, "port_developer", World::new)
}

pub(crate) fn exec(host: &HostRef, name: &str, op: &Json) -> Option<R<()>> {
    let rest = name.strip_prefix("pd_")?;
    let w = world(host);
    Some(match rest {
        "env" => env(&w, op),
        "lot" => lot(&w, op),
        "occupant" => occupant(&w, op),
        "create" => {
            if let Some(k) = opt(op, "kind") {
                match kind_of(k) {
                    Ok(k) => w.kind.set(k),
                    Err(e) => return Some(Err(e)),
                }
            }
            w.new_dev();
            Ok(())
        }
        "init" => w.dev().and_then(|d| check_expect(op, Json::Bool(d.init()))),
        "shutdown" => w.dev().and_then(|d| check_expect(op, Json::Bool(d.shutdown()))),
        "add_ref" => w.dev().and_then(|d| check_expect(op, ji(d.add_ref()))),
        "release" => w.dev().and_then(|d| check_expect(op, ji(d.release()))),
        "message" => message(&w, op),
        "month" => w.dev().and_then(|d| check_expect(op, Json::Bool(d.month_tick()))),
        "set_port" => set_port(&w, op),
        "identity" => identity(op),
        "query" => query(&w, op),
        "expect" => expect(&w, op),
        "expect_port" => expect_port(&w, op),
        "expect_map" => expect_map(&w, op),
        "expect_events" => w.events.expect_events(op),
        "expect_event_count" => w.events.expect_count(op),
        "clear_events" => {
            w.events.clear();
            Ok(())
        }
        "write" => write(&w, op),
        "read" => read(&w, op),
        _ => Err(format!("unknown op '{name}'")),
    })
}

fn token_of(j: &Json, what: &str) -> R<u64> {
    Ok(i64_of(j, what)? as u64)
}

fn kind_of(j: &Json) -> R<PortKind> {
    match str_of(j, "kind")? {
        "airport" => Ok(PortKind::Airport),
        "seaport" => Ok(PortKind::Seaport),
        other => Err(format!("unknown port kind '{other}'")),
    }
}

fn u32_list(j: &Json, what: &str) -> R<Vec<u32>> {
    arr(j, what)?.iter().map(|v| u32_of(v, what)).collect()
}

/// `pd_env`: `kind`; `line` (`[full, current]` of line 1, or null: no
/// line); `follow` (current expenses follow SetFullExpenses);
/// `department` (an existing department token, null for none); `offices`
/// (`[[CO$$ per subnetwork], [CO$$$ per subnetwork]]`); `terrain` (`[width,
/// depth]`); `water` (`[[x, z]]` water cells, replacing the set);
/// `create_lot` and `construct` (whether CreateLot and ConstructDefaultLot
/// succeed); `missing_configs` (lot configuration IDs the manager does not
/// have, replacing the set; ConstructDefaultLot then acts as the lot
/// developer does with a null configuration: state 2, condition 1, true);
/// `exemplars` (`{"0xINSTANCE": {properties} | null}` replacing shipped
/// exemplars, null removing one).
fn env(w: &World, op: &Json) -> R<()> {
    if let Some(v) = opt(op, "kind") {
        w.kind.set(kind_of(v)?);
    }
    match op.get("line") {
        None => {}
        Some(Json::Null) => w.line.set(None),
        Some(v) => {
            let l = arr(v, "line")?;
            if l.len() != 2 {
                return Err("line is [full, current]".into());
            }
            w.line.set(Some((i64_of(&l[0], "full")?, i64_of(&l[1], "current")?)));
        }
    }
    if let Some(v) = opt(op, "follow") {
        w.follow.set(bool_of(v, "follow")?);
    }
    match op.get("department") {
        None => {}
        Some(Json::Null) => w.department.set(None),
        Some(v) => w.department.set(Some(token_of(v, "department")?)),
    }
    if let Some(v) = opt(op, "offices") {
        let o = arr(v, "offices")?;
        if o.len() != 2 {
            return Err("offices is [[CO$$...], [CO$$$...]]".into());
        }
        *w.offices.borrow_mut() = (u32_list(&o[0], "offices")?, u32_list(&o[1], "offices")?);
    }
    if let Some(v) = opt(op, "terrain") {
        let s = u32_list(v, "terrain")?;
        if s.len() != 2 {
            return Err("terrain is [width, depth]".into());
        }
        w.terrain.set((s[0], s[1]));
    }
    if let Some(v) = opt(op, "water") {
        let mut set = BTreeSet::new();
        for c in arr(v, "water")? {
            let c = u32_list(c, "water cell")?;
            if c.len() != 2 {
                return Err("a water cell is [x, z]".into());
            }
            set.insert((c[0], c[1]));
        }
        *w.water.borrow_mut() = set;
    }
    if let Some(v) = opt(op, "create_lot") {
        w.create_lot.set(bool_of(v, "create_lot")?);
    }
    if let Some(v) = opt(op, "construct") {
        w.construct.set(bool_of(v, "construct")?);
    }
    if let Some(v) = opt(op, "missing_configs") {
        *w.missing_configs.borrow_mut() = u32_list(v, "missing_configs")?.into_iter().collect();
    }
    if let Some(v) = opt(op, "exemplars") {
        for (k, props) in v.as_object().ok_or("exemplars must be an object")? {
            let inst = Json::Str(k.clone()).as_i64().ok_or("bad instance")? as u32;
            let h = match props {
                Json::Null => None,
                p => Some(Rc::new(holder_of(p)?)),
            };
            w.exemplars.borrow_mut().insert(inst, h);
        }
    }
    Ok(())
}

/// `pd_lot`: creates or updates `lot`: `config` (`[id, growth stage]` or
/// null), `x`, `z`, `state`, `condition`, `days` (since the last state
/// change), `rect` (`[x0, z0, x1, z1]`; defaults to the location's cell),
/// `facing`, `compatible` (with the zone type), `subnetworks`, `arrived`
/// (freight arrivals).
fn lot(w: &World, op: &Json) -> R<()> {
    let tok = token_of(field(op, "lot")?, "lot")?;
    let exists = w.lots.borrow().contains_key(&tok);
    let mut l = if exists {
        w.lot(tok)
    } else {
        let x = opt(op, "x").map_or(Ok(0), |v| i64_of(v, "x"))? as i16;
        let z = opt(op, "z").map_or(Ok(0), |v| i64_of(v, "z"))? as i16;
        default_lot(x, z)
    };
    match op.get("config") {
        None => {}
        Some(Json::Null) => l.config = None,
        Some(v) => {
            let c = u32_list(v, "config")?;
            if c.len() != 2 {
                return Err("config is [id, growth stage]".into());
            }
            l.config = Some((c[0], c[1]));
        }
    }
    if let Some(v) = opt(op, "x") {
        l.x = i64_of(v, "x")? as i16;
    }
    if let Some(v) = opt(op, "z") {
        l.z = i64_of(v, "z")? as i16;
    }
    for (k, f) in [
        ("state", &mut l.state),
        ("condition", &mut l.condition),
        ("days", &mut l.days),
        ("facing", &mut l.facing),
        ("arrived", &mut l.arrived),
    ] {
        if let Some(v) = opt(op, k) {
            *f = u32_of(v, k)?;
        }
    }
    if let Some(v) = opt(op, "rect") {
        let r = arr(v, "rect")?;
        if r.len() != 4 {
            return Err("rect is [x0, z0, x1, z1]".into());
        }
        l.rect = CellRect {
            x0: i64_of(&r[0], "x0")? as i32,
            z0: i64_of(&r[1], "z0")? as i32,
            x1: i64_of(&r[2], "x1")? as i32,
            z1: i64_of(&r[3], "z1")? as i32,
        };
    } else if !exists {
        l.rect = default_lot(l.x, l.z).rect;
    }
    if let Some(v) = opt(op, "compatible") {
        l.compatible = bool_of(v, "compatible")?;
    }
    if let Some(v) = opt(op, "subnetworks") {
        l.subnetworks = u32_list(v, "subnetworks")?;
    }
    w.lots.borrow_mut().insert(tok, l);
    Ok(())
}

/// `pd_occupant`: `occupant` on `lot`, `type` ("building" or an ID;
/// default building), `group` (default the developer kind's), `preview`.
fn occupant(w: &World, op: &Json) -> R<()> {
    let occ = token_of(field(op, "occupant")?, "occupant")?;
    let occ_type = match op.get("type") {
        None => 0x2781_28A0,
        Some(j) if j.as_str() == Some("building") => 0x2781_28A0,
        Some(j) => u32_of(j, "type")?,
    };
    let o = Occ {
        preview: opt(op, "preview").map_or(Ok(false), |v| bool_of(v, "preview"))?,
        occ_type,
        group: opt(op, "group").map_or(Ok(w.kind.get().occupant_group()), |v| u32_of(v, "group"))?,
        lot: token_of(field(op, "lot")?, "lot")?,
    };
    w.occupants.borrow_mut().insert(occ, o);
    Ok(())
}

/// `pd_message`: `DoMessage(msg, data, old, new)`.
fn message(w: &World, op: &Json) -> R<()> {
    let d = w.dev()?;
    let msg = u32_of(field(op, "msg")?, "msg")?;
    let data = opt(op, "data").map_or(Ok(0), |v| token_of(v, "data"))?;
    let old = opt(op, "old").map_or(Ok(0), |v| u32_of(v, "old"))?;
    let new = opt(op, "new").map_or(Ok(0), |v| u32_of(v, "new"))?;
    check_expect(op, Json::Bool(d.do_message(msg, data, old, new)))
}

/// `pd_set_port`: overrides fields of port `index` (`stage`, `trips`,
/// `efficiency`, `usage`).
fn set_port(w: &World, op: &Json) -> R<()> {
    let d = w.dev()?;
    let i = i64_of(field(op, "index")?, "index")? as usize;
    let mut p = *d.ports().get(i).ok_or("no such port")?;
    if let Some(v) = opt(op, "stage") {
        p.stage = u32_of(v, "stage")?;
    }
    if let Some(v) = opt(op, "trips") {
        p.trips = u32_of(v, "trips")?;
    }
    if let Some(v) = opt(op, "efficiency") {
        p.efficiency = scalar::f32_of(v, "efficiency")?;
    }
    if let Some(v) = opt(op, "usage") {
        p.usage = scalar::f32_of(v, "usage")?;
    }
    d.set_port_fields(i, p);
    Ok(())
}

/// `pd_identity`: `kind` and `iids` (`{"0xIID": offset | false}`).
fn identity(op: &Json) -> R<()> {
    let kind = kind_of(field(op, "kind")?)?;
    for (k, want) in field(op, "iids")?.as_object().ok_or("iids must be an object")? {
        let iid = Json::Str(k.clone()).as_i64().ok_or("bad iid")? as u32;
        let got = query_interface(kind, iid).map_or(Json::Bool(false), ji);
        if !scalar::matches(&got, want, false) {
            return Err(format!("QueryInterface({k}) = {}", scalar::show(&got)));
        }
    }
    Ok(())
}

/// `pd_query`: `query` is one of `num_ports` (`args` `[size, stage]`),
/// `num_airports` (`[size]`), `num_seaports`, `num_trips`,
/// `avg_efficiency`, `least_efficient` (`[efficiency, x, z]` or false),
/// `all_port_lots`, `usage_values` (`[lot]`: `[usage, cap]` or false),
/// `maintenance_cost` (`[lot]`), `city_has_sea`, `cell_sea_access`
/// (`[x, z]`), `funding_ratio`, `required_budget`; checked against
/// `expect`.
fn query(w: &World, op: &Json) -> R<()> {
    let d = w.dev()?;
    let args: Vec<i64> = match opt(op, "args") {
        Some(a) => arr(a, "args")?.iter().map(|v| i64_of(v, "arg")).collect::<R<_>>()?,
        None => Vec::new(),
    };
    let a = |i: usize| args.get(i).copied().ok_or_else(|| format!("query needs argument {i}"));
    let got = match str_of(field(op, "query")?, "query")? {
        "num_ports" => ji(d.num_ports(a(0)? as u32, a(1)? as u32)),
        "num_airports" => ji(d.num_airports(a(0)? as u32)),
        "num_seaports" => ji(d.num_seaports()),
        "num_trips" => ji(d.num_trips()),
        "avg_efficiency" => jf(d.avg_efficiency()),
        "least_efficient" => d
            .least_efficient_port()
            .map_or(Json::Bool(false), |(e, x, z)| Json::Array(vec![jf(e), ji(x), ji(z)])),
        "all_port_lots" => Json::Array(d.all_port_lots().into_iter().map(t).collect()),
        "usage_values" => d
            .usage_values(a(0)? as u64)
            .map_or(Json::Bool(false), |(u, c)| Json::Array(vec![ji(u), ji(c)])),
        "maintenance_cost" => ji(d.maintenance_cost(a(0)? as u64)),
        "city_has_sea" => Json::Bool(d.city_has_sea()),
        "cell_sea_access" => ji(d.cell_sea_access(a(0)? as i32, a(1)? as i32)),
        "funding_ratio" => jf(d.funding_ratio()),
        "required_budget" => ji(d.required_budget()),
        other => return Err(format!("unknown query '{other}'")),
    };
    check_expect(op, got)
}

fn expect(w: &World, op: &Json) -> R<()> {
    let d = w.dev()?;
    let s = d.snapshot();
    let m = d.last_month();
    let (full, cur) = w.line.get().unwrap_or((0, 0));
    let got = vec![
        ("tuning_loaded".to_owned(), Json::Bool(s.tuning_loaded)),
        ("initialized".to_owned(), Json::Bool(s.initialized)),
        ("tuning_instance".to_owned(), ji(s.tuning_instance)),
        ("type_count".to_owned(), ji(s.type_count)),
        ("types".to_owned(), ji(d.types().len() as i64)),
        ("zone_type".to_owned(), ji(s.zone_type)),
        ("ports".to_owned(), ji(d.ports().len() as i64)),
        ("ref_count".to_owned(), ji(s.ref_count)),
        ("destroyed".to_owned(), Json::Bool(s.destroyed)),
        ("recheck_access".to_owned(), Json::Bool(s.recheck_access)),
        ("city_has_sea".to_owned(), Json::Bool(s.city_has_sea)),
        ("department".to_owned(), s.department.map_or(Json::Null, t)),
        ("full".to_owned(), ji(full)),
        ("current".to_owned(), ji(cur)),
        ("month_ran".to_owned(), Json::Bool(m.ran)),
        ("history".to_owned(), ji(m.history)),
        ("event".to_owned(), ji(m.event)),
        ("skipped".to_owned(), ji(m.skipped)),
    ];
    check_state(&got, field(op, "state")?)
}

/// `pd_expect_port`: port `index`'s fields (`stage`, `type`, `x`, `z`,
/// `efficiency`, `usage`, `trips`, `lot` (null: none), `needs_lookup`,
/// `swapping`, `access`).
fn expect_port(w: &World, op: &Json) -> R<()> {
    let d = w.dev()?;
    let i = i64_of(field(op, "index")?, "index")? as usize;
    let p = *d
        .ports()
        .get(i)
        .ok_or_else(|| format!("no port {i} ({} ports)", d.ports().len()))?;
    let mut got = vec![
        ("stage".to_owned(), ji(p.stage)),
        ("type".to_owned(), ji(p.type_index as i64)),
        ("x".to_owned(), ji(p.x)),
        ("z".to_owned(), ji(p.z)),
        ("efficiency".to_owned(), jf(p.efficiency)),
        ("usage".to_owned(), jf(p.usage)),
        ("trips".to_owned(), ji(p.trips)),
        ("lot".to_owned(), p.lot.map_or(Json::Null, t)),
        ("needs_lookup".to_owned(), Json::Bool(p.needs_lookup)),
        ("swapping".to_owned(), Json::Bool(p.swapping)),
    ];
    if op.get("state").and_then(|s| s.get("access")).is_some() {
        got.push(("access".to_owned(), Json::Bool(d.port_has_access(i))));
    }
    check_state(&got, field(op, "state")?)
}

/// `pd_expect_map`: the sea-access map, row-major (`cells`), and its size.
fn expect_map(w: &World, op: &Json) -> R<()> {
    let m = w.dev()?.water_map();
    if let Some(s) = opt(op, "size") {
        let s = u32_list(s, "size")?;
        if s != [m.width, m.height] {
            return Err(format!("map {}×{}, expected {s:?}", m.width, m.height));
        }
    }
    let want = u32_list(field(op, "cells")?, "cells")?;
    let got: Vec<u32> = m.cells.iter().map(|&c| u32::from(c)).collect();
    if got != want {
        return Err(format!("map cells {got:02X?}, expected {want:02X?}"));
    }
    Ok(())
}

/// `pd_write`: the record; `hex`, `slot`.
fn write(w: &World, op: &Json) -> R<()> {
    let d = w.dev()?;
    let mut out = ByteWriter::new();
    check_expect(op, Json::Bool(d.write(&mut out)))?;
    let bytes = out.into_bytes();
    if let Some(h) = opt(op, "hex") {
        let want = hex_decode(str_of(h, "hex")?)?;
        if bytes != want {
            return Err(format!(
                "record differs:\n    got      {}\n    expected {}",
                hex_encode(&bytes),
                hex_encode(&want)
            ));
        }
    }
    if let Some(slot) = opt(op, "slot") {
        w.records.borrow_mut().insert(str_of(slot, "slot")?.to_string(), bytes);
    }
    Ok(())
}

/// `pd_read`: `hex` or `slot` into the current developer (or a new one of
/// the world's kind with `fresh: true`).
fn read(w: &World, op: &Json) -> R<()> {
    let bytes = match opt(op, "hex") {
        Some(h) => hex_decode(str_of(h, "hex")?)?,
        None => {
            let slot = str_of(field(op, "slot")?, "slot")?;
            w.records
                .borrow()
                .get(slot)
                .cloned()
                .ok_or_else(|| format!("no record in slot '{slot}'"))?
        }
    };
    let fresh = opt(op, "fresh").and_then(boolean).unwrap_or(false);
    let d = if fresh { w.new_dev() } else { w.dev()? };
    check_expect(op, Json::Bool(d.read(&mut ByteReader::new(&bytes))))
}
