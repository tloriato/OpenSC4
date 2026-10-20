// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! [`HistoryWarehouse`]: state (§2), messages (§3), behaviour (§4), the
//! record (§7) and the interface methods (§8).

use std::collections::BTreeMap;

use super::stats::{fit_line, plain_range, widened_range, Precision};
use super::{
    granularity, msg, HistoryHost, Point, Series, AGENT_FLAGS, AGENT_LEVEL_DAY, AGENT_LEVEL_YEAR, AGENT_NAME,
    DATE_UNSET, PASS_ARMED, PASS_IDLE, PASS_RUNNING, PASS_TIME_LIMIT_MS, RECORD_VERSION,
};
use crate::julian::{civil_from_julian, julian_from_civil};
use crate::stream::{InStream, OutStream};

/// The history warehouse.
pub struct HistoryWarehouse {
    host: Box<dyn HistoryHost>,
    series: BTreeMap<u32, Series>,
    initialized: bool,
    pass_state: u32,
    /// The pass cursor: the key of its series, `None` at the end of the map.
    cursor: Option<u32>,
    pass_counter: u32,
    /// simulator.md §5.7: captured from [`crate::pacing::process`] at
    /// construction; deterministic pacing never reads the clock in the pass.
    pacing: crate::pacing::Pacing,
}

/// The day number of a civil date, as the warehouse's `u32` dates.
fn day(year: i32, month: i32, d: i32) -> u32 {
    julian_from_civil(year, month, d) as u32
}

/// The group key and period start of a point (§4.4).
fn group_of(date: u32, granularity: i32) -> (i32, u32) {
    let c = civil_from_julian(date as i32);
    match granularity {
        granularity::MONTH => (c.month, day(c.year, c.month, 1)),
        granularity::HALF_YEAR => {
            let half = (c.month - 1) / 6;
            (2 * c.year + half, day(c.year, 1 + 6 * half, 1))
        }
        granularity::YEAR => (c.year, day(c.year, 1, 1)),
        granularity::FIVE_YEARS => (c.year / 5, day(c.year, 1, 1)),
        // Every point in one group, which is never closed: the date is unused.
        _ => (-1000, date),
    }
}

impl HistoryWarehouse {
    /// The constructor's state (§2): no series, not initialised, pass idle.
    pub fn new(host: Box<dyn HistoryHost>) -> HistoryWarehouse {
        HistoryWarehouse {
            host,
            series: BTreeMap::new(),
            initialized: false,
            pass_state: PASS_IDLE,
            cursor: None,
            pass_counter: 0,
            pacing: crate::pacing::process(),
        }
    }

    /// The pacing this warehouse runs (simulator.md §5.7).
    pub fn pacing(&self) -> crate::pacing::Pacing {
        self.pacing
    }

    /// Shadows and tests: [`crate::pacing::Pacing::Original`] keeps the
    /// pass's 5 ms limit.
    pub fn set_pacing(&mut self, p: crate::pacing::Pacing) {
        self.pacing = p;
    }

    // ---- lifecycle (§3) -----------------------------------------------------------------------

    /// `Init` (slot 3): subscribes to the record and queue-length messages,
    /// registers the day and year agents (flags 3), idles the pass. There is
    /// no "already initialised" test (§3). Returns true.
    pub fn init(&mut self) -> bool {
        self.host.subscribe(msg::RECORD);
        self.host.subscribe(msg::SET_QUEUE_LENGTH);
        self.host.add_agent(AGENT_LEVEL_DAY, AGENT_NAME, AGENT_FLAGS);
        self.host.add_agent(AGENT_LEVEL_YEAR, AGENT_NAME, AGENT_FLAGS);
        self.pass_state = PASS_IDLE;
        self.initialized = true;
        true
    }

    /// `Shutdown` (slot 4): unsubscribes, removes the agent. The series are
    /// kept. Returns true.
    pub fn shutdown(&mut self) -> bool {
        self.host.unsubscribe(msg::RECORD);
        self.host.unsubscribe(msg::SET_QUEUE_LENGTH);
        self.host.remove_agent();
        self.initialized = false;
        true
    }

    /// `DoMessage` (§3). The day message returns the thinning step's result
    /// (false: deliver again this interval); every other message true.
    pub fn do_message(&mut self, msg_type: u32, data1: u32, data2: u32) -> bool {
        match msg_type {
            msg::RECORD => {
                self.store(data1, data2 as i32);
                true
            }
            msg::SET_QUEUE_LENGTH => self.set_size(data1, data2),
            msg::SIM_NEW_DAY => self.thinning_step(),
            msg::SIM_NEW_YEAR => {
                if self.pass_state == PASS_IDLE {
                    self.pass_state = PASS_ARMED;
                }
                true
            }
            _ => true,
        }
    }

    // ---- recording (§4.1, §4.2) -----------------------------------------------------------------

    /// `RecordHistoryData` (slot 12): sends the record message (type
    /// `0x89EFA536`, data1 = key, data2 = value) synchronously through the
    /// host; the warehouse's own `DoMessage` stores it. It does not touch the
    /// warehouse, so it takes only the host. Returns false (HW12, [compat]:
    /// the original returns the final `Release`'s EAX, 0 once the message is
    /// destroyed; no shipped caller reads it).
    pub fn record_history_data(host: &dyn HistoryHost, key: u32, value: i32) -> bool {
        host.send(msg::RECORD, key, value as u32);
        false
    }

    /// `Store(key, value)` (§4.1): a second record on the same day (or an
    /// earlier one) overwrites the last point; a full queue drops its first
    /// point. The count is compared unsigned with `maxCount` (HW8).
    pub fn store(&mut self, key: u32, value: i32) {
        let day = self.host.sim_date();
        let s = self.series.entry(key).or_default();
        match s.points.last_mut() {
            Some(last) if day <= last.date => {
                last.value = value;
                return;
            }
            // [quirk] An empty series compares the day with 0: a record on
            // day number 0 is lost (the original writes it into the list
            // header, §4.1). No real date is 0.
            None if day == 0 => return,
            _ => {}
        }
        if s.max_count != 0 && s.points.len() as u64 >= u64::from(s.max_count) {
            s.points.remove(0);
        }
        s.points.push(Point { value, date: day });
    }

    /// `SetSize(key, n)` (§4.2; slot 13 and message `0x89EFA537`): creates
    /// the series if needed, sets `maxCount := n`, and for `n > 0` (signed)
    /// drops the oldest points beyond n. Returns true.
    pub fn set_size(&mut self, key: u32, n: u32) -> bool {
        let s = self.series.entry(key).or_default();
        s.max_count = n;
        if (n as i32) > 0 && s.points.len() > n as usize {
            let extra = s.points.len() - n as usize;
            s.points.drain(..extra);
        }
        true
    }

    /// `SetHistoryDataQueueLength` (slot 13).
    pub fn set_history_data_queue_length(&mut self, key: u32, n: u32) -> bool {
        self.set_size(key, n)
    }

    // ---- the yearly thinning pass (§4.3) ------------------------------------------------------

    fn cursor_key(&self) -> Option<u32> {
        let k = self.cursor?;
        self.series.range(k..).next().map(|(&k, _)| k)
    }

    fn advance_cursor(&mut self) {
        use std::ops::Bound::{Excluded, Unbounded};
        self.cursor = self
            .cursor_key()
            .and_then(|k| self.series.range((Excluded(k), Unbounded)).next().map(|(&k, _)| k));
    }

    /// One step of the yearly pass, run at each SimNewDay (§4.3). Returns
    /// false when the simulator should deliver the day again.
    pub fn thinning_step(&mut self) -> bool {
        match self.pass_state {
            PASS_ARMED => {
                self.cursor = self.series.keys().next().copied();
                self.pass_counter = 0;
                self.pass_state = PASS_RUNNING;
            }
            PASS_RUNNING => {}
            _ => return true,
        }
        // Deterministic pacing (simulator.md §5.7): the item count only.
        let timed = !self.pacing.is_deterministic();
        let start = if timed { self.host.now_ms() } else { 0 };
        if self.series.is_empty() {
            self.pass_state = PASS_IDLE;
            return true;
        }
        let per_pass = (self.series.len() / 365 + 1) as u32;
        while self.pass_counter < per_pass {
            let Some(key) = self.cursor_key() else {
                self.cursor = None;
                self.pass_state = PASS_IDLE;
                break;
            };
            self.cursor = Some(key);
            if self.series.get(&key).is_some_and(|s| s.max_count == 0) {
                self.variable_coalesce(key);
                self.pass_counter += 1;
            }
            self.advance_cursor();
            if timed && self.host.now_ms().wrapping_sub(start) >= PASS_TIME_LIMIT_MS {
                break;
            }
        }
        if self.pass_counter >= per_pass {
            self.pass_counter = 0;
            true
        } else {
            false
        }
    }

    /// `VariableCoalesce(key)` (§4.3): thins one unlimited series by the
    /// current date's month `M` and year `Y`.
    pub fn variable_coalesce(&mut self, key: u32) {
        let c = civil_from_julian(self.host.sim_date() as i32);
        let (y, m) = (c.year, c.month);
        let d1 = if m == 1 { day(y - 1, 12, 1) } else { day(y, m - 1, 1) };
        let d2 = day(y - 10, 1, 1);
        let d3 = day(y - 50, 1, 1);
        let d4 = day(y - 100, 1, 1);
        let d5 = day(y - 500, 1, 1);
        self.dispose_range(key, 0, d5);
        self.coalesce(key, d5, d4, granularity::FIVE_YEARS);
        self.coalesce(key, d4, d3, granularity::YEAR);
        self.coalesce(key, d3, d2, granularity::HALF_YEAR);
        self.coalesce(key, d2, d1, granularity::MONTH);
    }

    // ---- Coalesce and Dispose (§4.4, §4.5) ----------------------------------------------------

    /// `Coalesce(key, from, to, granularity)` (slot 9, §4.4): within the
    /// inclusive range (not widened), each closed group keeps its first
    /// point, dated at the group's period start; the last group of the range
    /// stays as it is. False for a missing key or an empty range.
    pub fn coalesce(&mut self, key: u32, from: u32, to: u32, granularity: i32) -> bool {
        let Some(s) = self.series.get_mut(&key) else {
            return false;
        };
        let r = plain_range(&s.points, from, to);
        if r.is_empty() {
            return false;
        }
        let pts = &mut s.points;
        let mut end = r.end;
        let mut open = r.start;
        let (mut group, mut period) = group_of(pts[open].date, granularity);
        let mut i = open + 1;
        while i < end {
            let (k, p) = group_of(pts[i].date, granularity);
            if k == group {
                i += 1;
                continue;
            }
            // Close the group: its first point moves to the period start
            // (its value is kept), the others go.
            pts[open].date = period;
            let removed = i - (open + 1);
            pts.drain(open + 1..i);
            i -= removed;
            end -= removed;
            open = i;
            group = k;
            period = p;
            i += 1;
        }
        true
    }

    /// `Dispose(key)` (slot 10): erases every point; the series and its
    /// `maxCount` stay. False for a missing key.
    pub fn dispose(&mut self, key: u32) -> bool {
        match self.series.get_mut(&key) {
            Some(s) => {
                s.points.clear();
                true
            }
            None => false,
        }
    }

    /// `Dispose(key, from, to)` (slot 11): erases the points of the inclusive
    /// range except its last one [quirk]. False for a missing key or an empty
    /// range.
    pub fn dispose_range(&mut self, key: u32, from: u32, to: u32) -> bool {
        let Some(s) = self.series.get_mut(&key) else {
            return false;
        };
        let r = plain_range(&s.points, from, to);
        if r.is_empty() {
            return false;
        }
        s.points.drain(r.start..r.end - 1);
        true
    }

    // ---- queries (§4.6, §4.7) -----------------------------------------------------------------

    /// `GetCount` (slot 5): the number of points of the widened range, as
    /// `float`. `None` (false, outputs untouched) for a missing key.
    pub fn get_count(&self, key: u32, from: u32, to: u32) -> Option<f32> {
        let s = self.series.get(&key)?;
        Some(widened_range(&s.points, from, to).len() as f32)
    }

    /// `GetTrend` (slot 6): `(slope per day, value at from)` of the widened
    /// range's line, at the caller's precision. `None` for a missing key or
    /// an empty range.
    pub fn get_trend(&self, key: u32, from: u32, to: u32, prec: Precision) -> Option<(f32, f32)> {
        let s = self.series.get(&key)?;
        let r = widened_range(&s.points, from, to);
        let (slope, intercept) = fit_line(&s.points[r], from, prec)?;
        Some((slope as f32, intercept as f32))
    }

    /// `GetPoint` (slot 8): the line of the widened range with
    /// `from = to = date`, at `date` (its intercept). `None` for a missing key
    /// or an empty series.
    ///
    /// At a date with points on both sides the widened range is those three
    /// points and this is the regression value through them, which is **not**
    /// the recorded value at that date (§4.8). That is not an optional
    /// smoothing: the graph window (`cSC4GraphsViewWinProc`) draws every curve
    /// from `GetPoint` alone and never calls `GetCount`, so a sample date that
    /// falls on an interior point is drawn at this value (HW2). Answering the
    /// recorded value here would change every graph in the game.
    pub fn get_point(&self, key: u32, date: u32, prec: Precision) -> Option<f32> {
        let s = self.series.get(&key)?;
        let r = widened_range(&s.points, date, date);
        let (_, intercept) = fit_line(&s.points[r], date, prec)?;
        Some(intercept as f32)
    }

    /// `GetMinMaxAverage` (slot 7): `(min, max, avg)` over the inclusive range
    /// (not widened), plus `GetPoint` at `from` and at `to` when no point lies
    /// exactly there. `None` for a missing key or a series with no points.
    pub fn get_min_max_average(&self, key: u32, from: u32, to: u32, prec: Precision) -> Option<(f32, f32, f32)> {
        let s = self.series.get(&key)?;
        if s.points.is_empty() {
            return None;
        }
        let mut min = f32::MAX;
        let mut max = -f32::MAX;
        let mut sum = 0.0f32;
        let mut count = 0.0f32;
        let mut take = |v: f64| {
            // `sum += value`: one fadd at the caller's precision, stored to float.
            sum = prec.add(f64::from(sum), v) as f32;
            if v < f64::from(min) {
                min = v as f32;
            }
            if v > f64::from(max) {
                max = v as f32;
            }
            count += 1.0;
        };
        let (mut on_from, mut on_to) = (false, false);
        for p in &s.points[plain_range(&s.points, from, to)] {
            take(f64::from(p.value));
            on_from |= p.date == from;
            on_to |= p.date == to;
        }
        // Through the interface (slot 8): a non-empty series always answers.
        if !on_from {
            if let Some(v) = self.get_point(key, from, prec) {
                take(f64::from(v));
            }
        }
        if !on_to {
            if let Some(v) = self.get_point(key, to, prec) {
                take(f64::from(v));
            }
        }
        let avg = (f64::from(sum) / f64::from(count)) as f32;
        Some((min, max, avg))
    }

    // ---- persistence (§7) -----------------------------------------------------------------------

    /// `Write`: version 2, the series in ascending key order. False only on a
    /// stream error.
    pub fn write(&self, out: &mut dyn OutStream) -> bool {
        let mut ok = out.put_u16(RECORD_VERSION) && out.put_u32(self.series.len() as u32);
        for (&key, s) in &self.series {
            if !ok {
                break;
            }
            ok = out.put_u32(key) && out.put_u32(s.max_count) && out.put_u32(s.points.len() as u32);
            for p in &s.points {
                if !ok {
                    break;
                }
                ok = out.put_u32(p.date) && out.put_i32(p.value);
            }
        }
        ok && out.error() == 0
    }

    /// `Read` (§7): versions 0–2. The map is not cleared. A key that already
    /// exists (in the map or earlier in the record) keeps its series, the
    /// read one is discarded and `Read` returns false at once, leaving the
    /// rest of the record unread (HW7).
    pub fn read(&mut self, inp: &mut dyn InStream) -> bool {
        let Some(version) = inp.get_u16() else {
            return false;
        };
        if version > RECORD_VERSION {
            return false;
        }
        let Some(count) = inp.get_u32() else {
            return false;
        };
        for _ in 0..count {
            let Some(s) = Self::read_series(inp, version) else {
                return false;
            };
            if self.series.contains_key(&s.0) {
                return false;
            }
            self.series.insert(s.0, s.1);
        }
        inp.error() == 0
    }

    fn read_series(inp: &mut dyn InStream, version: u16) -> Option<(u32, Series)> {
        let key = inp.get_u32()?;
        let mut max_count = inp.get_u32()?;
        if version == 1 && max_count == 100 {
            max_count = 0;
        }
        let n = inp.get_u32()?;
        let mut s = Series {
            points: Vec::new(),
            max_count,
        };
        let mut prev = 0u32;
        for _ in 0..n {
            let mut date = inp.get_u32()?;
            let value = inp.get_i32()?;
            if date == DATE_UNSET {
                date = prev.wrapping_add(1);
            }
            if date <= prev {
                // [choice] With no point yet the original writes into its
                // list header; the value is dropped here.
                if let Some(last) = s.points.last_mut() {
                    last.value = value;
                }
            } else {
                s.points.push(Point { value, date });
                prev = date;
            }
        }
        Some((key, s))
    }

    // ---- inspection and test support ------------------------------------------------------------

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub fn pass_state(&self) -> u32 {
        self.pass_state
    }

    pub fn pass_counter(&self) -> u32 {
        self.pass_counter
    }

    /// The key of the cursor's series (`None` at the end of the map).
    pub fn cursor(&self) -> Option<u32> {
        self.cursor_key()
    }

    pub fn series(&self, key: u32) -> Option<&Series> {
        self.series.get(&key)
    }

    pub fn keys(&self) -> Vec<u32> {
        self.series.keys().copied().collect()
    }

    pub fn series_count(&self) -> usize {
        self.series.len()
    }

    pub fn total_points(&self) -> usize {
        self.series.values().map(|s| s.points.len()).sum()
    }

    /// Test support: replaces (or inserts) a series.
    pub fn set_series(&mut self, key: u32, s: Series) {
        self.series.insert(key, s);
    }

    /// Test support: sets the pass fields of §2.
    pub fn set_pass(&mut self, state: u32, cursor: Option<u32>, counter: u32) {
        self.pass_state = state;
        self.cursor = cursor;
        self.pass_counter = counter;
    }
}
