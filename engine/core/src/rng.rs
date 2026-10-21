// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The game's two random generators, bit-exact (`docs/specs/foundations.md`
//! §6.1). Clock-derived seeds are injected (`SeedSource`) so runs are
//! reproducible in tests (§6.3).
//!
//! The "double" methods run at the game's 24-bit x87 precision (§6.1
//! precision table, §18): every step is rounded to a 24-bit significand, so
//! their results are `float32` values returned as `f64`, and
//! `RandomDoubleUniform` can return exactly 1.0. The caller's `double`
//! arguments enter unrounded.

use crate::x87::r24;

/// Seed value that means "derive from the clock" (§6.1).
pub const CLOCK_SEED: u32 = 0xFFFF_FFFF;

/// Supplies the clock values the original reads for clock-based seeding.
pub trait SeedSource {
    /// Wall-clock derived seed (used for `Seed(0xFFFFFFFF)`).
    fn wall_clock_seed(&self) -> u32;
    /// The process clock (used when the double path's state is 0).
    fn process_clock(&self) -> u32;
}

/// A fixed seed source, for tests and replays.
#[derive(Clone, Copy, Debug)]
pub struct FixedSeeds {
    pub wall_clock: u32,
    pub process_clock: u32,
}

impl SeedSource for FixedSeeds {
    fn wall_clock_seed(&self) -> u32 {
        self.wall_clock
    }
    fn process_clock(&self) -> u32 {
        self.process_clock
    }
}

/// A generator's seed from the session seed `session` and the generator's
/// constant `k` (foundations §6.3 *[choice]*): `session XOR k`, with 0 and
/// `0xFFFFFFFF` (the values that make the original fall back to the clock)
/// moved by XOR 1.
pub fn derive_seed(session: u32, k: u32) -> u32 {
    let d = session ^ k;
    if d == 0 || d == CLOCK_SEED {
        d ^ 1
    } else {
        d
    }
}

/// Seconds from 1901-01-01 00:00 UTC (the `cRZTime` epoch) to 1970-01-01
/// 00:00 UTC: 25 202 days (crime.md §9).
pub const RZ_TIME_UNIX_OFFSET: i64 = 2_177_452_800;

/// The `cRZTime` value for a `time()` value (crime.md §9, CQ5): `time() +
/// 2177452800` truncated to 32 bits, u32 seconds since 1901-01-01 00:00 UTC.
///
/// The value is shared, not crime's own (CQ21, foundations §6.3): every
/// clock-seeded generator of the original calls the same `cRZTime()`
/// constructor, so the file-local generators of [`file_seed`] (each XORing
/// its own constant, [`clock_seed`]), the construction occupant's plain
/// reseed and the prop developer's generator all start from this value.
/// FreeCity4 normally replaces the clock with the session seed
/// ([`derive_seed`]); this is the value when the original's own is wanted.
///
/// [choice] (crime.md §9) This skips the library's own time-zone
/// arithmetic: the original rebuilds the value from the local calendar
/// fields with a fixed pre-2007 US daylight rule, so it is one hour off
/// where the host's rule disagrees, and 0 in that rule's spring-forward
/// hour.
pub fn rz_time_from_unix(unix_seconds: i64) -> u32 {
    unix_seconds.wrapping_add(RZ_TIME_UNIX_OFFSET) as u32
}

/// The original's clock seed of a file-local generator (foundations §6.3):
/// the `cRZTime` value ([`rz_time_from_unix`]) XOR the generator's constant
/// `k` ([`file_seed`]). [`derive_seed`]`(S, k)` gives the same value when
/// `S` is the `cRZTime` value, except where XOR gives 0 or `0xFFFFFFFF`.
pub fn clock_seed(rz_time: u32, k: u32) -> u32 {
    rz_time ^ k
}

/// The constants `K` of the Windows build's file-local generators
/// (foundations §6.3 table): the original seeds each with the local wall
/// clock XOR its constant, FreeCity4 with [`derive_seed`]`(S, K)`. The
/// rendering-only generators and those with no reader are left out.
pub mod file_seed {
    /// Building development simulator (`0x00B485F0`, `cRZRandom`).
    pub const BUILDING_DEVELOPMENT: u32 = 0x0C09_C0B8;
    /// Growth developers (`0x00B49080`, `cRZRandom`, shared).
    pub const GROWTH_DEVELOPER: u32 = 0xAC09_C0E8;
    /// Tract developers (`0x00B4AFD8`, `cRZRandom`, shared).
    pub const TRACT_DEVELOPER: u32 = 0x4C09_C113;
    /// Lot developer (`0x00B49AF0`, `cRZRandom`).
    pub const LOT_DEVELOPER: u32 = 0x6C09_C0F4;
    /// Zone developer (`0x00B4B398`, `cRZRandom`).
    pub const ZONE_DEVELOPER: u32 = 0x8C09_C68A;
    /// `cSC4Lot` (`0x00B49120`, `cRZRandomSuper`).
    pub const LOT: u32 = 0x0C09_C461;
    /// Budget simulator `DoMessage` (`0x00B47B78`, `cRZRandomSuper`).
    pub const BUDGET: u32 = 0x6C09_C3D7;
    /// Crime simulator (`0x00B48630`, `cRZRandomSuper`).
    pub const CRIME: u32 = 0x6C09_C3F6;
    /// Pollution simulator visual effects (`0x00B49BD0`, `cRZRandomSuper`).
    pub const POLLUTION: u32 = 0x4C09_C4D3;
    /// Wealth requester prop selection (`0x00B4A5C0`, `cRZRandomSuper`).
    pub const WEALTH_REQUESTER: u32 = 0x4C09_C51F;
    /// Network lot manager (`0x00B44870`, `cRZRandomSuper`).
    pub const NETWORK_LOT_MANAGER: u32 = 0x8C09_C379;
    /// Network construction crew (`0x00B43E88`, `cRZRandomSuper`).
    pub const NETWORK_CONSTRUCTION_CREW: u32 = 0x6C09_C07B;
    /// Construction occupant (`0x00B22FB0`, `cRZRandom`).
    pub const CONSTRUCTION_OCCUPANT: u32 = 0xEC09_C32A;
    /// Lot base-texture occupant (`0x00B23078`, `cRZRandom`).
    pub const BASE_TEXTURE_OCCUPANT: u32 = 0x8C09_C2D5;
    /// Lot retaining-wall occupant (`0x00B230C8`, `cRZRandom`).
    pub const RETAINING_WALL_OCCUPANT: u32 = 0x0C09_C347;
    /// Prop occupant (`0x00B231B0`, `cRZRandomSuper`).
    pub const PROP_OCCUPANT: u32 = 0xAC09_C2EC;
    /// Fire occupant, flames (`0x00B21648`, `cRZRandom`).
    pub const FIRE_OCCUPANT: u32 = 0xEC09_C147;
    /// Earthquake disaster (`0x00B22AE8`, `cRZRandom`).
    pub const EARTHQUAKE: u32 = 0x6C09_C1AB;
    /// Pipe-burst disaster (`0x00B22B60`, `cRZRandom`).
    pub const PIPE_BURST: u32 = 0xEC09_C1D1;
    /// Riot disaster (`0x00B22B80`, `cRZRandom`).
    pub const RIOT: u32 = 0x4C09_C187;
    /// Automata (`0x00B21D78`, `cRZRandomSuper`).
    pub const AUTOMATA: u32 = 0x4C09_C170;
    /// Region creation (`0x00B217C0`, `cRZRandom`).
    pub const REGION_CREATION: u32 = 0xAC35_7AF9;
    /// Advice (`0x00B22E58`, `cRZRandom`).
    pub const ADVICE: u32 = 0x2C09_C27B;
    /// Brush effects (`0x00B22BE8`, `cRZRandom`; effects.md §8.2, §14).
    pub const BRUSH_EFFECTS: u32 = 0xAC09_C1EF;
}

/// [choice] `RandomDoubleGaussian` gives up after this many rejected pairs
/// and returns 0 (a double path stuck at 0, e.g. seeded from a process clock
/// of 0, would never leave the polar loop).
pub const MAX_GAUSSIAN_TRIES: u32 = 1000;

/// 2^-32, exact as a `float` and as a `double`.
const TWO_POW_MINUS_32: f64 = 1.0 / 4_294_967_296.0;

/// `RandomDoubleUniform` of one signed 32-bit draw `i` (§6.1, both classes):
/// `r24(r24(i × 2^-32) + 0.5)`, in [0, 1] (`i ≥ 2^31 − 191` gives 1.0).
pub fn double_uniform_of(i: i32) -> f64 {
    r24(r24(f64::from(i) * TWO_POW_MINUS_32) + 0.5)
}

/// `RandomDoubleRangeUniform(a, b)` of a uniform value `u`:
/// `r24(r24(u × r24(b − a)) + a)`.
fn range_of(u: f64, a: f64, b: f64) -> f64 {
    r24(r24(u * r24(b - a)) + a)
}

/// The range mapping of `RandomDoubleRangeGaussian(a, b)` (§6.1) for a
/// Gaussian value `gauss`: `g := r24(gauss + 3)`; if not `g < 6`,
/// `g := 5.9999` (a `double`, unrounded); if `g < 0`, `g := 0`; result
/// `r24(r24(r24(b − a) × r24(g × 1/6)) + a)` with 1/6 a `double`.
pub fn gaussian_to_range(gauss: f64, a: f64, b: f64) -> f64 {
    let mut g = r24(gauss + 3.0);
    if g >= 6.0 || g.is_nan() {
        g = 5.9999;
    }
    if g < 0.0 {
        g = 0.0;
    }
    r24(r24(r24(b - a) * r24(g * (1.0 / 6.0))) + a)
}

/// `cRZRandom`: an LCG for the integer methods (`lcg`) and a multiplicative
/// generator for the double methods (`mul`).
#[derive(Clone, Debug)]
pub struct RzRandom {
    lcg: u32,
    mul: u32,
    cached_gaussian: Option<f64>,
    process_clock: u32,
}

impl RzRandom {
    /// `Seed(s)`; `s == 0xFFFFFFFF` takes the wall-clock seed from `seeds`.
    pub fn new(seed: u32, seeds: &dyn SeedSource) -> Self {
        let mut r = RzRandom {
            lcg: 0,
            mul: 0,
            cached_gaussian: None,
            process_clock: seeds.process_clock(),
        };
        r.seed(seed, seeds);
        r
    }

    pub fn seed(&mut self, seed: u32, seeds: &dyn SeedSource) {
        let s = if seed == CLOCK_SEED {
            seeds.wall_clock_seed()
        } else {
            seed
        };
        self.lcg = s;
        self.mul = s;
        self.cached_gaussian = None;
        self.process_clock = seeds.process_clock();
    }

    pub fn lcg_state(&self) -> u32 {
        self.lcg
    }

    pub fn mul_state(&self) -> u32 {
        self.mul
    }

    /// The Gaussian value cached by the last `gaussian()` call, if unused.
    pub fn cached_gaussian(&self) -> Option<f64> {
        self.cached_gaussian
    }

    /// Restores a saved generator (both paths and the Gaussian cache), as
    /// subsystems that persist their generator do (weather spec §10).
    pub fn set_state(&mut self, lcg: u32, mul: u32, cached_gaussian: Option<f64>) {
        self.lcg = lcg;
        self.mul = mul;
        self.cached_gaussian = cached_gaussian;
    }

    /// `RandomUint32Uniform()`.
    pub fn next_raw(&mut self) -> u32 {
        let t = u64::from(self.lcg) * 0x41C6_4E6D + 0x3039;
        self.lcg = t as u32;
        (t >> 16) as u32
    }

    /// `RandomUint32Uniform(n)`: 0 for n == 0 without advancing.
    pub fn uniform_u32(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        ((u64::from(self.next_raw()) * u64::from(n)) >> 32) as u32
    }

    /// `RandomSint32RangeUniform(lo, hi)`.
    pub fn range_i32(&mut self, lo: i32, hi: i32) -> i32 {
        lo.wrapping_add(self.uniform_u32(hi.wrapping_sub(lo) as u32) as i32)
    }

    /// `RandomDoubleUniform()`: [0, 1], 24-bit ([`double_uniform_of`]).
    pub fn uniform_f64(&mut self) -> f64 {
        if self.mul == 0 {
            self.mul = self.process_clock;
        }
        self.mul = self.mul.wrapping_mul(0x278D_DE6D);
        double_uniform_of(self.mul as i32)
    }

    /// `RandomDoubleRangeUniform(a, b)`: `r24(r24(u × r24(b − a)) + a)`.
    pub fn range_f64(&mut self, a: f64, b: f64) -> f64 {
        let u = self.uniform_f64();
        range_of(u, a, b)
    }

    /// `RandomDoubleGaussian()` (§6.1): Marsaglia polar method at 24 bits.
    /// `v1 := r24(2·u1 − 1)`, `v2 := r24(2·u2 − 1)` (u1 drawn first);
    /// `s := r24(r24(v2²) + r24(v1²))`, redrawn while `s ≥ 1` or `s = 0`;
    /// `f := r24(sqrt(r24(r24(ln s / s) × −2)))`. Returns `r24(f × v2)` and
    /// caches `r24(v1 × f)` for the next call. `ln` is the runtime's
    /// double-precision logarithm (only the division is rounded; §6.1
    /// [choice]). At most [`MAX_GAUSSIAN_TRIES`] pairs, then 0 [choice].
    pub fn gaussian(&mut self) -> f64 {
        if let Some(g) = self.cached_gaussian.take() {
            return g;
        }
        for _ in 0..MAX_GAUSSIAN_TRIES {
            let v1 = r24(2.0 * self.uniform_f64() - 1.0);
            let v2 = r24(2.0 * self.uniform_f64() - 1.0);
            let s = r24(r24(v2 * v2) + r24(v1 * v1));
            if s >= 1.0 || s == 0.0 {
                continue;
            }
            let f = r24(r24(r24(s.ln() / s) * -2.0).sqrt());
            self.cached_gaussian = Some(r24(v1 * f));
            return r24(f * v2);
        }
        0.0
    }

    /// `RandomDoubleRangeGaussian(a, b)` ([`gaussian_to_range`]).
    pub fn range_gaussian(&mut self, a: f64, b: f64) -> f64 {
        let g = self.gaussian();
        gaussian_to_range(g, a, b)
    }

    /// `RandomDoubleRangeGaussianFast(a, b)` (§6.1): draws u1, u2, u3 in
    /// order, then `r24(r24(r24(r24(r24(u3 + u2) + u1) × r24(b − a)) × 1/3) + a)`
    /// with 1/3 a `double`.
    pub fn range_gaussian_fast(&mut self, a: f64, b: f64) -> f64 {
        let u1 = self.uniform_f64();
        let u2 = self.uniform_f64();
        let u3 = self.uniform_f64();
        let sum = r24(r24(u3 + u2) + u1);
        r24(r24(r24(sum * r24(b - a)) * (1.0 / 3.0)) + a)
    }

    /// `RandomSint32RangeGaussianFast(lo, hi)` (foundations §6.1): always
    /// four raw draws, `a = (r1 + r2 + r3 + r4 + 2) >> 2` in 64 bits, then
    /// `lo + ((a × (u32)(hi − lo)) >> 32)`.
    pub fn range_i32_gaussian_fast(&mut self, lo: i32, hi: i32) -> i32 {
        let sum: u64 = (0..4).map(|_| u64::from(self.next_raw())).sum();
        let avg = ((sum + 2) / 4) as u32;
        let n = hi.wrapping_sub(lo) as u32;
        lo.wrapping_add(((u64::from(avg) * u64::from(n)) >> 32) as i32)
    }
}

const MT_N: usize = 624;
const MT_M: usize = 397;

/// `cRZRandomSuper`: MT19937 with the game's seeding (§6.1).
#[derive(Clone)]
pub struct RzRandomSuper {
    state: [u32; MT_N],
    index: usize,
}

impl std::fmt::Debug for RzRandomSuper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RzRandomSuper {{ index: {} }}", self.index)
    }
}

impl RzRandomSuper {
    pub fn new(seed: u32, seeds: &dyn SeedSource) -> Self {
        let mut r = RzRandomSuper {
            state: [0; MT_N],
            index: MT_N,
        };
        r.seed(seed, seeds);
        r
    }

    pub fn seed(&mut self, seed: u32, seeds: &dyn SeedSource) {
        let s = if seed == CLOCK_SEED {
            seeds.wall_clock_seed()
        } else {
            seed
        };
        let mut x = s | 1;
        for i in 0..MT_N {
            self.state[i] = (x & 0xFFFF_0000) | (x.wrapping_mul(69069) >> 16);
            x = x.wrapping_mul(0x1C58_7629).wrapping_add(0x10DCE);
        }
        self.twist();
        self.index = 0;
    }

    /// A generator loaded from a raw state (network-manager.md §17.3 replay
    /// rule 1): the 624 words and the position `k` (0..=624) of the next word
    /// to temper; `k = 624` reloads before the next draw.
    pub fn from_raw(words: &[u32; MT_N], k: usize) -> Self {
        RzRandomSuper {
            state: *words,
            index: k.min(MT_N),
        }
    }

    /// The raw state `(words, k)` (§17.2 of network-manager.md).
    pub fn raw(&self) -> (&[u32; MT_N], usize) {
        (&self.state, self.index)
    }

    /// The raw (untempered) state word `i` (tests).
    pub fn state_word(&self, i: usize) -> u32 {
        self.state[i]
    }

    fn twist(&mut self) {
        for i in 0..MT_N {
            let y = (self.state[i] & 0x8000_0000) | (self.state[(i + 1) % MT_N] & 0x7FFF_FFFF);
            let mut v = self.state[(i + MT_M) % MT_N] ^ (y >> 1);
            if y & 1 != 0 {
                v ^= 0x9908_B0DF;
            }
            self.state[i] = v;
        }
    }

    /// `RandomUint32Uniform()`: the next tempered word.
    pub fn next_u32(&mut self) -> u32 {
        if self.index >= MT_N {
            self.twist();
            self.index = 0;
        }
        let mut y = self.state[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9D2C_5680;
        y ^= (y << 15) & 0xEFC6_0000;
        y ^= y >> 18;
        y
    }

    /// `RandomUint32Uniform(n)`.
    pub fn uniform_u32(&mut self, n: u32) -> u32 {
        ((u64::from(self.next_u32()) * u64::from(n)) >> 32) as u32
    }

    /// `RandomSint32RangeUniform(lo, hi)`.
    pub fn range_i32(&mut self, lo: i32, hi: i32) -> i32 {
        lo.wrapping_add(self.uniform_u32(hi.wrapping_sub(lo) as u32) as i32)
    }

    /// `RandomDoubleUniform()`: [0, 1], 24-bit ([`double_uniform_of`]).
    pub fn uniform_f64(&mut self) -> f64 {
        double_uniform_of(self.next_u32() as i32)
    }

    /// `RandomDoubleRangeUniform(a, b)`: `r24(r24(u × r24(b − a)) + a)`.
    pub fn range_f64(&mut self, a: f64, b: f64) -> f64 {
        let u = self.uniform_f64();
        range_of(u, a, b)
    }

    /// `RandomUint32WeightedChoice(n, w)` (§6.1): `S` sums the positive
    /// weights in index order, each addition rounded to 24 bits;
    /// `r := RandomDoubleRangeUniform(0, S)`; the walk subtracts with
    /// `r := r24(r − w[i])` until `r < w[i]`.
    pub fn weighted_choice(&mut self, weights: &[f64]) -> u32 {
        let n = weights.len();
        if n < 2 {
            return 0;
        }
        let total = weights.iter().filter(|&&w| w > 0.0).fold(0.0, |s, &w| r24(s + w));
        if total == 0.0 {
            return self.uniform_u32(n as u32);
        }
        let mut r = self.range_f64(0.0, total);
        for (i, &w) in weights.iter().enumerate() {
            if w > 0.0 {
                if r < w {
                    return i as u32;
                }
                r = r24(r - w);
            }
        }
        (n - 1) as u32
    }
}

/// The C runtime `rand()` (foundations §12.1). Each replaced subsystem owns
/// one, seeded with 1 by default (§12.6 [choice]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrtRand {
    state: u32,
}

impl Default for CrtRand {
    fn default() -> Self {
        CrtRand { state: 1 }
    }
}

impl CrtRand {
    pub fn new(seed: u32) -> CrtRand {
        CrtRand { state: seed }
    }

    /// `srand(x)`.
    pub fn srand(&mut self, seed: u32) {
        self.state = seed;
    }

    pub fn state(&self) -> u32 {
        self.state
    }

    /// `rand()`: 0 … 32767.
    pub fn rand(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(214_013).wrapping_add(2_531_011);
        (self.state >> 16) & 0x7FFF
    }

    /// `std::random_shuffle(first, last)` (§12.3): exactly `n − 1` calls.
    pub fn random_shuffle<T>(&mut self, a: &mut [T]) {
        for i in 1..a.len() {
            let j = (self.rand() as usize) % (i + 1);
            a.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaussian_fast_int() {
        // foundations §6.1: seed 1, (0, 25) gives 10, then 7; always 4 draws.
        let seeds = FixedSeeds {
            wall_clock: 5,
            process_clock: 7,
        };
        let mut r = RzRandom::new(1, &seeds);
        assert_eq!(
            [r.range_i32_gaussian_fast(0, 25), r.range_i32_gaussian_fast(0, 25)],
            [10, 7]
        );
        let before = r.lcg_state();
        r.range_i32_gaussian_fast(3, 3);
        let mut probe = RzRandom::new(before, &seeds);
        for _ in 0..4 {
            probe.next_raw();
        }
        assert_eq!(r.lcg_state(), probe.lcg_state());
    }

    #[test]
    fn crt_rand_sequences() {
        let mut r = CrtRand::default();
        let got: Vec<u32> = (0..10).map(|_| r.rand()).collect();
        assert_eq!(got, [41, 18467, 6334, 26500, 19169, 15724, 11478, 29358, 26962, 24464]);
        r.srand(12345);
        assert_eq!([r.rand(), r.rand(), r.rand()], [7584, 19164, 25795]);
        let mut r = CrtRand::default();
        let mut a: Vec<u32> = (0..10).collect();
        r.random_shuffle(&mut a);
        assert_eq!(a, [8, 1, 9, 2, 0, 5, 7, 3, 4, 6]);
    }

    const SEEDS: FixedSeeds = FixedSeeds {
        wall_clock: 12345,
        process_clock: 777,
    };

    #[test]
    fn rz_random_integer_vectors() {
        let mut r = RzRandom::new(1, &SEEDS);
        assert_eq!(r.next_raw(), 0x0000_41C6);
        assert_eq!(r.lcg_state(), 0x41C6_7EA6);
        assert_eq!(r.next_raw(), 0x59D4_967E);
        assert_eq!(r.lcg_state(), 0x967E_B0E7);
        assert_eq!(r.next_raw(), 0xBF02_2781);
        assert_eq!(r.lcg_state(), 0x2781_E494);
        let before = r.lcg_state();
        assert_eq!(r.uniform_u32(0), 0);
        assert_eq!(r.lcg_state(), before);
    }

    #[test]
    fn rz_random_double_vectors() {
        let mut r = RzRandom::new(1, &SEEDS);
        // foundations §7 test 13 (24-bit, §6.1).
        let expect = [
            (0.654_508_471_488_952_6, 0x278D_DE6D),
            (0.481_738_716_363_906_86, 0xFB53_3A69),
            (0.851_256_132_125_854_5, 0x59EB_ECB5),
        ];
        for (v, state) in expect {
            assert_eq!(r.uniform_f64(), v);
            assert_eq!(r.mul_state(), state);
        }
        assert_eq!(RzRandom::new(1, &SEEDS).range_f64(0.0, 100.0), 65.450_843_811_035_16);
        assert_eq!(
            RzRandom::new(1, &SEEDS).range_gaussian_fast(0.0, 10.0),
            6.625_010_967_254_639
        );
    }

    #[test]
    fn gaussian_gives_up_on_a_stuck_double_path() {
        // Seed 0 with a process clock of 0: mul stays 0, every uniform is
        // 0.5, v1 = v2 = 0 and s = 0, so the polar loop never accepts a pair.
        let stuck = FixedSeeds {
            wall_clock: 0,
            process_clock: 0,
        };
        let mut r = RzRandom::new(0, &stuck);
        assert_eq!(r.gaussian(), 0.0);
        assert_eq!((r.mul_state(), r.cached_gaussian()), (0, None));
    }

    #[test]
    fn uniform_rounding_and_gaussian_clamp() {
        // foundations §7 test 16.
        assert_eq!(double_uniform_of(0x7FFF_FF41), 1.0);
        assert_eq!(double_uniform_of(0x7FFF_FF40), 0.999_999_940_395_355_2);
        assert_eq!(double_uniform_of(i32::MIN), 0.0);
        assert_eq!(gaussian_to_range(3.5, 0.0, 10.0), 9.999_833_106_994_629);
        assert_eq!(gaussian_to_range(-4.0, 0.0, 10.0), 0.0);
        // Every value is a float32.
        let mut r = RzRandom::new(7, &SEEDS);
        for _ in 0..200 {
            let g = r.gaussian();
            assert_eq!(f64::from(g as f32), g);
            let u = r.range_gaussian(-3.0, 9.0);
            assert!((-3.0..=9.0).contains(&u) && f64::from(u as f32) == u);
        }
    }

    #[test]
    fn rz_random_clock_seeding() {
        let r = RzRandom::new(CLOCK_SEED, &SEEDS);
        assert_eq!((r.lcg_state(), r.mul_state()), (12345, 12345));
        let mut z = RzRandom::new(0, &SEEDS);
        let _ = z.uniform_f64(); // mul 0 falls back to the process clock
        assert_eq!(z.mul_state(), 777u32.wrapping_mul(0x278D_DE6D));
    }

    #[test]
    fn rz_random_super_vectors() {
        let mut r = RzRandomSuper::new(1, &SEEDS);
        let mut table = RzRandomSuper {
            state: [0; MT_N],
            index: MT_N,
        };
        // Initial table before the twist.
        let mut x = 1u32;
        for i in 0..2 {
            table.state[i] = (x & 0xFFFF_0000) | (x.wrapping_mul(69069) >> 16);
            x = x.wrapping_mul(0x1C58_7629).wrapping_add(0x10DCE);
        }
        assert_eq!(table.state[0], 0x0000_0001);
        assert_eq!(table.state[1], 0x1C59_C359);
        assert_eq!(r.next_u32(), 0xD3F6_B9E5);
        assert_eq!(r.next_u32(), 0xEF28_4157);
        assert_eq!(r.next_u32(), 0x0D36_39C7);
        let mut r = RzRandomSuper::new(4357, &SEEDS);
        assert_eq!(r.next_u32(), 0xAAE6_4AC3);
        assert_eq!(r.next_u32(), 0x5E7C_47DA);
        assert_eq!(r.next_u32(), 0xB98E_8121);
    }

    /// CQ21: the shared `cRZTime` value, with crime.md §9's worked example
    /// (2026-10-14 12:00:00 US Central daylight time = 17:00:00 UTC,
    /// `time()` = 1 791 997 200).
    #[test]
    fn rz_time_value_cq21() {
        let rz = rz_time_from_unix(1_791_997_200);
        assert_eq!(rz, 3_969_450_000);
        assert_eq!(rz, 0xEC99_0010);
        assert_eq!(clock_seed(rz, file_seed::CRIME), 0x8090_C3E6);
        assert_eq!(derive_seed(rz, file_seed::CRIME), 0x8090_C3E6);
        // The epoch, and the u32 wrap (2037-02-07 06:28:16 UTC).
        assert_eq!(rz_time_from_unix(0), 2_177_452_800);
        assert_eq!(rz_time_from_unix(-RZ_TIME_UNIX_OFFSET), 0);
        assert_eq!(rz_time_from_unix(4_294_967_296 - RZ_TIME_UNIX_OFFSET), 0);
        assert_eq!(rz_time_from_unix(4_294_967_295 - RZ_TIME_UNIX_OFFSET), u32::MAX);
        // A cRZTime of 0 (the library's spring-forward hour) seeds with the
        // constant itself; the construction occupant's plain reseed is K = 0.
        assert_eq!(clock_seed(0, file_seed::CRIME), file_seed::CRIME);
        assert_eq!(clock_seed(rz, 0), rz);
    }

    #[test]
    fn weighted_choice() {
        let mut r = RzRandomSuper::new(99, &SEEDS);
        for _ in 0..100 {
            assert_eq!(r.weighted_choice(&[0.0, 1.0, 0.0]), 1);
            assert!(r.weighted_choice(&[0.0, -1.0, 0.0]) < 3);
        }
        assert_eq!(r.weighted_choice(&[5.0]), 0);
    }
}
