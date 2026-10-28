// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! clock24.md §12 (one test per item, `ck12_NN`) plus the worked examples of
//! §4.9.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::*;
use crate::stream::{ByteReader, ByteWriter, OutStream};

#[derive(Default)]
struct Fake {
    now: Cell<u64>,
    clock_reads: Cell<u32>,
    posts: RefCell<Vec<(u32, u32, u32)>>,
    calls: RefCell<Vec<String>>,
    cheats: Cell<bool>,
    tuning: Cell<Option<ClockTuning>>,
}

struct H(Rc<Fake>);

impl ClockHost for H {
    fn now_micros(&self) -> u64 {
        self.0.clock_reads.set(self.0.clock_reads.get() + 1);
        self.0.now.get()
    }
    fn post(&self, msg_type: u32, data2: u32, data3: u32) {
        self.0.posts.borrow_mut().push((msg_type, data2, data3));
    }
    fn add_system_service(&self) {
        self.0.calls.borrow_mut().push("add_system_service".into());
    }
    fn remove_system_service(&self) {
        self.0.calls.borrow_mut().push("remove_system_service".into());
    }
    fn add_to_on_idle(&self) {
        self.0.calls.borrow_mut().push("add_to_on_idle".into());
    }
    fn remove_from_on_idle(&self) {
        self.0.calls.borrow_mut().push("remove_from_on_idle".into());
    }
    fn has_cheat_manager(&self) -> bool {
        self.0.cheats.get()
    }
    fn register_cheat(&self, id: u32, name: &str) {
        self.0
            .calls
            .borrow_mut()
            .push(format!("register_cheat {id:08X} {name}"));
    }
    fn add_cheat_notification(&self, id: u32) {
        self.0
            .calls
            .borrow_mut()
            .push(format!("add_cheat_notification {id:08X}"));
    }
    fn remove_cheat_notification(&self, id: u32) {
        self.0
            .calls
            .borrow_mut()
            .push(format!("remove_cheat_notification {id:08X}"));
    }
    fn unregister_cheat(&self, id: u32) {
        self.0.calls.borrow_mut().push(format!("unregister_cheat {id:08X}"));
    }
    fn tuning(&self) -> Option<ClockTuning> {
        self.0.tuning.get()
    }
}

fn new() -> (Rc<Fake>, Clock24) {
    let f = Rc::new(Fake::default());
    f.now.set(1_000_000);
    (f.clone(), Clock24::new(Box::new(H(f))))
}

/// An initialised clock with the shipped tuning (magnification 135, night
/// 20 → 1) and the timer running from the fake's time.
fn shipped() -> (Rc<Fake>, Clock24) {
    let (f, mut c) = new();
    f.tuning.set(Some(SHIPPED_TUNING));
    assert!(c.init());
    (f, c)
}

impl Fake {
    fn advance(&self, us: u64) {
        self.now.set(self.now.get() + us);
    }
    fn take_posts(&self) -> Vec<(u32, u32, u32)> {
        std::mem::take(&mut *self.posts.borrow_mut())
    }
}

use super::msg::*;

/// The elapsed times that give steps of 61, 62 and 63 at magnification 135.
const E61: u64 = 451_852;
const E62: u64 = 459_260;
const E63: u64 = 466_676;

/// Runs one idle pass `e` microseconds after the last restart.
fn step(f: &Fake, c: &mut Clock24, e: u64) {
    let had = c.timer_elapsed_us();
    f.advance(e - had);
    assert!(c.on_idle());
}

// ---- §12.1 construction -------------------------------------------------------------------------

#[test]
fn ck12_01_construction() {
    let (_f, c) = new();
    let s = c.state();
    assert_eq!((s.seconds, s.visual_seconds), (43_200, 43_200));
    assert_eq!(s.magnification, 100.0);
    assert_eq!(s.min_magnification.to_bits(), 0x3D4C_CCCD);
    assert_eq!(s.max_magnification, 2000.0);
    assert_eq!((s.night_start, s.night_end), (20, 4));
    assert!(s.day_night_notifications);
    assert_eq!(s.visual_mode, 0);
    assert_eq!((s.pause_count, s.hidden_pause_count), (0, 0));
    assert!(!s.initialized);
    assert_eq!(s.timer, Timer::default());
    assert_eq!(c.time_percentage(), 0.5);
    assert_eq!(c.visual_time_percentage(), 0.5);
    // The factor is the float of 1.1574074e-05 (not the spec's 0x37C22E45).
    assert_eq!(PERCENT_FACTOR_BITS, 1.157_407_4e-5_f32.to_bits());
    let mut c2 = Clock24::new(Box::new(H(Rc::new(Fake::default()))));
    c2.set_times(75_600, 0);
    assert_eq!((c2.time_percentage(), c2.visual_time_percentage()), (0.875, 0.0));
    assert_eq!(c.hour(), 12);
    assert_eq!((c.minute(), c.second()), (0, 0));
    assert!(!c.is_night());
    assert_eq!((c.event_hour(0), c.event_hour(1), c.event_hour(2)), (20, 4, 0));
    for iid in [1, 0x681B_D53A, 0x4522_94AA, 0xE4FD_A3D4, 0x287F_B697] {
        assert!(answers_iid(iid), "{iid:08X}");
    }
    assert!(!answers_iid(0) && !answers_iid(CLSID_24_HOUR_CLOCK));
}

#[test]
fn ck12_01_init_and_shutdown() {
    let (f, mut c) = new();
    f.cheats.set(true);
    assert!(c.init());
    assert_eq!(
        *f.calls.borrow(),
        vec![
            "add_system_service".to_string(),
            "add_to_on_idle".into(),
            "register_cheat 281F7858 WhatTimeIzIt".into(),
            "add_cheat_notification 281F7858".into(),
            "register_cheat C9B73FBB Stopwatch".into(),
            "add_cheat_notification C9B73FBB".into(),
        ]
    );
    assert!(c.is_initialized() && c.timer().running);
    // A second Init does nothing.
    f.calls.borrow_mut().clear();
    assert!(c.init());
    assert!(f.calls.borrow().is_empty());
    f.advance(100);
    assert!(c.shutdown());
    assert_eq!(
        *f.calls.borrow(),
        vec![
            "remove_cheat_notification 281F7858".to_string(),
            "unregister_cheat 281F7858".into(),
            "remove_cheat_notification C9B73FBB".into(),
            "unregister_cheat C9B73FBB".into(),
            "remove_from_on_idle".into(),
            "remove_system_service".into(),
        ]
    );
    let s = c.state();
    assert_eq!((s.initialized, s.seconds, s.timer), (false, 0, Timer::default()));
    assert_eq!(s.visual_seconds, 43_200);
    f.calls.borrow_mut().clear();
    assert!(c.shutdown());
    assert!(f.calls.borrow().is_empty());
    assert!(f.posts.borrow().is_empty());
    // No cheat code manager: no cheat calls.
    let (f, mut c) = new();
    c.init();
    c.shutdown();
    assert_eq!(f.calls.borrow().len(), 4);
}

// ---- §12.2 tuning -------------------------------------------------------------------------------

#[test]
fn ck12_02_tuning() {
    let (_f, c) = shipped();
    assert_eq!(c.time_magnification(), 135.0);
    assert_eq!((c.event_hour(0), c.event_hour(1)), (20, 1));

    let (_f, mut c) = new();
    c.init();
    let s = c.state();
    assert_eq!((s.magnification, s.night_start, s.night_end), (100.0, 20, 4));

    for (m, want) in [(5000.0, 2000.0), (0.01, f32::from_bits(0x3D4C_CCCD))] {
        let (f, mut c) = new();
        f.tuning.set(Some(ClockTuning {
            magnification: Some(m),
            ..ClockTuning::default()
        }));
        c.init();
        assert_eq!(c.time_magnification(), want);
        assert_eq!((c.event_hour(0), c.event_hour(1)), (20, 4));
    }
    // A missing magnification property still goes through the setter.
    let (f, mut c) = new();
    c.set_time_magnification_min_max(1.0, 50.0);
    f.tuning.set(Some(ClockTuning {
        night_end: Some(30),
        ..ClockTuning::default()
    }));
    c.init();
    assert_eq!(c.time_magnification(), 50.0);
    assert_eq!((c.event_hour(0), c.event_hour(1)), (20, 30));
}

// ---- §12.3 the step threshold -----------------------------------------------------------------

#[test]
fn ck12_03_step_threshold() {
    let (f, mut c) = shipped();
    step(&f, &mut c, E61 - 1);
    assert_eq!(c.time_of_day(), 43_200);
    assert_eq!(c.timer_elapsed_us(), E61 - 1); // not restarted
    f.advance(1);
    assert!(c.on_idle());
    assert_eq!(c.time_of_day(), 43_261);
    assert_eq!(c.timer_elapsed_us(), 0);
    assert!(c.timer().running);
    step(&f, &mut c, 10_000_000);
    assert_eq!(c.time_of_day(), 43_261 + 1200);

    // Either pause counter: nothing (and the timer is stopped).
    for hidden in [false, true] {
        let (f, mut c) = shipped();
        if hidden {
            c.hidden_pause(true);
        } else {
            c.pause(true);
        }
        f.take_posts();
        f.advance(10_000_000);
        assert!(c.on_idle());
        assert_eq!(c.time_of_day(), 43_200);
        assert!(f.posts.borrow().is_empty());
    }
}

#[test]
fn ck12_03_step_formula() {
    // §4.9: f32(451,852 × 135) = 61,000,020; × the constant = 61.00002.
    assert_eq!(mul_u32_f32(451_852, 135.0), 61_000_020.0);
    assert_eq!(mul_u32_f32(451_852, 135.0) * f32::from_bits(STEP_FACTOR_BITS), 61.00002);
    assert_eq!(mul_u32_f32(451_851, 135.0), 60_999_884.0);
    assert!(mul_u32_f32(451_851, 135.0) * f32::from_bits(STEP_FACTOR_BITS) < 61.0);
    // Idle passes every 16,667 µs: 27 passes give 60, 28 give 63.
    let (f, mut c) = shipped();
    for _ in 0..27 {
        f.advance(16_667);
        c.on_idle();
    }
    assert_eq!(c.time_of_day(), 43_200);
    f.advance(16_667);
    c.on_idle();
    assert_eq!(c.time_of_day(), 43_263);
    // The unsigned conversion from 2^31 up: the float(uint32) idiom.
    assert_eq!(mul_u32_f32(0x8000_0001, 1.0), 2_147_483_648.0);
    assert_eq!(mul_u32_f32(u32::MAX, 0.5), 2_147_483_648.0);
    // A NaN magnification steps nothing (ftol gives 0).
    let (f, mut c) = new();
    c.init();
    c.set_time_magnification(f32::NAN);
    assert!(c.time_magnification().is_nan());
    f.advance(10_000_000);
    c.on_idle();
    assert_eq!(c.time_of_day(), 43_200);
}

// ---- §12.4 the hour step -------------------------------------------------------------------------

#[test]
fn ck12_04_hour_step() {
    let (f, mut c) = shipped();
    c.set_times(46_799, 46_799);
    f.take_posts();
    step(&f, &mut c, E61);
    assert_eq!(c.time_of_day(), 46_860);
    assert_eq!((c.hour(), c.minute(), c.second()), (13, 1, 0));
    assert_eq!(
        f.take_posts(),
        vec![
            (HOUR_CHANGED, 13, 0),
            (TIME_TICK, 13, 0),
            (VISUAL_HOUR_CHANGED, 13, 0),
            (VISUAL_TIME_TICK, 13, 0)
        ]
    );
    assert_eq!(c.visual_time_of_day(), 46_860);
}

/// §4.9 "An evening": 19:59:30 + 63, then 00:59:30 + 63.
#[test]
fn ck12_04_evening_example() {
    let (f, mut c) = shipped();
    c.set_times(71_970, 71_970);
    f.take_posts();
    step(&f, &mut c, E63);
    assert_eq!(c.time_of_day(), 72_033);
    assert_eq!(
        f.take_posts(),
        vec![
            (HOUR_CHANGED, 20, 0),
            (TIME_TICK, 20, 0),
            (VISUAL_HOUR_CHANGED, 20, 0),
            (VISUAL_TIME_TICK, 20, 0),
            (DAY_NIGHT_CHANGED, 0, 0),
        ]
    );
    c.set_times(3570, 3570);
    step(&f, &mut c, E63);
    assert_eq!(
        f.take_posts(),
        vec![
            (HOUR_CHANGED, 1, 0),
            (TIME_TICK, 1, 0),
            (VISUAL_HOUR_CHANGED, 1, 0),
            (VISUAL_TIME_TICK, 1, 0),
            (DAY_NIGHT_CHANGED, 1, 0),
        ]
    );
    // The day wraps at midnight.
    c.set_times(86_399, 86_399);
    step(&f, &mut c, E61);
    assert_eq!(c.time_of_day(), 60);
}

// ---- §12.5 five-minute ticks ---------------------------------------------------------------------

#[test]
fn ck12_05_five_minute_tick() {
    let (f, mut c) = shipped();
    c.set_times(43_740, 43_740);
    f.take_posts();
    step(&f, &mut c, E62);
    assert_eq!(c.time_of_day(), 43_802);
    assert_eq!(f.take_posts(), vec![(TIME_TICK, 12, 10), (VISUAL_TIME_TICK, 12, 10)]);
    c.set_times(43_500, 43_500);
    step(&f, &mut c, E61);
    assert_eq!(c.time_of_day(), 43_561);
    assert!(f.take_posts().is_empty());
    // A step that crosses several boundaries posts one tick.
    step(&f, &mut c, 10_000_000);
    assert_eq!(c.time_of_day(), 44_761);
    assert_eq!(f.take_posts(), vec![(TIME_TICK, 12, 26), (VISUAL_TIME_TICK, 12, 26)]);
}

// ---- §12.6 the night boundary ------------------------------------------------------------------

#[test]
fn ck12_06_night_boundary() {
    let (f, mut c) = shipped();
    let night: Vec<u32> = (0..24).filter(|&h| c.is_night_hour(h)).collect();
    assert_eq!(night, vec![0, 20, 21, 22, 23]);
    c.set_times(19 * 3600, 19 * 3600 + 3599);
    f.take_posts();
    c.change_visual_time(20 * 3600);
    assert_eq!(
        f.take_posts(),
        vec![
            (VISUAL_HOUR_CHANGED, 20, 0),
            (VISUAL_TIME_TICK, 20, 0),
            (DAY_NIGHT_CHANGED, 0, 0)
        ]
    );
    assert!(c.is_night());
    c.change_visual_time(3600);
    assert_eq!(f.take_posts().last(), Some(&(DAY_NIGHT_CHANGED, 1, 0)));
    assert!(!c.is_night());
    // Notifications off: the hour messages only.
    c.enable_day_night_transition_notifications(false);
    c.change_visual_time(20 * 3600);
    assert_eq!(
        f.take_posts(),
        vec![(VISUAL_HOUR_CHANGED, 20, 0), (VISUAL_TIME_TICK, 20, 0)]
    );
    // The same visual time: nothing.
    c.change_visual_time(20 * 3600);
    assert!(f.take_posts().is_empty());

    c.set_night_hours(20, 4);
    let night: Vec<u32> = (0..24).filter(|&h| c.is_night_hour(h)).collect();
    assert_eq!(night, vec![0, 1, 2, 3, 20, 21, 22, 23]);
    c.set_night_hours(5, 5);
    assert!((0..24).all(|h| !c.is_night_hour(h)));
    // Hours of 24 or more only change the modular comparison.
    c.set_night_hours(26, 3);
    let night: Vec<u32> = (0..24).filter(|&h| c.is_night_hour(h)).collect();
    assert_eq!(night, vec![2]);
}

// ---- §12.7 modes -----------------------------------------------------------------------------------

#[test]
fn ck12_07_modes() {
    let (f, mut c) = shipped();
    c.set_times(75_600, 75_600);
    f.take_posts();
    c.set_visual_time_mode(MODE_NOON);
    assert_eq!(
        f.take_posts(),
        vec![
            (VISUAL_HOUR_CHANGED, 12, 0),
            (VISUAL_TIME_TICK, 12, 0),
            (DAY_NIGHT_CHANGED, 1, 0)
        ]
    );
    step(&f, &mut c, E63);
    assert_eq!(c.time_of_day(), 75_663);
    assert_eq!(c.visual_time_of_day(), 43_200);
    assert!(f.take_posts().is_empty()); // 21:01:03: no game-time message either
    c.set_visual_time_mode(MODE_MIDNIGHT);
    assert_eq!(c.visual_time_of_day(), 0);
    assert_eq!(
        f.take_posts(),
        vec![
            (VISUAL_HOUR_CHANGED, 0, 0),
            (VISUAL_TIME_TICK, 0, 0),
            (DAY_NIGHT_CHANGED, 0, 0)
        ]
    );
    c.set_visual_time_mode(3);
    assert_eq!((c.visual_time_mode(), c.visual_time_of_day()), (3, 0));
    assert!(f.take_posts().is_empty());
    step(&f, &mut c, E63);
    assert_eq!(c.visual_time_of_day(), 0);
    f.take_posts();
    c.set_visual_time_mode(MODE_FOLLOW);
    assert_eq!(c.visual_time_of_day(), c.time_of_day());
    assert_eq!(
        f.take_posts(),
        vec![(VISUAL_HOUR_CHANGED, 21, 0), (VISUAL_TIME_TICK, 21, 0)]
    );
    c.set_visual_time_mode(MODE_FOLLOW);
    assert!(f.take_posts().is_empty());
}

// ---- §12.8 pause counters -----------------------------------------------------------------------

#[test]
fn ck12_08_pause_reads_clock_only_for_timer_transitions() {
    let (f, mut c) = shipped();
    f.clock_reads.set(0);
    c.pause(false);
    c.hidden_pause(false);
    assert_eq!(f.clock_reads.get(), 0);

    c.pause(true);
    assert_eq!(f.clock_reads.get(), 1);
    c.pause(true);
    c.hidden_pause(true);
    c.pause(false);
    c.pause(false);
    assert_eq!(f.clock_reads.get(), 1);

    c.hidden_pause(false);
    assert_eq!(f.clock_reads.get(), 2);
    c.hidden_pause(false);
    assert_eq!(f.clock_reads.get(), 2);
}

#[test]
fn ck12_08_stopped_timer_does_not_read_clock() {
    let (f, mut c) = new();
    c.on_idle();
    assert_eq!(c.timer_elapsed_us(), 0);
    c.pause(true);
    c.pause(true);
    c.pause(false);
    assert_eq!(f.clock_reads.get(), 0);
    c.pause(false);
    assert!(c.timer().running);
    assert_eq!(f.clock_reads.get(), 1);
}

#[test]
fn ck12_08_pause_counters() {
    for hidden in [false, true] {
        let (f, mut c) = shipped();
        let message = if hidden { HIDDEN_PAUSE_CHANGED } else { PAUSE_CHANGED };
        let set = |c: &mut Clock24, on| {
            if hidden {
                assert!(c.hidden_pause(on));
            } else {
                assert!(c.pause(on));
            }
        };
        let paused = |c: &Clock24| if hidden { c.is_hidden_paused() } else { c.is_paused() };
        f.take_posts();
        f.advance(300_000);
        set(&mut c, true);
        set(&mut c, true);
        assert_eq!(f.take_posts(), vec![(message, 1, 0)]);
        assert!(paused(&c));
        assert!(!c.timer().running);
        f.advance(5_000_000);
        assert_eq!(c.timer_elapsed_us(), 300_000);
        set(&mut c, false);
        assert!(f.take_posts().is_empty());
        set(&mut c, false);
        assert_eq!(f.take_posts(), vec![(message, 0, 0)]);
        assert!(!paused(&c));
        assert!(c.timer().running);
        // The elapsed time survived the pause.
        f.advance(200_000);
        assert_eq!(c.timer_elapsed_us(), 500_000);
        set(&mut c, false);
        assert!(f.take_posts().is_empty());
    }
    // The timer runs only when both counters are 0.
    let (f, mut c) = shipped();
    c.pause(true);
    c.hidden_pause(true);
    c.pause(false);
    assert!(!c.timer().running);
    c.hidden_pause(false);
    assert!(c.timer().running);
    assert_eq!(
        f.take_posts(),
        vec![
            (PAUSE_CHANGED, 1, 0),
            (HIDDEN_PAUSE_CHANGED, 1, 0),
            (PAUSE_CHANGED, 0, 0),
            (HIDDEN_PAUSE_CHANGED, 0, 0)
        ]
    );
}

// ---- §12.9 setters ---------------------------------------------------------------------------------

#[test]
fn ck12_09_setters() {
    let (f, mut c) = shipped();
    c.set_times(45_000, 45_000); // 12:30
    f.take_posts();
    c.set_hour(25);
    assert_eq!(c.time_of_day(), 3600);
    assert_eq!(f.take_posts(), vec![(HOUR_CHANGED, 1, 0), (TIME_TICK, 1, 0)]);
    c.set_times(45_000, 45_000);
    c.set_hour(12);
    assert_eq!(c.time_of_day(), 45_000);
    assert!(f.take_posts().is_empty());

    c.set_times(48_030, 45_000); // 13:20:30
    assert_eq!(c.set_minute(75), 47_700);
    assert_eq!(c.time_of_day(), 47_700);
    assert_eq!(c.set_seconds(61), 15);
    assert_eq!(c.time_of_day(), 47_701);

    c.set_time_percentage(-0.5);
    assert_eq!(c.time_of_day(), 66_496);
    c.set_time_percentage(0.999_999);
    assert_eq!(c.time_of_day(), 86_399);
    c.set_time_percentage(1.5);
    assert_eq!(c.time_of_day(), 43_200);

    let before = c.time_magnification();
    assert!(c.set_time_magnification_min_max(10.0, 5.0));
    assert_eq!(c.time_magnification_min_max(), (10.0, 10.0));
    assert_eq!(c.time_magnification(), before);
    assert!(c.set_time_magnification(3.0));
    assert_eq!(c.time_magnification(), 10.0);

    assert_eq!(c.visual_time_of_day(), 45_000);
    assert!(f.take_posts().is_empty());
}

// ---- §12.10 notifications ------------------------------------------------------------------------

#[test]
fn ck12_10_notifications() {
    let (f, mut c) = shipped();
    c.set_times(22 * 3600, 22 * 3600);
    f.take_posts();
    c.enable_day_night_transition_notifications(false);
    assert!(f.take_posts().is_empty());
    c.enable_day_night_transition_notifications(true);
    assert_eq!(f.take_posts(), vec![(DAY_NIGHT_CHANGED, 0, 0)]);
    c.enable_day_night_transition_notifications(true);
    assert!(f.take_posts().is_empty());
    // At noon, switching on reports day.
    c.set_times(NOON, NOON);
    c.enable_day_night_transition_notifications(false);
    c.enable_day_night_transition_notifications(true);
    assert_eq!(f.take_posts(), vec![(DAY_NIGHT_CHANGED, 1, 0)]);
}

// ---- §12.11 cheats ---------------------------------------------------------------------------------

#[test]
fn ck12_11_cheats() {
    let (f, mut c) = shipped();
    f.take_posts();
    assert!(c.do_message(msg::CHEAT, CHEAT_WHAT_TIME_IZ_IT, Some(b"WhatTimeIzIt 7".as_slice())));
    assert_eq!(c.time_of_day(), 7 * 3600);
    assert_eq!(f.take_posts(), vec![(HOUR_CHANGED, 7, 0), (TIME_TICK, 7, 0)]);
    assert!(c.do_message(msg::CHEAT, CHEAT_WHAT_TIME_IZ_IT, Some(b"WhatTimeIzIt".as_slice())));
    assert_eq!(c.time_of_day(), 7 * 3600);
    // §4.9: WhatTimeIzIt 25 at 12:30 gives hour 1; "abc" gives hour 0.
    c.set_times(45_000, 45_000);
    c.do_message(msg::CHEAT, CHEAT_WHAT_TIME_IZ_IT, Some(b"WhatTimeIzIt 25".as_slice()));
    assert_eq!(c.time_of_day(), 3600);
    c.do_message(msg::CHEAT, CHEAT_WHAT_TIME_IZ_IT, Some(b"WhatTimeIzIt abc".as_slice()));
    assert_eq!(c.time_of_day(), 0);
    f.take_posts();

    c.do_message(msg::CHEAT, CHEAT_STOPWATCH, Some(b"Stopwatch PAUSE".as_slice()));
    assert!(c.is_paused());
    assert_eq!(f.take_posts(), vec![(PAUSE_CHANGED, 1, 0)]);
    c.do_message(msg::CHEAT, CHEAT_STOPWATCH, Some(b"Stopwatch resume".as_slice()));
    assert!(!c.is_paused());
    c.do_message(msg::CHEAT, CHEAT_STOPWATCH, Some(b"Stopwatch magnify 5000".as_slice()));
    assert_eq!(c.time_magnification(), 2000.0);
    c.do_message(msg::CHEAT, CHEAT_STOPWATCH, Some(b"Stopwatch magnify".as_slice()));
    assert_eq!(c.time_magnification(), 2000.0);
    c.do_message(msg::CHEAT, CHEAT_STOPWATCH, Some(b"Stopwatch magnify 0.5".as_slice()));
    assert_eq!(c.time_magnification(), f32::from_bits(0x3D4C_CCCD));
    c.do_message(msg::CHEAT, CHEAT_STOPWATCH, Some(b"Stopwatch magnify 300".as_slice()));
    assert_eq!(c.time_magnification(), 300.0);
    // Anything else, a missing argument, another cheat or message: nothing.
    f.take_posts();
    let before = c.state();
    c.do_message(msg::CHEAT, CHEAT_STOPWATCH, Some(b"Stopwatch stop".as_slice()));
    c.do_message(msg::CHEAT, CHEAT_STOPWATCH, Some(b"Stopwatch".as_slice()));
    c.do_message(msg::CHEAT, CHEAT_STOPWATCH, None);
    c.do_message(msg::CHEAT, 0x1234, Some(b"x pause".as_slice()));
    assert!(c.do_message(0x1234, CHEAT_STOPWATCH, Some(b"Stopwatch pause".as_slice())));
    assert_eq!(c.state(), before);
    assert!(f.take_posts().is_empty());
}

#[test]
fn cheat_text_parsing() {
    let s = |t: &[u8]| -> Vec<String> {
        split_command_line(t)
            .into_iter()
            .map(|a| String::from_utf8(a).unwrap())
            .collect()
    };
    assert_eq!(s(b"  Stopwatch   magnify\t5 "), vec!["Stopwatch", "magnify", "5"]);
    assert_eq!(s(b"a \"b c\" \"\""), vec!["a", "b c", ""]);
    assert!(s(b"").is_empty());
    assert!(s(b" \t\n ").is_empty());
    // CK11: only space, tab and newline separate; CR is ordinary.
    assert_eq!(s(b"a\rb c"), vec!["a\rb", "c"]);
    // A separator inside an odd number of quotes is kept; only an argument
    // that starts and ends with a quote loses that outer pair.
    assert_eq!(s(b"a\"b c\"d e"), vec!["a\"b c\"d", "e"]);
    assert_eq!(s(b"\"x y\""), vec!["x y"]);
    assert_eq!(s(b"x \""), vec!["x", ""]);
    assert_eq!(s(b"x \"abc def"), vec!["x", "\"abc def"]);
    assert_eq!(s(b"Stopwatch pa\"use"), vec!["Stopwatch", "pa\"use"]);
    assert_eq!(s(b"Stopwatch \"pause\""), vec!["Stopwatch", "pause"]);
    assert_eq!(atol(b"12abc"), 12);
    assert_eq!(atol(b" -7"), -7);
    assert_eq!(atol(b"+3"), 3);
    assert_eq!(atol(b"abc"), 0);
    assert_eq!(atol(b"0.5"), 0);
    assert_eq!(atol(b"4294967297"), 1); // CK10: wraps
    assert_eq!(atol(b"4294967296"), 0);
    assert_eq!(atol(b"3000000000"), -1_294_967_296);
    assert_eq!(atol(b"-1"), -1);
}

// ---- §12.12 the record -------------------------------------------------------------------------

const EXAMPLE: [u8; 26] = [
    0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07, 0x43, 0x00, 0x00, 0xFA, 0x44, 0xC0, 0xA8, 0x00, 0x00, 0x14,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
];

fn record(c: &Clock24) -> Vec<u8> {
    let mut out = ByteWriter::new();
    assert!(c.write(&mut out));
    out.into_bytes()
}

fn v1(pause: i32, mag: f32, max: f32, seconds: u32, start: u32, end: u32, version: u16) -> Vec<u8> {
    let mut w = ByteWriter::new();
    w.put_u16(version);
    w.put_i32(pause);
    w.put_f32(mag);
    w.put_f32(max);
    w.put_u32(seconds);
    if version < 2 {
        w.put_u8(2);
        w.put_u64(0);
        w.put_u8(0);
    }
    w.put_u32(start);
    w.put_u32(end);
    w.into_bytes()
}

#[test]
fn ck12_12_save() {
    let (_f, c) = shipped();
    assert_eq!(record(&c), EXAMPLE.to_vec());
    assert_eq!(RECORD_LEN, EXAMPLE.len());

    let bytes = v1(0, 135.0, 2000.0, 50_000, 20, 1, 1);
    assert_eq!(bytes.len(), 36);
    let (_f, mut c) = new();
    let mut r = ByteReader::new(&bytes);
    assert!(c.read(&mut r));
    assert_eq!(r.remaining(), 0);
    let s = c.state();
    assert_eq!(
        (
            s.magnification,
            s.max_magnification,
            s.seconds,
            s.night_start,
            s.night_end
        ),
        (135.0, 2000.0, 50_000, 20, 1)
    );
    assert_eq!(s.timer, Timer::default());
    assert_eq!(s.visual_seconds, NOON);
    let again = record(&c);
    assert_eq!(again.len(), 26);
    assert_eq!(again, v1(0, 135.0, 2000.0, 50_000, 20, 1, 2));

    // Version 3 reads as version 2.
    let bytes = v1(0, 135.0, 2000.0, 60_000, 20, 1, 3);
    let (_f, mut c) = new();
    assert!(c.read(&mut ByteReader::new(&bytes)));
    assert_eq!(c.time_of_day(), 60_000);
    assert_eq!(c.event_hour(1), 1);

    // A truncated record fails; the fields read before stay.
    let mut bytes = v1(0, 135.0, 2000.0, 60_000, 20, 1, 2);
    bytes.truncate(20);
    let (_f, mut c) = new();
    assert!(!c.read(&mut ByteReader::new(&bytes)));
    let s = c.state();
    assert_eq!((s.seconds, s.night_start, s.night_end), (60_000, 20, 4));
}

/// Records shaped like the corpus (§7): version 1 with the `02 | 0×8 | 00`
/// timer record and version 2, the common values, `seconds` at the corpus
/// hours. Each reads, and rewriting gives version 2 with the same values.
#[test]
fn ck12_12_corpus_shaped_records() {
    let hours = [5, 10, 12, 13, 14, 15, 16, 17, 20];
    for (i, h) in hours.iter().enumerate() {
        for version in [1u16, 2] {
            let seconds = h * 3600 + (i as u32 * 397) % 3600;
            let bytes = v1(0, 135.0, 2000.0, seconds, 20, 1, version);
            let (_f, mut c) = new();
            assert!(c.read(&mut ByteReader::new(&bytes)));
            let out = record(&c);
            assert_eq!(out, v1(0, 135.0, 2000.0, seconds, 20, 1, 2));
        }
    }
}

// ---- §12.13 load order -----------------------------------------------------------------------------

#[test]
fn ck12_13_load_order() {
    let (f, mut c) = new();
    f.tuning.set(Some(SHIPPED_TUNING));
    let bytes = v1(0, 50.0, 1500.0, 75_600, 22, 5, 2);
    assert!(c.read(&mut ByteReader::new(&bytes)));
    assert!(c.init());
    let s = c.state();
    assert_eq!((s.magnification, s.max_magnification), (135.0, 1500.0));
    assert_eq!((s.night_start, s.night_end), (20, 1));
    assert_eq!((s.seconds, s.visual_seconds), (75_600, 43_200));
    assert!(s.timer.running);
    // §4.9 "Loading a city saved at night": mode 0 again changes nothing;
    // the first step gives 21:01:01 and moves the visual time.
    c.set_visual_time_mode(MODE_FOLLOW);
    assert!(f.take_posts().is_empty());
    step(&f, &mut c, E61);
    assert_eq!(c.time_of_day(), 75_661);
    assert_eq!(
        f.take_posts(),
        vec![
            (VISUAL_HOUR_CHANGED, 21, 0),
            (VISUAL_TIME_TICK, 21, 0),
            (DAY_NIGHT_CHANGED, 0, 0)
        ]
    );
    // A saved pause count leaves the timer stopped until Stopwatch resume.
    let (f, mut c) = new();
    f.tuning.set(Some(SHIPPED_TUNING));
    assert!(c.read(&mut ByteReader::new(&v1(1, 135.0, 2000.0, 43_200, 20, 1, 2))));
    c.init();
    assert!(!c.timer().running && c.is_paused());
    f.advance(10_000_000);
    c.on_idle();
    assert_eq!(c.time_of_day(), 43_200);
    c.cheat(CHEAT_STOPWATCH, b"Stopwatch resume");
    assert!(c.timer().running);
}

// ---- re-answers10: CK14, CK15 (tests 16, 17) -------------------------------------------------------

#[test]
fn ck12_17_timer_restart_and_stop() {
    // CK15: Restart on a stopped timer starts it with elapsed 0; Stop on a
    // stopped timer changes nothing.
    let mut t = Timer::default();
    t.stop(500);
    assert_eq!(t, Timer::default());
    t.restart(1_000);
    assert!(t.running);
    assert_eq!(t.elapsed(1_000), 0);
    assert_eq!(t.elapsed(1_250), 250);
    t.stop(2_000);
    t.stop(9_000);
    assert!(!t.running);
    assert_eq!(t.elapsed(9_999), 1_000);
}

#[test]
fn ck12_16_negative_pause_count_freezes_the_clock() {
    let (f, mut c) = new();
    f.tuning.set(Some(SHIPPED_TUNING));
    let mut w = ByteWriter::new();
    w.put_u16(2);
    w.put_i32(-1);
    w.put_f32(135.0);
    w.put_f32(2000.0);
    w.put_u32(43_200);
    w.put_u32(20);
    w.put_u32(1);
    assert!(c.read(&mut ByteReader::new(&w.into_bytes())));
    assert!(!c.is_paused());
    assert!(c.init());
    assert!(!c.timer().running);
    f.advance(10_000_000);
    c.on_idle();
    assert_eq!(c.time_of_day(), 43_200);
    c.pause(false);
    assert_eq!(c.state().pause_count, -1);
    c.hidden_pause(true);
    c.hidden_pause(false);
    f.advance(10_000_000);
    c.on_idle();
    assert_eq!(c.time_of_day(), 43_200);
    f.take_posts();
    c.pause(true); // -1 -> 0: no message
    assert!(f.take_posts().is_empty());
    c.hidden_pause(true);
    c.hidden_pause(false);
    assert!(c.timer().running);
    step(&f, &mut c, 1_000_000);
    assert_eq!(c.time_of_day(), 43_335);
}
