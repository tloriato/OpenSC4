// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! history.md §12 (one test per item, `hw12_NN`) plus the worked examples
//! of §4.8, the precision of §4.7 and the record of §7.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::*;
use crate::julian::julian_from_civil;
use crate::stream::{ByteReader, ByteWriter, OutStream};

#[derive(Default)]
struct Fake {
    date: Cell<u32>,
    now: Cell<u32>,
    /// Added to the clock after every reading.
    step: Cell<u32>,
    log: RefCell<Vec<String>>,
    sent: RefCell<Vec<(u32, u32, u32)>>,
}

struct H(Rc<Fake>);

impl HistoryHost for H {
    fn sim_date(&self) -> u32 {
        self.0.date.get()
    }
    fn now_ms(&self) -> u32 {
        let t = self.0.now.get();
        self.0.now.set(t.wrapping_add(self.0.step.get()));
        t
    }
    fn subscribe(&self, msg_type: u32) {
        self.0.log.borrow_mut().push(format!("subscribe {msg_type:08X}"));
    }
    fn unsubscribe(&self, msg_type: u32) {
        self.0.log.borrow_mut().push(format!("unsubscribe {msg_type:08X}"));
    }
    fn add_agent(&self, level: u32, name: &str, flags: u32) {
        self.0
            .log
            .borrow_mut()
            .push(format!("add_agent {level} {name} {flags}"));
    }
    fn remove_agent(&self) {
        self.0.log.borrow_mut().push("remove_agent".into());
    }
    fn send(&self, msg_type: u32, data1: u32, data2: u32) {
        self.0.sent.borrow_mut().push((msg_type, data1, data2));
    }
}

fn new() -> (Rc<Fake>, HistoryWarehouse) {
    let f = Rc::new(Fake::default());
    f.date.set(julian::JULIAN_2000_01_01 as u32);
    (f.clone(), HistoryWarehouse::new(Box::new(H(f))))
}

use crate::julian;

fn d(y: i32, m: i32, dd: i32) -> u32 {
    julian_from_civil(y, m, dd) as u32
}

fn pts(v: &[(u32, i32)]) -> Vec<Point> {
    v.iter().map(|&(date, value)| Point::new(date, value)).collect()
}

fn series(v: &[(u32, i32)], max_count: u32) -> Series {
    Series {
        points: pts(v),
        max_count,
    }
}

fn points_of(w: &HistoryWarehouse, key: u32) -> Vec<(u32, i32)> {
    w.series(key)
        .unwrap()
        .points
        .iter()
        .map(|p| (p.date, p.value))
        .collect()
}

const K: u32 = 0xAA11_F837;
const P24: Precision = Precision::Bits24;
const P53: Precision = Precision::Bits53;

// ---- §12.1 Init --------------------------------------------------------------------------------

#[test]
fn hw12_01_init_registers_and_query_interface() {
    let (f, mut w) = new();
    assert!(w.init());
    assert!(w.is_initialized());
    assert_eq!(w.pass_state(), PASS_IDLE);
    assert_eq!(
        *f.log.borrow(),
        vec![
            "subscribe 89EFA536".to_string(),
            "subscribe 89EFA537".into(),
            "add_agent 4 cSC4HistoryWarehouse 3".into(),
            "add_agent 7 cSC4HistoryWarehouse 3".into(),
        ]
    );
    assert!(answers_iid(1) && answers_iid(0x69F4_A803) && answers_iid(0xE4FD_A3D4));
    assert!(!answers_iid(IID_GZ_MESSAGE_TARGET2));
    assert!(!answers_iid(0) && !answers_iid(0x89EF_A536));
    // No "already initialised" test: a second Init registers again.
    f.log.borrow_mut().clear();
    w.init();
    assert_eq!(f.log.borrow().len(), 4);
    // Shutdown keeps the series.
    w.store(K, 5);
    f.log.borrow_mut().clear();
    assert!(w.shutdown());
    assert!(!w.is_initialized());
    assert_eq!(
        *f.log.borrow(),
        vec![
            "unsubscribe 89EFA536".to_string(),
            "unsubscribe 89EFA537".into(),
            "remove_agent".into()
        ]
    );
    assert_eq!(w.series(K).unwrap().points.len(), 1);
}

// ---- §12.2 Store ---------------------------------------------------------------------------------

#[test]
fn hw12_02_store_recording_example() {
    let (f, mut w) = new();
    w.set_size(K, 2);
    for (day, v) in [(100, 5), (101, 6), (101, 7)] {
        f.date.set(day);
        w.store(K, v);
    }
    assert_eq!(points_of(&w, K), vec![(100, 5), (101, 7)]);
    f.date.set(102);
    w.store(K, 8);
    assert_eq!(points_of(&w, K), vec![(101, 7), (102, 8)]);
    // An earlier day also overwrites the last point (unsigned compare).
    f.date.set(50);
    w.store(K, 9);
    assert_eq!(points_of(&w, K), vec![(101, 7), (102, 9)]);
    // A new key gets maxCount 0.
    w.store(0x1234, 1);
    assert_eq!(w.series(0x1234).unwrap().max_count, 0);
    // The record message stores; RecordHistoryData only sends.
    f.date.set(200);
    assert!(w.do_message(msg::RECORD, 0x77, (-3i32) as u32));
    assert_eq!(points_of(&w, 0x77), vec![(200, -3)]);
    // HW12: it returns false (the original's final Release leaves EAX = 0).
    assert!(!HistoryWarehouse::record_history_data(&H(f.clone()), 0x78, -4));
    assert_eq!(*f.sent.borrow(), vec![(msg::RECORD, 0x78, (-4i32) as u32)]);
    assert!(w.series(0x78).is_none());
}

#[test]
fn hw12_02_full_queue_drops_one_point_per_record() {
    let (f, mut w) = new();
    w.set_series(K, series(&[(1, 1), (2, 2), (3, 3), (4, 4)], 0));
    // maxCount set without trimming (n = -1 is not > 0), then 2: trims to 2.
    w.set_series(K, series(&[(1, 1), (2, 2), (3, 3), (4, 4)], 2));
    f.date.set(5);
    w.store(K, 5);
    assert_eq!(points_of(&w, K), vec![(2, 2), (3, 3), (4, 4), (5, 5)]);
}

// ---- §12.3 SetSize -----------------------------------------------------------------------------

#[test]
fn hw12_03_set_size() {
    let (_f, mut w) = new();
    let ten: Vec<(u32, i32)> = (1..=10).map(|i| (i, i as i32)).collect();
    w.set_series(K, series(&ten, 0));
    assert!(w.set_size(K, 3));
    assert_eq!(points_of(&w, K), vec![(8, 8), (9, 9), (10, 10)]);
    assert_eq!(w.series(K).unwrap().max_count, 3);

    w.set_series(K, series(&ten, 0));
    assert!(w.set_size(K, 0));
    assert_eq!(points_of(&w, K).len(), 10);

    assert!(w.set_size(K, (-1i32) as u32));
    assert_eq!(points_of(&w, K).len(), 10);
    assert_eq!(w.series(K).unwrap().max_count, 0xFFFF_FFFF);

    // The message does the same and creates a series.
    assert!(w.do_message(msg::SET_QUEUE_LENGTH, 0x55, 365));
    assert_eq!(w.series(0x55).unwrap().max_count, 365);
    assert!(w.set_history_data_queue_length(0x56, 1));
}

// ---- §12.4 Coalesce ----------------------------------------------------------------------------

#[test]
fn hw12_04_coalesce_month_example() {
    let (_f, mut w) = new();
    w.set_series(
        K,
        series(
            &[
                (d(2024, 3, 5), 10),
                (d(2024, 3, 20), 12),
                (d(2024, 4, 2), 15),
                (d(2024, 4, 30), 11),
                (d(2024, 5, 3), 9),
            ],
            0,
        ),
    );
    assert!(w.coalesce(K, 0, u32::MAX, granularity::MONTH));
    assert_eq!(
        points_of(&w, K),
        vec![(d(2024, 3, 1), 10), (d(2024, 4, 1), 15), (d(2024, 5, 3), 9)]
    );
}

#[test]
fn hw12_04_coalesce_half_year_five_years_and_month_number() {
    let (_f, mut w) = new();
    w.set_series(
        K,
        series(
            &[
                (d(2010, 3, 10), 1),
                (d(2010, 5, 10), 2),
                (d(2010, 8, 10), 3),
                (d(2010, 11, 10), 4),
            ],
            0,
        ),
    );
    assert!(w.coalesce(K, 0, u32::MAX, granularity::HALF_YEAR));
    assert_eq!(
        points_of(&w, K),
        vec![(d(2010, 1, 1), 1), (d(2010, 8, 10), 3), (d(2010, 11, 10), 4)]
    );

    let yearly: Vec<(u32, i32)> = (2011..=2019).map(|y| (d(y, 6, 15), y)).collect();
    w.set_series(K, series(&yearly, 0));
    assert!(w.coalesce(K, 0, u32::MAX, granularity::FIVE_YEARS));
    let mut want = vec![(d(2011, 1, 1), 2011)];
    want.extend((2015..=2019).map(|y| (d(y, 6, 15), y)));
    assert_eq!(points_of(&w, K), want);

    // Month number only [quirk]: twelve months apart, nothing between: one group.
    w.set_series(
        K,
        series(&[(d(2010, 3, 1), 1), (d(2011, 3, 1), 2), (d(2011, 5, 1), 3)], 0),
    );
    assert!(w.coalesce(K, 0, u32::MAX, granularity::MONTH));
    assert_eq!(points_of(&w, K), vec![(d(2010, 3, 1), 1), (d(2011, 5, 1), 3)]);

    // Years: closed groups move to January 1.
    w.set_series(
        K,
        series(&[(d(2001, 2, 1), 1), (d(2001, 9, 1), 2), (d(2002, 4, 1), 3)], 0),
    );
    assert!(w.coalesce(K, 0, u32::MAX, granularity::YEAR));
    assert_eq!(points_of(&w, K), vec![(d(2001, 1, 1), 1), (d(2002, 4, 1), 3)]);
}

#[test]
fn hw12_04_coalesce_range_and_failures() {
    let (_f, mut w) = new();
    assert!(!w.coalesce(K, 0, u32::MAX, 0)); // missing key
    let v = [(d(2024, 3, 5), 10), (d(2024, 3, 20), 12), (d(2024, 4, 2), 15)];
    w.set_series(K, series(&v, 0));
    assert!(!w.coalesce(K, d(2025, 1, 1), d(2025, 12, 31), 0)); // empty range
    assert!(!w.coalesce(K, 5, 4, 0)); // from > to: empty

    // Only the range is walked: the April point is outside it, so the March
    // group is the last group of the range and stays.
    assert!(w.coalesce(K, 0, d(2024, 3, 31), 0));
    assert_eq!(points_of(&w, K), v.to_vec());
    // Another granularity: one group, never closed.
    assert!(w.coalesce(K, 0, u32::MAX, 7));
    assert_eq!(points_of(&w, K), v.to_vec());
}

// ---- §12.5 the thinning pass ------------------------------------------------------------------

/// Points that a month coalesce changes when the date is in January 2026.
fn thinnable() -> Series {
    series(&[(d(2025, 10, 5), 1), (d(2025, 10, 20), 2), (d(2025, 11, 3), 3)], 0)
}

fn thinned() -> Vec<(u32, i32)> {
    vec![(d(2025, 10, 1), 1), (d(2025, 11, 3), 3)]
}

#[test]
fn hw12_05_thinning_pass() {
    let (f, mut w) = new();
    f.date.set(d(2026, 1, 1));
    for k in [1, 3, 5] {
        w.set_series(k, thinnable());
    }
    for k in [2, 4] {
        let mut s = thinnable();
        s.max_count = 2;
        w.set_series(k, s);
    }
    // No pass before SimNewYear.
    assert!(w.do_message(msg::SIM_NEW_DAY, 0, 0));
    assert_eq!(points_of(&w, 1).len(), 3);
    assert!(w.do_message(msg::SIM_NEW_YEAR, 0, 0));
    assert_eq!(w.pass_state(), PASS_ARMED);

    let thin = |w: &HistoryWarehouse| -> Vec<bool> { (1..=5).map(|k| points_of(w, k) == thinned()).collect() };
    assert!(w.do_message(msg::SIM_NEW_DAY, 0, 0));
    assert_eq!(thin(&w), [true, false, false, false, false]);
    assert_eq!(
        (w.pass_state(), w.cursor(), w.pass_counter()),
        (PASS_RUNNING, Some(2), 0)
    );
    assert!(w.do_message(msg::SIM_NEW_DAY, 0, 0));
    assert_eq!(thin(&w), [true, false, true, false, false]);
    assert!(w.do_message(msg::SIM_NEW_DAY, 0, 0));
    assert_eq!(thin(&w), [true, false, true, false, true]);
    assert_eq!(w.pass_state(), PASS_RUNNING);
    // The fourth day reaches the end: state 0, false once, then true.
    assert!(!w.do_message(msg::SIM_NEW_DAY, 0, 0));
    assert_eq!(w.pass_state(), PASS_IDLE);
    assert!(w.do_message(msg::SIM_NEW_DAY, 0, 0));
    // The limited series were skipped.
    assert_eq!(points_of(&w, 2).len(), 3);
    // SimNewYear during a pass does not restart it.
    w.set_pass(PASS_RUNNING, Some(3), 0);
    assert!(w.do_message(msg::SIM_NEW_YEAR, 0, 0));
    assert_eq!(w.pass_state(), PASS_RUNNING);
}

#[test]
fn hw12_05_per_pass_and_time_limit() {
    let (f, mut w) = new();
    f.date.set(d(2026, 1, 1));
    for k in 0..400 {
        w.set_series(k, thinnable());
    }
    w.do_message(msg::SIM_NEW_YEAR, 0, 0);
    // 400 series: perPass = 400 / 365 + 1 = 2.
    assert!(w.thinning_step());
    let count = |w: &HistoryWarehouse| (0..400).filter(|&k| points_of(w, k) == thinned()).count();
    assert_eq!(count(&w), 2);
    // A timer that shows 5 ms after one series stops the call early: false.
    f.step.set(5);
    assert!(!w.thinning_step());
    assert_eq!((count(&w), w.pass_counter()), (3, 1));
    assert!(w.thinning_step());
    assert_eq!((count(&w), w.pass_counter()), (4, 0));
    // No series: the pass ends at once.
    let (f2, mut empty) = new();
    f2.date.set(d(2026, 1, 1));
    empty.do_message(msg::SIM_NEW_YEAR, 0, 0);
    assert!(empty.thinning_step());
    assert_eq!(empty.pass_state(), PASS_IDLE);
    // Any other state returns true and does nothing.
    w.set_pass(7, Some(0), 0);
    assert!(w.thinning_step());
    assert_eq!(w.pass_state(), 7);
}

#[test]
fn deterministic_thinning_ignores_the_clock() {
    // simulator.md §5.7 [choice]: under deterministic pacing every call thins
    // its perPass series, whatever the clock shows.
    let (f, mut w) = new();
    w.set_pacing(crate::pacing::Pacing::Deterministic);
    assert!(w.pacing().is_deterministic());
    f.date.set(d(2026, 1, 1));
    for k in 0..400 {
        w.set_series(k, thinnable());
    }
    w.do_message(msg::SIM_NEW_YEAR, 0, 0);
    f.step.set(50);
    let count = |w: &HistoryWarehouse| (0..400).filter(|&k| points_of(w, k) == thinned()).count();
    assert!(w.thinning_step());
    assert_eq!(count(&w), 2);
    assert!(w.thinning_step());
    assert_eq!(count(&w), 4);
    // The clock was never read.
    assert_eq!(f.now.get(), 0);
}

#[test]
fn hw12_05_variable_coalesce_dates() {
    // January: D1 is December 1 of the previous year.
    let (f, mut w) = new();
    f.date.set(d(2026, 1, 15));
    let mut v: Vec<(u32, i32)> = vec![
        (d(1400, 1, 1), 1),
        (d(1500, 6, 1), 2),
        (d(1526, 1, 1), 3), // D5: the last disposed point survives
        (d(1600, 3, 1), 4),
        (d(1604, 3, 1), 5),
        (d(1700, 3, 1), 6),
        (d(1926, 1, 1), 7), // D4
        (d(1930, 2, 1), 8),
        (d(1930, 8, 1), 9),
        (d(1976, 1, 1), 10), // D3
        (d(1980, 2, 1), 11),
        (d(1980, 3, 1), 12),
        (d(1980, 8, 1), 13),
        (d(2016, 1, 1), 14), // D2
        (d(2020, 5, 3), 15),
        (d(2020, 5, 9), 16),
        (d(2025, 12, 1), 17), // D1
        (d(2025, 12, 2), 18),
    ];
    w.set_series(K, series(&v, 0));
    w.variable_coalesce(K);
    v = vec![
        (d(1526, 1, 1), 3),
        (d(1600, 1, 1), 4), // five-year group 320 (1600, 1604) closed at 1700
        (d(1700, 1, 1), 6), // group 340, closed by the D4 point (in the range)
        (d(1926, 1, 1), 7),
        (d(1930, 1, 1), 8), // year group 1930 closed at 1976
        (d(1976, 1, 1), 10),
        (d(1980, 1, 1), 11), // half-year group closed at 1980-08-01
        (d(1980, 7, 1), 13), // closed by the D2 point
        (d(2016, 1, 1), 14),
        (d(2020, 5, 1), 15), // month group closed at 2025-12-01
        (d(2025, 12, 1), 17),
        (d(2025, 12, 2), 18),
    ];
    // Ranges are inclusive and share their bounds: the bound point is the
    // last group of one coalesce (kept as it is) and closes the group before
    // it, then opens the first group of the next coalesce.
    assert_eq!(points_of(&w, K), v);
}

/// The Konradshohe shape (§4.8): a series recorded every day from 2000-01-01
/// through 2026-01-27, thinned by every yearly pass (the only series, so on
/// January 1st): half-years up to the last pass's D2, months up to its D1,
/// daily after.
#[test]
fn hw12_05_yearly_passes_over_26_years() {
    let (f, mut w) = new();
    let start = d(2000, 1, 1);
    let end = d(2026, 1, 27);
    let mut prev_year = 2000;
    for day in start..=end {
        f.date.set(day);
        let c = crate::julian::civil_from_julian(day as i32);
        if c.year != prev_year {
            prev_year = c.year;
            w.do_message(msg::SIM_NEW_YEAR, 0, 0);
        }
        // The day's agents: the pass step (again while it asks), then a record.
        while !w.do_message(msg::SIM_NEW_DAY, 0, 0) {}
        w.store(K, (day - start) as i32);
    }
    let got: Vec<u32> = points_of(&w, K).iter().map(|p| p.0).collect();
    let mut want = Vec::new();
    for y in 2000..=2015 {
        want.push(d(y, 1, 1));
        want.push(d(y, 7, 1));
    }
    want.push(d(2016, 1, 1));
    for y in 2016..=2025 {
        for m in 1..=12 {
            if (y, m) != (2016, 1) {
                want.push(d(y, m, 1));
            }
        }
    }
    want.extend(d(2025, 12, 2)..=end);
    assert_eq!(got, want);
    // Each kept point keeps the value recorded on its first day.
    for p in &w.series(K).unwrap().points {
        if p.date <= d(2025, 12, 1) {
            assert_eq!(p.value as u32, p.date - start, "date {}", p.date);
        }
    }
}

// ---- §12.6 Dispose -----------------------------------------------------------------------------

#[test]
fn hw12_06_dispose() {
    let (_f, mut w) = new();
    w.set_series(K, series(&[(1, 1), (5, 5), (6, 6), (7, 7), (8, 8), (20, 20)], 4));
    assert!(w.dispose_range(K, 2, 10));
    assert_eq!(points_of(&w, K), vec![(1, 1), (8, 8), (20, 20)]);
    assert!(!w.dispose_range(K, 2, 7)); // empty range now
    assert!(!w.dispose_range(0x999, 0, 10)); // missing key
    assert!(w.dispose(K));
    assert!(w.series(K).unwrap().points.is_empty());
    assert_eq!(w.series(K).unwrap().max_count, 4);
    assert!(!w.dispose(0x999));
}

// ---- §12.7 ranges and GetPoint --------------------------------------------------------------

#[test]
fn hw12_07_get_point() {
    let (_f, mut w) = new();
    let t = 1000u32;
    w.set_series(K, series(&[(t - 1, 0), (t, 10), (t + 1, 0)], 0));
    let v = w.get_point(K, t, P24).unwrap();
    assert_eq!(v, 10.0f32 / 3.0);
    assert_eq!(w.get_point(K, t, P53).unwrap(), (10.0f64 / 3.0) as f32);
    w.set_series(K, series(&[(t - 1, 0), (t + 1, 10)], 0));
    assert_eq!(w.get_point(K, t, P24), Some(5.0));
    // Before the first point, after the last, at the last, at the first.
    assert_eq!(w.get_point(K, t - 5, P24), Some(0.0));
    assert_eq!(w.get_point(K, t + 5, P24), Some(10.0));
    assert_eq!(w.get_point(K, t + 1, P24), Some(10.0));
    assert_eq!(w.get_point(K, t - 1, P24), Some(0.0));
    // Interpolation between two points.
    w.set_series(K, series(&[(10, 4), (20, 8)], 0));
    assert_eq!(w.get_point(K, 15, P24), Some(6.0));
    // Missing key or empty series: false.
    assert_eq!(w.get_point(0x999, 15, P24), None);
    w.set_series(K, series(&[], 0));
    assert_eq!(w.get_point(K, 15, P24), None);
}

// ---- HW2: the graph window draws from GetPoint alone -------------------------------------------

/// HW2 (re-answers22): the graph window (`cSC4GraphsViewWinProc`) reads a series
/// through slot 8 only — one `GetPoint` per sample date, never `GetCount` — so
/// what it draws at a sample date is whatever `GetPoint` answers. At an interior
/// point that is the regression value through the point and its two neighbours
/// (§4.7, §4.8), **not** the recorded value there: the smoothing is visible in
/// every graph. These cases pin that, so a later change that answers the
/// recorded value (or the plain mean, or a neighbour average) fails here.
#[test]
fn hw2_graph_window_draws_the_smoothed_value() {
    let (_f, mut w) = new();
    // §4.8: the recorded value at 1000 is 10 and the drawn one is 10/3.
    let t = 1000u32;
    w.set_series(K, series(&[(t - 1, 0), (t, 10), (t + 1, 0)], 0));
    let drawn = w.get_point(K, t, P24).unwrap();
    assert_ne!(drawn, 10.0, "the graph must not draw the recorded value");
    assert_eq!(drawn, 10.0f32 / 3.0);

    // A sample date on every interior point of a longer series: each is drawn
    // at its own three-point regression, which differs from the record as soon
    // as the series is not already a straight line.
    w.set_series(K, series(&[(10, 0), (20, 10), (30, 3), (40, 20), (50, 5)], 0));
    // date 30: neighbours 10 and 40 with 10 and 20; the regression through
    // (20, 10), (30, 3), (40, 20) is not 3.
    let at30 = w.get_point(K, 30, P24).unwrap();
    assert_ne!(at30, 3.0, "an interior point is drawn smoothed, not as recorded");
    // The end points are drawn as recorded (no predecessor / successor).
    assert_eq!(w.get_point(K, 10, P24), Some(0.0));
    assert_eq!(w.get_point(K, 50, P24), Some(5.0));

    // `GetCount` counts the widened range and is never what the window draws
    // (it does not call slot 5); a change to `GetCount` must not reach the
    // curves, and the two answer different things at the same date.
    assert_eq!(w.get_count(K, 30, 30), Some(3.0));
    assert_ne!(Some(at30), w.get_count(K, 30, 30));
}

#[test]
fn hw12_07_widened_ranges() {
    let p = pts(&[(10, 1), (20, 2), (30, 3), (40, 4)]);
    assert_eq!(widened_range(&p, 45, 50), 3..4); // after the last
    assert_eq!(widened_range(&p, 0, 5), 0..1); // before the first
    assert_eq!(widened_range(&p, 22, 28), 1..3); // a gap
    assert_eq!(widened_range(&p, 18, 22), 0..3); // one point with neighbours
    assert_eq!(widened_range(&p, 5, 12), 0..2); // the first point: no predecessor
    assert_eq!(widened_range(&p, 38, 45), 3..4); // runs to the last point: not widened
    assert_eq!(widened_range(&p, 15, 35), 1..3); // two points: as they are

    // HW6: from > to, neither pre-check: the first point after `to` and its
    // predecessor (test 11: 35 / 15 gives days 10 and 20).
    assert_eq!(widened_range(&p, 25, 15), 0..2);
    assert_eq!(widened_range(&p, 35, 15), 0..2);
    assert_eq!(widened_range(&p, 50, 45), 3..4);
    assert_eq!(widened_range(&p, 8, 5), 0..1);
    assert_eq!(widened_range(&p, 40, 30), 2..4); // the point on `to` is excluded
    assert_eq!(plain_range(&p, 35, 15), 0..0);
    assert_eq!(widened_range(&[], 0, 100), 0..0);
    assert_eq!(plain_range(&p, 20, 30), 1..3);
    assert!(plain_range(&p, 21, 29).is_empty());
    assert!(plain_range(&p, 30, 20).is_empty());
}

#[test]
fn hw12_07_get_count_counts_the_widened_range() {
    let (_f, mut w) = new();
    w.set_series(K, series(&[(10, 1), (20, 2), (30, 3), (40, 4)], 0));
    assert_eq!(w.get_count(K, 18, 22), Some(3.0));
    assert_eq!(w.get_count(K, 22, 28), Some(2.0));
    assert_eq!(w.get_count(K, 0, 100), Some(4.0));
    assert_eq!(w.get_count(0x999, 0, 100), None);
    w.set_series(K, series(&[], 0));
    assert_eq!(w.get_count(K, 0, 100), Some(0.0));
}

// ---- §12.8 GetTrend ------------------------------------------------------------------------------

#[test]
fn hw12_08_get_trend() {
    let (_f, mut w) = new();
    let from = 5000u32;
    w.set_series(K, series(&[(from, 0), (from + 10, 10), (from + 20, 20)], 0));
    for p in [P24, P53] {
        let (slope, intercept) = w.get_trend(K, from, from + 20, p).unwrap();
        assert!((slope - 1.0).abs() < 1e-6, "{p:?}: slope {slope}");
        assert!(intercept.abs() < 1e-5, "{p:?}: intercept {intercept}");
    }
    w.set_series(K, series(&[(from + 3, 42)], 0));
    assert_eq!(w.get_trend(K, from, from + 20, P24), Some((0.0, 42.0)));
    assert_eq!(w.get_trend(0x999, from, from + 20, P24), None);
    w.set_series(K, series(&[], 0));
    assert_eq!(w.get_trend(K, from, from + 20, P24), None);
}

/// The 24-bit sequence of §4.7, written out with `f32` operations for small
/// integer inputs (every operand is then a float, so each `f32` operation is
/// the single 24-bit rounding of the x87): points (0, 1), (1, 2), (2, 4).
fn trend_f32_by_hand() -> (f32, f32) {
    let xs = [0f32, 1.0, 2.0];
    let ys = [1f32, 2.0, 4.0];
    let n = 3f32;
    let (mut sx, mut sy) = (0f32, 0f32);
    for i in 0..3 {
        sx += xs[i];
        sy += ys[i];
    }
    let (mx, my) = (sx / n, sy / n);
    let (mut ssx, mut ssy) = (0f32, 0f32);
    for i in 0..3 {
        let (dx, dy) = (xs[i] - mx, ys[i] - my);
        ssx += dx * dx;
        ssy += dy * dy;
    }
    let (dsx, dsy) = ((ssx / n).sqrt(), (ssy / n).sqrt());
    let mut s = 0f32;
    for i in 0..3 {
        s += ((xs[i] - mx) / dsx) * ((ys[i] - my) / dsy);
    }
    let r = s / n;
    let slope = (dsy / dsx) * r;
    let intercept = (sy - slope * sx) / n;
    (slope, intercept)
}

#[test]
fn hw12_08_precision_24_and_53() {
    let (_f, mut w) = new();
    let from = 7000u32;
    w.set_series(K, series(&[(from, 1), (from + 1, 2), (from + 2, 4)], 0));
    let t24 = w.get_trend(K, from, from + 2, P24).unwrap();
    assert_eq!(t24, trend_f32_by_hand());
    let t53 = w.get_trend(K, from, from + 2, P53).unwrap();
    // The same formulas in double, then stored to float.
    let want = {
        let (xs, ys) = ([0f64, 1.0, 2.0], [1f64, 2.0, 4.0]);
        let (sx, sy): (f64, f64) = (xs.iter().sum(), ys.iter().sum());
        let (mx, my) = (sx / 3.0, sy / 3.0);
        let ssx: f64 = xs.iter().map(|x| (x - mx) * (x - mx)).sum();
        let ssy: f64 = ys.iter().map(|y| (y - my) * (y - my)).sum();
        let (dsx, dsy) = ((ssx / 3.0).sqrt(), (ssy / 3.0).sqrt());
        let s: f64 = (0..3).map(|i| ((xs[i] - mx) / dsx) * ((ys[i] - my) / dsy)).sum();
        let slope = (dsy / dsx) * (s / 3.0);
        (slope as f32, ((sy - slope * sx) / 3.0) as f32)
    };
    assert_eq!(t53, want);
    // The two precisions give different floats here (HW3).
    assert_ne!(t24, t53, "24-bit {t24:?} 53-bit {t53:?}");
}

#[test]
fn add_r24_rounds_once() {
    // 2^24 + 1 is a tie: to even (2^24).
    assert_eq!(add_r24(16_777_216.0, 1.0), 16_777_216.0);
    // 2^24 + 3 → 2^24 + 4 (tie to even).
    assert_eq!(add_r24(16_777_216.0, 3.0), 16_777_220.0);
    // An int32 operand with a fraction: the f64 sum rounds to a midpoint,
    // the exact sum lies above it: 2^30 + 64.5 + 2^-24 → 2^30 + 128.
    let a = 1_073_741_824.0 + 64.0; // 2^30 + 64: a 24-bit midpoint
    let b = 0.5 + f64::from(2f32.powi(-24)); // a float with a tiny tail
    let s = a + b;
    assert_eq!(s, a + 0.5); // the f64 sum loses the tail
    assert_eq!(add_r24(a, b), 1_073_741_824.0 + 128.0);
    assert_eq!(add_r24(-a, -b), -(1_073_741_824.0 + 128.0));
    // Below the midpoint: toward zero.
    assert_eq!(add_r24(a, -f64::from(2f32.powi(-24))), 1_073_741_824.0);
    assert_eq!(add_r24(0.0, 0.0), 0.0);
}

// ---- §12.9 GetMinMaxAverage ----------------------------------------------------------------------

#[test]
fn hw12_09_get_min_max_average() {
    let (_f, mut w) = new();
    w.set_series(K, series(&[(10, 4), (20, 8), (30, 6)], 0));
    let (min, max, avg) = w.get_min_max_average(K, 15, 30, P24).unwrap();
    assert_eq!((min, max), (6.0, 8.0));
    assert_eq!(avg, 20.0f32 / 3.0);
    // Points on both ends: no GetPoint; avg = 18 / 3.
    assert_eq!(w.get_min_max_average(K, 10, 30, P24), Some((4.0, 8.0, 6.0)));
    // Neither end on a point: two GetPoint values, 4.8 at 12 and 6.4 at 28.
    let (min, max, avg) = w.get_min_max_average(K, 12, 28, P24).unwrap();
    assert_eq!(min, 4.8f32);
    assert_eq!(max, 8.0);
    assert_eq!(avg, ((4.8f32 + 8.0f32) + 6.4f32) / 3.0);
    // Empty series or missing key: false.
    w.set_series(K, series(&[], 0));
    assert_eq!(w.get_min_max_average(K, 10, 30, P24), None);
    assert_eq!(w.get_min_max_average(0x999, 10, 30, P24), None);
    // A range with no point: the two GetPoint values only.
    w.set_series(K, series(&[(10, 4), (20, 8)], 0));
    assert_eq!(w.get_min_max_average(K, 30, 40, P24), Some((8.0, 8.0, 8.0)));
    // from = to with no point: GetPoint twice (the same value).
    assert_eq!(w.get_min_max_average(K, 15, 15, P24), Some((6.0, 6.0, 6.0)));
}

#[test]
fn hw12_09_min_max_compare_ints_exactly() {
    let (_f, mut w) = new();
    // 16777217 does not fit a float: min/max store it rounded, the sum rounds.
    w.set_series(K, series(&[(10, 16_777_217), (11, 1)], 0));
    let (min, max, avg) = w.get_min_max_average(K, 10, 11, P24).unwrap();
    assert_eq!((min, max), (1.0, 16_777_216.0));
    // sum: 0 + 16777217 → 16777216 (tie to even), + 1 → 16777216 again.
    assert_eq!(avg, 8_388_608.0);
}

// ---- §12.10 the record --------------------------------------------------------------------------

fn record(w: &HistoryWarehouse) -> Vec<u8> {
    let mut out = ByteWriter::new();
    assert!(w.write(&mut out));
    out.into_bytes()
}

#[test]
fn hw12_10_save_round_trip() {
    let (_f, mut w) = new();
    w.set_series(0x20, series(&[(d(2001, 1, 1), -5), (d(2001, 1, 2), 7)], 365));
    w.set_series(0x10, series(&[(d(2000, 1, 1), 1)], 0));
    w.set_series(0x30, series(&[], 2));
    let bytes = record(&w);
    // Layout: version, count, then the series in ascending key order.
    let mut want = ByteWriter::new();
    want.put_u16(2);
    want.put_u32(3);
    for (key, max, p) in [
        (0x10u32, 0u32, vec![(d(2000, 1, 1), 1)]),
        (0x20, 365, vec![(d(2001, 1, 1), -5), (d(2001, 1, 2), 7)]),
        (0x30, 2, vec![]),
    ] {
        want.put_u32(key);
        want.put_u32(max);
        want.put_u32(p.len() as u32);
        for (date, v) in p {
            want.put_u32(date);
            want.put_i32(v);
        }
    }
    assert_eq!(bytes, want.into_bytes());
    let (_f2, mut back) = new();
    assert!(back.read(&mut ByteReader::new(&bytes)));
    assert_eq!(record(&back), bytes);
}

/// One saved series: key, maxCount and its (date, value) pairs.
type Saved<'a> = (u32, u32, &'a [Pair]);
type Pair = (u32, i32);

fn v1_record(series: &[Saved], version: u16) -> Vec<u8> {
    let mut w = ByteWriter::new();
    w.put_u16(version);
    w.put_u32(series.len() as u32);
    for (key, max, pts) in series {
        w.put_u32(*key);
        w.put_u32(*max);
        w.put_u32(pts.len() as u32);
        for (date, v) in pts.iter() {
            w.put_u32(*date);
            w.put_i32(*v);
        }
    }
    w.into_bytes()
}

#[test]
fn hw12_10_read_quirks() {
    let j = julian::JULIAN_2000_01_01 as u32;
    // v1: the triple 2000-01-01 date is one point with the last value; 100 → 0.
    let bytes = v1_record(&[(K, 100, &[(j, 1), (j, 2), (j, 3), (j + 31, 4)])], 1);
    let (_f, mut w) = new();
    assert!(w.read(&mut ByteReader::new(&bytes)));
    assert_eq!(points_of(&w, K), vec![(j, 3), (j + 31, 4)]);
    assert_eq!(w.series(K).unwrap().max_count, 0);
    // v2 keeps 100.
    let bytes = v1_record(&[(K, 100, &[])], 2);
    let (_f, mut w) = new();
    assert!(w.read(&mut ByteReader::new(&bytes)));
    assert_eq!(w.series(K).unwrap().max_count, 100);
    // Version 3 is rejected.
    let bytes = v1_record(&[(K, 0, &[(j, 1)])], 3);
    let (_f, mut w) = new();
    assert!(!w.read(&mut ByteReader::new(&bytes)));
    assert_eq!(w.series_count(), 0);
    // 0xCDCDCDCD dates become prev + 1; a first date of 0 is dropped [choice].
    let bytes = v1_record(&[(K, 0, &[(0, 9), (j, 1), (DATE_UNSET, 2), (DATE_UNSET, 3)])], 2);
    let (_f, mut w) = new();
    assert!(w.read(&mut ByteReader::new(&bytes)));
    assert_eq!(points_of(&w, K), vec![(j, 1), (j + 1, 2), (j + 2, 3)]);
    // Read does not clear; a duplicate key keeps the existing series and
    // stops the read with false (HW7): the later series is not read.
    let bytes = v1_record(&[(0x98, 0, &[(j, 49)]), (K, 0, &[(j, 50)]), (0x99, 0, &[(j, 51)])], 2);
    assert!(!w.read(&mut ByteReader::new(&bytes)));
    assert_eq!(points_of(&w, K), vec![(j, 1), (j + 1, 2), (j + 2, 3)]);
    assert_eq!(points_of(&w, 0x98), vec![(j, 49)]);
    assert!(w.series(0x99).is_none());
    // Test 14: a record listing keys A, A, B reads A, returns false, and B
    // is absent.
    let bytes = v1_record(&[(0xA, 0, &[(j, 1)]), (0xA, 0, &[(j, 2)]), (0xB, 0, &[(j, 3)])], 2);
    let (_f, mut w) = new();
    assert!(!w.read(&mut ByteReader::new(&bytes)));
    assert_eq!(w.keys(), vec![0xA]);
    assert_eq!(points_of(&w, 0xA), vec![(j, 1)]);
    // A truncated record fails; complete series before it stay.
    let mut bytes = v1_record(&[(0x1, 0, &[(j, 1)]), (0x2, 0, &[(j, 2)])], 2);
    bytes.truncate(bytes.len() - 2);
    let (_f, mut w) = new();
    assert!(!w.read(&mut ByteReader::new(&bytes)));
    assert_eq!(w.keys(), vec![1]);
}

/// A version-1 record shaped like the corpus (§7.1: 62 series, maxCount 0, 1,
/// 2 and 365, six series starting with the triple 2000-01-01 date): it reads,
/// and rewriting gives version 2 with the duplicates collapsed.
#[test]
fn hw12_10_corpus_shaped_v1_record() {
    let j = julian::JULIAN_2000_01_01 as u32;
    let mut all: Vec<(u32, u32, Vec<Pair>)> = Vec::new();
    for i in 0..62u32 {
        let max = [0, 1, 2, 365][(i % 4) as usize];
        let mut p = Vec::new();
        if i < 6 {
            p.extend([(j, 1), (j, 2), (j, 3), (j + 5, 4)]);
        } else {
            let n = if max == 0 { 40 } else { max.min(30) };
            p.extend((0..n).map(|k| (j + k, (k * i) as i32)));
        }
        all.push((0x0A00_0000 + i * 0x101, max, p));
    }
    let refs: Vec<Saved> = all.iter().map(|(k, m, p)| (*k, *m, p.as_slice())).collect();
    let bytes = v1_record(&refs, 1);
    let (_f, mut w) = new();
    assert!(w.read(&mut ByteReader::new(&bytes)));
    assert_eq!(w.series_count(), 62);
    for (k, m, p) in &all {
        let s = w.series(*k).unwrap();
        assert_eq!(s.max_count, *m);
        let expect = if p.len() == 4 && p[0].0 == p[1].0 { 2 } else { p.len() };
        assert_eq!(s.points.len(), expect);
    }
    let again = record(&w);
    assert_eq!(&again[..2], &[2, 0]);
    let (_f, mut w2) = new();
    assert!(w2.read(&mut ByteReader::new(&again)));
    assert_eq!(record(&w2), again);
}

// ---- re-answers10: HW5, HW6, HW8, HW9, HW12 (tests 11-15) -------------------------------------

#[test]
fn hw12_11_inverted_ranges() {
    let (_f, mut w) = new();
    w.set_series(K, series(&[(10, 1), (20, 2), (30, 3), (40, 4)], 0));
    assert_eq!(w.get_count(K, 35, 15), Some(2.0));
    assert_eq!(w.get_count(K, 50, 45), Some(1.0));
    assert_eq!(w.get_count(K, 8, 5), Some(1.0));
    // GetMinMaxAverage(35, 15) reads no point and adds GetPoint(35) and
    // GetPoint(15): the two-point line through (10, 1) and (20, 2) based at
    // 35 gives 3.5, and the 3-point line around 20 gives 2.
    let p35 = w.get_point(K, 35, P24).unwrap();
    let p15 = w.get_point(K, 15, P24).unwrap();
    assert_eq!(p35, 3.5);
    assert_eq!(p15, 1.5);
    let (min, max, avg) = w.get_min_max_average(K, 35, 15, P24).unwrap();
    assert_eq!((min, max, avg), (1.5, 3.5, 2.5));
}

#[test]
fn hw12_12_statistics_order() {
    let (_f, mut w) = new();
    let from = 5000;
    w.set_series(K, series(&[(from, 0), (from + 1, 1), (from + 3, 1)], 0));
    let (s, i) = w.get_trend(K, from, from + 3, P24).unwrap();
    assert_eq!((s.to_bits(), i.to_bits()), (0x3E92_4922, 0x3E92_4928));
    w.set_series(K, series(&[(from, 0), (from + 1, 3), (from + 3, 8)], 0));
    let (s, i) = w.get_trend(K, from, from + 3, P24).unwrap();
    assert_eq!((s.to_bits(), i.to_bits()), (0x4029_2492, 0x3E12_492B));
}

#[test]
fn hw12_13_min_max_average_from_equals_to() {
    let (_f, mut w) = new();
    w.set_series(K, series(&[(10, 4)], 0));
    assert_eq!(w.get_min_max_average(K, 15, 15, P24), Some((4.0, 4.0, 4.0)));
    w.set_series(K, series(&[(10, 4), (20, 8)], 0));
    assert_eq!(w.get_min_max_average(K, 15, 15, P24), Some((6.0, 6.0, 6.0)));
    // With a point on that date neither GetPoint call happens.
    assert_eq!(w.get_min_max_average(K, 20, 20, P24), Some((8.0, 8.0, 8.0)));
}

#[test]
fn hw12_15_record_and_unlimited_trim() {
    let (f, mut w) = new();
    assert!(!HistoryWarehouse::record_history_data(&H(f.clone()), K, 1));
    // SetSize(K, -1): maxCount 0xFFFFFFFF never trims (HW8).
    w.set_size(K, u32::MAX);
    for d in 1..=5u32 {
        f.date.set(100 + d);
        w.store(K, d as i32);
    }
    assert_eq!(w.series(K).unwrap().points.len(), 5);
    // A series longer than its maxCount (after Read) keeps its length.
    w.set_series(K, series(&[(1, 1), (2, 2), (3, 3)], 2));
    f.date.set(10);
    w.store(K, 9);
    assert_eq!(points_of(&w, K), vec![(2, 2), (3, 3), (10, 9)]);
    // [quirk] A record on day number 0 into an empty series is lost.
    f.date.set(0);
    w.store(0x5, 7);
    assert_eq!(w.series(0x5).unwrap().points.len(), 0);
}
