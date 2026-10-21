// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Conformance ops for `docs/specs/dates.md`: pure calendar / time functions
//! (`dt_fn`) and labelled `cGZDate` objects (`dt_new`, `dt_call`, `dt_expect`).
//! See README.md ("Dates and times ops").

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use fc4_core::dates::time::{begin_dst, end_dst, is_dst};
use fc4_core::dates::{self, GzDate, RzDate, RzTime, TimeZone};

use crate::json::Json;
use crate::runner::HostRef;
use crate::scalar::{self, arg, check_expect, check_state, ji, opt, str_of, u32_of, R};

#[derive(Default)]
pub(crate) struct World {
    dates: RefCell<BTreeMap<String, GzDate>>,
}

fn world(host: &HostRef) -> Rc<World> {
    scalar::world(host, "dates", || Rc::new(World::default()))
}

pub(crate) fn exec(host: &HostRef, name: &str, op: &Json) -> Option<R<()>> {
    let rest = name.strip_prefix("dt_")?;
    Some(match rest {
        "fn" => func(op),
        "new" => new(&world(host), op),
        "call" => call(&world(host), op),
        "expect" => expect(&world(host), op),
        _ => Err(format!("unknown op '{name}'")),
    })
}

fn a(op: &Json, i: usize) -> R<u32> {
    u32_of(arg(op, i)?, "args")
}

fn i32_arg(op: &Json, i: usize) -> R<i32> {
    Ok(a(op, i)? as i32)
}

/// `"tz": {"timezone": seconds west, "daylight": 0|1}`, default UTC.
fn tz_of(op: &Json) -> R<TimeZone> {
    match opt(op, "tz") {
        None => Ok(TimeZone::UTC),
        Some(t) => Ok(TimeZone {
            timezone: opt(t, "timezone")
                .map(|v| u32_of(v, "timezone"))
                .transpose()?
                .unwrap_or(0) as i32,
            daylight: opt(t, "daylight")
                .map(|v| u32_of(v, "daylight"))
                .transpose()?
                .unwrap_or(0) as i32,
        }),
    }
}

fn mdy_json((m, d, y): (u32, u32, u32)) -> Json {
    Json::Array(vec![ji(m), ji(d), ji(y)])
}

/// The local date of an instant (seconds after 1901-01-01), as `[m, d, y]`
/// of the UTC day.
fn utc_day(t: u32) -> Json {
    mdy_json(dates::mdy(dates::DAY_1901_01_01.wrapping_add(t / 86_400)))
}

fn func(op: &Json) -> R<()> {
    let f = str_of(op.get("fn").ok_or("missing 'fn'")?, "fn")?;
    let tz = tz_of(op)?;
    let got = match f {
        "jday" => ji(dates::jday(a(op, 0)?, a(op, 1)?, a(op, 2)?)),
        "mdy" => mdy_json(dates::mdy(a(op, 0)?)),
        "weekday" => ji(dates::weekday(a(op, 0)?)),
        "day_of_year" => ji(RzDate::from_day(a(op, 0)?).day_of_year()),
        "first_day_of_month" => ji(RzDate::from_day(a(op, 0)?).first_day_of_month(a(op, 1)?)),
        "previous" => ji(RzDate::from_day(a(op, 0)?).previous(a(op, 1)?).n),
        "leap_year" => Json::Bool(dates::leap_year(a(op, 0)?)),
        "days_in_year" => ji(dates::days_in_year(a(op, 0)?)),
        "day_within_month" => Json::Bool(dates::day_within_month(a(op, 0)?, a(op, 1)?, a(op, 2)?)),
        "month_from_name" => ji(dates::month_from_name(str_of(arg(op, 0)?, "args")?.as_bytes())),
        "date_string" => Json::Str(dates::date_string(a(op, 0)?, a(op, 1)?)),
        "set_day_of_year" => ji(RzDate::default().set_day_of_year(a(op, 0)?, a(op, 1)?)),
        "begin_dst" => ji(begin_dst(a(op, 0)?, tz)),
        "end_dst" => ji(end_dst(a(op, 0)?, tz)),
        "begin_dst_day" => utc_day(begin_dst(a(op, 0)?, tz)),
        "end_dst_day" => utc_day(end_dst(a(op, 0)?, tz)),
        "is_dst" => Json::Bool(is_dst(a(op, 0)?, tz)),
        // args: day number, hour, minute, second
        "time_from_local" => ji(RzTime::from_local(a(op, 0)?, a(op, 1)?, a(op, 2)?, a(op, 3)?, tz).secs),
        "time_local_secs" => ji(RzTime::from_secs(a(op, 0)?).local_secs(tz)),
        "time_hour" => ji(RzTime::from_secs(a(op, 0)?).hour(tz)),
        "time_minute" => ji(RzTime::from_secs(a(op, 0)?).minute(tz)),
        "time_date" => ji(RzTime::from_secs(a(op, 0)?).date(tz).n),
        _ => return Err(format!("unknown dt_fn '{f}'")),
    };
    check_expect(op, got)
}

fn obj_name(op: &Json) -> R<String> {
    Ok(str_of(op.get("obj").ok_or("missing 'obj'")?, "obj")?.to_string())
}

/// `{"obj": "A", "day": n}` or `{"obj": "A", "mdy": [m, d, y]}`; count 0.
fn new(w: &World, op: &Json) -> R<()> {
    let name = obj_name(op)?;
    let g = if let Some(m) = opt(op, "mdy") {
        let v = scalar::arr(m, "mdy")?;
        let f = |i: usize| {
            v.get(i)
                .ok_or("'mdy' needs 3 numbers".to_string())
                .and_then(|j| u32_of(j, "mdy"))
        };
        GzDate::from_mdy(f(0)?, f(1)?, f(2)?)
    } else {
        GzDate::from_day(opt(op, "day").map(|v| u32_of(v, "day")).transpose()?.unwrap_or(0))
    };
    w.dates.borrow_mut().insert(name, g);
    Ok(())
}

fn day_of(w: &World, label: &Json) -> R<u32> {
    match label {
        Json::Str(s) => w
            .dates
            .borrow()
            .get(s)
            .map(GzDate::day_number)
            .ok_or_else(|| format!("no date '{s}'")),
        other => u32_of(other, "date"),
    }
}

/// A date argument: a label of a `dt_new` object or a day number.
fn d(w: &World, op: &Json, i: usize) -> R<u32> {
    day_of(w, arg(op, i)?)
}

/// `{"obj", "method", "args", "expect"}`: one `cIGZDate` slot. Slots that
/// return the interface pointer (37-40) return `"this"`; the out date of
/// `max_date` / `min_date` / `previous` / `clone` is the label in `"out"`.
fn call(w: &World, op: &Json) -> R<()> {
    let name = obj_name(op)?;
    let method = str_of(op.get("method").ok_or("missing 'method'")?, "method")?;
    let tz = tz_of(op)?;
    let out_name = opt(op, "out")
        .map(|o| str_of(o, "out").map(str::to_string))
        .transpose()?;
    // Resolve date arguments before borrowing the object mutably.
    let date_args: Vec<u32> = match method {
        "between" => vec![d(w, op, 0)?, d(w, op, 1)?],
        "compare_to" | "lt" | "le" | "gt" | "ge" | "eq" | "ne" | "sub_date" | "add_date" | "copy" | "max_date"
        | "min_date" => vec![d(w, op, 0)?],
        _ => Vec::new(),
    };
    let mut dates = w.dates.borrow_mut();
    let mut g = *dates.get(&name).ok_or_else(|| format!("no date '{name}'"))?;
    let mut out = match &out_name {
        Some(o) if method != "clone" => Some(*dates.get(o).ok_or_else(|| format!("no date '{o}'"))?),
        _ => None,
    };
    let need_out = |o: &mut Option<GzDate>| -> R<()> {
        if o.is_none() {
            Err(format!("'{method}' needs 'out'"))
        } else {
            Ok(())
        }
    };
    let this = Json::Str("this".into());
    let got = match method {
        "query_interface" => Json::Bool(g.query_interface(a(op, 0)?)),
        "add_ref" => ji(g.add_ref()),
        "release" => {
            let r = g.release();
            if r.freed {
                dates.remove(&name);
                drop(dates);
                return check_expect(op, ji(r.count));
            }
            ji(r.count)
        }
        "set_month_name" => ji(g.set_month_name(str_of(arg(op, 0)?, "args")?.as_bytes(), a(op, 1)?, a(op, 2)?)),
        "set_day" => ji(g.set_day(a(op, 0)?)),
        "set_time" => ji(g.set_time(RzTime::from_secs(a(op, 0)?), tz)),
        "set_mdy" => ji(g.set_mdy(a(op, 0)?, a(op, 1)?, a(op, 2)?)),
        "set_day_of_year" => ji(g.set_day_of_year(a(op, 0)?, a(op, 1)?)),
        "copy" => ji(g.copy_from(date_args[0])),
        "clone" => {
            let o = out_name.as_ref().ok_or("'clone' needs 'out'")?;
            dates.insert(o.clone(), g.clone_date());
            Json::Bool(true)
        }
        "between" => Json::Bool(g.between(date_args[0], date_args[1])),
        "compare_to" => ji(g.compare_to(date_args[0])),
        "week_day" => ji(g.week_day()),
        "year" => ji(g.year()),
        "day_of_year" => ji(g.day_of_year()),
        "day_of_month" => ji(g.day_of_month()),
        "first_day_of_month_of" => ji(g.first_day_of_month_of(a(op, 0)?)),
        "first_day_of_month" => ji(g.first_day_of_month()),
        "hash" => ji(g.hash()),
        "is_valid" => Json::Bool(g.is_valid()),
        "leap" => Json::Bool(g.leap()),
        "max_date" => {
            need_out(&mut out)?;
            ji(g.max_date(date_args[0], out.as_mut().unwrap()))
        }
        "min_date" => {
            need_out(&mut out)?;
            ji(g.min_date(date_args[0], out.as_mut().unwrap()))
        }
        "month" => ji(g.month()),
        "previous" => {
            need_out(&mut out)?;
            ji(g.previous(a(op, 0)?, out.as_mut().unwrap()))
        }
        "year_last_two_digits" => ji(g.year_last_two_digits()),
        "day_number" => ji(g.day_number()),
        "lt" => Json::Bool(g.lt(date_args[0])),
        "le" => Json::Bool(g.le(date_args[0])),
        "gt" => Json::Bool(g.gt(date_args[0])),
        "ge" => Json::Bool(g.ge(date_args[0])),
        "eq" => Json::Bool(g.eq_date(date_args[0])),
        "ne" => Json::Bool(g.ne_date(date_args[0])),
        "sub_days" => ji(g.sub_days(i32_arg(op, 0)?)),
        "sub_date" => ji(g.sub_date(date_args[0])),
        "add_days" => ji(g.add_days(i32_arg(op, 0)?)),
        "add_date" => ji(g.add_date(date_args[0])),
        "inc" => {
            g.inc();
            this
        }
        "dec" => {
            g.dec();
            this
        }
        "add_assign" => {
            g.add_assign_days(i32_arg(op, 0)?);
            this
        }
        "sub_assign" => {
            g.sub_assign_days(i32_arg(op, 0)?);
            this
        }
        "date_string" => Json::Str(g.date_string(a(op, 0)?)),
        "day_within_month" => Json::Bool(g.day_within_month(a(op, 0)?, a(op, 1)?, a(op, 2)?)),
        "days_in_year" => ji(g.days_in_year(a(op, 0)?)),
        "jday" => ji(g.jday(a(op, 0)?, a(op, 1)?, a(op, 2)?)),
        "leap_year" => ji(g.leap_year(a(op, 0)?)),
        "post_inc" => ji(g.date.post_inc().n),
        "post_dec" => ji(g.date.post_dec().n),
        _ => return Err(format!("unknown dt_call method '{method}'")),
    };
    dates.insert(name, g);
    if let (Some(o), Some(v)) = (out_name, out) {
        if method != "clone" {
            dates.insert(o, v);
        }
    }
    drop(dates);
    check_expect(op, got)
}

/// `{"obj", "state": {"day", "refs", "exists"}}`.
fn expect(w: &World, op: &Json) -> R<()> {
    let name = obj_name(op)?;
    let dates = w.dates.borrow();
    let got = match dates.get(&name) {
        Some(g) => vec![
            ("exists".to_string(), Json::Bool(true)),
            ("day".to_string(), ji(g.day_number())),
            ("refs".to_string(), ji(g.refs)),
        ],
        None => vec![("exists".to_string(), Json::Bool(false))],
    };
    check_state(&got, op.get("state").ok_or("missing 'state'")?)
}
