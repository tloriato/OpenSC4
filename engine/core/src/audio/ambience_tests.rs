// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Tests for [`super`]: the conformance list of audio-ambience.md §10 and
//! lifetime / edge cases.

use super::*;

#[derive(Debug, Clone, PartialEq)]
enum Call {
    Layer(u32, &'static str, i64, i64),
    Post(PostKind, SndRequest),
    Sub(u32),
    Unsub(u32),
    Log(String),
    Fresh(FreshCall),
    Load(u32, u32, u32),
    Cond(u32, Option<(u32, u32)>),
    Link(u32),
    Unlink(u32),
    TickAdd,
    TickRemove,
    Walk(i32, i32),
}

type Log = std::rc::Rc<std::cell::RefCell<Vec<Call>>>;

struct TLayer {
    id: u32,
    ok: bool,
    log: Log,
}

impl TLayer {
    fn rec(&self, what: &'static str, a: i64, b: i64) {
        self.log.borrow_mut().push(Call::Layer(self.id, what, a, b));
    }
}

impl AmbienceLayer for TLayer {
    fn init(&mut self) -> bool {
        self.rec("init", 0, 0);
        self.ok
    }
    fn set_score_master_volume(&mut self, v: u32) {
        self.rec("volume", v as i64, 0);
    }
    fn handle_cell(&mut self, x: i32, z: i32) {
        self.rec("cell", x as i64, z as i64);
    }
    fn handle_occupant(&mut self, o: OccupantRef) {
        self.rec("occ", o as i64, 0);
    }
    fn begin_update(&mut self, r: CellRect) {
        self.rec("begin", r.x0 as i64, r.x1 as i64);
    }
    fn update(&mut self, e: u32, _p: [f32; 3], _v: [f32; 3]) {
        self.rec("update", e as i64, 0);
    }
    fn end_update(&mut self) {
        self.rec("end", 0, 0);
    }
    fn pause(&mut self, f: bool) {
        self.rec("pause", f as i64, 0);
    }
}

struct Host {
    log: Log,
    services: bool,
    tracked_ok: bool,
    tuning: AmbienceTuning,
    prefs: Option<(bool, u32)>,
    failing_layers: Vec<u32>,
    cells: (i32, i32),
    zoom: u32,
    listener: [f32; 3],
    clock: Option<(u32, u32)>,
    next_id: u32,
    init_walk: Vec<OccupantInfo>,
    cell_occ: Vec<(i32, i32, u32)>,
    load_ok: bool,
}

impl Host {
    fn new() -> Self {
        Host {
            log: Log::default(),
            services: true,
            tracked_ok: true,
            tuning: AmbienceTuning::default(),
            prefs: Some((true, 7)),
            failing_layers: vec![],
            cells: (64, 64),
            zoom: 4,
            listener: [512.0, 0.0, 512.0],
            clock: None,
            next_id: 100,
            init_walk: vec![],
            cell_occ: vec![],
            load_ok: true,
        }
    }
    fn push(&self, c: Call) {
        self.log.borrow_mut().push(c);
    }
    fn take(&self) -> Vec<Call> {
        std::mem::take(&mut *self.log.borrow_mut())
    }
    fn posts(&self) -> Vec<SndRequest> {
        self.take()
            .into_iter()
            .filter_map(|c| match c {
                Call::Post(_, r) => Some(r),
                _ => None,
            })
            .collect()
    }
}

impl AmbienceHost for Host {
    type Layer = TLayer;
    fn framework_init(&mut self) -> bool {
        true
    }
    fn acquire_scape_manager(&mut self) -> bool {
        self.services
    }
    fn acquire_snd_sys(&mut self) -> bool {
        true
    }
    fn register_event_target(&mut self) {}
    fn unregister_event_target(&mut self) {}
    fn acquire_message_server(&mut self) -> bool {
        true
    }
    fn subscribe(&mut self, t: u32) {
        self.push(Call::Sub(t));
    }
    fn unsubscribe(&mut self, t: u32) {
        self.push(Call::Unsub(t));
    }
    fn create_tracked_set(&mut self) -> bool {
        self.tracked_ok
    }
    fn release_services(&mut self) {}
    fn add_to_tick_list(&mut self) {
        self.push(Call::TickAdd);
    }
    fn remove_from_tick_list(&mut self) {
        self.push(Call::TickRemove);
    }
    fn post(&mut self, k: PostKind, r: SndRequest) {
        self.push(Call::Post(k, r));
    }
    fn generate_instance_id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }
    fn read_tuning(&mut self) -> AmbienceTuning {
        self.tuning.clone()
    }
    fn audio_preferences(&mut self) -> Option<(bool, u32)> {
        self.prefs
    }
    fn create_layer(&mut self, id: u32) -> TLayer {
        TLayer {
            id,
            ok: !self.failing_layers.contains(&id),
            log: self.log.clone(),
        }
    }
    fn log(&mut self, line: &str) {
        self.push(Call::Log(line.to_string()));
    }
    fn city_cell_counts(&mut self) -> (i32, i32) {
        self.cells
    }
    fn cell_to_world(&mut self, x: i32, z: i32) -> (f32, f32) {
        (x as f32 * 16.0, z as f32 * 16.0)
    }
    fn world_to_cell(&mut self, x: f32, z: f32) -> (i32, i32) {
        ((x / 16.0).floor() as i32, (z / 16.0).floor() as i32)
    }
    fn zoom_index(&mut self) -> u32 {
        self.zoom
    }
    fn listener(&mut self) -> ([f32; 3], [f32; 3]) {
        (self.listener, [0.0, 0.0, 1.0])
    }
    fn occupants_of_types(&mut self, types: &[u32]) -> Vec<OccupantInfo> {
        assert_eq!(types, &INIT_WALK_TYPES);
        self.init_walk.clone()
    }
    fn occupants_in_cell(&mut self, x: i32, z: i32) -> Vec<OccupantRef> {
        self.push(Call::Walk(x, z));
        self.cell_occ
            .iter()
            .filter(|c| c.0 == x && c.1 == z)
            .map(|c| c.2)
            .collect()
    }
    fn lot_of_occupant(&mut self, o: OccupantRef) -> Option<u32> {
        Some(o + 1)
    }
    fn condition_update_unspecified(&mut self, lot: u32, v: Option<(u32, u32)>) {
        self.push(Call::Cond(lot, v));
    }
    fn link_automaton(&mut self, o: OccupantRef) {
        self.push(Call::Link(o));
    }
    fn unlink_automaton(&mut self, o: OccupantRef) {
        self.push(Call::Unlink(o));
    }
    fn occupant_position(&mut self, _o: OccupantRef) -> [f32; 3] {
        [10.7, 2.2, -3.9]
    }
    fn on_off_sound_property(&mut self, _o: OccupantRef) -> Option<u32> {
        Some(0x55)
    }
    fn clock(&mut self) -> Option<(u32, u32)> {
        self.clock
    }
    fn fresh_player_call(&mut self, c: FreshCall) {
        self.push(Call::Fresh(c));
    }
    fn fresh_player_load_score(&mut self, t: u32, g: u32, i: u32) -> bool {
        self.push(Call::Load(t, g, i));
        self.load_ok
    }
}

type M = AmbienceManager<TLayer>;

fn ready(h: &mut Host) -> M {
    let mut m = M::new();
    assert!(m.init(h));
    assert!(m.do_message(h, true, &AmbienceMessage::PostCityInit));
    h.take();
    m
}

fn occ(id: u32) -> OccupantInfo {
    OccupantInfo {
        occupant: id,
        position: [1.9, 2.5, -3.7],
        ..Default::default()
    }
}

fn ins(m: &mut M, h: &mut Host, o: &OccupantInfo, t: u32) {
    let msg = AmbienceMessage::Occupant {
        msg_type: t,
        occupant: Some(o.clone()),
    };
    assert!(m.do_message(h, true, &msg));
}

#[test]
fn constructor_values() {
    let m = M::new();
    assert_eq!(AMBIENCE_MANAGER_SIZE, 0x120);
    assert!(m.ticking);
    assert!(!m.city_ready);
    assert_eq!(m.stage, Stage::Begin0);
    assert_eq!(m.tracked_count, 0);
    assert!(m.margins.is_none());
}

// §10.1
#[test]
fn missing_margins_default_to_70() {
    let mut h = Host::new();
    let mut m = M::new();
    m.read_tuning(&mut h);
    assert_eq!(m.margins, Some([70.0; 5]));
    assert_eq!(DEFAULT_MARGIN.to_bits(), 0x428C_0000);
    h.tuning.margins = Some(vec![0.0, 0.0, 188.0, 150.0, 73.0]);
    m.read_tuning(&mut h);
    assert_eq!(m.margins, Some([0.0, 0.0, 188.0, 150.0, 73.0]));
}

// §10.2
#[test]
fn init_subscribes_twelve_in_order() {
    let mut h = Host::new();
    let mut m = M::new();
    assert!(m.init(&mut h));
    let subs: Vec<u32> = h
        .take()
        .into_iter()
        .filter_map(|c| match c {
            Call::Sub(t) => Some(t),
            _ => None,
        })
        .collect();
    assert_eq!(subs, SUBSCRIBED_MESSAGES.to_vec());
    assert!(subs.contains(&MSG_EXEMPLAR_CHANGED));
    assert_eq!(subs.len(), 12);
}

#[test]
fn init_failures_return_false() {
    let mut h = Host::new();
    h.services = false;
    assert!(!M::new().init(&mut h));
    assert!(h.take().is_empty());
    let mut h = Host::new();
    h.tracked_ok = false;
    let mut m = M::new();
    assert!(!m.init(&mut h));
    assert!(m.tracked.is_none());
}

#[test]
fn shutdown_unsubscribes_same_set_and_releases() {
    let mut h = Host::new();
    h.tuning.layer_ids = Some(vec![1, 2]);
    let mut m = ready(&mut h);
    assert_eq!(m.layers.len(), 2);
    assert!(m.shutdown(&mut h));
    let mut un: Vec<u32> = h
        .take()
        .into_iter()
        .filter_map(|c| match c {
            Call::Unsub(t) => Some(t),
            _ => None,
        })
        .collect();
    let mut want = SUBSCRIBED_MESSAGES.to_vec();
    un.sort();
    want.sort();
    assert_eq!(un, want);
    assert!(m.layers.is_empty());
}

#[test]
fn city_init_creates_layers_and_drops_failures() {
    let mut h = Host::new();
    h.tuning.layer_ids = Some(vec![0xA, 0xB, 0xC]);
    h.failing_layers = vec![0xB];
    let mut m = M::new();
    m.init(&mut h);
    h.take();
    assert!(m.do_message(&mut h, true, &AmbienceMessage::PostCityInit));
    let log = h.take();
    assert!(log.contains(&Call::Log(layer_init_failure_message(0xB))));
    assert!(log.contains(&Call::Layer(0xA, "volume", 7, 0)));
    assert!(!log.contains(&Call::Layer(0xB, "volume", 7, 0)));
    assert!(log.contains(&Call::TickAdd));
    assert_eq!(
        layer_init_failure_message(0xB),
        "Error : cSC4AudioAmbienceManager::_CreateAmbienceLayers : Failed to init layer (0000000b)\n"
    );
    assert_eq!(m.layers.iter().map(|l| l.id).collect::<Vec<_>>(), vec![0xA, 0xC]);
    assert!(m.city_ready && m.ticking);
    assert_eq!(
        m.city_world,
        WorldRect {
            x0: 0.0,
            z0: 0.0,
            x1: 1008.0,
            z1: 1008.0
        }
    );
}

#[test]
fn preference_clear_gives_zero_volume() {
    let mut h = Host::new();
    h.tuning.layer_ids = Some(vec![1]);
    h.prefs = Some((false, 9));
    let mut m = M::new();
    m.init(&mut h);
    m.handle_city_init(&mut h);
    assert!(h.take().contains(&Call::Layer(1, "volume", 0, 0)));
}

#[test]
fn city_shutdown_stops_ticking_and_clears() {
    let mut h = Host::new();
    h.tuning.layer_ids = Some(vec![1]);
    let mut m = ready(&mut h);
    let mut o = occ(0x1000);
    o.alarm_times = Some(vec![7.0]);
    o.one_shot_alarm_sound = Some(3);
    ins(&mut m, &mut h, &o, MSG_INSERT_OCCUPANT);
    assert!(m.do_message(&mut h, true, &AmbienceMessage::PreCityShutdown));
    assert!(!m.ticking && m.layers.is_empty() && m.alarms.is_empty());
    assert!(h.take().contains(&Call::TickRemove));
    m.on_tick(&mut h, 5, || true);
    assert!(h.take().is_empty());
}

#[test]
fn non_payload_message_returns_zero() {
    let mut h = Host::new();
    let mut m = ready(&mut h);
    assert!(!m.do_message(&mut h, false, &AmbienceMessage::PostCityInit));
    assert!(m.do_message(&mut h, true, &AmbienceMessage::Other(0x1234)));
}

// §10.3
#[test]
fn time_of_day_drops_minutes() {
    assert_eq!(time_of_day(7, 59), 7.0);
    assert_eq!(time_of_day(0, 0), 0.0);
    let mut h = Host::new();
    let mut m = ready(&mut h);
    let mut o = occ(0x2000);
    o.alarm_times = Some(vec![7.5]);
    o.one_shot_alarm_sound = Some(0x99);
    ins(&mut m, &mut h, &o, MSG_INSERT_OCCUPANT);
    h.take();
    h.clock = Some((7, 59));
    m.update_alarms(&mut h);
    assert!(h.take().is_empty());
    h.clock = Some((8, 0));
    m.update_alarms(&mut h);
    let log = h.take();
    assert_eq!(
        log,
        vec![
            Call::Post(
                PostKind::Extended,
                SndRequest::Start {
                    sound_id: 0x99,
                    instance_id: 101,
                    looping: false,
                    position: [10, 2, -3]
                }
            ),
            Call::Post(
                PostKind::Extended,
                SndRequest::SetPosition {
                    instance_id: 101,
                    position: [10, 2, -3]
                }
            ),
        ]
    );
    m.update_alarms(&mut h);
    assert!(h.take().is_empty());
}

#[test]
fn one_shot_at_midnight_never_fires_after_wrap() {
    let mut h = Host::new();
    let mut m = ready(&mut h);
    let mut o = occ(0x2000);
    o.alarm_times = Some(vec![0.0]);
    o.one_shot_alarm_sound = Some(1);
    ins(&mut m, &mut h, &o, MSG_INSERT_OCCUPANT);
    h.clock = Some((23, 0));
    m.update_alarms(&mut h);
    h.clock = Some((0, 0));
    m.update_alarms(&mut h);
    assert!(h.posts().is_empty());
}

// §10.4
#[test]
fn on_off_alarm_22_to_6() {
    let mut h = Host::new();
    let mut m = ready(&mut h);
    let mut o = occ(0x3000);
    o.alarm_times = Some(vec![22.0, 6.0]);
    o.on_off_alarm_sound = Some(0x55);
    ins(&mut m, &mut h, &o, MSG_INSERT_OCCUPANT);
    h.take();
    let iid = occupant_instance_id(0x3000, 0x55);
    h.clock = Some((21, 0));
    m.update_alarms(&mut h);
    assert!(h.posts().is_empty());
    h.clock = Some((22, 0));
    m.update_alarms(&mut h);
    assert_eq!(
        h.posts(),
        vec![SndRequest::Start {
            sound_id: 0x55,
            instance_id: iid,
            looping: true,
            position: [10, 2, -3]
        }]
    );
    for hr in [23, 0, 3] {
        h.clock = Some((hr, 30));
        m.update_alarms(&mut h);
        assert!(h.posts().is_empty(), "hour {hr}");
    }
    h.clock = Some((6, 0));
    m.update_alarms(&mut h);
    assert_eq!(h.posts(), vec![SndRequest::Stop { instance_id: iid }]);
    h.clock = Some((7, 0));
    m.update_alarms(&mut h);
    assert!(h.posts().is_empty());
}

#[test]
fn no_clock_no_alarms() {
    let mut h = Host::new();
    let mut m = ready(&mut h);
    let mut o = occ(1);
    o.alarm_times = Some(vec![1.0]);
    o.one_shot_alarm_sound = Some(1);
    ins(&mut m, &mut h, &o, MSG_INSERT_OCCUPANT);
    m.update_alarms(&mut h);
    assert_eq!(m.alarms[0].last, 0.0);
}

// §10.5
#[test]
fn preview_occupant_is_silent() {
    let mut h = Host::new();
    let mut m = ready(&mut h);
    let mut o = occ(0x4000);
    o.preview = true;
    o.ambient_sound = Some(1);
    o.second_sound = Some(2);
    o.condition_sound_a = Some(3);
    o.alarm_times = Some(vec![1.0]);
    o.one_shot_alarm_sound = Some(4);
    for t in [MSG_INSERT_OCCUPANT, MSG_MOVE_OCCUPANT, MSG_REMOVE_OCCUPANT] {
        ins(&mut m, &mut h, &o, t);
    }
    assert!(h.take().is_empty());
    assert_eq!(m.tracked_count, 0);
    assert!(m.alarms.is_empty());
}

#[test]
fn insert_move_remove_sounds() {
    let mut h = Host::new();
    let mut m = ready(&mut h);
    let mut o = occ(0x5000);
    o.is_automaton = true;
    o.ambient_sound = Some(0x10);
    o.second_sound = Some(0x20);
    o.condition_sound_a = Some(0x30);
    ins(&mut m, &mut h, &o, MSG_INSERT_OCCUPANT);
    let p = [1, 2, -3];
    assert_eq!(
        h.take(),
        vec![
            Call::Link(0x5000),
            Call::Post(
                PostKind::Normal,
                SndRequest::Start {
                    sound_id: 0x10,
                    instance_id: 0x5010,
                    looping: true,
                    position: p
                }
            ),
            Call::Post(
                PostKind::Normal,
                SndRequest::SetPosition {
                    instance_id: 0x5010,
                    position: p
                }
            ),
            Call::Link(0x5000),
            Call::Post(
                PostKind::Normal,
                SndRequest::Start {
                    sound_id: 0x20,
                    instance_id: 0x5020,
                    looping: false,
                    position: p
                }
            ),
            Call::Post(
                PostKind::Normal,
                SndRequest::SetPosition {
                    instance_id: 0x5020,
                    position: p
                }
            ),
            Call::Cond(0x5001, None),
        ]
    );
    assert_eq!(m.tracked_count, 1);
    ins(&mut m, &mut h, &o, MSG_INSERT_OCCUPANT);
    assert_eq!(m.tracked_count, 1, "a second insert does not count twice");
    h.take();
    ins(&mut m, &mut h, &o, MSG_MOVE_OCCUPANT);
    assert_eq!(
        h.posts(),
        vec![
            SndRequest::SetPosition {
                instance_id: 0x5010,
                position: p
            },
            SndRequest::SetPosition {
                instance_id: 0x5030,
                position: p
            },
        ]
    );
    ins(&mut m, &mut h, &o, MSG_REMOVE_OCCUPANT);
    assert_eq!(
        h.take(),
        vec![
            Call::Unlink(0x5000),
            Call::Post(PostKind::Normal, SndRequest::Stop { instance_id: 0x5030 }),
            Call::Unlink(0x5000),
            Call::Post(PostKind::Normal, SndRequest::Stop { instance_id: 0x5010 }),
        ]
    );
    assert_eq!(m.tracked_count, 0);
    ins(&mut m, &mut h, &o, MSG_REMOVE_OCCUPANT);
    assert_eq!(m.tracked_count, 0);
    assert_eq!(h.posts().len(), 2, "stop is posted even when nothing runs");
}

#[test]
fn state_changed_does_nothing() {
    let mut h = Host::new();
    let mut m = ready(&mut h);
    let mut o = occ(1);
    o.ambient_sound = Some(1);
    ins(&mut m, &mut h, &o, MSG_STATE_CHANGED);
    assert!(h.take().is_empty());
    assert_eq!(m.tracked_count, 0);
}

#[test]
fn init_walk_starts_no_sound() {
    let mut h = Host::new();
    let mut o = occ(0x6000);
    o.ambient_sound = Some(1);
    o.second_sound = Some(2);
    o.alarm_times = Some(vec![5.0]);
    o.one_shot_alarm_sound = Some(3);
    o.on_off_alarm_sound = Some(4);
    h.init_walk = vec![o];
    let m = ready(&mut h);
    assert_eq!(m.tracked_count, 1);
    assert_eq!(m.alarms.len(), 2);
}

#[test]
fn remove_drops_matching_alarm_only() {
    let mut h = Host::new();
    let mut m = ready(&mut h);
    for id in [1, 2] {
        let mut o = occ(id);
        o.alarm_times = Some(vec![5.0]);
        o.one_shot_alarm_sound = Some(3);
        o.on_off_alarm_sound = Some(4);
        ins(&mut m, &mut h, &o, MSG_INSERT_OCCUPANT);
    }
    let mut o = occ(1);
    o.on_off_alarm_sound = Some(4);
    ins(&mut m, &mut h, &o, MSG_REMOVE_OCCUPANT);
    let left: Vec<_> = m.alarms.iter().map(|a| (a.occupant, a.kind)).collect();
    assert_eq!(
        left,
        vec![(1, AlarmKind::OneShot), (2, AlarmKind::OneShot), (2, AlarmKind::OnOff)]
    );
}

#[test]
fn condition_changed_needs_lot() {
    let mut h = Host::new();
    let mut m = ready(&mut h);
    let msg = AmbienceMessage::ConditionChanged {
        lot: None,
        values: (1, 2),
    };
    assert!(m.do_message(&mut h, true, &msg));
    assert!(h.take().is_empty());
    let msg = AmbienceMessage::ConditionChanged {
        lot: Some(9),
        values: (1, 2),
    };
    m.do_message(&mut h, true, &msg);
    assert_eq!(h.take(), vec![Call::Cond(9, Some((1, 2)))]);
}

#[test]
fn automata_codes() {
    let mut h = Host::new();
    let mut m = ready(&mut h);
    let mut o = occ(0x100);
    o.ambient_sound = Some(0x1);
    let ev = |code, kind, slot| {
        AmbienceMessage::AutomataEvent(AutomatonEvent {
            code,
            kind,
            slot_e4_0x10: slot,
            occupant: o.clone(),
        })
    };
    let p = [1, 2, -3];
    m.do_message(&mut h, true, &ev(9, 0, 0));
    assert_eq!(
        h.posts(),
        vec![SndRequest::SetPosition {
            instance_id: 0x101,
            position: p
        }]
    );
    m.do_message(&mut h, true, &ev(13, 0x14, 1));
    assert_eq!(
        h.posts(),
        vec![
            SndRequest::Start {
                sound_id: 1,
                instance_id: 0x101,
                looping: false,
                position: p
            },
            SndRequest::SetPosition {
                instance_id: 0x101,
                position: p
            },
        ]
    );
    m.do_message(&mut h, true, &ev(13, 0x10, 1));
    assert_eq!(h.posts(), vec![SndRequest::Stop { instance_id: 0x101 }]);
    m.do_message(&mut h, true, &ev(13, 0x10, 0));
    assert_eq!(h.posts().len(), 2);
    m.do_message(&mut h, true, &ev(12, 0x1E, 0));
    assert_eq!(h.posts(), vec![SndRequest::Stop { instance_id: 0x101 }]);
    m.do_message(&mut h, true, &ev(13, 0x11, 0));
    m.do_message(&mut h, true, &ev(5, 0x10, 0));
    assert!(h.posts().is_empty());
}

#[test]
fn railway_crossing() {
    let mut h = Host::new();
    let mut m = ready(&mut h);
    let mut o = occ(0x200);
    m.do_message(
        &mut h,
        true,
        &AmbienceMessage::ApproachingIntersection {
            automaton: Some(o.clone()),
        },
    );
    m.do_message(
        &mut h,
        true,
        &AmbienceMessage::ApproachingIntersection { automaton: None },
    );
    assert!(h.posts().is_empty());
    o.crossing_sound = Some(0x7);
    m.do_message(
        &mut h,
        true,
        &AmbienceMessage::ApproachingIntersection { automaton: Some(o) },
    );
    let p = [1, 2, -3];
    assert_eq!(
        h.posts(),
        vec![
            SndRequest::Start {
                sound_id: 7,
                instance_id: 0x207,
                looping: false,
                position: p
            },
            SndRequest::SetPosition {
                instance_id: 0x207,
                position: p
            },
        ]
    );
}

#[test]
fn attention_rect_clamps_and_rounds_even() {
    let mut h = Host::new();
    h.tuning.margins = Some(vec![0.0, 0.0, 188.0, 150.0, 73.0]);
    h.zoom = 4;
    h.listener = [100.0, 0.0, 1000.0];
    let mut m = ready(&mut h);
    // x: 27/16=1 -> 0, 173/16=10 -> 10; z: 927/16=57 -> 56, 1073/16=67 -> 63 -> 62
    assert_eq!(
        m.current,
        CellRect {
            x0: 0,
            z0: 56,
            x1: 10,
            z1: 62
        }
    );
    assert_eq!(m.previous, m.current, "first call writes both");
    assert_eq!(
        m.attention_world,
        WorldRect {
            x0: 27.0,
            z0: 927.0,
            x1: 173.0,
            z1: 1073.0
        }
    );
    h.listener = [-500.0, 0.0, -500.0];
    m.do_message(&mut h, true, &AmbienceMessage::ListenerPosChanged);
    assert_eq!(
        m.previous,
        CellRect {
            x0: 0,
            z0: 56,
            x1: 10,
            z1: 62
        }
    );
    assert_eq!(m.current, CellRect::default());
    h.zoom = 9;
    m.do_message(&mut h, true, &AmbienceMessage::ListenerPosChanged);
    assert_eq!(m.current, CellRect::default());
    assert_eq!(m.zoom, 9);
}

// §10.6
#[test]
fn stage_update_cell_order_and_stages() {
    let mut h = Host::new();
    h.tuning.layer_ids = Some(vec![1]);
    h.tuning.margins = Some(vec![10.0; 5]);
    h.listener = [45.0, 0.0, 45.0];
    h.cell_occ = vec![(3, 3, 0xAB)];
    let mut m = ready(&mut h);
    assert_eq!(
        m.current,
        CellRect {
            x0: 2,
            z0: 2,
            x1: 2,
            z1: 2
        }
    );
    m.on_tick(&mut h, 33, || true);
    assert_eq!(m.seeded_blocks(), &[(1, 1)]);
    assert_eq!(
        h.take(),
        vec![
            Call::Layer(1, "begin", 2, 2),
            Call::Walk(2, 2),
            Call::Walk(3, 2),
            Call::Walk(2, 3),
            Call::Walk(3, 3),
            Call::Layer(1, "occ", 0xAB, 0),
            Call::Layer(1, "cell", 2, 2),
            Call::Layer(1, "cell", 3, 2),
            Call::Layer(1, "cell", 2, 3),
            Call::Layer(1, "cell", 3, 3),
            Call::Layer(1, "update", 33, 0),
            Call::Layer(1, "end", 0, 0),
        ]
    );
    assert_eq!(m.stage, Stage::Begin1);
}

#[test]
fn budget_spreads_walk_over_ticks() {
    let mut h = Host::new();
    h.tuning.layer_ids = Some(vec![1]);
    h.tuning.margins = Some(vec![40.0; 5]);
    h.listener = [64.0, 0.0, 64.0];
    let mut m = ready(&mut h);
    assert_eq!(
        m.current,
        CellRect {
            x0: 0,
            z0: 0,
            x1: 6,
            z1: 6
        }
    );
    let mut seen = vec![];
    let mut ticks = 0;
    loop {
        let mut left = 3;
        m.on_tick(&mut h, 1, || {
            left -= 1;
            left >= 0
        });
        ticks += 1;
        let log = h.take();
        seen.extend(log.iter().filter_map(|c| match c {
            Call::Layer(_, "cell", x, z) if x % 2 == 0 && z % 2 == 0 => Some((*x, *z)),
            _ => None,
        }));
        assert_eq!(
            log.iter()
                .filter(|c| matches!(c, Call::Layer(_, "update", _, _)))
                .count(),
            1
        );
        if log.contains(&Call::Layer(1, "end", 0, 0)) {
            break;
        }
        assert!(ticks < 20);
    }
    assert_eq!(seen.len(), 16, "every block of the 4x4-block rectangle");
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), 16);
    assert_eq!(m.stage, Stage::Begin1);
    m.on_tick(&mut h, 1, || false);
    assert!(h.take().contains(&Call::Layer(1, "begin", 0, 6)));
    assert_eq!(m.stage, Stage::Walk);
}

#[test]
fn ticking_disabled_does_nothing() {
    let mut h = Host::new();
    h.tuning.layer_ids = Some(vec![1]);
    let mut m = ready(&mut h);
    m.ticking = false;
    m.on_tick(&mut h, 1, || true);
    assert!(h.take().is_empty());
}

#[test]
fn events_master_volume_and_fresh_player() {
    let mut h = Host::new();
    h.tuning.layer_ids = Some(vec![1, 2]);
    let mut m = ready(&mut h);
    let ev = |id, value, fresh| SndEventIn {
        id,
        value,
        value2: 6,
        fresh_player: fresh,
        score_tgi: Some((1, 2, 3)),
    };
    assert!(m.do_event(&mut h, &ev(EVT_MASTER_VOLUME, 42, false)));
    assert_eq!(
        h.take(),
        vec![Call::Layer(1, "volume", 42, 0), Call::Layer(2, "volume", 42, 0)]
    );
    assert!(!m.do_event(&mut h, &ev(EVT_FRESH_STOP, 0, false)));
    assert!(!m.do_event(&mut h, &ev(0x1234, 0, true)));
    assert!(h.take().is_empty());
    let cases = [
        (EVT_FRESH_STOP, 0, vec![FreshCall::Stop, FreshCall::Shutdown]),
        (EVT_FRESH_PLAY_PERCENT, 5, vec![FreshCall::SetPlayPercent(5)]),
        (
            EVT_FRESH_FADE_OUT,
            0,
            vec![FreshCall::FadeVolume {
                ms: 3000,
                target: 0,
                flag: 1,
            }],
        ),
        (EVT_FRESH_SET_VOLUME, 8, vec![FreshCall::SetVolume(8)]),
        (
            EVT_FRESH_PLAY,
            2,
            vec![FreshCall::SetHeadRelative(true), FreshCall::Play],
        ),
        (EVT_FRESH_MIN_MAX_DISTANCE, 4, vec![FreshCall::SetMinMaxDistance(4, 6)]),
    ];
    for (id, v, want) in cases {
        assert!(m.do_event(&mut h, &ev(id, v, true)));
        let got: Vec<_> = h
            .take()
            .into_iter()
            .filter_map(|c| match c {
                Call::Fresh(f) => Some(f),
                _ => None,
            })
            .collect();
        assert_eq!(got, want, "{id:08x}");
    }
    h.load_ok = false;
    assert!(m.do_event(&mut h, &ev(EVT_FRESH_LOAD_SCORE, 0, true)));
    assert_eq!(
        h.take(),
        vec![Call::Load(1, 2, 3), Call::Log(score_load_failure_message(1, 2, 3))]
    );
    assert_eq!(
        score_load_failure_message(1, 2, 0xABC),
        "Error: cSC4AudioAmbienceLayer::DoEvent : Failed to load freshness score (00000001:00000002:00000abc)\n"
    );
}

#[test]
fn focus_pauses_layers_and_exemplar_rereads_tuning() {
    let mut h = Host::new();
    h.tuning.layer_ids = Some(vec![1]);
    let mut m = ready(&mut h);
    assert!(m.do_message(&mut h, true, &AmbienceMessage::AppFocus { flag: true }));
    assert_eq!(h.take(), vec![Call::Layer(1, "pause", 1, 0)]);
    h.tuning.margins = Some(vec![1.0; 5]);
    h.tuning.layer_ids = Some(vec![5, 6]);
    assert!(m.do_message(&mut h, true, &AmbienceMessage::ExemplarChanged));
    assert_eq!(m.margins, Some([1.0; 5]));
    assert_eq!(m.layer_ids, vec![5, 6]);
    assert_eq!(m.layers.len(), 1, "layers are not recreated");
}

#[test]
fn instance_id_wraps() {
    assert_eq!(occupant_instance_id(0xFFFF_FFF0, 0x20), 0x10);
}
