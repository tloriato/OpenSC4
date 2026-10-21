// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! `cGZDate` (`docs/specs/dates.md` §3) and its 46-slot `cIGZDate` (§4) as a
//! value model: a day number and a reference count. Every slot has one method,
//! named after the slot, returning what the original returns in EAX / AL
//! (§3.2: the `Set` overloads and `Copy` return the day number, slots 33-36
//! a day number, slots 37-40 "this", modelled as `()` here).
//!
//! Other dates are passed as their `DayNumber()`; an out date is any
//! [`DateSink`], whose `copy` models `out.Copy(date)` (slot 8).

use super::{date_string, day_within_month, days_in_year, jday, leap_year, RzDate, RzTime, TimeZone};

/// `cIGZUnknown`'s IID, which `QueryInterface` also answers (§3.1).
pub const IID_IGZUNKNOWN: u32 = 1;
/// `cIGZDate`'s IID.
pub const IID_IGZDATE: u32 = 0x4337_8255;

/// The result of `Release` (slot 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReleaseResult {
    /// EAX: the new count, 0 when freed.
    pub count: u32,
    /// The deleting destructor ran; the owner must drop the object.
    pub freed: bool,
}

/// An out date of `MaxDate` / `MinDate` / `Previous`: `copy(n)` is
/// `out.Copy(date)` for a date whose `DayNumber()` is `n`, returning what that
/// `Copy` returns.
pub trait DateSink {
    fn copy(&mut self, n: u32) -> u32;
}

/// A `cGZDate`: 16 bytes in the original (own vtable, day number, `cIGZDate`
/// vtable, reference count 0 at construction).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GzDate {
    pub date: RzDate,
    pub refs: u32,
}

impl DateSink for GzDate {
    fn copy(&mut self, n: u32) -> u32 {
        self.set_day(n)
    }
}

impl GzDate {
    /// From a day number (also the `cRZDate`, `cIGZDate` and copy
    /// constructors, which all take the other's day number); count 0.
    pub const fn from_day(n: u32) -> GzDate {
        GzDate {
            date: RzDate { n },
            refs: 0,
        }
    }

    /// `cGZDate(m, d, y)`.
    pub fn from_mdy(m: u32, d: u32, y: u32) -> GzDate {
        GzDate::from_day(jday(m, d, y))
    }

    /// `cGZDate()`: today (see [`RzDate::today`]).
    pub fn today(local: &super::LocalTm) -> GzDate {
        GzDate::from_day(RzDate::today(local).n)
    }

    fn n(&self) -> u32 {
        self.date.n
    }

    // --- slots 0-2: cIGZUnknown -------------------------------------------

    /// Slot 0: IID 1 or `0x43378255` AddRefs and answers true; anything else
    /// answers false (the out pointer is the glue's business).
    pub fn query_interface(&mut self, iid: u32) -> bool {
        if iid == IID_IGZUNKNOWN || iid == IID_IGZDATE {
            self.add_ref();
            true
        } else {
            false
        }
    }

    /// Slot 1: increments the count and returns it (a plain 32-bit counter,
    /// wrapping like the original's).
    pub fn add_ref(&mut self) -> u32 {
        self.refs = self.refs.wrapping_add(1);
        self.refs
    }

    /// Slot 2: a count below 2 frees the object and returns 0 *[quirk]*;
    /// otherwise decrements and returns the new count.
    pub fn release(&mut self) -> ReleaseResult {
        if self.refs < 2 {
            self.refs = 0;
            ReleaseResult { count: 0, freed: true }
        } else {
            self.refs -= 1;
            ReleaseResult {
                count: self.refs,
                freed: false,
            }
        }
    }

    // --- slots 3-8: setters (EAX = the new day number) ----------------------

    /// Slot 3: `Set(monthName, d, y)`; `name` without its terminator.
    pub fn set_month_name(&mut self, name: &[u8], d: u32, y: u32) -> u32 {
        self.date.set_month_name(name, d, y)
    }

    /// Slot 4: `Set(dayNumber)`, unchecked.
    pub fn set_day(&mut self, n: u32) -> u32 {
        self.date.set_day(n)
    }

    /// Slot 5: `Set(cRZTime const&)`.
    pub fn set_time(&mut self, t: RzTime, tz: TimeZone) -> u32 {
        self.date.set_time(t, tz)
    }

    /// Slot 6: `Set(m, d, y)`.
    pub fn set_mdy(&mut self, m: u32, d: u32, y: u32) -> u32 {
        self.date.set_mdy(m, d, y)
    }

    /// Slot 7: `Set(dayOfYear, y)`.
    pub fn set_day_of_year(&mut self, day_of_year: u32, y: u32) -> u32 {
        self.date.set_day_of_year(day_of_year, y)
    }

    /// Slot 8: `Copy(from)`: slot 4 with `from.DayNumber()`.
    pub fn copy_from(&mut self, from: u32) -> u32 {
        self.set_day(from)
    }

    /// Slot 9: `Clone(out)`: a new date with this day number, AddRef'd (count
    /// 1). The SDK's `bool` result is unspecified (DT3); the glue answers true.
    pub fn clone_date(&self) -> GzDate {
        GzDate {
            date: self.date,
            refs: 1,
        }
    }

    // --- slots 10-26: queries ----------------------------------------------

    pub fn between(&self, a: u32, b: u32) -> bool {
        self.date.between(a, b)
    }
    pub fn compare_to(&self, other: u32) -> i32 {
        self.date.compare_to(other)
    }
    pub fn week_day(&self) -> u32 {
        self.date.weekday()
    }
    pub fn year(&self) -> u32 {
        self.date.year()
    }
    pub fn day_of_year(&self) -> u32 {
        self.date.day_of_year()
    }
    pub fn day_of_month(&self) -> u32 {
        self.date.day_of_month()
    }
    /// Slot 16.
    pub fn first_day_of_month_of(&self, month: u32) -> u32 {
        self.date.first_day_of_month(month)
    }
    /// Slot 17: for `Month()`.
    pub fn first_day_of_month(&self) -> u32 {
        self.date.first_day_of_month(self.date.month())
    }
    /// Slot 18 (the same body as slot 26).
    pub fn hash(&self) -> u32 {
        self.n()
    }
    pub fn is_valid(&self) -> bool {
        self.date.is_valid()
    }
    pub fn leap(&self) -> bool {
        self.date.leap()
    }
    /// Slot 21: `out.Copy(max(this, other))`.
    pub fn max_date(&self, other: u32, out: &mut dyn DateSink) -> u32 {
        out.copy(self.date.max_with(other))
    }
    /// Slot 22: `out.Copy(min(this, other))`.
    pub fn min_date(&self, other: u32, out: &mut dyn DateSink) -> u32 {
        out.copy(self.date.min_with(other))
    }
    pub fn month(&self) -> u32 {
        self.date.month()
    }
    /// Slot 24: `out.Copy(Previous(weekday))`.
    pub fn previous(&self, weekday: u32, out: &mut dyn DateSink) -> u32 {
        out.copy(self.date.previous(weekday).n)
    }
    pub fn year_last_two_digits(&self) -> u32 {
        self.year() % 100
    }
    pub fn day_number(&self) -> u32 {
        self.n()
    }

    // --- slots 27-32: comparisons (unsigned) --------------------------------

    pub fn lt(&self, other: u32) -> bool {
        self.n() < other
    }
    pub fn le(&self, other: u32) -> bool {
        self.n() <= other
    }
    pub fn gt(&self, other: u32) -> bool {
        self.n() > other
    }
    pub fn ge(&self, other: u32) -> bool {
        self.n() >= other
    }
    pub fn eq_date(&self, other: u32) -> bool {
        self.n() == other
    }
    pub fn ne_date(&self, other: u32) -> bool {
        self.n() != other
    }

    // --- slots 33-36: day-number arithmetic, the date unchanged -------------

    /// Slot 33: `n - days`.
    pub fn sub_days(&self, days: i32) -> u32 {
        self.n().wrapping_sub(days as u32)
    }
    /// Slot 34: `n - other.n`.
    pub fn sub_date(&self, other: u32) -> u32 {
        self.n().wrapping_sub(other)
    }
    /// Slot 35: `n + days`.
    pub fn add_days(&self, days: i32) -> u32 {
        self.n().wrapping_add(days as u32)
    }
    /// Slot 36: `n + other.n`.
    pub fn add_date(&self, other: u32) -> u32 {
        self.n().wrapping_add(other)
    }

    // --- slots 37-40: in place (EAX = this) ---------------------------------

    pub fn inc(&mut self) {
        self.date.n = self.n().wrapping_add(1);
    }
    pub fn dec(&mut self) {
        self.date.n = self.n().wrapping_sub(1);
    }
    pub fn add_assign_days(&mut self, days: i32) {
        self.date.n = self.add_days(days);
    }
    pub fn sub_assign_days(&mut self, days: i32) {
        self.date.n = self.sub_days(days);
    }

    // --- slots 41-45 --------------------------------------------------------

    /// Slot 41: the text copied into the `cIGZString`. The SDK's `bool` result
    /// is unspecified (DT3).
    pub fn date_string(&self, format: u32) -> String {
        date_string(self.n(), format)
    }
    pub fn day_within_month(&self, m: u32, d: u32, y: u32) -> bool {
        day_within_month(m, d, y)
    }
    pub fn days_in_year(&self, y: u32) -> u32 {
        days_in_year(y)
    }
    pub fn jday(&self, m: u32, d: u32, y: u32) -> u32 {
        jday(m, d, y)
    }
    /// Slot 45: EAX 0 / 1.
    pub fn leap_year(&self, y: u32) -> u32 {
        u32::from(leap_year(y))
    }
}
