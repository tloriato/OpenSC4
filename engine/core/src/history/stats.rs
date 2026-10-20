// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Ranges (§4.6) and the regression statistics (§4.7) of the history
//! warehouse, at the caller's x87 precision (foundations §18, HW3).

use super::Point;
use crate::x87::r24;

/// The x87 precision of the caller (foundations §18.1): the simulators run at
/// 24 bits, Lua calls at 53 bits (HW3). The statistics' `double` variables
/// hold whatever the registers held, so the precision decides every rounding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Precision {
    /// PC = 00: every operation rounds to a 24-bit significand (the
    /// simulators, the budget, the demand simulator).
    Bits24,
    /// PC = 10: IEEE `double` arithmetic (inside Lua calls: advisors,
    /// `trend_value`).
    Bits53,
}

impl Precision {
    /// From a precision in bits as the glue reads it from the control word:
    /// 24 → [`Precision::Bits24`]; anything else → [`Precision::Bits53`]
    /// ([choice]: the game never sets 64-bit precision, foundations §18.1).
    pub fn from_bits(bits: u32) -> Precision {
        if bits == 24 {
            Precision::Bits24
        } else {
            Precision::Bits53
        }
    }

    /// One rounding of an exactly computed `f64` result (r24 at 24 bits).
    fn round(self, x: f64) -> f64 {
        match self {
            Precision::Bits24 => r24(x),
            Precision::Bits53 => x,
        }
    }

    /// One x87 `fadd` / `fsub` at this precision: the exact sum rounded once.
    pub fn add(self, a: f64, b: f64) -> f64 {
        match self {
            Precision::Bits24 => add_r24(a, b),
            Precision::Bits53 => a + b,
        }
    }

    /// One `fmul`. Every product of the statistics multiplies two values that
    /// are already rounded to this precision; at 24 bits their exact product
    /// fits an `f64`, so `r24` of the `f64` product is the single rounding.
    pub fn mul(self, a: f64, b: f64) -> f64 {
        self.round(a * b)
    }

    /// One `fdiv`. At 24 bits the operands have at most 24 significant bits
    /// (or are a small count), so the `f64` quotient rounded by `r24` is the
    /// correctly rounded result (53 ≥ 2·24 + 2).
    pub fn div(self, a: f64, b: f64) -> f64 {
        self.round(a / b)
    }

    /// One `fsqrt` (correctly rounded at 24 bits by the same argument).
    pub fn sqrt(self, a: f64) -> f64 {
        self.round(a.sqrt())
    }
}

/// Mask of the `f64` significand bits below a 24-bit significand.
const LOW29: u64 = (1 << 29) - 1;
const HALF29: u64 = 1 << 28;

/// The exact sum `a + b` rounded once to a 24-bit significand (round to
/// nearest even), as one x87 `fadd` under 24-bit precision (foundations
/// §18.3). Unlike `r24(a + b)` this has no double rounding when an operand
/// carries more than 24 significant bits (an `int32` value loaded by `fild`):
/// the `f64` sum `s` and its error `e` (TwoSum) are exact, and only when `s`
/// lies exactly halfway between two 24-bit values does `e` decide.
pub fn add_r24(a: f64, b: f64) -> f64 {
    let s = a + b;
    if !s.is_finite() || s == 0.0 {
        return r24(s);
    }
    let bp = s - a;
    let ap = s - bp;
    let e = (a - ap) + (b - bp);
    let bits = s.to_bits();
    if e == 0.0 || bits & LOW29 != HALF29 {
        return r24(s);
    }
    // `s` is a midpoint and the exact sum lies on the side of `e`.
    let toward_zero = f64::from_bits(bits & !LOW29);
    let away = f64::from_bits((bits & !LOW29) + (1 << 29));
    if (e > 0.0) == (s > 0.0) {
        away
    } else {
        toward_zero
    }
}

/// The range walk of §4.6 (HW6, `cVecRange`): one walk from the first point,
/// every date comparison unsigned. With `widen` the two pre-checks come
/// first (`from` after the last point: the last point; `to` before the first
/// point: the first point). Until the range has begun each point is tested
/// first against `to` (`date > to`: with widening its predecessor and
/// itself, without widening nothing), then against `from` (`date ≥ from`
/// begins the range). Once begun, the first point after `to` ends it; with
/// widening a one-point range becomes its predecessor, itself and that
/// successor. A walk that reaches the end after beginning runs to the last
/// point (never widened); one that never began is empty.
pub fn walk_range(points: &[Point], from: u32, to: u32, widen: bool) -> std::ops::Range<usize> {
    let n = points.len();
    if widen && n > 0 {
        if from > points[n - 1].date {
            return n - 1..n;
        }
        if to < points[0].date {
            return 0..1;
        }
    }
    let mut begin: Option<usize> = None;
    for (i, p) in points.iter().enumerate() {
        match begin {
            None => {
                if p.date > to {
                    return if widen { i.saturating_sub(1)..i + 1 } else { 0..0 };
                }
                if p.date >= from {
                    begin = Some(i);
                }
            }
            Some(b) => {
                if p.date > to {
                    if widen && i - b == 1 {
                        return b.saturating_sub(1)..i + 1;
                    }
                    return b..i;
                }
            }
        }
    }
    match begin {
        Some(b) => b..n,
        None => 0..0,
    }
}

/// The points with `from ≤ date ≤ to`, without widening (§4.6), as an index
/// range, found by the walk of [`walk_range`]. On the date-ordered lists the
/// warehouse keeps this is exactly the inclusive range (empty when
/// `from > to`); only a `Coalesce` over a range that does not start on a
/// period boundary can disorder a list, and the walk then follows the list.
pub fn plain_range(points: &[Point], from: u32, to: u32) -> std::ops::Range<usize> {
    walk_range(points, from, to, false)
}

/// The widened range of §4.6 (every query except `GetMinMaxAverage`). With
/// `from > to` and neither pre-check true it is the first point after `to`
/// and its predecessor (HW6).
pub fn widened_range(points: &[Point], from: u32, to: u32) -> std::ops::Range<usize> {
    walk_range(points, from, to, true)
}

/// The statistics of §4.7 over a range, with `x = date − base` (int32) and
/// `y = value`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stats {
    pub n: usize,
    pub sum_x: f64,
    pub sum_y: f64,
    pub mean_x: f64,
    pub mean_y: f64,
    pub sx: f64,
    pub sy: f64,
    pub r: f64,
}

fn xy(p: &Point, base: u32) -> (f64, f64) {
    (f64::from(p.date.wrapping_sub(base) as i32), f64::from(p.value))
}

/// The statistics of §4.7, every operation rounded at precision `prec`.
///
/// Formulas and helpers (foundations §18.3; `add` = [`add_r24`] at 24 bits):
/// - `Σx`, `Σy`: running `add` in point order (the `int32` operands enter
///   exactly);
/// - `meanX = Σx / n`, `meanY = Σy / n`: `div` (0 when n = 0);
/// - `sx = √(Σ(x − meanX)² / n)`: `d = add(x, −meanX)`, `add(sum, mul(d, d))`,
///   then `div`, then `sqrt`; `sy` likewise;
/// - `r = Σ((x − meanX)/sx · (y − meanY)/sy) / n`: [choice] read as the
///   product of the two standardised values, `mul(div(dx, sx), div(dy, sy))`,
///   summed with `add`, then `div` by n; 0 when sx, sy or n is 0.
pub fn statistics(points: &[Point], base: u32, prec: Precision) -> Stats {
    let n = points.len();
    let mut st = Stats {
        n,
        sum_x: 0.0,
        sum_y: 0.0,
        mean_x: 0.0,
        mean_y: 0.0,
        sx: 0.0,
        sy: 0.0,
        r: 0.0,
    };
    if n == 0 {
        return st;
    }
    for p in points {
        let (x, y) = xy(p, base);
        st.sum_x = prec.add(st.sum_x, x);
        st.sum_y = prec.add(st.sum_y, y);
    }
    let nf = n as f64;
    st.mean_x = prec.div(st.sum_x, nf);
    st.mean_y = prec.div(st.sum_y, nf);
    let (mut ssx, mut ssy) = (0.0, 0.0);
    for p in points {
        let (x, y) = xy(p, base);
        let dx = prec.add(x, -st.mean_x);
        let dy = prec.add(y, -st.mean_y);
        ssx = prec.add(ssx, prec.mul(dx, dx));
        ssy = prec.add(ssy, prec.mul(dy, dy));
    }
    st.sx = prec.sqrt(prec.div(ssx, nf));
    st.sy = prec.sqrt(prec.div(ssy, nf));
    if st.sx != 0.0 && st.sy != 0.0 {
        // HW5: the reciprocals once, each rounded; the standardised values
        // are multiplied by them.
        let ix = prec.div(1.0, st.sx);
        let iy = prec.div(1.0, st.sy);
        let mut sum = 0.0;
        for p in points {
            let (x, y) = xy(p, base);
            let zx = prec.mul(prec.add(x, -st.mean_x), ix);
            let zy = prec.mul(prec.add(y, -st.mean_y), iy);
            sum = prec.add(sum, prec.mul(zx, zy));
        }
        st.r = prec.div(sum, nf);
    }
    st
}

/// The regression line of a non-empty range (§4.7): `(slope, intercept)` at
/// the precision's rounding, or `None` for an empty range. With `sx = 0` the
/// slope is 0 and the intercept the first point's value; otherwise
/// `slope = mul(div(sy, sx), r)` and `intercept = div(add(Σy, −mul(slope,
/// Σx)), n)`, the value at `x = 0` (the base date).
///
/// `GetPoint` fits this line over the widened range of one date (§4.7): at an
/// interior point the range is the point and its two neighbours, so the answer
/// is the smoothed value. The game's graph window is drawn from those answers
/// alone, so this smoothing is visible in every graph and must be kept (HW2).
pub fn fit_line(points: &[Point], base: u32, prec: Precision) -> Option<(f64, f64)> {
    let first = points.first()?;
    let st = statistics(points, base, prec);
    if st.sx == 0.0 {
        return Some((0.0, f64::from(first.value)));
    }
    let slope = prec.mul(prec.div(st.sy, st.sx), st.r);
    let intercept = prec.div(prec.add(st.sum_y, -prec.mul(slope, st.sum_x)), st.n as f64);
    Some((slope, intercept))
}
