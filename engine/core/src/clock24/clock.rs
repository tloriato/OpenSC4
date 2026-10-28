// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! [`Clock24`]: state (§2), messages (§3), behaviour (§4), tuning (§6), the
//! record (§7) and the interface methods (§8).

use super::timer::Timer;
use super::{
    hour_of, minute_of, msg, second_of, ClockHost, CHEAT_NAME_STOPWATCH, CHEAT_NAME_WHAT_TIME_IZ_IT, CHEAT_STOPWATCH,
    CHEAT_WHAT_TIME_IZ_IT, DEFAULT_MAGNIFICATION, DEFAULT_MAX_MAGNIFICATION, DEFAULT_MIN_MAGNIFICATION_BITS,
    DEFAULT_NIGHT_END, DEFAULT_NIGHT_START, MAX_STEP, MODE_FOLLOW, MODE_MIDNIGHT, MODE_NOON, NOON, PERCENT_FACTOR_BITS,
    RECORD_VERSION, SECONDS_PER_DAY, STEP_FACTOR_BITS, STEP_THRESHOLD,
};
use crate::stream::{InStream, OutStream};
use crate::x87::{ftol32, mul_i64_f32, u32_to_float};

/// `f32(v × f)` for an unsigned `v` under 24-bit precision (foundations
/// §18.3): below 2³¹ the value is loaded exactly and the product rounded
/// once (`x87::mul_i64_f32`); from 2³¹ up the `float(uint32)` idiom
/// (`x87::u32_to_float`, one rounding) comes first, then the product is
/// rounded once.
pub fn mul_u32_f32(v: u32, f: f32) -> f32 {
    if (v as i32) >= 0 {
        mul_i64_f32(i64::from(v), f)
    } else {
        (u32_to_float(v as i32) as f32) * f
    }
}

/// The framework's command-line split (§4.8, CK11):
///
/// 1. the separators are exactly space, tab and newline (`\r` is ordinary);
/// 2. separators at the start and the end are removed; nothing left gives no
///    arguments;
/// 3. a separator ends the argument only when an even number of `"` has been
///    seen since the argument began; a run of separators is one break;
/// 4. the text after the last break is the last argument;
/// 5. an argument that both starts and ends with `"` loses its last
///    character and then, if it is not empty, its first. No other quote is
///    removed.
pub fn split_command_line(text: &[u8]) -> Vec<Vec<u8>> {
    let is_sep = |b: u8| matches!(b, b' ' | b'\t' | b'\n');
    let lo = text.iter().position(|&b| !is_sep(b)).unwrap_or(text.len());
    let hi = text.iter().rposition(|&b| !is_sep(b)).map_or(lo, |i| i + 1);
    let text = &text[lo..hi];
    let mut args = Vec::new();
    if text.is_empty() {
        return args;
    }
    let mut cur = Vec::new();
    let mut quotes = 0u32;
    let mut i = 0;
    while i < text.len() {
        let b = text[i];
        if is_sep(b) && quotes % 2 == 0 {
            args.push(std::mem::take(&mut cur));
            quotes = 0;
            while i < text.len() && is_sep(text[i]) {
                i += 1;
            }
            continue;
        }
        if b == b'"' {
            quotes += 1;
        }
        cur.push(b);
        i += 1;
    }
    args.push(cur);
    for a in &mut args {
        if !a.is_empty() && a[0] == b'"' && a[a.len() - 1] == b'"' {
            a.pop();
            if !a.is_empty() {
                a.remove(0);
            }
        }
    }
    args
}

/// C `atol` (§4.8, CK10): leading white space (`isspace`), one optional
/// sign, then digits up to the first non-digit, accumulated in wrapping
/// 32-bit arithmetic; no digits give 0.
pub fn atol(s: &[u8]) -> i32 {
    let mut i = 0;
    while i < s.len() && matches!(s[i], b' ' | b'\t' | b'\r' | b'\n' | 0x0B | 0x0C) {
        i += 1;
    }
    let mut neg = false;
    if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
        neg = s[i] == b'-';
        i += 1;
    }
    let mut v = 0i32;
    while i < s.len() && s[i].is_ascii_digit() {
        v = v.wrapping_mul(10).wrapping_add(i32::from(s[i] - b'0'));
        i += 1;
    }
    if neg {
        v.wrapping_neg()
    } else {
        v
    }
}

/// Diagnostics: every field of §2 and the timer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClockState {
    pub initialized: bool,
    pub pause_count: i32,
    pub hidden_pause_count: i32,
    pub magnification: f32,
    pub min_magnification: f32,
    pub max_magnification: f32,
    pub seconds: u32,
    pub visual_seconds: u32,
    pub night_start: u32,
    pub night_end: u32,
    pub day_night_notifications: bool,
    pub visual_mode: i32,
    pub timer: Timer,
}

/// The 24-hour clock.
pub struct Clock24 {
    host: Box<dyn ClockHost>,
    initialized: bool,
    pause_count: i32,
    hidden_pause_count: i32,
    magnification: f32,
    min_magnification: f32,
    max_magnification: f32,
    seconds: u32,
    visual_seconds: u32,
    timer: Timer,
    night_start: u32,
    night_end: u32,
    day_night_notifications: bool,
    visual_mode: i32,
}

impl Clock24 {
    /// The constructor's state (§2): noon, magnification 100 in [0.05, 2000],
    /// night 20 → 4, notifications on, mode 0, the timer stopped at 0.
    pub fn new(host: Box<dyn ClockHost>) -> Clock24 {
        Clock24 {
            host,
            initialized: false,
            pause_count: 0,
            hidden_pause_count: 0,
            magnification: DEFAULT_MAGNIFICATION,
            min_magnification: f32::from_bits(DEFAULT_MIN_MAGNIFICATION_BITS),
            max_magnification: DEFAULT_MAX_MAGNIFICATION,
            seconds: NOON,
            visual_seconds: NOON,
            timer: Timer::default(),
            night_start: DEFAULT_NIGHT_START,
            night_end: DEFAULT_NIGHT_END,
            day_night_notifications: true,
            visual_mode: MODE_FOLLOW,
        }
    }

    fn now(&self) -> u64 {
        self.host.now_micros()
    }

    fn post(&self, msg_type: u32, data2: u32, data3: u32) {
        self.host.post(msg_type, data2, data3);
    }

    // ---- lifecycle (§4.2) ----------------------------------------------------------------------

    /// `Init` (slot 3): the system service and the on-idle list, the cheats,
    /// the timer when neither counter is set, then the tuning (§6). Returns
    /// true; a second call does nothing.
    pub fn init(&mut self) -> bool {
        if self.initialized {
            return true;
        }
        self.host.add_system_service();
        self.host.add_to_on_idle();
        if self.host.has_cheat_manager() {
            for (id, name) in [
                (CHEAT_WHAT_TIME_IZ_IT, CHEAT_NAME_WHAT_TIME_IZ_IT),
                (CHEAT_STOPWATCH, CHEAT_NAME_STOPWATCH),
            ] {
                self.host.register_cheat(id, name);
                self.host.add_cheat_notification(id);
            }
        }
        if self.pause_count == 0 && self.hidden_pause_count == 0 {
            let now = self.now();
            self.timer.start(now);
        }
        self.read_tuning();
        self.initialized = true;
        true
    }

    /// `ReadSimulatorTunableValues` (§6): no exemplar changes nothing; a
    /// missing magnification still passes the current value through the
    /// setter (so it is clamped); missing hours keep theirs.
    fn read_tuning(&mut self) {
        let Some(t) = self.host.tuning() else {
            return;
        };
        self.set_time_magnification(t.magnification.unwrap_or(self.magnification));
        if let Some(h) = t.night_start {
            self.night_start = h;
        }
        if let Some(h) = t.night_end {
            self.night_end = h;
        }
    }

    /// `Shutdown` (slot 4): the timer reset and `seconds := 0`, the cheats
    /// unregistered (notification first), the on-idle entry and the system
    /// service removed. Returns true; does nothing when not initialised.
    pub fn shutdown(&mut self) -> bool {
        if !self.initialized {
            return true;
        }
        self.timer.reset();
        self.seconds = 0;
        // [choice] Only with a cheat code manager (there is nothing to call
        // without one).
        if self.host.has_cheat_manager() {
            for id in [CHEAT_WHAT_TIME_IZ_IT, CHEAT_STOPWATCH] {
                self.host.remove_cheat_notification(id);
                self.host.unregister_cheat(id);
            }
        }
        self.host.remove_from_on_idle();
        self.host.remove_system_service();
        self.initialized = false;
        true
    }

    // ---- the step (§4.3) --------------------------------------------------------------------------

    /// `OnIdle` (system-service slot 11). Returns true.
    ///
    /// `step = trunc(f32(f32(e × magnification) × 9.99999997e-07f))`:
    /// [`mul_u32_f32`], one `f32` multiplication, `x87::ftol32` as `u32`.
    pub fn on_idle(&mut self) -> bool {
        if self.pause_count > 0 || self.hidden_pause_count > 0 {
            return true;
        }
        let now = if self.timer.running { self.now() } else { 0 };
        let e = self.timer.elapsed(now);
        let product = mul_u32_f32(e, self.magnification);
        let mut step = ftol32(f64::from(product * f32::from_bits(STEP_FACTOR_BITS))) as u32;
        if step <= STEP_THRESHOLD {
            return true;
        }
        if step > MAX_STEP {
            step = MAX_STEP;
        }
        let old_hour = self.hour();
        let old_minute = self.minute();
        self.seconds = self.seconds.wrapping_add(step) % SECONDS_PER_DAY;
        let new_hour = self.hour();
        if new_hour != old_hour {
            self.post(msg::HOUR_CHANGED, new_hour, 0);
            self.post(msg::TIME_TICK, new_hour, 0);
        } else if self.minute() / 5 != old_minute / 5 {
            self.post(msg::TIME_TICK, new_hour, self.minute());
        }
        if self.visual_mode == MODE_FOLLOW {
            self.change_visual_time(self.seconds);
        }
        let now = self.now();
        self.timer.restart(now);
        true
    }

    // ---- visual time (§4.4, §4.5) -----------------------------------------------------------------

    /// `ChangeVisualTime(v)` (§4.4).
    pub fn change_visual_time(&mut self, v: u32) {
        if self.visual_seconds == v {
            return;
        }
        let old_h = self.visual_hour();
        let old_m = self.visual_minute();
        self.visual_seconds = v;
        let new_h = self.visual_hour();
        if new_h != old_h {
            self.post(msg::VISUAL_HOUR_CHANGED, new_h, 0);
            self.post(msg::VISUAL_TIME_TICK, new_h, 0);
            if self.day_night_notifications && self.is_night_hour(old_h) != self.is_night_hour(new_h) {
                self.post(msg::DAY_NIGHT_CHANGED, u32::from(!self.is_night_hour(new_h)), 0);
            }
        } else if self.visual_minute() / 5 != old_m / 5 {
            self.post(msg::VISUAL_TIME_TICK, new_h, self.visual_minute());
        }
    }

    /// `SetVisualTimeMode` (slot 5, §4.5).
    pub fn set_visual_time_mode(&mut self, mode: i32) {
        if mode == self.visual_mode {
            return;
        }
        self.visual_mode = mode;
        match mode {
            MODE_FOLLOW => self.change_visual_time(self.seconds),
            MODE_NOON => self.change_visual_time(NOON),
            MODE_MIDNIGHT => self.change_visual_time(0),
            _ => {}
        }
    }

    /// `GetVisualTimeMode` (slot 6).
    pub fn visual_time_mode(&self) -> i32 {
        self.visual_mode
    }

    /// `EnableDayNightTransitionNotifications` (slot 29, §4.5): switching on
    /// reports the current state.
    pub fn enable_day_night_transition_notifications(&mut self, on: bool) {
        if on == self.day_night_notifications {
            return;
        }
        self.day_night_notifications = on;
        if on {
            self.post(msg::DAY_NIGHT_CHANGED, u32::from(!self.is_night()), 0);
        }
    }

    // ---- derived values (§4.1) --------------------------------------------------------------------

    /// `GetTimePercentage` (slot 7): `f32(seconds) × 1.1574074e-05f`
    /// ([`mul_u32_f32`]).
    pub fn time_percentage(&self) -> f32 {
        mul_u32_f32(self.seconds, f32::from_bits(PERCENT_FACTOR_BITS))
    }

    /// `GetVisualTimePercentage` (slot 9).
    pub fn visual_time_percentage(&self) -> f32 {
        mul_u32_f32(self.visual_seconds, f32::from_bits(PERCENT_FACTOR_BITS))
    }

    /// `GetHour` (slot 10).
    pub fn hour(&self) -> u32 {
        hour_of(self.seconds)
    }

    /// `GetVisualHour` (slot 12).
    pub fn visual_hour(&self) -> u32 {
        hour_of(self.visual_seconds)
    }

    /// `GetMinute` (slot 13).
    pub fn minute(&self) -> u32 {
        minute_of(self.seconds)
    }

    /// `GetVisualMinute` (slot 15).
    pub fn visual_minute(&self) -> u32 {
        minute_of(self.visual_seconds)
    }

    /// `GetSeconds` (slot 16): the second within the minute.
    pub fn second(&self) -> u32 {
        second_of(self.seconds)
    }

    /// `GetVisualSeconds` (slot 18): the visual second within the minute.
    pub fn visual_second(&self) -> u32 {
        second_of(self.visual_seconds)
    }

    /// `GetEventHour(i)` (slot 19): night start for 0, night end for 1, else 0.
    pub fn event_hour(&self, index: u32) -> u32 {
        match index {
            0 => self.night_start,
            1 => self.night_end,
            _ => 0,
        }
    }

    /// `isNightHour(h)` (§4.1), unsigned 32-bit arithmetic.
    pub fn is_night_hour(&self, h: u32) -> bool {
        let rel = h.wrapping_sub(self.night_start).wrapping_add(24) % 24;
        let len = self.night_end.wrapping_sub(self.night_start).wrapping_add(24) % 24;
        rel < len
    }

    /// `IsNight` (slot 28): the visual hour.
    pub fn is_night(&self) -> bool {
        self.is_night_hour(self.visual_hour())
    }

    // ---- setters (§4.6) ---------------------------------------------------------------------------

    /// `SetHour` (slot 11).
    pub fn set_hour(&mut self, h: u32) {
        let h = if h >= 24 { h % 24 } else { h };
        if h != self.hour() {
            self.seconds = h * 3600;
            self.post(msg::HOUR_CHANGED, h, 0);
            self.post(msg::TIME_TICK, h, 0);
        }
    }

    /// `SetMinute` (slot 14): returns the new `seconds`.
    pub fn set_minute(&mut self, m: u32) -> u32 {
        let m = if m >= 60 { m % 60 } else { m };
        self.seconds = self.hour().wrapping_mul(60).wrapping_add(m).wrapping_mul(60);
        self.seconds
    }

    /// `SetSeconds` (slot 17): returns `GetMinute()`.
    pub fn set_seconds(&mut self, s: u32) -> u32 {
        let s = if s >= 60 { s % 60 } else { s };
        self.seconds = self
            .hour()
            .wrapping_mul(60)
            .wrapping_add(self.minute())
            .wrapping_mul(60)
            .wrapping_add(s);
        self.minute()
    }

    /// `SetTimePercentage` (slot 8): `u32(trunc(f32(p × 86400.0f))) mod
    /// 86400`; one `f32` multiplication, `x87::ftol32` read as `u32` (a
    /// negative product wraps, [quirk]).
    pub fn set_time_percentage(&mut self, p: f32) {
        let t = ftol32(f64::from(p * 86_400.0f32)) as u32;
        self.seconds = t % SECONDS_PER_DAY;
    }

    /// `GetTimeMagnification` (slot 20).
    pub fn time_magnification(&self) -> f32 {
        self.magnification
    }

    /// `SetTimeMagnification` (slot 21): clamped to the minimum, then to the
    /// maximum; a NaN is stored unchanged. Returns true.
    pub fn set_time_magnification(&mut self, m: f32) -> bool {
        self.magnification = m;
        if m < self.min_magnification {
            self.magnification = self.min_magnification;
        }
        if self.magnification > self.max_magnification {
            self.magnification = self.max_magnification;
        }
        true
    }

    /// `GetTimeMagnificationMinMax` (slot 22): `(min, max)`.
    pub fn time_magnification_min_max(&self) -> (f32, f32) {
        (self.min_magnification, self.max_magnification)
    }

    /// `SetTimeMagnificationMinMax` (slot 23): `max < min` → `max := min`;
    /// the magnification is not clamped again. Returns true.
    pub fn set_time_magnification_min_max(&mut self, min: f32, max: f32) -> bool {
        self.min_magnification = min;
        self.max_magnification = max;
        if max < min {
            self.max_magnification = min;
        }
        true
    }

    // ---- pause counters (§4.7) -------------------------------------------------------------------

    /// `Pause` (slot 24). Returns true.
    pub fn pause(&mut self, on: bool) -> bool {
        let other = self.hidden_pause_count;
        self.pause_counter(on, false, other);
        true
    }

    /// `HiddenPause` (slot 26). Returns true.
    pub fn hidden_pause(&mut self, on: bool) -> bool {
        let other = self.pause_count;
        self.pause_counter(on, true, other);
        true
    }

    fn pause_counter(&mut self, on: bool, hidden: bool, other: i32) {
        let (count, message) = if hidden {
            (&mut self.hidden_pause_count, msg::HIDDEN_PAUSE_CHANGED)
        } else {
            (&mut self.pause_count, msg::PAUSE_CHANGED)
        };
        if on {
            *count = count.wrapping_add(1);
            if *count == 1 {
                if other == 0 && self.timer.running {
                    let now = self.now();
                    self.timer.stop(now);
                }
                self.host.post(message, 1, 0);
            }
        } else if *count > 0 {
            *count -= 1;
            if *count == 0 {
                if other == 0 {
                    let now = self.now();
                    self.timer.start(now);
                }
                self.host.post(message, 0, 0);
            }
        }
    }

    /// `IsPaused` (slot 25).
    pub fn is_paused(&self) -> bool {
        self.pause_count > 0
    }

    /// `IsHiddenPaused` (slot 27).
    pub fn is_hidden_paused(&self) -> bool {
        self.hidden_pause_count > 0
    }

    // ---- cheats (§4.8) ----------------------------------------------------------------------------

    /// `DoMessage`: the cheat message with the cheat ID and the typed text;
    /// every message returns true.
    pub fn do_message(&mut self, msg_type: u32, data1: u32, text: Option<&[u8]>) -> bool {
        if msg_type == msg::CHEAT {
            self.cheat(data1, text.unwrap_or(&[]));
        }
        true
    }

    /// One cheat: `WhatTimeIzIt h` or `Stopwatch pause|resume|magnify n`.
    pub fn cheat(&mut self, id: u32, text: &[u8]) {
        let argv = split_command_line(text);
        match id {
            CHEAT_WHAT_TIME_IZ_IT => {
                if let Some(a) = argv.get(1) {
                    self.set_hour(atol(a) as u32);
                }
            }
            CHEAT_STOPWATCH => {
                let Some(a) = argv.get(1) else {
                    return;
                };
                match a.to_ascii_lowercase().as_slice() {
                    b"pause" => {
                        self.pause(true);
                    }
                    b"resume" => {
                        self.pause(false);
                    }
                    b"magnify" => {
                        if let Some(n) = argv.get(2) {
                            self.set_time_magnification(atol(n) as f32);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    // ---- persistence (§7) ------------------------------------------------------------------------

    /// `Write`: version 2, 26 bytes. False only on a stream error.
    pub fn write(&self, out: &mut dyn OutStream) -> bool {
        out.put_u16(RECORD_VERSION)
            && out.put_i32(self.pause_count)
            && out.put_f32(self.magnification)
            && out.put_f32(self.max_magnification)
            && out.put_u32(self.seconds)
            && out.put_u32(self.night_start)
            && out.put_u32(self.night_end)
            && out.error() == 0
    }

    /// `Read`: no version check (a version of 2 or more reads as 2); below 2
    /// a 10-byte timer record after `seconds` is read and discarded. Each
    /// field is stored as it is read; the first failed read stops and
    /// returns false.
    pub fn read(&mut self, inp: &mut dyn InStream) -> bool {
        self.read_fields(inp).is_some() && inp.error() == 0
    }

    fn read_fields(&mut self, inp: &mut dyn InStream) -> Option<()> {
        let version = inp.get_u16()?;
        self.pause_count = inp.get_i32()?;
        self.magnification = inp.get_f32()?;
        self.max_magnification = inp.get_f32()?;
        self.seconds = inp.get_u32()?;
        if version < 2 {
            // A temporary timer: units, elapsed, running.
            inp.get_u8()?;
            inp.get_u64()?;
            inp.get_u8()?;
        }
        self.night_start = inp.get_u32()?;
        self.night_end = inp.get_u32()?;
        Some(())
    }

    // ---- inspection and test support -------------------------------------------------------------

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// The game time of day, `seconds` (§2).
    pub fn time_of_day(&self) -> u32 {
        self.seconds
    }

    /// The visual time of day, `visualSeconds` (§2).
    pub fn visual_time_of_day(&self) -> u32 {
        self.visual_seconds
    }

    pub fn timer(&self) -> Timer {
        self.timer
    }

    /// The timer's elapsed microseconds now.
    pub fn timer_elapsed_us(&self) -> u64 {
        let now = if self.timer.running { self.now() } else { 0 };
        self.timer.elapsed_us(now)
    }

    pub fn state(&self) -> ClockState {
        ClockState {
            initialized: self.initialized,
            pause_count: self.pause_count,
            hidden_pause_count: self.hidden_pause_count,
            magnification: self.magnification,
            min_magnification: self.min_magnification,
            max_magnification: self.max_magnification,
            seconds: self.seconds,
            visual_seconds: self.visual_seconds,
            night_start: self.night_start,
            night_end: self.night_end,
            day_night_notifications: self.day_night_notifications,
            visual_mode: self.visual_mode,
            timer: self.timer,
        }
    }

    /// Test support: sets the two times directly (no messages).
    pub fn set_times(&mut self, seconds: u32, visual_seconds: u32) {
        self.seconds = seconds;
        self.visual_seconds = visual_seconds;
    }

    /// Test support: sets the night hours directly.
    pub fn set_night_hours(&mut self, start: u32, end: u32) {
        self.night_start = start;
        self.night_end = end;
    }
}
