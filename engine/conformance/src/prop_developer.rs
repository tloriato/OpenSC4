// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Conformance ops for `docs/specs/prop-developer.md`: a scripted city (lots
//! and their configuration objects, network lots, props, prop profiles and
//! families, the host requesters, power, the clock, the lighting manager,
//! the 3D view) and ops that drive a prop developer. See README.md ("Prop
//! developer ops").
//!
//! Records are written and read with references as `u32 token, u32 class`
//! (the object ID is the token), so the spec's corpus records read and
//! write back byte for byte.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::rc::{Rc, Weak};

use fc4_core::grid::{CellRect, ScanState};
use fc4_core::lot_developer::CfgObject;
use fc4_core::prop_developer::{
    member_generator, member_seed, query_interface, wealth_generator, wealth_query_interface, PropDeveloper, PropHost,
    PropMessage, PropRandom, PropRef, Requester, SharedRandom, WealthRequester, CLSID_WEALTH_REQUESTER,
    PROP_REQUESTER_INDEX,
};
use fc4_core::property::PropertyHolder;
use fc4_core::stream::{ByteReader, ByteWriter, OutStream, RefResolver, RefValue, SaveObject};
use fc4_core::variant::{Elem, Variant};

use crate::foundations::{hex_decode, hex_encode};
use crate::json::Json;
use crate::runner::{boolean, field, HostRef};
use crate::scalar::{
    self, arr, bool_of, check_expect, check_state, f32_of, holder_of, i64_of, jf, ji, opt, str_of, u32_of, EventLog, R,
};

/// The occupant class of a prop (`cSC4PropOccupant`).
const PROP_CLASS: u32 = 0x2977_AA47;

#[derive(Clone)]
struct Lot {
    x: i32,
    z: i32,
    size: (i32, i32),
    livable: bool,
    building: u32,
    occupancy: f32,
    objects: Option<Rc<Vec<CfgObject>>>,
    wealth: u32,
    state: u32,
}

#[derive(Clone)]
struct NetLot {
    token: u64,
    objects: Rc<Vec<CfgObject>>,
    wealth: u32,
}

#[derive(Clone)]
struct Prop {
    lot: Option<u64>,
    cell: (i32, i32),
    tag: Option<u32>,
    ptype: u32,
    class: u32,
    props: PropertyHolder,
    state: u32,
    pos: [f32; 3],
    tod: (f32, f32),
}

#[derive(Clone, Copy)]
struct HostRequester {
    share: i32,
    satisfaction: u32,
    custom: bool,
    /// What `SelectProp` answers: `None` = the first ID.
    select: Option<u32>,
}

/// A generator that logs its draws: a real one or a script of results.
struct Logged {
    world: Weak<World>,
    name: &'static str,
    real: Option<Box<dyn PropRandom>>,
}

impl Logged {
    fn draw(&mut self, call: &str, args: Vec<(&str, Json)>, real: impl FnOnce(&mut dyn PropRandom) -> i64) -> i64 {
        let w = self.world.upgrade();
        let v = match (&mut self.real, &w) {
            (Some(r), _) => real(r.as_mut()),
            (None, Some(w)) => w.script(self.name).unwrap_or(0),
            (None, None) => 0,
        };
        if let Some(w) = w {
            let mut f = vec![("gen", Json::Str(self.name.into())), ("call", Json::Str(call.into()))];
            f.extend(args);
            f.push(("value", ji(v)));
            w.events.push("draw", f);
        }
        v
    }
}

impl PropRandom for Logged {
    fn uniform_u32(&mut self, n: u32) -> u32 {
        self.draw("U", vec![("n", ji(n))], |r| i64::from(r.uniform_u32(n))) as u32
    }
    fn range_i32(&mut self, lo: i32, hi: i32) -> i32 {
        self.draw("RS", vec![("lo", ji(lo)), ("hi", ji(hi))], |r| {
            i64::from(r.range_i32(lo, hi))
        }) as i32
    }
}

pub(crate) struct World {
    me: RefCell<Weak<World>>,
    events: EventLog,
    dev: RefCell<Option<Rc<PropDeveloper>>>,
    cells: Cell<(i32, i32)>,
    lots: RefCell<BTreeMap<u64, Lot>>,
    network: RefCell<BTreeMap<(i32, i32), NetLot>>,
    zones: RefCell<BTreeMap<(i32, i32), u32>>,
    props: RefCell<BTreeMap<u64, Prop>>,
    /// Props removed through the prop manager: the occupant object stays
    /// alive for its removal message (`0x99EF1143`), so its type and
    /// properties can still be read (§4.7).
    gone: RefCell<BTreeMap<u64, Prop>>,
    /// prop ID → (satisfaction, wealth)
    profiles: RefCell<BTreeMap<u32, (u32, u8)>>,
    families: RefCell<BTreeMap<u32, Vec<u32>>>,
    /// prop ID → the properties a created prop of that type starts with
    exemplars: RefCell<BTreeMap<u32, PropertyHolder>>,
    requesters: RefCell<BTreeMap<u64, HostRequester>>,
    unpowered: RefCell<BTreeSet<(i32, i32)>>,
    clock: Cell<Option<(u32, u32)>>,
    mode: Cell<u32>,
    night: Cell<bool>,
    view: Cell<bool>,
    triggers_on: RefCell<BTreeSet<u32>>,
    date: Cell<u32>,
    heights: Cell<(f32, f32)>,
    managers: Cell<[PropRef; 3]>,
    wealth_token: Cell<u64>,
    /// The session's wealth file generator (shared by every developer of a seed).
    wealth: RefCell<Option<(u32, SharedRandom)>>,
    scripts: RefCell<BTreeMap<&'static str, VecDeque<i64>>>,
    next: Cell<u64>,
    records: RefCell<BTreeMap<String, Vec<u8>>>,
}

impl World {
    fn new() -> Rc<World> {
        let w = Rc::new(World {
            me: RefCell::new(Weak::new()),
            events: EventLog::default(),
            dev: RefCell::new(None),
            cells: Cell::new((64, 64)),
            lots: RefCell::new(BTreeMap::new()),
            network: RefCell::new(BTreeMap::new()),
            zones: RefCell::new(BTreeMap::new()),
            props: RefCell::new(BTreeMap::new()),
            gone: RefCell::new(BTreeMap::new()),
            profiles: RefCell::new(BTreeMap::new()),
            families: RefCell::new(BTreeMap::new()),
            exemplars: RefCell::new(BTreeMap::new()),
            requesters: RefCell::new(BTreeMap::new()),
            unpowered: RefCell::new(BTreeSet::new()),
            clock: Cell::new(None),
            mode: Cell::new(0),
            night: Cell::new(false),
            view: Cell::new(true),
            triggers_on: RefCell::new(BTreeSet::new()),
            date: Cell::new(0),
            heights: Cell::new((0.0, 0.0)),
            managers: Cell::new([
                PropRef::new(0x1331_0038, 0x498F_9B01),
                PropRef::new(0x1137_876C, 0xA990_BD10),
                PropRef::new(0x13B4_35FC, 0xE9A6_CD3B),
            ]),
            wealth_token: Cell::new(0x13B5_2FC0),
            wealth: RefCell::new(None),
            scripts: RefCell::new(BTreeMap::new()),
            next: Cell::new(0x9000),
            records: RefCell::new(BTreeMap::new()),
        });
        *w.me.borrow_mut() = Rc::downgrade(&w);
        w
    }

    fn rc(&self) -> Rc<World> {
        self.me.borrow().upgrade().expect("world alive")
    }

    fn dev(&self) -> R<Rc<PropDeveloper>> {
        self.dev
            .borrow()
            .clone()
            .ok_or_else(|| "no prop developer (pv_create)".into())
    }

    fn script(&self, name: &'static str) -> Option<i64> {
        self.scripts.borrow_mut().get_mut(name).and_then(VecDeque::pop_front)
    }

    fn logged(&self, name: &'static str, real: Option<Box<dyn PropRandom>>) -> Logged {
        Logged {
            world: Rc::downgrade(&self.rc()),
            name,
            real,
        }
    }

    /// A developer with the session generators of `seed`, or scripted ones.
    fn new_dev(&self, seed: Option<u32>) -> Rc<PropDeveloper> {
        let member = self.logged(
            "member",
            seed.map(|s| Box::new(member_generator(s)) as Box<dyn PropRandom>),
        );
        let wealth: SharedRandom = match seed {
            Some(s) => {
                let cur = self.wealth.borrow().clone();
                match cur {
                    Some((k, r)) if k == s => r,
                    _ => {
                        let r: SharedRandom =
                            Rc::new(RefCell::new(self.logged("wealth", Some(Box::new(wealth_generator(s))))));
                        *self.wealth.borrow_mut() = Some((s, r.clone()));
                        r
                    }
                }
            }
            None => Rc::new(RefCell::new(self.logged("wealth", None))),
        };
        let d = PropDeveloper::new(self.rc() as Rc<dyn PropHost>, Box::new(member), wealth, 0xD0);
        *self.dev.borrow_mut() = Some(d.clone());
        d
    }

    fn lot(&self, lot: u64) -> Option<Lot> {
        self.lots.borrow().get(&lot).cloned()
    }

    fn prop(&self, prop: u64) -> Option<Prop> {
        self.props.borrow().get(&prop).cloned()
    }

    /// A live prop, or one removed whose object is still alive.
    fn prop_object(&self, prop: u64) -> Option<Prop> {
        self.prop(prop).or_else(|| self.gone.borrow().get(&prop).cloned())
    }

    fn edit_prop(&self, prop: u64, f: impl FnOnce(&mut Prop)) {
        if let Some(p) = self.props.borrow_mut().get_mut(&prop) {
            f(p);
        }
    }

    fn new_prop(&self, lot: Option<u64>, cell: (i32, i32), tag: Option<u32>, id: u32) -> u64 {
        let t = self.next.get();
        self.next.set(t + 1);
        let props = self.exemplars.borrow().get(&id).cloned().unwrap_or_default();
        self.props.borrow_mut().insert(
            t,
            Prop {
                lot,
                cell,
                tag,
                ptype: id,
                class: PROP_CLASS,
                props,
                state: 0,
                pos: [cell.0 as f32 * 16.0 + 8.0, 0.0, cell.1 as f32 * 16.0 + 8.0],
                tod: (0.0, 0.0),
            },
        );
        t
    }
}

fn t(v: u64) -> Json {
    ji(v as i64)
}

fn u32_list(j: &Json, what: &str) -> R<Vec<u32>> {
    arr(j, what)?.iter().map(|v| u32_of(v, what)).collect()
}

fn token_of(j: &Json, what: &str) -> R<u64> {
    Ok(i64_of(j, what)? as u64)
}

fn pair(j: &Json, what: &str) -> R<(i32, i32)> {
    let a = arr(j, what)?;
    if a.len() != 2 {
        return Err(format!("{what} is [x, z]"));
    }
    Ok((i64_of(&a[0], what)? as i32, i64_of(&a[1], what)? as i32))
}

fn rect_json(r: CellRect) -> Json {
    Json::Array(vec![ji(r.x1), ji(r.z1), ji(r.x2), ji(r.z2)])
}

impl PropHost for World {
    fn city_cells(&self) -> (i32, i32) {
        self.cells.get()
    }
    fn cell_width(&self) -> (f32, f32) {
        (16.0, 16.0)
    }
    fn managers(&self) -> [PropRef; 3] {
        self.managers.get()
    }
    fn create_wealth_requester(&self) -> u64 {
        let tok = self.wealth_token.get();
        self.events.push("create_wealth_requester", vec![("token", t(tok))]);
        tok
    }
    fn add_agent(&self, level: u32, name: &str, flags: u32) {
        self.events.push(
            "add_agent",
            vec![
                ("level", ji(level)),
                ("name", Json::Str(name.into())),
                ("flags", ji(flags)),
            ],
        );
    }
    fn remove_agents(&self, name: &str) {
        self.events
            .push("remove_agents", vec![("name", Json::Str(name.into()))]);
    }
    fn subscribe(&self, msg: u32) {
        self.events.push("subscribe", vec![("msg", ji(msg))]);
    }
    fn unsubscribe_all(&self) {
        self.events.push("unsubscribe_all", vec![]);
    }
    fn register_animation_tick(&self, priority: u32) {
        self.events.push("animation_tick", vec![("priority", ji(priority))]);
    }
    fn unregister_animation_tick(&self) {
        self.events.push("unregister_animation_tick", vec![]);
    }
    fn post_lot_state_props(&self, x: i32, z: i32, entered_3: bool) {
        self.events.push(
            "post_lot_state_props",
            vec![("x", ji(x)), ("z", ji(z)), ("entered", Json::Bool(entered_3))],
        );
    }
    fn hold(&self, token: u64) {
        self.events.push("hold", vec![("token", t(token))]);
    }
    fn release(&self, token: u64) {
        self.events.push("release", vec![("token", t(token))]);
    }
    fn requester_share(&self, r: u64, rect: CellRect) -> i32 {
        self.events
            .push("share", vec![("requester", t(r)), ("rect", rect_json(rect))]);
        self.requesters.borrow().get(&r).map_or(0, |q| q.share)
    }
    fn requester_satisfaction(&self, r: u64) -> u32 {
        self.requesters.borrow().get(&r).map_or(0, |q| q.satisfaction)
    }
    fn requester_custom(&self, r: u64) -> bool {
        self.requesters.borrow().get(&r).is_some_and(|q| q.custom)
    }
    fn requester_select(&self, r: u64, ids: &[u32], x: i32, z: i32) -> u32 {
        self.events.push(
            "select",
            vec![
                ("requester", t(r)),
                ("ids", Json::Array(ids.iter().map(|&i| ji(i)).collect())),
                ("x", ji(x)),
                ("z", ji(z)),
            ],
        );
        match self.requesters.borrow().get(&r).and_then(|q| q.select) {
            Some(v) => v,
            None => ids.first().copied().unwrap_or(0),
        }
    }
    fn lot_at(&self, x: i32, z: i32, origin_only: bool) -> Option<u64> {
        self.lots
            .borrow()
            .iter()
            .find(|(_, l)| {
                if origin_only {
                    l.x == x && l.z == z
                } else {
                    (l.x..l.x + l.size.0).contains(&x) && (l.z..l.z + l.size.1).contains(&z)
                }
            })
            .map(|(&k, _)| k)
    }
    fn lot_livable(&self, lot: u64) -> bool {
        self.lot(lot).is_some_and(|l| l.livable)
    }
    fn lot_building_type(&self, lot: u64) -> u32 {
        self.lot(lot).map_or(0, |l| l.building)
    }
    fn lot_occupancy(&self, lot: u64) -> f32 {
        self.lot(lot).map_or(0.0, |l| l.occupancy)
    }
    fn lot_config_objects(&self, lot: u64) -> Option<Rc<Vec<CfgObject>>> {
        self.lot(lot).and_then(|l| l.objects)
    }
    fn lot_wealth(&self, lot: u64) -> u32 {
        self.lot(lot).map_or(0, |l| l.wealth)
    }
    fn lot_location(&self, lot: u64) -> (i32, i32) {
        self.lot(lot).map_or((0, 0), |l| (l.x, l.z))
    }
    fn lot_size(&self, lot: u64) -> (i32, i32) {
        self.lot(lot).map_or((0, 0), |l| l.size)
    }
    fn lot_state(&self, lot: u64) -> u32 {
        self.lot(lot).map_or(0, |l| l.state)
    }
    fn zone_type(&self, x: i32, z: i32) -> u32 {
        self.zones.borrow().get(&(x, z)).copied().unwrap_or(0)
    }
    fn network_lot_at(&self, x: i32, z: i32) -> Option<u64> {
        self.network.borrow().get(&(x, z)).map(|n| n.token)
    }
    fn network_lot_objects(&self, nl: u64) -> Rc<Vec<CfgObject>> {
        self.network
            .borrow()
            .values()
            .find(|n| n.token == nl)
            .map(|n| n.objects.clone())
            .unwrap_or_default()
    }
    fn network_wealth(&self, x: i32, z: i32) -> u32 {
        self.network.borrow().get(&(x, z)).map_or(0, |n| n.wealth)
    }
    fn lot_props(&self, lot: u64) -> Vec<u64> {
        self.props
            .borrow()
            .iter()
            .filter(|(_, p)| p.lot == Some(lot))
            .map(|(&k, _)| k)
            .collect()
    }
    fn street_props(&self, x: i32, z: i32) -> Vec<u64> {
        self.props
            .borrow()
            .iter()
            .filter(|(_, p)| p.lot.is_none() && p.cell == (x, z))
            .map(|(&k, _)| k)
            .collect()
    }
    fn street_props_in_rect(&self, r: CellRect) -> Vec<u64> {
        self.props
            .borrow()
            .iter()
            .filter(|(_, p)| p.lot.is_none() && (r.x1..=r.x2).contains(&p.cell.0) && (r.z1..=r.z2).contains(&p.cell.1))
            .map(|(&k, _)| k)
            .collect()
    }
    fn remove_prop(&self, prop: u64) {
        self.events.push("remove_prop", vec![("prop", t(prop))]);
        if let Some(p) = self.props.borrow_mut().remove(&prop) {
            self.gone.borrow_mut().insert(prop, p);
        }
    }
    fn prop_type_valid(&self, id: u32) -> bool {
        self.profiles.borrow().contains_key(&id)
    }
    fn props_by_family(&self, family: u32) -> Vec<u32> {
        self.families.borrow().get(&family).cloned().unwrap_or_default()
    }
    fn prop_satisfaction(&self, id: u32) -> u32 {
        self.profiles.borrow().get(&id).map_or(0, |p| p.0)
    }
    fn prop_wealth(&self, id: u32) -> u8 {
        self.profiles.borrow().get(&id).map_or(0, |p| p.1)
    }
    fn create_lot_prop(&self, lot: u64, index: usize, id: u32) -> Option<u64> {
        let l = self.lot(lot)?;
        let tag = l.objects.as_ref().and_then(|o| o.get(index)).map(|o| o.tag);
        let p = self.new_prop(Some(lot), (l.x, l.z), tag, id);
        self.events.push(
            "create_prop",
            vec![
                ("lot", t(lot)),
                ("index", ji(index as i64)),
                ("id", ji(id)),
                ("prop", t(p)),
            ],
        );
        Some(p)
    }
    fn create_network_prop(&self, nl: u64, index: usize, id: u32) -> Option<u64> {
        let (cell, tag) = {
            let net = self.network.borrow();
            let (c, n) = net.iter().find(|(_, n)| n.token == nl)?;
            (*c, n.objects.get(index).map(|o| o.tag))
        };
        let p = self.new_prop(None, cell, tag, id);
        self.events.push(
            "create_network_prop",
            vec![
                ("network_lot", t(nl)),
                ("index", ji(index as i64)),
                ("id", ji(id)),
                ("prop", t(p)),
            ],
        );
        Some(p)
    }
    fn prop_class(&self, prop: u64) -> u32 {
        self.prop(prop).map_or(PROP_CLASS, |p| p.class)
    }
    fn prop_properties(&self, prop: u64) -> Option<Rc<PropertyHolder>> {
        self.prop_object(prop).map(|p| Rc::new(p.props))
    }
    fn prop_lot_tag(&self, prop: u64) -> Option<u32> {
        self.prop(prop).and_then(|p| p.tag)
    }
    fn set_prop_requester(&self, prop: u64, r: u32) {
        self.edit_prop(prop, |p| {
            p.props.add(PROP_REQUESTER_INDEX, Variant::Scalar(Elem::U32(r)))
        });
    }
    fn prop_type(&self, prop: u64) -> u32 {
        self.prop_object(prop).map_or(0, |p| p.ptype)
    }
    fn prop_state(&self, prop: u64) -> u32 {
        self.prop(prop).map_or(0, |p| p.state)
    }
    fn set_prop_state(&self, prop: u64, state: u32) {
        self.edit_prop(prop, |p| p.state = state);
        self.events
            .push("prop_state", vec![("prop", t(prop)), ("state", ji(state))]);
    }
    fn set_prop_timer(&self, prop: u64, mask: u32, on: bool) {
        self.events.push(
            "timer",
            vec![("prop", t(prop)), ("mask", ji(mask)), ("on", Json::Bool(on))],
        );
    }
    fn set_random_chance(&self, prop: u64, chance: u8) {
        self.events
            .push("chance", vec![("prop", t(prop)), ("chance", ji(chance))]);
    }
    fn set_time_of_day_range(&self, prop: u64, start: f32, end: f32) {
        self.edit_prop(prop, |p| p.tod = (start, end));
        self.events.push(
            "time_of_day_range",
            vec![("prop", t(prop)), ("start", jf(start)), ("end", jf(end))],
        );
    }
    fn time_of_day_range(&self, prop: u64) -> (f32, f32) {
        self.prop(prop).map_or((0.0, 0.0), |p| p.tod)
    }
    fn set_date_range(&self, prop: u64, date: u32, duration: u32, flag: u8) {
        self.events.push(
            "date_range",
            vec![
                ("prop", t(prop)),
                ("date", ji(date)),
                ("duration", ji(duration)),
                ("flag", ji(flag)),
            ],
        );
    }
    fn tick_date(&self, prop: u64, date: u32) {
        self.events
            .push("tick_date", vec![("prop", t(prop)), ("date", ji(date))]);
    }
    fn prop_position(&self, prop: u64) -> [f32; 3] {
        self.prop(prop).map_or([0.0; 3], |p| p.pos)
    }
    fn set_prop_position(&self, prop: u64, pos: [f32; 3]) {
        self.edit_prop(prop, |p| p.pos = pos);
        self.events.push(
            "position",
            vec![
                ("prop", t(prop)),
                ("pos", Json::Array(pos.iter().map(|&v| jf(v)).collect())),
            ],
        );
    }
    fn set_prop_flag(&self, prop: u64, flag: u32, on: bool) {
        self.events.push(
            "flag",
            vec![("prop", t(prop)), ("flag", ji(flag)), ("on", Json::Bool(on))],
        );
    }
    fn network_first_cell(&self, occupant: u64) -> (i32, i32) {
        // Network occupants are named by their cell: 0xXXXXZZZZ.
        (((occupant >> 16) & 0xFFFF) as i32, (occupant & 0xFFFF) as i32)
    }
    fn position_to_cell(&self, x: f32, z: f32) -> (i32, i32) {
        ((x / 16.0).floor() as i32, (z / 16.0).floor() as i32)
    }
    fn is_powered(&self, x: i32, z: i32) -> bool {
        !self.unpowered.borrow().contains(&(x, z))
    }
    fn view_available(&self) -> bool {
        self.view.get()
    }
    fn view_trigger_state(&self, trigger: u32) -> bool {
        self.triggers_on.borrow().contains(&trigger)
    }
    fn sim_date(&self) -> u32 {
        self.date.get()
    }
    fn clock_time(&self) -> Option<(u32, u32)> {
        self.clock.get()
    }
    fn visual_time_mode(&self) -> u32 {
        self.mode.get()
    }
    fn lighting_is_night(&self) -> bool {
        self.night.get()
    }
    fn terrain_heights(&self, _x: f32, _z: f32) -> (f32, f32) {
        self.heights.get()
    }
}

fn world(host: &HostRef) -> Rc<World> {
    scalar::world(host, "prop_developer", World::new)
}

pub(crate) fn exec(host: &HostRef, name: &str, op: &Json) -> Option<R<()>> {
    let rest = name.strip_prefix("pv_")?;
    let w = world(host);
    Some(match rest {
        "env" => env(&w, op),
        "lot" => lot(&w, op),
        "network_lot" => network_lot(&w, op),
        "prop" => prop(&w, op),
        "requester" => requester(&w, op),
        "script" => script(&w, op),
        "create" => create(&w, op),
        "init" => w.dev().and_then(|d| check_expect(op, Json::Bool(d.init()))),
        "shutdown" => w.dev().and_then(|d| check_expect(op, Json::Bool(d.shutdown()))),
        "add_ref" => w.dev().and_then(|d| check_expect(op, ji(d.add_ref()))),
        "release" => w.dev().and_then(|d| check_expect(op, ji(d.release()))),
        "add_requester" => add_requester(&w, op),
        "remove_requester" => w.dev().and_then(|d| {
            let tok = token_of(field(op, "token")?, "token")?;
            check_expect(op, Json::Bool(d.remove_prop_requester(tok)))
        }),
        "message" => message(&w, op),
        "step" => w.dev().and_then(|d| {
            let got = d
                .block_step()
                .map_or(Json::Null, |(x, z)| Json::Array(vec![ji(x), ji(z)]));
            check_expect(op, got)
        }),
        "update_block" => w.dev().and_then(|d| {
            let (x, z) = pair(field(op, "block")?, "block")?;
            d.update_block(x, z);
            Ok(())
        }),
        "update_all" => w.dev().map(|d| d.update_all_optional_props()),
        "signs" => signs(&w, op),
        "wealth_select" => wealth_select(&w, op),
        "identity" => identity(op),
        "expect" => expect(&w, op),
        "expect_block" => expect_block(&w, op),
        "expect_lists" => expect_lists(&w, op),
        "expect_prop" => expect_prop(&w, op),
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

/// `pv_env`: `cells` (`[x, z]`), `clock` (`[hour, minute]` or null: no
/// 24-hour clock), `visual_mode`, `night` (the lighting manager's IsNight),
/// `view` (the 3D view exists), `triggers_on` (the view's triggers that are
/// on), `date`, `heights` (`[terrain, water]`), `unpowered` (`[[x, z]]`,
/// replacing the set), `zones` (`[[x, z, type]]`), `profiles`
/// (`[[id, satisfaction, wealth]]`), `families` (`{"0xFAMILY": [ids]}`),
/// `exemplars` (`{"0xID": {properties}}`: what a created prop of that type
/// starts with), `wealth_token` (the token of a new wealth requester).
fn env(w: &World, op: &Json) -> R<()> {
    if let Some(v) = opt(op, "cells") {
        w.cells.set(pair(v, "cells")?);
    }
    match op.get("clock") {
        None => {}
        Some(Json::Null) => w.clock.set(None),
        Some(v) => {
            let (h, m) = pair(v, "clock")?;
            w.clock.set(Some((h as u32, m as u32)));
        }
    }
    if let Some(v) = opt(op, "visual_mode") {
        w.mode.set(u32_of(v, "visual_mode")?);
    }
    if let Some(v) = opt(op, "night") {
        w.night.set(bool_of(v, "night")?);
    }
    if let Some(v) = opt(op, "view") {
        w.view.set(bool_of(v, "view")?);
    }
    if let Some(v) = opt(op, "triggers_on") {
        *w.triggers_on.borrow_mut() = u32_list(v, "triggers_on")?.into_iter().collect();
    }
    if let Some(v) = opt(op, "date") {
        w.date.set(u32_of(v, "date")?);
    }
    if let Some(v) = opt(op, "heights") {
        let h = arr(v, "heights")?;
        if h.len() != 2 {
            return Err("heights is [terrain, water]".into());
        }
        w.heights.set((f32_of(&h[0], "terrain")?, f32_of(&h[1], "water")?));
    }
    if let Some(v) = opt(op, "unpowered") {
        let mut s = BTreeSet::new();
        for c in arr(v, "unpowered")? {
            s.insert(pair(c, "unpowered cell")?);
        }
        *w.unpowered.borrow_mut() = s;
    }
    if let Some(v) = opt(op, "zones") {
        for z in arr(v, "zones")? {
            let z = u32_list(z, "zone")?;
            if z.len() != 3 {
                return Err("a zone is [x, z, type]".into());
            }
            w.zones.borrow_mut().insert((z[0] as i32, z[1] as i32), z[2]);
        }
    }
    if let Some(v) = opt(op, "profiles") {
        for p in arr(v, "profiles")? {
            let p = u32_list(p, "profile")?;
            if p.len() != 3 {
                return Err("a profile is [id, satisfaction, wealth]".into());
            }
            w.profiles.borrow_mut().insert(p[0], (p[1], p[2] as u8));
        }
    }
    if let Some(v) = opt(op, "families") {
        for (k, ids) in v.as_object().ok_or("families must be an object")? {
            let f = Json::Str(k.clone()).as_i64().ok_or("bad family")? as u32;
            w.families.borrow_mut().insert(f, u32_list(ids, "family")?);
        }
    }
    if let Some(v) = opt(op, "exemplars") {
        for (k, props) in v.as_object().ok_or("exemplars must be an object")? {
            let id = Json::Str(k.clone()).as_i64().ok_or("bad prop ID")? as u32;
            w.exemplars.borrow_mut().insert(id, holder_of(props)?);
        }
    }
    if let Some(v) = opt(op, "wealth_token") {
        w.wealth_token.set(token_of(v, "wealth_token")?);
    }
    Ok(())
}

/// Configuration objects: `[{"tag": id, "ids": [..], "type": 1, "flags": 1}]`
/// (type and flags default to an optional prop).
fn objects_of(j: &Json) -> R<Vec<CfgObject>> {
    arr(j, "objects")?
        .iter()
        .map(|o| {
            Ok(CfgObject {
                object_type: opt(o, "type").map_or(Ok(1), |v| u32_of(v, "type"))?,
                flags: opt(o, "flags").map_or(Ok(1), |v| u32_of(v, "flags"))? as u8,
                tag: u32_of(field(o, "tag")?, "tag")?,
                ids: opt(o, "ids").map_or(Ok(Vec::new()), |v| u32_list(v, "ids"))?,
                ..CfgObject::default()
            })
        })
        .collect()
}

/// `pv_lot`: creates or updates `lot`: `x`, `z`, `size` (`[w, d]`, default
/// 1×1), `livable` (default true), `building` (the building type, default
/// 0x100; 0 = none), `occupancy`, `wealth`, `state` (default 3), `objects`
/// (the configuration's objects, null: no configuration).
fn lot(w: &World, op: &Json) -> R<()> {
    let tok = token_of(field(op, "lot")?, "lot")?;
    let mut l = w.lot(tok).unwrap_or(Lot {
        x: 0,
        z: 0,
        size: (1, 1),
        livable: true,
        building: 0x100,
        occupancy: 0.0,
        objects: Some(Rc::new(Vec::new())),
        wealth: 1,
        state: 3,
    });
    if let Some(v) = opt(op, "x") {
        l.x = i64_of(v, "x")? as i32;
    }
    if let Some(v) = opt(op, "z") {
        l.z = i64_of(v, "z")? as i32;
    }
    if let Some(v) = opt(op, "size") {
        l.size = pair(v, "size")?;
    }
    if let Some(v) = opt(op, "livable") {
        l.livable = bool_of(v, "livable")?;
    }
    if let Some(v) = opt(op, "building") {
        l.building = u32_of(v, "building")?;
    }
    if let Some(v) = opt(op, "occupancy") {
        l.occupancy = f32_of(v, "occupancy")?;
    }
    if let Some(v) = opt(op, "wealth") {
        l.wealth = u32_of(v, "wealth")?;
    }
    if let Some(v) = opt(op, "state") {
        l.state = u32_of(v, "state")?;
    }
    match op.get("objects") {
        None => {}
        Some(Json::Null) => l.objects = None,
        Some(v) => l.objects = Some(Rc::new(objects_of(v)?)),
    }
    w.lots.borrow_mut().insert(tok, l);
    Ok(())
}

/// `pv_network_lot`: a network lot `token` at `cell` with `objects` and
/// `wealth` (GetWealthForNetwork at the cell).
fn network_lot(w: &World, op: &Json) -> R<()> {
    let cell = pair(field(op, "cell")?, "cell")?;
    let n = NetLot {
        token: token_of(field(op, "token")?, "token")?,
        objects: Rc::new(opt(op, "objects").map_or(Ok(Vec::new()), objects_of)?),
        wealth: opt(op, "wealth").map_or(Ok(0), |v| u32_of(v, "wealth"))?,
    };
    w.network.borrow_mut().insert(cell, n);
    Ok(())
}

/// `pv_prop`: creates or updates prop `prop`: `lot` (null: a street prop),
/// `cell` (a street prop's cell), `tag` (null: none), `type`, `class`,
/// `properties` (replacing them), `state`, `pos` (`[x, y, z]`; default the
/// middle of the cell).
fn prop(w: &World, op: &Json) -> R<()> {
    let tok = token_of(field(op, "prop")?, "prop")?;
    let mut p = w.prop(tok).unwrap_or(Prop {
        lot: None,
        cell: (0, 0),
        tag: None,
        ptype: 0,
        class: PROP_CLASS,
        props: PropertyHolder::new(),
        state: 0,
        pos: [8.0, 0.0, 8.0],
        tod: (0.0, 0.0),
    });
    match op.get("lot") {
        None => {}
        Some(Json::Null) => p.lot = None,
        Some(v) => {
            let l = token_of(v, "lot")?;
            p.lot = Some(l);
            if let Some(lot) = w.lot(l) {
                p.cell = (lot.x, lot.z);
            }
        }
    }
    if let Some(v) = opt(op, "cell") {
        p.cell = pair(v, "cell")?;
    }
    match op.get("tag") {
        None => {}
        Some(Json::Null) => p.tag = None,
        Some(v) => p.tag = Some(u32_of(v, "tag")?),
    }
    if let Some(v) = opt(op, "type") {
        p.ptype = u32_of(v, "type")?;
    }
    if let Some(v) = opt(op, "class") {
        p.class = u32_of(v, "class")?;
    }
    if let Some(v) = opt(op, "properties") {
        p.props = holder_of(v)?;
    }
    if let Some(v) = opt(op, "state") {
        p.state = u32_of(v, "state")?;
    }
    match opt(op, "pos") {
        Some(v) => {
            let a = arr(v, "pos")?;
            if a.len() != 3 {
                return Err("pos is [x, y, z]".into());
            }
            p.pos = [f32_of(&a[0], "x")?, f32_of(&a[1], "y")?, f32_of(&a[2], "z")?];
        }
        None if opt(op, "cell").is_some() || opt(op, "lot").is_some() => {
            p.pos = [p.cell.0 as f32 * 16.0 + 8.0, 0.0, p.cell.1 as f32 * 16.0 + 8.0];
        }
        None => {}
    }
    w.props.borrow_mut().insert(tok, p);
    Ok(())
}

/// `pv_requester`: a host requester `token`: `share`, `satisfaction`,
/// `custom`, `select` (what SelectProp answers; null: the first ID).
fn requester(w: &World, op: &Json) -> R<()> {
    let tok = token_of(field(op, "token")?, "token")?;
    let mut q = w.requesters.borrow().get(&tok).copied().unwrap_or(HostRequester {
        share: 0,
        satisfaction: 0,
        custom: false,
        select: None,
    });
    if let Some(v) = opt(op, "share") {
        q.share = i64_of(v, "share")? as i32;
    }
    if let Some(v) = opt(op, "satisfaction") {
        q.satisfaction = u32_of(v, "satisfaction")?;
    }
    if let Some(v) = opt(op, "custom") {
        q.custom = bool_of(v, "custom")?;
    }
    match op.get("select") {
        None => {}
        Some(Json::Null) => q.select = None,
        Some(v) => q.select = Some(u32_of(v, "select")?),
    }
    w.requesters.borrow_mut().insert(tok, q);
    Ok(())
}

/// `pv_script`: appends results for the scripted generators (`member`,
/// `wealth`); used by a developer created without `seed`.
fn script(w: &World, op: &Json) -> R<()> {
    for name in ["member", "wealth"] {
        if let Some(v) = opt(op, name) {
            let vals: Vec<i64> = arr(v, name)?.iter().map(|x| i64_of(x, name)).collect::<R<_>>()?;
            w.scripts.borrow_mut().entry(name).or_default().extend(vals);
        }
    }
    Ok(())
}

/// `pv_create`: a new developer; `seed` (the session seed: real
/// generators, the wealth one shared by every developer of that seed) or
/// none (scripted generators, see `pv_script`). Every draw is logged as a
/// `draw` event.
fn create(w: &World, op: &Json) -> R<()> {
    let seed = opt(op, "seed").map(|v| u32_of(v, "seed")).transpose()?;
    w.new_dev(seed);
    Ok(())
}

/// `pv_add_requester`: `token` and `class` (class 0x89C49986 is a wealth
/// requester answered by the developer).
fn add_requester(w: &World, op: &Json) -> R<()> {
    let d = w.dev()?;
    let tok = token_of(field(op, "token")?, "token")?;
    let class = u32_of(field(op, "class")?, "class")?;
    let r = if tok == 0 {
        Requester::Null
    } else if class == CLSID_WEALTH_REQUESTER {
        Requester::Wealth(WealthRequester::new(tok))
    } else {
        Requester::Host(PropRef::new(tok, class))
    };
    d.add_prop_requester(r);
    Ok(())
}

/// `pv_message`: `DoMessage` with `msg`, `object`, `data1`…`data4`.
fn message(w: &World, op: &Json) -> R<()> {
    let d = w.dev()?;
    let g = |k: &str| opt(op, k).map_or(Ok(0), |v| u32_of(v, k));
    let m = PropMessage {
        msg_type: u32_of(field(op, "msg")?, "msg")?,
        object: opt(op, "object").map_or(Ok(0), |v| token_of(v, "object"))?,
        data1: g("data1")?,
        data2: g("data2")?,
        data3: g("data3")?,
        data4: g("data4")?,
    };
    check_expect(op, Json::Bool(d.do_message(&m)))
}

/// `pv_signs`: `show` (ShowSignsAndLabels) or `toggle: true`; `expect`
/// checks AreSignsAndLabelsVisible afterwards.
fn signs(w: &World, op: &Json) -> R<()> {
    let d = w.dev()?;
    if let Some(v) = opt(op, "show") {
        d.show_signs(bool_of(v, "show")?);
    }
    if opt(op, "toggle").and_then(boolean).unwrap_or(false) {
        d.toggle_signs();
    }
    check_expect(op, Json::Bool(d.signs_visible()))
}

/// `pv_wealth_select`: the wealth requester's SelectProp(`ids`, `x`, `z`).
fn wealth_select(w: &World, op: &Json) -> R<()> {
    let d = w.dev()?;
    let ids = u32_list(field(op, "ids")?, "ids")?;
    let x = i64_of(field(op, "x")?, "x")? as i32;
    let z = i64_of(field(op, "z")?, "z")? as i32;
    check_expect(op, ji(d.wealth_select(&ids, x, z)))
}

/// `pv_identity`: `iids` (`{"0xIID": offset | false}`) of the developer,
/// `wealth_iids` of the wealth requester, `member_seed` (`[S, seed]`).
fn identity(op: &Json) -> R<()> {
    for (key, qi) in [
        ("iids", query_interface as fn(u32) -> Option<u32>),
        ("wealth_iids", wealth_query_interface),
    ] {
        let Some(m) = opt(op, key) else { continue };
        for (k, want) in m.as_object().ok_or("iids must be an object")? {
            let iid = Json::Str(k.clone()).as_i64().ok_or("bad iid")? as u32;
            let got = qi(iid).map_or(Json::Bool(false), ji);
            if !scalar::matches(&got, want, false) {
                return Err(format!("{key}: QueryInterface({k}) = {}", scalar::show(&got)));
            }
        }
    }
    if let Some(v) = opt(op, "member_seed") {
        let s = u32_list(v, "member_seed")?;
        if s.len() != 2 || member_seed(s[0]) != s[1] {
            return Err(format!(
                "member_seed({:#x}) = {:#x}",
                s.first().copied().unwrap_or(0),
                member_seed(s.first().copied().unwrap_or(0))
            ));
        }
    }
    Ok(())
}

fn expect(w: &World, op: &Json) -> R<()> {
    let d = w.dev()?;
    let s = d.snapshot();
    let it = &s.scanner;
    let state = match it.state {
        ScanState::Fresh => "fresh",
        ScanState::Running => "running",
        ScanState::Done => "done",
    };
    let got = vec![
        ("initialized".to_owned(), Json::Bool(s.initialized)),
        ("scan_flag".to_owned(), ji(s.scan_flag)),
        ("scanner".to_owned(), Json::Str(state.into())),
        ("scan_byte".to_owned(), ji(s.scan_byte)),
        ("scan_at".to_owned(), Json::Array(vec![ji(it.x), ji(it.z)])),
        (
            "clip".to_owned(),
            Json::Array(vec![ji(it.x0), ji(it.z0), ji(it.x1), ji(it.z1)]),
        ),
        ("wealth_added".to_owned(), Json::Bool(s.wealth_added)),
        ("night_cursor".to_owned(), ji(s.night_cursor)),
        ("night_mode".to_owned(), ji(s.night_mode)),
        ("night_batch".to_owned(), ji(s.night_batch)),
        ("night_acc".to_owned(), ji(s.night_acc)),
        ("is_night".to_owned(), Json::Bool(s.is_night)),
        ("tod_last".to_owned(), jf(s.tod_last)),
        ("signs_visible".to_owned(), Json::Bool(s.signs_visible)),
        ("ref_count".to_owned(), ji(s.ref_count)),
        ("destroyed".to_owned(), Json::Bool(s.destroyed)),
        ("requesters".to_owned(), ji(d.requesters().len() as i64)),
        ("night".to_owned(), ji(d.night_list().len() as i64)),
        ("triggers".to_owned(), ji(d.trigger_map().len() as i64)),
        ("time_of_day".to_owned(), ji(d.time_of_day_list().len() as i64)),
        ("dated".to_owned(), ji(d.dated_list().len() as i64)),
        ("power".to_owned(), ji(d.power_list().len() as i64)),
        ("signs".to_owned(), ji(d.sign_list().len() as i64)),
        ("crt_state".to_owned(), ji(d.crt().state())),
    ];
    check_state(&got, field(op, "state")?)
}

fn u32s_json(v: &[u32]) -> Json {
    Json::Array(v.iter().map(|&x| ji(x)).collect())
}

/// `pv_expect_block`: the last block update.
fn expect_block(w: &World, op: &Json) -> R<()> {
    let b = w.dev()?.last_block();
    let got = vec![
        ("block".to_owned(), Json::Array(vec![ji(b.block.0), ji(b.block.1)])),
        ("lots".to_owned(), ji(b.lots)),
        ("occ_sum".to_owned(), jf(b.occ_sum)),
        ("slots".to_owned(), ji(b.slots)),
        ("kept_all".to_owned(), ji(b.kept_all)),
        ("target".to_owned(), ji(b.target)),
        ("want".to_owned(), u32s_json(&b.want)),
        ("kept".to_owned(), u32s_json(&b.kept)),
        ("removed".to_owned(), ji(b.removed)),
        ("created".to_owned(), ji(b.created)),
        ("member_draws".to_owned(), ji(b.member_draws)),
        ("stopped".to_owned(), Json::Bool(b.stopped)),
    ];
    check_state(&got, field(op, "state")?)
}

fn tokens_json(v: &[PropRef]) -> Json {
    Json::Array(v.iter().map(|p| t(p.token)).collect())
}

/// `pv_expect_lists`: `requesters` (`[[token, class]]`), `night`,
/// `time_of_day`, `dated`, `power`, `signs` (tokens, in order), `triggers`
/// (`[[trigger, [tokens]]]`, iteration order), `classes` (`{"0xTOKEN":
/// class}` of any listed prop).
fn expect_lists(w: &World, op: &Json) -> R<()> {
    let d = w.dev()?;
    let mut all: Vec<PropRef> = Vec::new();
    let mut got = vec![(
        "requesters".to_owned(),
        Json::Array(
            d.requesters()
                .iter()
                .map(|r| Json::Array(vec![t(r.token()), ji(r.class_id())]))
                .collect(),
        ),
    )];
    for (k, list) in [
        ("night", d.night_list()),
        ("time_of_day", d.time_of_day_list()),
        ("dated", d.dated_list()),
        ("power", d.power_list()),
        ("signs", d.sign_list()),
    ] {
        got.push((k.to_owned(), tokens_json(&list)));
        all.extend(list);
    }
    let map = d.trigger_map();
    got.push((
        "triggers".to_owned(),
        Json::Array(
            map.iter()
                .map(|(k, v)| Json::Array(vec![ji(*k), tokens_json(v)]))
                .collect(),
        ),
    ));
    all.extend(map.into_iter().flat_map(|(_, v)| v));
    if let Some(want) = opt(op, "classes") {
        for (k, c) in want.as_object().ok_or("classes must be an object")? {
            let tok = Json::Str(k.clone()).as_i64().ok_or("bad token")? as u64;
            let want = u32_of(c, "class")?;
            match all.iter().find(|p| p.token == tok) {
                Some(p) if p.class_id == want => {}
                Some(p) => return Err(format!("{k}: class {:#x}, expected {want:#x}", p.class_id)),
                None => return Err(format!("{k} is in no list")),
            }
        }
    }
    let want = op.as_object().ok_or("op must be an object")?;
    let keys: Vec<&String> = want
        .iter()
        .map(|(k, _)| k)
        .filter(|k| *k != "op" && *k != "classes")
        .collect();
    let mut errors = Vec::new();
    for k in keys {
        let w = op.get(k).expect("key");
        match got.iter().find(|(g, _)| g == k) {
            None => errors.push(format!("unknown list '{k}'")),
            Some((_, g)) if !scalar::matches(g, w, false) => {
                errors.push(format!("{k}: got {}, expected {}", scalar::show(g), scalar::show(w)))
            }
            _ => {}
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

/// `pv_expect_prop`: prop `prop` in the world: `exists`, `state`, `type`,
/// `lot`, `tag`, `requester` (property 0x89A1C16C, null: none), `pos`.
fn expect_prop(w: &World, op: &Json) -> R<()> {
    let tok = token_of(field(op, "prop")?, "prop")?;
    let p = w.prop(tok);
    let got = vec![
        ("exists".to_owned(), Json::Bool(p.is_some())),
        ("state".to_owned(), p.as_ref().map_or(Json::Null, |p| ji(p.state))),
        ("type".to_owned(), p.as_ref().map_or(Json::Null, |p| ji(p.ptype))),
        ("lot".to_owned(), p.as_ref().and_then(|p| p.lot).map_or(Json::Null, t)),
        ("tag".to_owned(), p.as_ref().and_then(|p| p.tag).map_or(Json::Null, ji)),
        (
            "requester".to_owned(),
            p.as_ref()
                .and_then(|p| p.props.get(PROP_REQUESTER_INDEX))
                .and_then(|v| v.items().first().map(|e| ji(e.as_i64())))
                .unwrap_or(Json::Null),
        ),
        (
            "pos".to_owned(),
            p.as_ref()
                .map_or(Json::Null, |p| Json::Array(p.pos.iter().map(|&v| jf(v)).collect())),
        ),
    ];
    check_state(&got, field(op, "state")?)
}

// ---- records: references as (u32 token, u32 class) ----------------------------------------------

struct TokenWriter(ByteWriter);

impl OutStream for TokenWriter {
    fn put_u8(&mut self, v: u8) -> bool {
        self.0.put_u8(v)
    }
    fn put_u16(&mut self, v: u16) -> bool {
        self.0.put_u16(v)
    }
    fn put_u32(&mut self, v: u32) -> bool {
        self.0.put_u32(v)
    }
    fn put_u64(&mut self, v: u64) -> bool {
        self.0.put_u64(v)
    }
    fn put_f32(&mut self, v: f32) -> bool {
        self.0.put_f32(v)
    }
    fn put_f64(&mut self, v: f64) -> bool {
        self.0.put_f64(v)
    }
    fn put_bytes(&mut self, v: &[u8]) -> bool {
        self.0.put_bytes(v)
    }
    fn put_str(&mut self, v: &[u8]) -> bool {
        self.0.put_str(v)
    }
    fn put_ref(&mut self, r: &RefValue) -> bool {
        match r {
            RefValue::Null => self.0.put_u32(0),
            RefValue::Object(o) => self.0.put_u32(o.host_token() as u32) && self.0.put_u32(o.class_id()),
            RefValue::Host { token, class_id } => self.0.put_u32(*token as u32) && self.0.put_u32(*class_id),
        }
    }
    fn error(&self) -> i32 {
        OutStream::error(&self.0)
    }
}

struct TokenResolver;

impl RefResolver for TokenResolver {
    fn resolve(&mut self, class_id: u32, object_id: u32) -> Option<RefValue> {
        Some(RefValue::Host {
            token: u64::from(object_id),
            class_id,
        })
    }
}

/// `pv_write`: the record (references as token, class); `hex`, `slot`.
fn write(w: &World, op: &Json) -> R<()> {
    let d = w.dev()?;
    let mut out = TokenWriter(ByteWriter::new());
    check_expect(op, Json::Bool(d.write(&mut out)))?;
    let bytes = out.0.into_bytes();
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
    if let Some(n) = opt(op, "length") {
        let n = i64_of(n, "length")? as usize;
        if bytes.len() != n {
            return Err(format!("record is {} bytes, expected {n}", bytes.len()));
        }
    }
    if let Some(slot) = opt(op, "slot") {
        w.records.borrow_mut().insert(str_of(slot, "slot")?.to_string(), bytes);
    }
    Ok(())
}

/// `pv_read`: `hex` or `slot` into the current developer (or a new one with
/// `fresh: true`, created as the last `pv_create` was: `seed` may be given).
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
    let d = if fresh {
        let seed = opt(op, "seed").map(|v| u32_of(v, "seed")).transpose()?;
        w.new_dev(seed)
    } else {
        w.dev()?
    };
    let mut res = TokenResolver;
    check_expect(op, Json::Bool(d.read(&mut ByteReader::with_resolver(&bytes, &mut res))))
}
