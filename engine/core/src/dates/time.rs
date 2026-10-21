// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! `cRZTime` (`docs/specs/dates.md` §2, crime.md §9): an unsigned count of
//! seconds since 1901-01-01 00:00 UTC, 0 = "no time". The C runtime's
//! `_timezone` / `_daylight` and the host's local calendar fields are typed
//! inputs; the daylight rule is the library's own fixed pre-2007 US rule (§2.2),
//! never the host's.

use super::{jday, mdy, RzDate, DAY_1901_01_01};

/// The C runtime's time-zone variables, supplied by the host.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TimeZone {
    /// `_timezone`: seconds **west** of UTC (US Central is 21600).
    pub timezone: i32,
    /// `_daylight`: non-zero when the zone has daylight time.
    pub daylight: i32,
}

impl TimeZone {
    /// UTC without daylight time.
    pub const UTC: TimeZone = TimeZone {
        timezone: 0,
        daylight: 0,
    };
}

/// The fields of the host's `localtime(time())`, normalised: `month` 1-12,
/// `day` 1-31, the full `year`, `hour` 0-23, `minute` / `second` 0-59 (60 for a
/// leap second, which simply adds a second).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LocalTm {
    pub year: u32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

/// The host's wall clock (*[compat-limit]*: only the rule is matched).
pub trait WallClock {
    /// `localtime(time())`.
    fn local_now(&self) -> LocalTm;
    /// `_timezone` and `_daylight`.
    fn time_zone(&self) -> TimeZone;
}

const HOUR: u32 = 3600;
const DAY: u32 = 86_400;

/// Seconds after 1901-01-01 00:00 of `day`'s local midnight plus `secs`, then
/// `+ _timezone` (all wrapping).
fn instant(day: u32, secs: u32, tz: TimeZone) -> u32 {
    day.wrapping_sub(DAY_1901_01_01)
        .wrapping_mul(DAY)
        .wrapping_add(secs)
        .wrapping_add_signed(tz.timezone)
}

/// `BeginDST(Y)` (§2.2): 1974 6 January, 1975 23 February, otherwise the Sunday
/// on or before 8 April; 02:00 local standard time, then `+ _timezone`.
pub fn begin_dst(y: u32, tz: TimeZone) -> u32 {
    let day = match y {
        1974 => jday(1, 6, 1974),
        1975 => jday(2, 23, 1975),
        _ => RzDate::from_day(jday(4, 8, y)).previous(7).n,
    };
    instant(day, 2 * HOUR, tz)
}

/// `EndDST(Y)` (§2.2): the Sunday on or before 31 October, 01:00 local standard
/// time, then `+ _timezone`.
pub fn end_dst(y: u32, tz: TimeZone) -> u32 {
    instant(RzDate::from_day(jday(10, 31, y)).previous(7).n, HOUR, tz)
}

/// `IsDST(t)` (§2.2): false when `_daylight` is 0; otherwise
/// `BeginDST(Y) <= t < EndDST(Y)` (unsigned) with `Y` the year of day
/// `t / 86400` after 1901-01-01.
pub fn is_dst(t: u32, tz: TimeZone) -> bool {
    if tz.daylight == 0 {
        return false;
    }
    let y = mdy(DAY_1901_01_01.wrapping_add(t / DAY)).2;
    begin_dst(y, tz) <= t && t < end_dst(y, tz)
}

/// A `cRZTime`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RzTime {
    pub secs: u32,
}

impl RzTime {
    pub const fn from_secs(secs: u32) -> RzTime {
        RzTime { secs }
    }

    /// `cRZTime(date, h, m, s)` (§2.1): the UTC instant of a local wall-clock
    /// time; date 0 gives 0, and a time inside the spring-forward hour gives 0
    /// *[quirk]*.
    pub fn from_local(date: u32, h: u32, m: u32, s: u32, tz: TimeZone) -> RzTime {
        if date == 0 {
            return RzTime { secs: 0 };
        }
        let mut t = date
            .wrapping_sub(DAY_1901_01_01)
            .wrapping_mul(24)
            .wrapping_add(h)
            .wrapping_sub(1)
            .wrapping_mul(HOUR)
            .wrapping_add(m.wrapping_mul(60))
            .wrapping_add(s);
        if t != 0 {
            t = t.wrapping_add_signed(tz.timezone);
        }
        let later = t.wrapping_add(HOUR);
        let secs = match (is_dst(t, tz), is_dst(later, tz)) {
            (true, true) => t,
            (_, false) => later,
            (false, true) => 0,
        };
        RzTime { secs }
    }

    /// `cRZTime()` (crime.md §9): "now", rebuilt from the local calendar fields
    /// through [`RzTime::from_local`].
    pub fn now(local: &LocalTm, tz: TimeZone) -> RzTime {
        let date = jday(local.month, local.day, local.year);
        RzTime::from_local(date, local.hour, local.minute, local.second, tz)
    }

    /// `cRZTime()` read through a [`WallClock`].
    pub fn now_from(clock: &dyn WallClock) -> RzTime {
        RzTime::now(&clock.local_now(), clock.time_zone())
    }

    pub fn is_dst(self, tz: TimeZone) -> bool {
        is_dst(self.secs, tz)
    }

    /// `LocalSecs()` (§2.3): `t - _timezone`, plus 3600 when `IsDST(t -
    /// _timezone)` *[quirk]*.
    pub fn local_secs(self, tz: TimeZone) -> u32 {
        let u = self.secs.wrapping_add_signed(tz.timezone.wrapping_neg());
        if is_dst(u, tz) {
            u.wrapping_add(HOUR)
        } else {
            u
        }
    }

    /// `Hour()`: `(LocalSecs() mod 86400) / 3600`.
    pub fn hour(self, tz: TimeZone) -> u32 {
        (self.local_secs(tz) % DAY) / HOUR
    }

    /// `Minute()`: `(LocalSecs() mod 3600) / 60`.
    pub fn minute(self, tz: TimeZone) -> u32 {
        (self.local_secs(tz) % HOUR) / 60
    }

    /// The local date of the instant (`cRZDate::Set(time)`).
    pub fn date(self, tz: TimeZone) -> RzDate {
        let mut d = RzDate::default();
        d.set_time(self, tz);
        d
    }
}
