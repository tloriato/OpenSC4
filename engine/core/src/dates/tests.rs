// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! `docs/specs/dates.md` §8 conformance tests and edge cases.

use super::time::{begin_dst, end_dst, is_dst};
use super::*;

const CENTRAL: TimeZone = TimeZone {
    timezone: 21_600,
    daylight: 1,
};
const CENTRAL_NO_DST: TimeZone = TimeZone {
    timezone: 21_600,
    daylight: 0,
};

#[test]
fn t1_day_numbers() {
    assert_eq!(jday(1, 1, 2000), 2_451_545);
    assert_eq!(jday(1, 1, 1901), 2_415_386);
    assert_eq!(jday(2, 29, 2000), 2_451_604);
    assert_eq!(jday(12, 31, 1999), 2_451_544);
    assert_eq!(jday(3, 1, 2000), 2_451_605);
    assert_eq!(jday(10, 15, 1582), 2_299_161);
    assert_eq!(jday(1, 1, 2100), 2_488_070);
    assert_eq!(jday(3, 1, 0), 1_721_120);
    assert_eq!(jday(1, 1, 0), 1_043_250_630);
}

#[test]
fn t2_invalid_dates_give_zero() {
    assert_eq!(jday(2, 29, 1900), 0);
    assert_eq!(jday(13, 1, 2000), 0);
    assert_eq!(jday(0, 1, 2000), 0);
    assert_eq!(jday(2, 0, 2000), 0);
    assert_eq!(jday(4, 31, 2000), 0);
    assert_eq!(jday(u32::MAX, u32::MAX, u32::MAX), 0);
    assert!(day_within_month(2, 29, 2000));
    assert!(!day_within_month(2, 30, 2000));
}

#[test]
fn t3_round_trip_and_fields() {
    let d = RzDate::from_day(2_451_604);
    assert_eq!(d.mdy(), (2, 29, 2000));
    assert_eq!(d.weekday(), 2);
    assert_eq!(d.day_of_year(), 60);
    let d = RzDate::from_day(2_451_545);
    assert_eq!(d.weekday(), 6);
    assert_eq!(d.day_of_year(), 1);
    assert_eq!(RzDate::from_day(2_451_544).day_of_year(), 365);
    assert_eq!(mdy(0), (2, 28, 2_935_093));
    assert_eq!(weekday(0), 1);
}

#[test]
fn round_trip_every_day_years_1_to_3000() {
    let mut n = jday(1, 1, 1);
    for y in 1..=3000u32 {
        for m in 1..=12 {
            for d in 1..=31 {
                if !day_within_month(m, d, y) {
                    continue;
                }
                assert_eq!(jday(m, d, y), n, "{y}-{m}-{d}");
                assert_eq!(mdy(n), (m, d, y));
                n += 1;
            }
        }
    }
}

#[test]
fn wrapping_edges_never_panic() {
    for n in [0, 1, 1_721_118, 1_721_119, 1_721_120, u32::MAX - 1, u32::MAX] {
        let d = RzDate::from_day(n);
        let _ = (d.mdy(), d.weekday(), d.day_of_year(), d.leap(), d.date_string(0));
        let _ = d.first_day_of_month(2);
        for w in [0, 1, 7, 8, u32::MAX] {
            let _ = d.previous(w);
        }
    }
    // (2^32 - 1) + 1 wraps to 0: WeekDay(2^32 - 1) = WeekDay(-1 + 1) = (0 + 6) mod 7 + 1.
    assert_eq!(weekday(u32::MAX), 7);
    let mut g = GzDate::from_day(0);
    g.dec();
    assert_eq!(g.day_number(), u32::MAX);
    g.inc();
    assert_eq!(g.day_number(), 0);
    let _ = RzTime::from_local(
        u32::MAX,
        u32::MAX,
        u32::MAX,
        u32::MAX,
        TimeZone {
            timezone: i32::MIN,
            daylight: 1,
        },
    );
    let _ = RzTime::from_secs(u32::MAX).hour(TimeZone {
        timezone: i32::MAX,
        daylight: 1,
    });
}

#[test]
fn t4_leap_years() {
    assert!(leap_year(2000) && leap_year(2400) && leap_year(0));
    assert!(!leap_year(1900) && !leap_year(2100));
    assert_eq!(days_in_year(1999), 365);
    assert_eq!(days_in_year(2000), 366);
}

#[test]
fn t5_first_day_of_month() {
    let d2000 = RzDate::from_mdy(6, 1, 2000);
    assert_eq!(d2000.first_day_of_month(3), 61);
    assert_eq!(d2000.first_day_of_month(2), 32);
    assert_eq!(d2000.first_day_of_month(13), 0);
    assert_eq!(d2000.first_day_of_month(0), 0);
    assert_eq!(RzDate::from_mdy(6, 1, 1999).first_day_of_month(3), 60);
    assert_eq!(RzDate::from_mdy(6, 1, 1999).first_day_of_month(12), 335);
}

#[test]
fn t6_previous() {
    let d = RzDate::from_mdy(1, 1, 2000);
    assert_eq!(d.previous(7), RzDate::from_mdy(12, 26, 1999));
    assert_eq!(d.previous(6), d);
    assert_eq!(d.previous(1), RzDate::from_mdy(12, 27, 1999));
    // Outside 1-7: the same formula. a = 5; w = 0 gives b = 2^32 - 1 > a, so
    // back = a + 7 - b = 13 (mod 2^32).
    assert_eq!(d.previous(0).n, d.n - 13);
    // w = 8: b = 7 > 5, back = 5.
    assert_eq!(d.previous(8).n, d.n - 5);
}

#[test]
fn t7_set_day_of_year() {
    let mut d = RzDate::default();
    assert_eq!(d.set_day_of_year(0, 0), 2_415_386);
    assert_eq!(d.set_day_of_year(1, 0), 2_415_387);
    assert_eq!(d.set_day_of_year(60, 2000), jday(2, 29, 2000));
    assert_eq!(d.set_day_of_year(366, 1999), jday(1, 1, 2000));
    assert_eq!(RzDate::EPOCH_1901.mdy(), (1, 1, 1901));
}

#[test]
fn t8_month_names() {
    let mut d = RzDate::default();
    assert_eq!(d.set_month_name(b"march", 1, 2000), jday(3, 1, 2000));
    assert_eq!(d.set_month_name(b"MAY", 1, 2000), jday(5, 1, 2000));
    assert_eq!(d.set_month_name(b"Smarch", 1, 2000), jday(1, 1, 2000));
    assert_eq!(month_from_name(b"dec"), 12);
    assert_eq!(month_from_name(b"Ma"), 1);
    assert_eq!(month_from_name(b""), 1);
    assert_eq!(month_from_name(b"Augustus"), 8);
}

#[test]
fn t9_text() {
    let n = jday(3, 4, 2000);
    assert_eq!(date_string(n, 0), "3/4/2000");
    assert_eq!(date_string(n, 1), "3/4/2000");
    assert_eq!(date_string(n, 0xFFFF_FFFF), "3/4/2000");
    assert_eq!(date_string(n, 2), "4/3/2000");
    assert_eq!(date_string(n, 4), "4.3.2000");
    assert_eq!(date_string(n, 7), "4-3-2000");
    assert_eq!(date_string(n, 9), "2000.3.4");
    assert_eq!(date_string(n, 15), "2000/3/4");
    assert_eq!(date_string(n, 16), "4/3/2000");
    assert_eq!(date_string(jday(1, 2, 33), 0), "1/2/  33");
    assert_eq!(date_string(jday(1, 2, 33), 9), "  33.1.2");
    assert_eq!(date_string(0, 0), "2/28/2935093");
    for n in [0, u32::MAX, 1_721_119] {
        for f in [0, 3, 9] {
            assert!(date_string(n, f).len() <= DATE_STRING_MAX_LEN);
        }
    }
}

#[test]
fn t10_com_object() {
    let mut g = GzDate::from_day(0);
    assert_eq!(g.refs, 0);
    assert_eq!(g.set_mdy(1, 1, 2000), 2_451_545);
    assert_eq!(g.sub_days(1), 2_451_544);
    assert_eq!(g.day_number(), 2_451_545);
    g.inc();
    assert_eq!(g.day_number(), 2_451_546);
    let z = GzDate::from_day(0);
    assert!(!z.is_valid());
    assert_eq!(z.month(), 2);
    assert!(g.query_interface(IID_IGZDATE));
    assert_eq!(g.refs, 1);
    assert!(g.query_interface(IID_IGZUNKNOWN));
    assert!(!g.query_interface(0x1234_5678));
    assert_eq!(g.refs, 2);
}

#[test]
fn com_set_returns_day_number_low_byte_quirk() {
    // §3.2: a valid date whose day number is a multiple of 256 reads as false in AL.
    let mut g = GzDate::default();
    let n = g.set_day(2_451_456);
    assert_eq!(n % 256, 0);
    assert_eq!(g.set_mdy(13, 1, 2000), 0);
    assert_eq!(g.copy_from(5), 5);
}

#[test]
fn com_release_frees_below_two() {
    let mut g = GzDate::default();
    assert_eq!(g.release(), ReleaseResult { count: 0, freed: true });
    let mut g = GzDate::default();
    assert_eq!(g.add_ref(), 1);
    assert_eq!(g.release(), ReleaseResult { count: 0, freed: true });
    let mut g = GzDate::default();
    g.add_ref();
    g.add_ref();
    g.add_ref();
    assert_eq!(g.release(), ReleaseResult { count: 2, freed: false });
    assert_eq!(g.release(), ReleaseResult { count: 1, freed: false });
    assert_eq!(g.release(), ReleaseResult { count: 0, freed: true });
}

#[test]
fn com_all_slots() {
    let mut g = GzDate::from_mdy(3, 4, 2000);
    let n = g.day_number();
    assert_eq!(g.hash(), n);
    let c = g.clone_date();
    assert_eq!((c.day_number(), c.refs), (n, 1));
    assert!(g.between(n + 5, n - 5) && g.between(n, n) && !g.between(n + 1, n + 2));
    assert_eq!((g.compare_to(n - 1), g.compare_to(n), g.compare_to(n + 1)), (1, 0, -1));
    assert_eq!(g.compare_to(u32::MAX), -1);
    assert_eq!(g.week_day(), 6);
    assert_eq!(g.year(), 2000);
    assert_eq!(g.day_of_year(), 64);
    assert_eq!(g.day_of_month(), 4);
    assert_eq!(g.first_day_of_month(), 61);
    assert_eq!(g.first_day_of_month_of(1), 1);
    assert!(g.leap());
    assert_eq!(g.year_last_two_digits(), 0);
    let mut out = GzDate::default();
    assert_eq!(g.max_date(n + 3, &mut out), n + 3);
    assert_eq!(out.day_number(), n + 3);
    assert_eq!(g.min_date(n + 3, &mut out), n);
    assert_eq!(g.previous(7, &mut out), n - 6);
    assert!(g.lt(n + 1) && g.le(n) && g.gt(n - 1) && g.ge(n) && g.eq_date(n) && g.ne_date(0));
    assert!(!g.lt(n) && !g.gt(n));
    assert_eq!(g.sub_date(n - 10), 10);
    assert_eq!(g.add_date(10), n + 10);
    assert_eq!(g.add_days(-1), n - 1);
    assert_eq!(g.sub_days(-1), n + 1);
    g.add_assign_days(10);
    assert_eq!(g.day_number(), n + 10);
    g.sub_assign_days(10);
    g.dec();
    assert_eq!(g.day_number(), n - 1);
    g.inc();
    assert_eq!(g.date_string(0), "3/4/2000");
    assert!(g.day_within_month(2, 29, 2000));
    assert_eq!(g.days_in_year(1900), 365);
    assert_eq!(g.jday(1, 1, 2000), 2_451_545);
    assert_eq!((g.leap_year(2000), g.leap_year(1900)), (1, 0));
    assert_eq!(g.set_day_of_year(1, 0), 2_415_387);
    assert_eq!(g.set_month_name(b"oct", 1, 2000), jday(10, 1, 2000));
    // postfix on the own vtable returns the old value
    let old = g.date.post_inc();
    assert_eq!((old.n, g.day_number()), (jday(10, 1, 2000), jday(10, 2, 2000)));
    let old = g.date.post_dec();
    assert_eq!((old.n, g.day_number()), (jday(10, 2, 2000), jday(10, 1, 2000)));
    // Max / Min keep this date on a tie (observable only through the value).
    assert_eq!(g.date.max_with(g.day_number()), g.day_number());
}

fn day_of(t: u32) -> (u32, u32, u32) {
    mdy(DAY_1901_01_01 + t / 86_400)
}

#[test]
fn t11_daylight_rule() {
    // BeginDST/EndDST are local standard time + _timezone: in US Central 02:00
    // local is 08:00 UTC, still the same UTC day.
    assert_eq!(day_of(begin_dst(2000, CENTRAL)), (4, 2, 2000));
    assert_eq!(day_of(begin_dst(1996, CENTRAL)), (4, 7, 1996));
    assert_eq!(day_of(begin_dst(1990, CENTRAL)), (4, 8, 1990));
    assert_eq!(day_of(end_dst(2000, CENTRAL)), (10, 29, 2000));
    assert_eq!(day_of(end_dst(1990, CENTRAL)), (10, 28, 1990));
    assert_eq!(day_of(begin_dst(1974, CENTRAL)), (1, 6, 1974));
    assert_eq!(day_of(begin_dst(1975, CENTRAL)), (2, 23, 1975));
    assert_eq!(begin_dst(2000, CENTRAL) % 86_400, 8 * 3600);
    assert_eq!(end_dst(2000, CENTRAL) % 86_400, 7 * 3600);
    let b = begin_dst(2000, CENTRAL);
    let e = end_dst(2000, CENTRAL);
    assert!(!is_dst(b - 1, CENTRAL) && is_dst(b, CENTRAL));
    assert!(is_dst(e - 1, CENTRAL) && !is_dst(e, CENTRAL));
    assert!(!is_dst(b, CENTRAL_NO_DST) && !is_dst(e - 1, CENTRAL_NO_DST));
}

#[test]
fn crime_worked_example() {
    // crime.md §9: 2026-10-14 12:00:00 local, US Central -> 0xEC990010.
    let local = LocalTm {
        year: 2026,
        month: 10,
        day: 14,
        hour: 12,
        minute: 0,
        second: 0,
    };
    let t = RzTime::now(&local, CENTRAL);
    assert_eq!(t.secs, 3_969_450_000);
    assert_eq!(t.secs, 0xEC99_0010);
    assert_eq!(t.secs ^ 0x6C09_C3F6, 0x8090_C3E6);
    // 2026-10-28: standard time by the library rule -> 3600 more for the same local reading.
    let a = RzTime::from_local(jday(10, 28, 2026), 12, 0, 0, CENTRAL);
    let b = RzTime::from_local(jday(10, 28, 2026), 11, 0, 0, CENTRAL);
    assert_eq!(a.secs - b.secs, 3600);
    assert_eq!(b.secs - t.secs, 14 * 86_400);
}

#[test]
fn from_local_and_back() {
    // Standard time, no daylight: plain UTC conversion.
    let t = RzTime::from_local(jday(1, 15, 2000), 10, 30, 15, CENTRAL);
    assert_eq!(
        t.secs,
        (jday(1, 15, 2000) - DAY_1901_01_01) * 86_400 + 16 * 3600 + 30 * 60 + 15
    );
    assert_eq!((t.hour(CENTRAL), t.minute(CENTRAL)), (10, 30));
    assert_eq!(t.date(CENTRAL), RzDate::from_mdy(1, 15, 2000));
    // Summer: one hour earlier in UTC; read back as the same local time.
    let s = RzTime::from_local(jday(7, 1, 2000), 10, 0, 0, CENTRAL);
    assert_eq!(s.secs, (jday(7, 1, 2000) - DAY_1901_01_01) * 86_400 + 15 * 3600);
    assert_eq!(s.hour(CENTRAL), 10);
    // Date 0 gives 0; Set(time 0) gives 0.
    assert_eq!(RzTime::from_local(0, 10, 0, 0, CENTRAL).secs, 0);
    let mut d = RzDate::from_day(5);
    assert_eq!(d.set_time(RzTime::default(), CENTRAL), 0);
    // UTC with no daylight is exact.
    let u = RzTime::from_local(jday(1, 1, 1970), 0, 0, 0, TimeZone::UTC);
    assert_eq!(u.secs, 2_177_452_800);
    assert_eq!(RzTime::from_local(jday(1, 1, 1901), 0, 0, 0, TimeZone::UTC).secs, 0);
}

#[test]
fn spring_forward_hour_is_zero() {
    // 2000-04-02: library DST starts 02:00 local standard = 08:00 UTC.
    let day = jday(4, 2, 2000);
    assert_eq!(RzTime::from_local(day, 2, 30, 0, CENTRAL).secs, 0);
    assert_ne!(RzTime::from_local(day, 1, 59, 59, CENTRAL).secs, 0);
    let three = RzTime::from_local(day, 3, 0, 0, CENTRAL);
    assert_eq!(three.secs, begin_dst(2000, CENTRAL));
    // Without daylight the hour is ordinary.
    assert_ne!(RzTime::from_local(day, 2, 30, 0, CENTRAL_NO_DST).secs, 0);
}

struct FixedClock(LocalTm, TimeZone);

impl WallClock for FixedClock {
    fn local_now(&self) -> LocalTm {
        self.0
    }
    fn time_zone(&self) -> TimeZone {
        self.1
    }
}

#[test]
fn today_and_now_from_host_clock() {
    let clock = FixedClock(
        LocalTm {
            year: 2003,
            month: 1,
            day: 14,
            hour: 9,
            minute: 5,
            second: 0,
        },
        CENTRAL,
    );
    assert_eq!(RzDate::today_from(&clock), RzDate::from_mdy(1, 14, 2003));
    assert_eq!(GzDate::today(&clock.0).day_number(), jday(1, 14, 2003));
    let t = RzTime::now_from(&clock);
    assert_eq!((t.hour(CENTRAL), t.minute(CENTRAL)), (9, 5));
    let bad = FixedClock(
        LocalTm {
            year: 2003,
            month: 0,
            day: 0,
            hour: 0,
            minute: 0,
            second: 0,
        },
        CENTRAL,
    );
    assert_eq!(RzDate::today_from(&bad).n, 0);
    assert_eq!(RzTime::now_from(&bad).secs, 0);
}
