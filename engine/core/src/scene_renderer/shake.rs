// SPDX-License-Identifier: GPL-3.0-or-later
//! Screen-shake ownership and explicitly bounded numerical paths (§§6.3,10).
use super::{Result, Unsupported};
use crate::rng::{FixedSeeds, RzRandom};
use std::sync::OnceLock;

/// Explicit process-owned tables, shared by portable renderer instances.
/// Callers cannot supply computed pattern entries. Sine beyond zero is unsupported.
#[derive(Default)]
pub struct Patterns {
    random: OnceLock<[[f32; 2]; 64]>,
}
impl Patterns {
    pub fn random(&self) -> &[[f32; 2]; 64] {
        self.random.get_or_init(random_pattern)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Parameters {
    pub duration: f32,
    pub fade: f32,
    pub amplitude: Vec<f32>,
    pub rate: Vec<f32>,
    pub aspect: f32,
    pub pattern: i32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub start: f32,
    pub phase: f32,
    pub position: Option<[f32; 2]>,
    pub parameters: Parameters,
    pub handle: u32,
}
#[derive(Default)]
pub struct Shakes {
    nodes: Vec<Node>,
    next_handle: u32,
    last_time: f32,
    offsets: (i32, i32),
}
impl Shakes {
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }
    pub fn offsets(&self) -> (i32, i32) {
        self.offsets
    }
    pub fn last_time(&self) -> f32 {
        self.last_time
    }
    /// The supplied clock is elapsed time since the current list's restart.
    /// Return bool requests the embedding's RAW timer restart, not a shake
    /// outcome. Empty-list starts have start=0 and last_time=0 after restart.
    pub fn start(
        &mut self,
        parameters: &Parameters,
        position: Option<[f32; 2]>,
        now: f32,
    ) -> Result<(u32, bool)> {
        validate(parameters)?;
        if !now.is_finite() || position.is_some_and(|p| p.iter().any(|v| !v.is_finite())) {
            return Err(Unsupported::Domain("nonfinite shake start"));
        }
        let next = self
            .next_handle
            .checked_add(1)
            .ok_or(Unsupported::Overflow)?;
        self.nodes
            .try_reserve(1)
            .map_err(|_| Unsupported::Allocation)?;
        let mut amplitude = Vec::new();
        let mut rate = Vec::new();
        amplitude
            .try_reserve(parameters.amplitude.len())
            .map_err(|_| Unsupported::Allocation)?;
        rate.try_reserve(parameters.rate.len())
            .map_err(|_| Unsupported::Allocation)?;
        amplitude.extend_from_slice(&parameters.amplitude);
        rate.extend_from_slice(&parameters.rate);
        let restart = self.nodes.is_empty();
        if restart {
            self.last_time = 0.0;
        }
        let handle = self.next_handle;
        self.next_handle = next;
        self.nodes.insert(
            0,
            Node {
                start: if restart { 0.0 } else { now },
                phase: 0.0,
                position,
                parameters: Parameters {
                    amplitude,
                    rate,
                    ..*parameters
                },
                handle,
            },
        );
        Ok((handle, restart))
    }
    pub fn start_overload(
        &mut self,
        amplitude: f32,
        duration: f32,
        now: f32,
    ) -> Result<(u32, bool)> {
        // RND4 published portable choice: fade=0, never emulate stack garbage.
        let mut a = Vec::new();
        let mut r = Vec::new();
        a.try_reserve(2).map_err(|_| Unsupported::Allocation)?;
        r.try_reserve(3).map_err(|_| Unsupported::Allocation)?;
        a.extend_from_slice(&[amplitude, 0.0]);
        r.extend_from_slice(&[0.0, 60.0, 0.0]);
        self.start(
            &Parameters {
                duration,
                fade: 0.0,
                amplitude: a,
                rate: r,
                aspect: 1.0,
                pattern: 0,
            },
            None,
            now,
        )
    }
    pub fn stop(&mut self, handle: u32, now: f32) -> Result<()> {
        let Some(i) = self.nodes.iter().position(|n| n.handle == handle) else {
            return Ok(());
        };
        if self.nodes[i].parameters.fade == 0.0 {
            self.nodes.remove(i);
            return Ok(());
        }
        let n = &mut self.nodes[i];
        // Strict domain ensures f64 evaluates this extended expression exactly
        // before its single f32 store; wider exponent-spread domains unsupported.
        if ![now, n.parameters.fade, n.parameters.duration, n.start]
            .into_iter()
            .all(dyadic)
        {
            return Err(Unsupported::Domain("extended shake stop arithmetic"));
        }
        let candidate = (f64::from(now)
            + (f64::from(n.parameters.fade) - f64::from(n.parameters.duration)))
            as f32;
        if candidate < n.start {
            n.start = candidate;
        }
        Ok(())
    }
    /// Exact bounded SetShakeOffsets: disabled policy, expiry, phase, zero
    /// contributions, table phase zero, and zoom>=3. Arbitrary interpolation,
    /// FSIN and wider extended accumulation explicitly return Unsupported.
    pub fn set_offsets(
        &mut self,
        enabled: bool,
        now: f32,
        centre: [f32; 2],
        radius: f32,
        zoom: i32,
        patterns: &Patterns,
    ) -> Result<()> {
        self.offsets = (0, 0);
        if !enabled || self.nodes.is_empty() {
            return Ok(());
        }
        if ![now, self.last_time, centre[0], centre[1], radius]
            .into_iter()
            .all(dyadic)
            || radius <= 0.0
        {
            return Err(Unsupported::Domain("shake frame domain"));
        }
        let dt = now - self.last_time;
        if dt < 0.0 {
            return Err(Unsupported::Domain("backwards shake time"));
        }
        self.last_time = now;
        let mut sum = [0.0f32; 2];
        let mut i = 0;
        while i < self.nodes.len() {
            let n = &mut self.nodes[i];
            if !dyadic(n.start) {
                return Err(Unsupported::Domain("shake start domain"));
            }
            let t = now - n.start;
            if t > n.parameters.duration {
                self.nodes.remove(i);
                continue;
            }
            if t < 0.0 {
                return Err(Unsupported::Domain("negative shake age"));
            }
            let weight = if let Some(p) = n.position {
                positional_weight(p, centre, radius)?
            } else {
                1.0
            };
            if weight < 0.0 {
                i += 1;
                continue;
            }
            let u = f64::from(t) / f64::from(n.parameters.duration);
            if !dyadic(u as f32) || f64::from(u as f32) != u {
                return Err(Unsupported::Domain("non-exact shake curve ratio"));
            }
            let a = sample_exact(&n.parameters.amplitude, u as f32)?;
            let rate = sample_exact(&n.parameters.rate, u as f32)?;
            let phase = f64::from(n.phase) + f64::from(rate) * f64::from(t.min(dt));
            if !dyadic(n.phase)
                || !dyadic(rate)
                || !dyadic(t.min(dt))
                || phase < 0.0
                || phase > 1024.0
            {
                return Err(Unsupported::Domain("extended shake phase"));
            }
            // Phase store is rounded once, as published.
            n.phase = phase as f32;
            let aw = f64::from(a) * f64::from(weight);
            if aw != 0.0 && matches!(n.parameters.pattern, 0 | 1) {
                if n.phase % 64.0 != 0.0 {
                    return Err(Unsupported::Algorithm("extended shake table interpolation"));
                }
                let sample = if n.parameters.pattern == 0 {
                    patterns.random()[0]
                } else {
                    [0.0, 0.0]
                }; // sin(0)=0
                let ax = aw / f64::from(n.parameters.aspect);
                let ay = aw * f64::from(n.parameters.aspect);
                if !dyadic(a)
                    || !dyadic(weight)
                    || !dyadic(n.parameters.aspect)
                    || f64::from(ax as f32) != ax
                    || f64::from(ay as f32) != ay
                {
                    return Err(Unsupported::Domain("extended shake aspect"));
                }
                for (axis, scale) in [ax, ay].into_iter().enumerate() {
                    let product = f64::from(sample[axis]) * scale;
                    let next = f64::from(sum[axis]) + product;
                    if !product.is_finite()
                        || f64::from(product as f32) != product
                        || f64::from(next as f32) != next
                    {
                        return Err(Unsupported::Domain("extended shake accumulation"));
                    }
                    sum[axis] = next as f32;
                }
            }
            i += 1;
        }
        if zoom < 3 && sum != [0.0; 2] {
            return Err(Unsupported::Algorithm("extended repeated shake scale"));
        }
        let scale = if zoom == 3 { f64::from(0.6f32) } else { 1.0 };
        self.offsets = (
            round_offset(f64::from(sum[0]) * scale)?,
            round_offset(f64::from(sum[1]) * scale)?,
        );
        Ok(())
    }
}
fn validate(p: &Parameters) -> Result<()> {
    if p.duration <= 0.0
        || p.aspect == 0.0
        || p.fade < 0.0
        || ![p.duration, p.fade, p.aspect]
            .into_iter()
            .all(f32::is_finite)
        || p.amplitude.is_empty()
        || p.rate.is_empty()
        || p.amplitude.iter().chain(&p.rate).any(|v| !v.is_finite())
    {
        return Err(Unsupported::Domain("invalid shake parameters"));
    }
    Ok(())
}
fn dyadic(v: f32) -> bool {
    v.is_finite() && v.abs() <= 1024.0 && (v * 1024.0).fract() == 0.0
}
pub fn sample_exact(curve: &[f32], u: f32) -> Result<f32> {
    if curve.is_empty()
        || curve.len() > 1024
        || !dyadic(u)
        || !(0.0..=1.0).contains(&u)
        || curve.iter().any(|v| !dyadic(*v))
    {
        return Err(Unsupported::Domain("shake curve domain"));
    }
    if curve.len() == 1 {
        return Ok(curve[0]);
    }
    let x = (curve.len() - 1) as f64 * f64::from(u);
    let i = x.floor() as usize;
    let r = x - i as f64;
    if r <= 0.0 {
        return Ok(curve[i]);
    }
    let value = f64::from(curve[i]) + (f64::from(curve[i + 1]) - f64::from(curve[i])) * r;
    if f64::from(value as f32) != value {
        return Err(Unsupported::Domain("extended shake curve sample"));
    }
    Ok(value as f32)
}
pub fn positional_weight(position: [f32; 2], centre: [f32; 2], radius: f32) -> Result<f32> {
    if !position
        .into_iter()
        .chain(centre)
        .chain([radius])
        .all(dyadic)
        || radius <= 0.0
    {
        return Err(Unsupported::Domain("shake positional inputs"));
    }
    let inv = 1.0 / (f64::from(radius) * f64::from(radius));
    if f64::from(inv as f32) != inv {
        return Err(Unsupported::Domain("extended positional reciprocal"));
    }
    let dx = f64::from(position[0]) - f64::from(centre[0]);
    let dz = f64::from(position[1]) - f64::from(centre[1]);
    let w = 1.0 - (dx * dx + dz * dz) * inv;
    if f64::from(w as f32) != w {
        return Err(Unsupported::Domain("extended positional weight"));
    }
    Ok(w as f32)
}
pub fn random_pattern() -> [[f32; 2]; 64] {
    let mut random = RzRandom::new(
        88,
        &FixedSeeds {
            wall_clock: 0,
            process_clock: 0,
        },
    );
    std::array::from_fn(|_| {
        [
            random.range_f64(-10.0, 10.0) as f32,
            random.range_f64(-10.0, 10.0) as f32,
        ]
    })
}
pub fn round_offset(v: f64) -> Result<i32> {
    if !v.is_finite() || v.abs() >= 4194304.0 {
        return Err(Unsupported::Domain("shake rounding range"));
    }
    // Literal float trick; unlike casting v first this preserves X's extended
    // product before the add (within the exact f64 bounded domain).
    if f64::from(v as f32) == v {
        return Ok(crate::x87::rhe(v as f32));
    }
    Ok(((v + 12582912.0) as f32).to_bits() as i32 - 0x4b400000)
}
