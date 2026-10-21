// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Dates and times (`docs/specs/dates.md`): the `cRZDate` calendar (§1), the
//! `cRZTime` wall-clock instant (§2, in [`time`]) and the reference-counted
//! `cGZDate` / `cIGZDate` COM date (§3, §4, in [`com`]).
//!
//! All arithmetic is unsigned 32-bit and wraps modulo 2^32 exactly as §1 says;
//! nothing here can panic on any input. The host's clock and C-runtime time-zone
//! variables are typed inputs ([`time::TimeZone`], [`time::LocalTm`],
//! [`time::WallClock`]); no rule of the current OS is assumed.
//!
//! [`crate::julian`] is the simulator's signed helper and is unrelated.

pub mod com;
pub mod time;

#[cfg(test)]
mod tests;

pub use com::{DateSink, GzDate, ReleaseResult, IID_IGZDATE, IID_IGZUNKNOWN};
pub use time::{LocalTm, RzTime, TimeZone, WallClock};

/// Day number of 1901-01-01, the epoch of `cRZTime` and of `Set(dayOfYear, 0)`.
pub const DAY_1901_01_01: u32 = 2_415_386;
/// Day number of 2000-01-01.
pub const DAY_2000_01_01: u32 = 2_451_545;
/// The constant of the `Jday` / `Mdy` formulas (§1.2, §1.3).
const JDAY_BASE: u32 = 1_721_119;

/// The longest text [`date_string`] can produce: three 10-digit fields and two
/// separators (the original writes into a caller buffer; ours is a `String`).
pub const DATE_STRING_MAX_LEN: usize = 32;

const MONTH_LENGTHS: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
const FIRST_DAY: [u32; 12] = [1, 32, 60, 91, 121, 152, 182, 213, 244, 274, 305, 335];
const MONTH_NAMES: [&[u8; 3]; 12] = [
    b"Jan", b"Feb", b"Mar", b"Apr", b"May", b"Jun", b"Jul", b"Aug", b"Sep", b"Oct", b"Nov", b"Dec",
];

/// `LeapYear(y)` (§1.1): the Gregorian rule; year 0 is leap.
pub fn leap_year(y: u32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// `DaysInYear(y)` (§1.4): 366 or 365.
pub fn days_in_year(y: u32) -> u32 {
    if leap_year(y) {
        366
    } else {
        365
    }
}

/// `DayWithinMonth(m, d, y)` (§1.1).
pub fn day_within_month(m: u32, d: u32, y: u32) -> bool {
    if !(1..=12).contains(&m) || d == 0 {
        return false;
    }
    let len = MONTH_LENGTHS[(m - 1) as usize] + u32::from(m == 2 && leap_year(y));
    d <= len
}

/// `Jday(m, d, y)` (§1.2): the day number, 0 for an invalid date. January and
/// February of year 0 wrap to a meaningless number *[quirk]*.
pub fn jday(m: u32, d: u32, y: u32) -> u32 {
    if !day_within_month(m, d, y) {
        return 0;
    }
    let (mp, yp) = if m > 2 { (m - 3, y) } else { (m + 9, y.wrapping_sub(1)) };
    (146_097u32.wrapping_mul(yp / 100) / 4)
        .wrapping_add(1461u32.wrapping_mul(yp % 100) / 4)
        .wrapping_add((153 * mp + 2) / 5)
        .wrapping_add(d)
        .wrapping_add(JDAY_BASE)
}

/// `Mdy(n)` (§1.3): `(month, day, year)` in the same unsigned arithmetic.
/// `Mdy(0)` is (2, 28, 2935093) *[quirk]*.
pub fn mdy(n: u32) -> (u32, u32, u32) {
    let t = n.wrapping_sub(JDAY_BASE).wrapping_mul(4);
    let c = t.wrapping_sub(1) / 146_097;
    let u = (t.wrapping_sub(146_097u32.wrapping_mul(c)).wrapping_sub(1) / 4).wrapping_mul(4);
    let j = u.wrapping_add(3) / 1461;
    let v = (u.wrapping_sub(1461u32.wrapping_mul(j)).wrapping_add(7) / 4).wrapping_mul(5);
    let m = v.wrapping_sub(3) / 153;
    let d = v.wrapping_sub(153u32.wrapping_mul(m)).wrapping_add(2) / 5;
    let y = 100u32.wrapping_mul(c).wrapping_add(j);
    if m < 10 {
        (m + 3, d, y)
    } else {
        (m.wrapping_sub(9), d, y.wrapping_add(1))
    }
}

/// `WeekDay()` of a day number (§1.4): 1 = Monday ... 7 = Sunday.
pub fn weekday(n: u32) -> u32 {
    (n.wrapping_add(1) % 7 + 6) % 7 + 1
}

/// The month of a 3-character name (§1.5): each of `Jan` ... `Dec` is compared
/// with the first three characters ignoring case, the **last** match wins and no
/// match gives January *[quirk]*.
///
/// `name` is the C string without its terminator. Literal reading (DT2): the
/// comparison is a C `strnicmp(name, "Jan", 3)`, so a name shorter than three
/// characters matches nothing, and case folding is ASCII only (the C locale).
pub fn month_from_name(name: &[u8]) -> u32 {
    let mut month = 1;
    for (i, want) in MONTH_NAMES.iter().enumerate() {
        let hit = name.len() >= 3
            && name[..3]
                .iter()
                .zip(want.iter())
                .all(|(a, b)| a.eq_ignore_ascii_case(b));
        if hit {
            month = i as u32 + 1;
        }
    }
    month
}

/// `DateString(out, format)` (§1.6) of a day number. `%4u` pads the year with
/// spaces to four characters; longer years are written in full.
pub fn date_string(n: u32, format: u32) -> String {
    let (m, d, y) = mdy(n);
    match format {
        0 | 1 | 0xFFFF_FFFF => format!("{m}/{d}/{y:>4}"),
        3 | 4 | 10 | 11 => format!("{d}.{m}.{y:>4}"),
        7 | 8 | 14 => format!("{d}-{m}-{y:>4}"),
        9 => format!("{y:>4}.{m}.{d}"),
        15 => format!("{y:>4}/{m}/{d}"),
        _ => format!("{d}/{m}/{y:>4}"),
    }
}

/// A `cRZDate`: one unsigned day number, 0 = "no date" (§0, §1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RzDate {
    pub n: u32,
}

impl RzDate {
    /// The static 1901-01-01 date (`Set(0, 0)`, §1.5).
    pub const EPOCH_1901: RzDate = RzDate { n: DAY_1901_01_01 };

    /// From a day number, unchecked.
    pub const fn from_day(n: u32) -> RzDate {
        RzDate { n }
    }

    /// `cRZDate(m, d, y)`.
    pub fn from_mdy(m: u32, d: u32, y: u32) -> RzDate {
        RzDate { n: jday(m, d, y) }
    }

    /// `cRZDate()`: today, the local date of the host's `localtime(time())`
    /// *[compat-limit]*; the host supplies the local calendar fields.
    pub fn today(local: &LocalTm) -> RzDate {
        RzDate::from_mdy(local.month, local.day, local.year)
    }

    /// `cRZDate()` read through a [`WallClock`].
    pub fn today_from(clock: &dyn WallClock) -> RzDate {
        RzDate::today(&clock.local_now())
    }

    pub fn day_number(self) -> u32 {
        self.n
    }

    pub fn is_valid(self) -> bool {
        self.n != 0
    }

    pub fn mdy(self) -> (u32, u32, u32) {
        mdy(self.n)
    }

    pub fn month(self) -> u32 {
        mdy(self.n).0
    }

    pub fn day_of_month(self) -> u32 {
        mdy(self.n).1
    }

    pub fn year(self) -> u32 {
        mdy(self.n).2
    }

    pub fn weekday(self) -> u32 {
        weekday(self.n)
    }

    /// `n - Jday(12, 31, Year() - 1)`: 1 for 1 January.
    pub fn day_of_year(self) -> u32 {
        self.n.wrapping_sub(jday(12, 31, self.year().wrapping_sub(1)))
    }

    /// `FirstDayOfMonth(m)`: day of year of the 1st of `m` in this date's year;
    /// 0 for `m` outside 1-12.
    pub fn first_day_of_month(self, m: u32) -> u32 {
        if !(1..=12).contains(&m) {
            return 0;
        }
        FIRST_DAY[(m - 1) as usize] + u32::from(m > 2 && leap_year(self.year()))
    }

    /// `Leap()`: `LeapYear(Year())`.
    pub fn leap(self) -> bool {
        leap_year(self.year())
    }

    /// `Previous(w)`: the latest date on or before this one on weekday `w`;
    /// any `w` follows the same unsigned formula (§1.4).
    pub fn previous(self, w: u32) -> RzDate {
        let a = self.weekday() - 1;
        let b = w.wrapping_sub(1);
        let back = if a >= b {
            a - b
        } else {
            a.wrapping_add(7).wrapping_sub(b)
        };
        RzDate {
            n: self.n.wrapping_sub(back),
        }
    }

    /// `CompareTo(other)`: -1, 0 or 1 by unsigned day number.
    pub fn compare_to(self, other: u32) -> i32 {
        match self.n.cmp(&other) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }
    }

    /// `Between(a, b)`: inclusive, either order.
    pub fn between(self, a: u32, b: u32) -> bool {
        a.min(b) <= self.n && self.n <= a.max(b)
    }

    /// `Max(other)`: the later; this date on a tie.
    pub fn max_with(self, other: u32) -> u32 {
        if other > self.n {
            other
        } else {
            self.n
        }
    }

    /// `Min(other)`: the earlier; this date on a tie.
    pub fn min_with(self, other: u32) -> u32 {
        if other < self.n {
            other
        } else {
            self.n
        }
    }

    /// `Set(m, d, y)`: `Jday`, 0 when invalid. Returns the new day number.
    pub fn set_mdy(&mut self, m: u32, d: u32, y: u32) -> u32 {
        self.n = jday(m, d, y);
        self.n
    }

    /// `Set(dayOfYear, y)`: year 0 counts from 1901-01-01; no range check.
    pub fn set_day_of_year(&mut self, day_of_year: u32, y: u32) -> u32 {
        self.n = if y == 0 {
            DAY_1901_01_01.wrapping_add(day_of_year)
        } else {
            jday(12, 31, y - 1).wrapping_add(day_of_year)
        };
        self.n
    }

    /// `Set(monthName, d, y)` (see [`month_from_name`]).
    pub fn set_month_name(&mut self, name: &[u8], d: u32, y: u32) -> u32 {
        self.set_mdy(month_from_name(name), d, y)
    }

    /// `Set(n)`, unchecked.
    pub fn set_day(&mut self, n: u32) -> u32 {
        self.n = n;
        self.n
    }

    /// `Set(time)`: 0 for time 0, else the local date of the instant.
    pub fn set_time(&mut self, t: RzTime, tz: TimeZone) -> u32 {
        self.n = if t.secs == 0 {
            0
        } else {
            (t.local_secs(tz) / 86_400).wrapping_add(DAY_1901_01_01)
        };
        self.n
    }

    /// `DateString(out, format)`.
    pub fn date_string(self, format: u32) -> String {
        date_string(self.n, format)
    }

    /// Postfix `++` of the cGZDate own vtable: returns the old value.
    pub fn post_inc(&mut self) -> RzDate {
        let old = *self;
        self.n = self.n.wrapping_add(1);
        old
    }

    /// Postfix `--`: returns the old value.
    pub fn post_dec(&mut self) -> RzDate {
        let old = *self;
        self.n = self.n.wrapping_sub(1);
        old
    }
}
