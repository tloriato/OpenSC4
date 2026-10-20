// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Conformance ops for `docs/specs/city-detail.md`: a scripted city (the
//! detail preference, lots with configuration objects, their prop and
//! flora occupants, object placements and ID arrays, network lots, a
//! scripted or the session generator) and ops that drive the city detail
//! manager. See README.md ("City detail ops").

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::rc::{Rc, Weak};

use fc4_core::city_detail::{
    query_interface, CityDetailHost, CityDetailManager, DetailRandom, DetailSession, ObjectPlacement,
    CLSID_CITY_DETAIL, OCCUPANT_TYPE_FLORA, OCCUPANT_TYPE_PROP,
};
use fc4_core::lot_developer::CfgObject;
use fc4_core::stream::{ByteReader, ByteWriter, SaveObject};

use crate::foundations::{hex_decode, hex_encode};
use crate::json::Json;
use crate::runner::{boolean, field, HostRef};
use crate::scalar::{self, arr, bool_of, check_expect, check_state, i64_of, ji, opt, str_of, u32_of, EventLog, R};

#[derive(Clone)]
struct Occ {
    lot: u64,
    occ_type: u32,
    tag: Option<u32>,
}

pub(crate) struct World {
    me: RefCell<Weak<World>>,
    events: EventLog,
    mgr: RefCell<Option<Rc<CityDetailManager>>>,
    session: RefCell<Rc<DetailSession>>,
    setting: Cell<u32>,
    lots: RefCell<Vec<u64>>,
    configs: RefCell<BTreeMap<u64, Rc<Vec<CfgObject>>>>,
    occupants: RefCell<BTreeMap<u64, Occ>>,
    ids: RefCell<BTreeMap<(u64, usize), Vec<u32>>>,
    network: RefCell<Vec<(i32, i32, u64)>>,
    create: Cell<bool>,
    next: Cell<u64>,
    stub: Cell<bool>,
    u32s: RefCell<VecDeque<u32>>,
    records: RefCell<BTreeMap<String, Vec<u8>>>,
}

/// The scripted generator: queued values (then 0); every draw is an event.
struct Stub(Weak<World>);

impl DetailRandom for Stub {
    fn uniform_u32(&mut self, n: u32) -> u32 {
        let w = self.0.upgrade().expect("world alive");
        w.events.push("draw", vec![("n", ji(n))]);
        let v = w.u32s.borrow_mut().pop_front();
        v.unwrap_or(0)
    }
}

/// The session generator, with every draw an event.
struct Logged(Weak<World>, fc4_core::rng::RzRandom);

impl DetailRandom for Logged {
    fn uniform_u32(&mut self, n: u32) -> u32 {
        let v = self.1.uniform_u32(n);
        let w = self.0.upgrade().expect("world alive");
        w.events.push("draw", vec![("n", ji(n)), ("value", ji(v))]);
        v
    }
}

impl World {
    fn new() -> Rc<World> {
        let w = Rc::new(World {
            me: RefCell::new(Weak::new()),
            events: EventLog::default(),
            mgr: RefCell::new(None),
            session: RefCell::new(DetailSession::new(0x1234_5678)),
            setting: Cell::new(3),
            lots: RefCell::new(Vec::new()),
            configs: RefCell::new(BTreeMap::new()),
            occupants: RefCell::new(BTreeMap::new()),
            ids: RefCell::new(BTreeMap::new()),
            network: RefCell::new(Vec::new()),
            create: Cell::new(true),
            next: Cell::new(0x9000),
            stub: Cell::new(true),
            u32s: RefCell::new(VecDeque::new()),
            records: RefCell::new(BTreeMap::new()),
        });
        *w.me.borrow_mut() = Rc::downgrade(&w);
        w
    }

    fn rc(&self) -> Rc<World> {
        self.me.borrow().upgrade().expect("world alive")
    }

    fn mgr(&self) -> R<Rc<CityDetailManager>> {
        self.mgr
            .borrow()
            .clone()
            .ok_or_else(|| "no city detail manager (cd_create)".into())
    }

    fn new_mgr(&self) -> Rc<CityDetailManager> {
        let m = CityDetailManager::new(self.rc() as Rc<dyn CityDetailHost>, self.session.borrow().clone(), 0xC0);
        *self.mgr.borrow_mut() = Some(m.clone());
        m
    }

    /// Arms the generator of the next increase pass.
    fn arm(&self, m: &CityDetailManager) {
        if self.stub.get() {
            m.set_next_random(Box::new(Stub(self.me.borrow().clone())));
        } else {
            let s = self.session.borrow().clone();
            // n counts this manager's passes (CD5).
            let seed = s.pass_seed(m.passes());
            let r = fc4_core::rng::RzRandom::new(
                seed,
                &fc4_core::rng::FixedSeeds {
                    wall_clock: seed,
                    process_clock: seed,
                },
            );
            m.set_next_random(Box::new(Logged(self.me.borrow().clone(), r)));
        }
    }

    fn on_lot(&self, lot: u64) -> Vec<u64> {
        self.occupants
            .borrow()
            .iter()
            .filter(|(_, o)| o.lot == lot)
            .map(|(k, _)| *k)
            .collect()
    }

    fn add(&self, lot: u64, occ_type: u32) -> Option<u64> {
        if !self.create.get() {
            return None;
        }
        let t = self.next.get();
        self.next.set(t + 1);
        self.occupants.borrow_mut().insert(
            t,
            Occ {
                lot,
                occ_type,
                tag: None,
            },
        );
        Some(t)
    }
}

/// The default placement of object `index`: x = 16 index + 8, y = 0,
/// z = 8, orientation index mod 4; the lot travels in `y` for the fake.
fn placement(lot: u64, index: usize) -> ObjectPlacement {
    ObjectPlacement {
        x: index as f32 * 16.0 + 8.0,
        y: lot as f32,
        z: 8.0,
        orientation: (index % 4) as u32,
    }
}

impl CityDetailHost for World {
    fn detail_setting(&self) -> u32 {
        self.setting.get()
    }
    fn lots(&self) -> Vec<u64> {
        self.lots.borrow().clone()
    }
    fn lot_config_objects(&self, lot: u64) -> Option<Rc<Vec<CfgObject>>> {
        self.configs.borrow().get(&lot).cloned()
    }
    fn lot_occupants(&self, lot: u64) -> Vec<u64> {
        self.on_lot(lot)
    }
    fn occupants_in_lot_rect(&self, lot: u64) -> Vec<u64> {
        self.on_lot(lot)
    }
    fn occupant_type(&self, occupant: u64) -> u32 {
        self.occupants.borrow().get(&occupant).map_or(0, |o| o.occ_type)
    }
    fn occupant_lot_tag(&self, occupant: u64) -> Option<u32> {
        self.occupants.borrow().get(&occupant).and_then(|o| o.tag)
    }
    fn demolish_flora(&self, occupant: u64) {
        self.events
            .push("demolish_flora", vec![("occupant", ji(occupant as i64))]);
        self.occupants.borrow_mut().remove(&occupant);
    }
    fn remove_prop(&self, occupant: u64) {
        self.events.push("remove_prop", vec![("occupant", ji(occupant as i64))]);
        self.occupants.borrow_mut().remove(&occupant);
    }
    fn object_placement(&self, lot: u64, index: usize) -> ObjectPlacement {
        self.events
            .push("placement", vec![("lot", ji(lot as i64)), ("index", ji(index as i64))]);
        placement(lot, index)
    }
    fn object_id_array(&self, lot: u64, index: usize) -> Vec<u32> {
        if let Some(v) = self.ids.borrow().get(&(lot, index)) {
            return v.clone();
        }
        self.configs
            .borrow()
            .get(&lot)
            .and_then(|c| c.get(index).map(|o| o.ids.clone()))
            .unwrap_or_default()
    }
    fn add_lot_prop(&self, id: u32, p: ObjectPlacement) -> Option<u64> {
        let t = self.add(p.y as u64, OCCUPANT_TYPE_PROP);
        self.events.push(
            "add_prop",
            vec![
                ("id", ji(id)),
                ("x", Json::Float(f64::from(p.x))),
                ("z", Json::Float(f64::from(p.z))),
                ("orientation", ji(p.orientation)),
                ("occupant", t.map_or(Json::Null, |t| ji(t as i64))),
            ],
        );
        t
    }
    fn add_flora(&self, id: u32, p: ObjectPlacement) -> Option<u64> {
        let t = self.add(p.y as u64, OCCUPANT_TYPE_FLORA);
        self.events.push(
            "add_flora",
            vec![
                ("id", ji(id)),
                ("x", Json::Float(f64::from(p.x))),
                ("z", Json::Float(f64::from(p.z))),
                ("occupant", t.map_or(Json::Null, |t| ji(t as i64))),
            ],
        );
        t
    }
    fn set_lot_tag(&self, occupant: u64, tag: u32) {
        self.events
            .push("set_tag", vec![("occupant", ji(occupant as i64)), ("tag", ji(tag))]);
        if let Some(o) = self.occupants.borrow_mut().get_mut(&occupant) {
            o.tag = Some(tag);
        }
    }
    fn network_lots(&self) -> Vec<(i32, i32, u64)> {
        self.network.borrow().clone()
    }
    fn redevelop_network_lot(&self, x: i32, z: i32, network_lot: u64) {
        self.events.push(
            "redevelop",
            vec![("x", ji(x)), ("z", ji(z)), ("lot", ji(network_lot as i64))],
        );
    }
}

fn world(host: &HostRef) -> Rc<World> {
    scalar::world(host, "city_detail", World::new)
}

pub(crate) fn exec(host: &HostRef, name: &str, op: &Json) -> Option<R<()>> {
    let rest = name.strip_prefix("cd_")?;
    let w = world(host);
    Some(match rest {
        "env" => env(&w, op),
        "lot" => lot(&w, op),
        "occupant" => occupant(&w, op),
        "ids" => ids(&w, op),
        "create" => {
            w.new_mgr();
            Ok(())
        }
        "init" => w.mgr().and_then(|m| {
            w.arm(&m);
            check_expect(op, Json::Bool(m.init()))
        }),
        "shutdown" => w.mgr().and_then(|m| check_expect(op, Json::Bool(m.shutdown()))),
        "add_ref" => w.mgr().and_then(|m| check_expect(op, ji(m.add_ref()))),
        "release" => w.mgr().and_then(|m| check_expect(op, ji(m.release()))),
        "get_level" => w.mgr().and_then(|m| check_expect(op, ji(m.city_detail()))),
        "identity" => identity(&w, op),
        "set" => w.mgr().and_then(|m| {
            m.set_level(u32_of(field(op, "level")?, "level")?);
            Ok(())
        }),
        "expect" => expect(&w, op),
        "expect_occupants" => expect_occupants(&w, op),
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

/// `cd_env`: `setting` (the preference), `session_seed` (a new session),
/// `random` ("stub" or "session"), `u32s` (the stub's queue), `network`
/// (`[[x, z, lot]]`), `create` (whether AddLotProp / AddNewFloraOccupant
/// answer an occupant).
fn env(w: &World, op: &Json) -> R<()> {
    if let Some(v) = opt(op, "setting") {
        w.setting.set(u32_of(v, "setting")?);
    }
    if let Some(v) = opt(op, "session_seed") {
        *w.session.borrow_mut() = DetailSession::new(u32_of(v, "session_seed")?);
    }
    if let Some(v) = opt(op, "random") {
        match str_of(v, "random")? {
            "stub" => w.stub.set(true),
            "session" => w.stub.set(false),
            other => return Err(format!("unknown generator '{other}'")),
        }
    }
    if let Some(v) = opt(op, "u32s") {
        for d in arr(v, "u32s")? {
            w.u32s.borrow_mut().push_back(u32_of(d, "u32")?);
        }
    }
    if let Some(v) = opt(op, "network") {
        let mut n = Vec::new();
        for e in arr(v, "network")? {
            let t = arr(e, "network entry")?;
            if t.len() != 3 {
                return Err("a network entry is [x, z, lot]".into());
            }
            n.push((
                i64_of(&t[0], "x")? as i32,
                i64_of(&t[1], "z")? as i32,
                token_of(&t[2], "lot")?,
            ));
        }
        *w.network.borrow_mut() = n;
    }
    if let Some(v) = opt(op, "create") {
        w.create.set(bool_of(v, "create")?);
    }
    Ok(())
}

fn type_of(j: &Json) -> R<u32> {
    match j.as_str() {
        Some("prop") => Ok(1),
        Some("flora") => Ok(4),
        Some("texture") => Ok(2),
        _ => u32_of(j, "type"),
    }
}

/// `cd_lot`: a lot (appended to the lot list) with `objects`
/// (`[{type: "prop" | "flora" | n, flags?, tag, ids?}]`; `ids` defaults to
/// `[tag]`), or `objects: null` (no configuration).
fn lot(w: &World, op: &Json) -> R<()> {
    let lot = token_of(field(op, "lot")?, "lot")?;
    w.lots.borrow_mut().push(lot);
    match op.get("objects") {
        None | Some(Json::Null) => {
            w.configs.borrow_mut().remove(&lot);
        }
        Some(v) => {
            let mut objs = Vec::new();
            for o in arr(v, "objects")? {
                let tag = u32_of(field(o, "tag")?, "tag")?;
                objs.push(CfgObject {
                    object_type: type_of(field(o, "type")?)?,
                    flags: opt(o, "flags").map_or(Ok(0), |v| u32_of(v, "flags"))? as u8,
                    tag,
                    ids: match opt(o, "ids") {
                        Some(v) => arr(v, "ids")?.iter().map(|i| u32_of(i, "id")).collect::<R<_>>()?,
                        None => vec![tag],
                    },
                    ..CfgObject::default()
                });
            }
            w.configs.borrow_mut().insert(lot, Rc::new(objs));
        }
    }
    Ok(())
}

fn occ_type_of(j: &Json) -> R<u32> {
    match j.as_str() {
        Some("prop") => Ok(OCCUPANT_TYPE_PROP),
        Some("flora") => Ok(OCCUPANT_TYPE_FLORA),
        Some("building") => Ok(0x2781_28A0),
        _ => u32_of(j, "type"),
    }
}

/// `cd_occupant`: `occupant` on `lot`, `type` ("prop", "flora",
/// "building" or an ID), `tag` (null or absent: no tag).
fn occupant(w: &World, op: &Json) -> R<()> {
    let occ = token_of(field(op, "occupant")?, "occupant")?;
    let o = Occ {
        lot: token_of(field(op, "lot")?, "lot")?,
        occ_type: occ_type_of(field(op, "type")?)?,
        tag: match op.get("tag") {
            None | Some(Json::Null) => None,
            Some(v) => Some(u32_of(v, "tag")?),
        },
    };
    w.occupants.borrow_mut().insert(occ, o);
    Ok(())
}

/// `cd_ids`: the lot configuration manager's ID array for object `index`
/// of `lot` (families expanded).
fn ids(w: &World, op: &Json) -> R<()> {
    let lot = token_of(field(op, "lot")?, "lot")?;
    let index = i64_of(field(op, "index")?, "index")? as usize;
    let v = arr(field(op, "ids")?, "ids")?
        .iter()
        .map(|i| u32_of(i, "id"))
        .collect::<R<Vec<_>>>()?;
    w.ids.borrow_mut().insert((lot, index), v);
    Ok(())
}

fn identity(w: &World, op: &Json) -> R<()> {
    if let Some(v) = opt(op, "iids") {
        for (k, want) in v.as_object().ok_or("iids must be an object")? {
            let iid = Json::Str(k.clone()).as_i64().ok_or("bad iid")? as u32;
            if query_interface(iid) != bool_of(want, k)? {
                return Err(format!("QueryInterface({k}) = {}", query_interface(iid)));
            }
        }
    }
    if let Some(v) = opt(op, "class") {
        let want = u32_of(v, "class")?;
        let got = w.mgr().map_or(CLSID_CITY_DETAIL, |m| m.class_id());
        if got != want {
            return Err(format!("class 0x{got:08X}, expected 0x{want:08X}"));
        }
    }
    Ok(())
}

fn expect(w: &World, op: &Json) -> R<()> {
    let m = w.mgr()?;
    let s = m.snapshot();
    let l = m.last_outcome();
    let got = vec![
        ("level".to_owned(), ji(s.level)),
        ("ref_count".to_owned(), ji(s.ref_count)),
        ("destroyed".to_owned(), Json::Bool(s.destroyed)),
        ("last_pass".to_owned(), ji(l.pass)),
        ("removed".to_owned(), ji(l.removed)),
        ("created".to_owned(), ji(l.created)),
        ("draws".to_owned(), ji(l.draws)),
        ("network_lots".to_owned(), ji(l.network_lots)),
        ("session_passes".to_owned(), ji(m.passes())),
    ];
    check_state(&got, field(op, "state")?)
}

/// `cd_expect_occupants`: the occupants of `lot`, in token order, as
/// `[[occupant, "prop" | "flora" | type, tag or null]]`.
fn expect_occupants(w: &World, op: &Json) -> R<()> {
    let lot = token_of(field(op, "lot")?, "lot")?;
    let got: Vec<Json> = w
        .on_lot(lot)
        .into_iter()
        .map(|t| {
            let o = w.occupants.borrow()[&t].clone();
            let ty = match o.occ_type {
                OCCUPANT_TYPE_PROP => Json::Str("prop".into()),
                OCCUPANT_TYPE_FLORA => Json::Str("flora".into()),
                other => ji(other),
            };
            Json::Array(vec![ji(t as i64), ty, o.tag.map_or(Json::Null, ji)])
        })
        .collect();
    let got = Json::Array(got);
    let want = field(op, "occupants")?;
    if scalar::matches(&got, want, false) {
        Ok(())
    } else {
        Err(format!(
            "occupants of lot {lot:#x}: {}, expected {}",
            scalar::show(&got),
            scalar::show(want)
        ))
    }
}

/// `cd_write`: the record; `hex`, `slot`.
fn write(w: &World, op: &Json) -> R<()> {
    let m = w.mgr()?;
    let mut out = ByteWriter::new();
    check_expect(op, Json::Bool(m.write(&mut out)))?;
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

/// `cd_read`: `hex` or `slot` into the current manager (or a new one with
/// `fresh: true`).
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
    let m = if fresh { w.new_mgr() } else { w.mgr()? };
    check_expect(op, Json::Bool(m.read(&mut ByteReader::new(&bytes))))
}
