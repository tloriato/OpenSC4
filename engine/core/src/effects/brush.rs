// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The brush effect (`docs/specs/effects.md` §8), the component kind that
//! deforms terrain.
//!
//! A brush component is built from a **brush record** (§8.1, [`shipped`]) and
//! draws its strength and radius when it is created (§8.2). At runtime it is a
//! stroke or a one-shot: the stroke accumulates `acc += rate × dt` and applies
//! `floor(acc)` brushes per tick, which is what §17 tests 1 and 2 pin.

use super::shipped::BrushRecord;
use crate::x87::r24;

/// `tBrushParameters` (terrain.md §9.2), the fields a brush effect fills in.
///
/// The original's struct is `0x38` bytes and also carries an out rectangle
/// (`+0x24..+0x30`) that `ApplyBrush` writes; that is the terrain's business
/// and is not modelled here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrushParams {
    /// `+0x00`, `+0x04`: the stroke's start, in metres.
    pub from_x: f32,
    pub from_z: f32,
    /// `+0x08`, `+0x0C`: the stroke's end, in metres.
    pub to_x: f32,
    pub to_z: f32,
    /// `+0x10`: the level index 0-4; below 0 means the terrain default.
    pub level: i32,
    /// `+0x14`.
    pub redisplay: bool,
    /// `+0x18`.
    pub strength: f32,
    /// `+0x1C`.
    pub radius_scale: f32,
    /// `+0x20`, passed to `RedisplayTerrain`. §8.3 fixes it at 3.
    pub data4: u32,
    /// `+0x34`: the target height of a levelling component.
    pub target_height: f32,
}

/// `data4` of a brush effect's parameters (§8.3 step 2).
pub const BRUSH_DATA4: u32 = 3;

/// `n := floor(acc)` of §8.3 step 4.
///
/// The original uses the "add 1.5·2²³" round-to-nearest idiom with an equality
/// fix-up, which gives exactly `floor` for `0 ≤ acc < 2²²`. The fix-up is
/// applied to the idiom's result here: when the rounded value is above `acc`,
/// step back one. *[choice]* the spec states the property, not the fix-up's
/// shape; [`floor_count_is_exactly_floor`] asserts the property over the range
/// the stroke can reach.
pub fn floor_count(acc: f32) -> u32 {
    let rounded = crate::x87::rhe(acc);
    if rounded <= 0 {
        return 0;
    }
    let stepped = if (rounded as f32) > acc { rounded - 1 } else { rounded };
    if stepped <= 0 {
        0
    } else {
        stepped as u32
    }
}

/// The draws a brush component's factory makes (§8.2).
///
/// Both draws use the **static** `cRZRandom` at `0x00B22BE8`, which is
/// process-wide and shared with other callers (foundations §6.3 "brush
/// effects"); the core takes it as a parameter rather than owning it.
pub trait EffectsRandom {
    /// `RandomDoubleRangeUniform(a, b)`.
    fn range_f64(&mut self, a: f64, b: f64) -> f64;
}

impl EffectsRandom for crate::rng::RzRandom {
    fn range_f64(&mut self, a: f64, b: f64) -> f64 {
        crate::rng::RzRandom::range_f64(self, a, b)
    }
}

/// The brush effect's runtime state (§8.3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrushEffect {
    /// The record this component was built from.
    pub record: BrushRecord,
    /// The strength drawn at creation (§8.2).
    pub strength: f32,
    /// The radius scale drawn at creation (§8.2).
    pub radius_scale: f32,
    /// `active`, the byte `IsActive` returns.
    active: bool,
    /// `longOp`: a long terrain operation is open.
    long_op: bool,
    /// `acc`, the application accumulator.
    pub acc: f32,
    /// `t`, the elapsed effect-seconds of this stroke.
    pub t: f32,
    /// `prev`, the previous position.
    pub prev: (f32, f32),
    /// `cur`, the current position.
    pub cur: (f32, f32),
}

impl BrushEffect {
    /// §8.2: a brush component draws its strength first, then its radius, from
    /// the static generator. Every shipped record has a degenerate range, so
    /// both values are fixed — but **the two draws still advance the
    /// generator** (§14 quirk 7).
    pub fn new(record: BrushRecord, random: &mut dyn EffectsRandom) -> BrushEffect {
        let strength = random.range_f64(f64::from(record.strength[0]), f64::from(record.strength[1])) as f32;
        let radius_scale =
            random.range_f64(f64::from(record.radius_scale[0]), f64::from(record.radius_scale[1])) as f32;
        BrushEffect {
            record,
            strength,
            radius_scale,
            active: false,
            long_op: false,
            acc: 0.0,
            t: 0.0,
            prev: (0.0, 0.0),
            cur: (0.0, 0.0),
        }
    }

    /// §8.3 `Start(mode)`: `prev := cur`, `active := 1`, `longOp := 0`,
    /// `acc := 0`, `t := 0`. The mode is not used.
    pub fn start(&mut self) {
        self.prev = self.cur;
        self.active = true;
        self.long_op = false;
        self.acc = 0.0;
        self.t = 0.0;
    }

    /// §8.3 `Stop(mode)`: end the long terrain operation if one is open, then
    /// `active := 0`.
    pub fn stop(&mut self) -> bool {
        let ended_long_op = self.long_op;
        if self.long_op {
            self.long_op = false;
        }
        self.active = false;
        ended_long_op
    }

    /// §8.3 `IsActive`: the `active` byte.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// §8.3 `SetTransform`: the component's current position, in metres.
    pub fn set_position(&mut self, x: f32, z: f32) {
        self.cur = (x, z);
    }

    /// Whether the next update has to open a long terrain operation (§8.3 step
    /// 1 of the stroke branch).
    pub fn needs_long_operation(&self) -> bool {
        self.active && !self.record.one_shot && !self.long_op
    }

    /// Marks the long terrain operation as open (§8.3 stroke step 1).
    pub fn begin_long_operation(&mut self) {
        self.long_op = true;
    }

    /// Whether a long terrain operation is currently open.
    pub fn long_operation_open(&self) -> bool {
        self.long_op
    }

    /// §8.3 `Update(dt)` for a **one-shot**: `active := 0`, then
    /// `Apply(trunc(rate))`. Nothing else — no long operation, no accumulator.
    ///
    /// Returns the number of `ApplyBrush` calls to make.
    pub fn update_one_shot(&mut self) -> u32 {
        self.active = false;
        crate::x87::ftol32(f64::from(self.record.rate)).max(0) as u32
    }

    /// §8.3 `Update(dt)` for a **stroke**, minus the long-operation bookkeeping
    /// the caller performs through [`BrushEffect::needs_long_operation`] and
    /// [`BrushEffect::begin_long_operation`].
    ///
    /// 2. `acc := r24(r24(dt × rate) + acc)`
    /// 3. `t := r24(dt + t)`
    /// 4. `n := floor(acc)`
    /// 5. `Apply(n)`
    /// 6. `acc := r24(acc − n)`
    /// 7. if `length > 0` and `t > length`: `active := 0`
    ///
    /// Returns the number of `ApplyBrush` calls to make.
    pub fn update_stroke(&mut self, dt: f32) -> u32 {
        self.acc = r24(r24(f64::from(dt) * f64::from(self.record.rate)) + f64::from(self.acc)) as f32;
        self.t = r24(f64::from(dt) + f64::from(self.t)) as f32;
        let n = floor_count(self.acc);
        self.acc = r24(f64::from(self.acc) - f64::from(n)) as f32;
        if self.record.length > 0.0 && self.t > self.record.length {
            self.active = false;
        }
        n
    }

    /// §8.3 `Apply(n)`: the `n` brush applications.
    ///
    /// Returns the parameters of each call, in order, with `redisplay` set only
    /// on the last. An empty list means nothing is applied.
    ///
    /// *[quirk]* when `prev` or `cur` is outside the city nothing is applied
    /// and **`prev` is not updated** (§8.3 step 1, quirk 4).
    pub fn applications(&mut self, n: u32, in_bounds: impl Fn(f32, f32) -> bool) -> Vec<BrushParams> {
        if n == 0 {
            // `prev := cur` still happens when n = 0 (§8.3 step 4).
            self.prev = self.cur;
            return Vec::new();
        }
        if !in_bounds(self.prev.0, self.prev.1) || !in_bounds(self.cur.0, self.cur.1) {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(n as usize);
        for i in 0..n {
            out.push(BrushParams {
                from_x: self.prev.0,
                from_z: self.prev.1,
                to_x: self.cur.0,
                to_z: self.cur.1,
                level: self.record.level as i32,
                redisplay: i + 1 == n,
                strength: self.strength,
                radius_scale: self.radius_scale,
                data4: BRUSH_DATA4,
                target_height: self.record.target_height,
            });
        }
        self.prev = self.cur;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::shipped::{brush_id, BrushRecord};

    /// A generator that always returns the midpoint, so the draws are
    /// deterministic and the fact that **two** draws happen is visible.
    struct Mid {
        draws: u32,
    }
    impl EffectsRandom for Mid {
        fn range_f64(&mut self, a: f64, b: f64) -> f64 {
            self.draws += 1;
            (a + b) * 0.5
        }
    }

    fn stroke(rate: f32, length: f32) -> BrushRecord {
        BrushRecord {
            brush_id: brush_id::VOLCANO,
            rate,
            length,
            one_shot: false,
            level: 1,
            strength: [1.0, 1.0],
            radius_scale: [0.5, 0.5],
            target_height: 100.0,
        }
    }

    /// §8.3 step 4: the count is exactly `floor` over the range the stroke can
    /// reach.
    #[test]
    fn floor_count_is_exactly_floor() {
        for i in 0..200u32 {
            for (frac, name) in [(0.0f32, "exact"), (0.25, "quarter"), (0.5, "half"), (0.999, "near")] {
                let x = i as f32 + frac;
                assert_eq!(floor_count(x), i, "{name} at {x}");
            }
        }
        assert_eq!(floor_count(0.0), 0);
        assert_eq!(floor_count(0.99999), 0);
        assert_eq!(floor_count(1.99998), 1);
    }

    /// §17 test 1: brush record 2 (rate 30, length 3.0, stroke) with ticks of
    /// 33,333 µs at dilation 1: `dt' = 33333`, `dt = r24(0.033333)`, the
    /// per-tick `n` is 0, 1, 1, 1, …, and the stroke ends after **91 ticks**
    /// with **90 applications**, `t = 3.0333` and leftover `acc = 0.99909`.
    #[test]
    fn brush_stroke_count_at_dilation_one() {
        let mut b = BrushEffect::new(stroke(30.0, 3.0), &mut Mid { draws: 0 });
        b.set_position(100.0, 100.0);
        b.start();

        // §6: dt' = trunc(r24(us * dilation)); dt = r24(dt' * 1e-6).
        let dt = r24(33_333.0 * 1e-6) as f32;
        assert_eq!(dt, 0.033333, "§17 test 1 gives dt = r24(0.033333)");

        let mut per_tick = Vec::new();
        let mut total = 0u32;
        let mut ticks = 0u32;
        while b.is_active() {
            ticks += 1;
            let n = b.update_stroke(dt);
            per_tick.push(n);
            total += n;
            let _ = b.applications(n, |_, _| true);
            if ticks > 200 {
                break;
            }
        }
        assert_eq!(per_tick[0], 0, "the first tick applies nothing");
        assert_eq!(per_tick[1], 1);
        assert_eq!(per_tick[2], 1);
        assert_eq!(per_tick[3], 1);
        assert_eq!(ticks, 91, "the stroke ends after 91 ticks");
        assert_eq!(total, 90, "90 applications");
        assert!((b.t - 3.0333).abs() < 1e-3, "t = {}, expected 3.0333", b.t);
        assert!(
            (b.acc - 0.99909).abs() < 1e-4,
            "leftover acc = {}, expected 0.99909",
            b.acc
        );
    }

    /// §17 test 2: the same at dilation 1.5. `dt' = trunc(49999.5) = 49999`,
    /// `n` = 1, 1, 2, 1, 2, …, and it ends after **61 ticks** with **91
    /// applications**.
    #[test]
    fn brush_stroke_count_at_dilation_one_and_a_half() {
        let mut b = BrushEffect::new(stroke(30.0, 3.0), &mut Mid { draws: 0 });
        b.set_position(100.0, 100.0);
        b.start();

        let dt_prime = crate::x87::ftol32(r24(33_333.0 * 1.5)) as u32;
        assert_eq!(dt_prime, 49_999, "§17 test 2: trunc(49999.5) = 49999");
        let dt = r24(f64::from(dt_prime) * 1e-6) as f32;

        let mut per_tick = Vec::new();
        let mut total = 0u32;
        let mut ticks = 0u32;
        while b.is_active() {
            ticks += 1;
            let n = b.update_stroke(dt);
            per_tick.push(n);
            total += n;
            let _ = b.applications(n, |_, _| true);
            if ticks > 200 {
                break;
            }
        }
        assert_eq!(&per_tick[..5], &[1, 1, 2, 1, 2], "n = 1, 1, 2, 1, 2, …");
        assert_eq!(ticks, 61, "61 ticks");
        assert_eq!(total, 91, "91 applications");
    }

    /// §17 test 2 tail: at 100,000 µs and dilation 1 each tick applies exactly
    /// 3, for 31 ticks (93 applications).
    #[test]
    fn brush_stroke_at_a_hundred_thousand_microseconds() {
        let mut b = BrushEffect::new(stroke(30.0, 3.0), &mut Mid { draws: 0 });
        b.set_position(100.0, 100.0);
        b.start();

        let dt = r24(100_000.0 * 1e-6) as f32;
        let mut total = 0u32;
        let mut ticks = 0u32;
        while b.is_active() {
            ticks += 1;
            let n = b.update_stroke(dt);
            assert_eq!(n, 3, "tick {ticks} applies exactly 3");
            total += n;
            let _ = b.applications(n, |_, _| true);
            if ticks > 200 {
                break;
            }
        }
        assert_eq!(ticks, 31);
        assert_eq!(total, 93);
    }

    /// §17 test 4: `meteorcrater_b` (brush record 12) is a one-shot with rate
    /// 2, so its first update calls `ApplyBrush(0x8A33E17A)` **twice** from
    /// `prev` to `cur`, with level 3, strength 6, radius 0.7, `data4 = 3` and
    /// target 100; redisplay off then on. The brush is then inactive with **no**
    /// long terrain operation.
    #[test]
    fn one_shot_crater_applies_twice_and_never_opens_a_long_operation() {
        let rec = crate::effects::shipped::BRUSH_RECORDS[12];
        assert!(rec.one_shot);
        assert_eq!(rec.rate, 2.0);

        let mut b = BrushEffect::new(rec, &mut Mid { draws: 0 });
        assert_eq!(b.strength, 6.0, "the degenerate range draws its midpoint");
        assert_eq!(b.radius_scale, 0.7);
        b.set_position(10.0, 20.0);
        b.start();
        b.cur = (11.0, 21.0);

        assert!(!b.needs_long_operation(), "a one-shot opens no long operation");
        let n = b.update_one_shot();
        assert_eq!(n, 2, "trunc(rate) = 2 applications");
        assert!(!b.is_active());

        let apps = b.applications(n, |_, _| true);
        assert_eq!(apps.len(), 2);
        for (i, a) in apps.iter().enumerate() {
            assert_eq!(a.from_x, 10.0);
            assert_eq!(a.from_z, 20.0);
            assert_eq!(a.to_x, 11.0);
            assert_eq!(a.to_z, 21.0);
            assert_eq!(a.level, 3);
            assert_eq!(a.strength, 6.0);
            assert_eq!(a.radius_scale, 0.7);
            assert_eq!(a.data4, 3);
            assert_eq!(a.target_height, 100.0);
            assert_eq!(a.redisplay, i == 1, "redisplay off then on");
        }
        assert!(!b.long_operation_open());
    }

    /// §8.3 quirk 4 / §17 test 5: a stroke whose `prev` is outside the city
    /// applies nothing, and **keeps the old `prev`** — so it keeps applying
    /// nothing until both ends are inside again, which for a stroke means a
    /// restart (`Start` sets `prev := cur`).
    #[test]
    fn out_of_bounds_applies_nothing_and_keeps_prev() {
        let mut b = BrushEffect::new(stroke(30.0, 3.0), &mut Mid { draws: 0 });
        let in_city = |x: f32, z: f32| x >= 0.0 && z >= 0.0 && x <= 64.0 && z <= 64.0;

        b.set_position(-5.0, 10.0);
        b.start();
        assert_eq!(b.prev, (-5.0, 10.0), "Start sets prev := cur");
        b.cur = (10.0, 10.0);

        let apps = b.applications(3, in_city);
        assert!(apps.is_empty(), "nothing is applied");
        assert_eq!(b.prev, (-5.0, 10.0), "prev is not updated (quirk 4)");

        // Still out: prev never catches up on its own.
        b.cur = (12.0, 12.0);
        assert!(b.applications(3, in_city).is_empty());
        assert_eq!(b.prev, (-5.0, 10.0), "it keeps the old prev");

        // Both ends inside again: it applies and advances prev.
        b.start();
        b.cur = (12.0, 12.0);
        let apps = b.applications(3, in_city);
        assert_eq!(apps.len(), 3);
        assert_eq!(b.prev, (12.0, 12.0));
    }

    /// §8.3: `prev := cur` happens even when `n = 0`.
    #[test]
    fn zero_applications_still_advance_prev() {
        let mut b = BrushEffect::new(stroke(30.0, 3.0), &mut Mid { draws: 0 });
        b.set_position(1.0, 2.0);
        b.start();
        b.cur = (3.0, 4.0);
        assert!(b.applications(0, |_, _| true).is_empty());
        assert_eq!(b.prev, (3.0, 4.0));
    }

    /// §8.2: the two draws happen in order even when the ranges are degenerate
    /// (§14 quirk 7: creating an effect consumes random draws whether or not it
    /// is started).
    #[test]
    fn creation_draws_strength_then_radius() {
        let mut mid = Mid { draws: 0 };
        let b = BrushEffect::new(stroke(30.0, 3.0), &mut mid);
        assert_eq!(mid.draws, 2, "strength, then radius");
        assert_eq!(b.strength, 1.0);
        assert_eq!(b.radius_scale, 0.5);
    }

    /// §8.3 `Stop` ends the long terrain operation exactly once.
    #[test]
    fn stop_ends_the_long_operation_once() {
        let mut b = BrushEffect::new(stroke(30.0, 3.0), &mut Mid { draws: 0 });
        b.set_position(1.0, 1.0);
        b.start();
        assert!(b.needs_long_operation());
        b.begin_long_operation();
        assert!(!b.needs_long_operation());
        assert!(b.stop(), "Stop reports the long operation it ended");
        assert!(!b.stop(), "a second Stop ends nothing");
    }
}
