// SPDX-License-Identifier: GPL-3.0-or-later
//! Literal axis/ANGLE reading and row-matrix composition, graphics-formats §3.11.

pub type Vector3 = [f32; 3];
pub type Rotation = [[f32; 3]; 3];

/// Reuse the existing 24-bit numerical primitive. Its low-exponent fallback
/// stores binary32; scale exact powers of two around it for tiny axis squares
/// so normalization has no extra binary32 underflow before the specified stores.
fn round_wide(value: f64) -> f64 {
    if value.abs() < f64::from(f32::MIN_POSITIVE) {
        let scale = 2f64.powi(256);
        crate::advisors::r24_wide(value * scale) / scale
    } else {
        crate::advisors::r24_wide(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub version: u8,
    pub flags: u8,
    pub rotation: Rotation,
    pub translation: Vector3,
    pub scale: f32,
}

impl Transform {
    pub const IDENTITY: Self = Self {
        version: 0,
        flags: 0,
        rotation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        translation: [0.0; 3],
        scale: 1.0,
    };

    pub fn from_record(record: [f32; 7]) -> Self {
        let [px, py, pz, ax, ay, az, angle] = record;
        // GF13's deliberate choice: a zero axis means no rotation even when
        // the angle is nonzero. This is NOT the data's quaternion interpretation.
        let rotation = if ax == 0.0 && ay == 0.0 && az == 0.0 {
            Self::IDENTITY.rotation
        } else {
            // Published association: (z² + y²) + x². r24_wide preserves the
            // existing numerical primitive's wide exponent. Each normalized
            // component is then separately stored to binary32. This is not a
            // claim of full x87/FSIN/FCOS equivalence (GF19/GF22).
            let r = round_wide;
            let [ax, ay, az] = [ax, ay, az].map(f64::from);
            let squared = r(r(r(az * az) + r(ay * ay)) + r(ax * ax));
            let inverse_length = r(1.0 / r(squared.sqrt()));
            let [nx, ny, nz] = [ax, ay, az].map(|a| r(a * inverse_length) as f32);
            let half_angle = r(f64::from(angle) * -0.5);
            let sine = half_angle.sin();
            let x = f64::from(nx) * sine;
            let y = f64::from(ny) * sine;
            let z = f64::from(nz) * sine;
            let w = half_angle.cos();
            [
                [
                    1.0 - 2.0 * y * y - 2.0 * z * z,
                    2.0 * x * y + 2.0 * z * w,
                    2.0 * x * z - 2.0 * y * w,
                ],
                [
                    2.0 * x * y - 2.0 * z * w,
                    1.0 - 2.0 * x * x - 2.0 * z * z,
                    2.0 * y * z + 2.0 * x * w,
                ],
                [
                    2.0 * x * z + 2.0 * y * w,
                    2.0 * y * z - 2.0 * x * w,
                    1.0 - 2.0 * x * x - 2.0 * y * y,
                ],
            ]
            .map(|row| row.map(|value| value as f32))
        };
        Self {
            version: 0,
            flags: super::MODEL_EFFECT_TRANSFORM_FLAGS,
            rotation,
            translation: [px, py, pz],
            scale: 1.0,
        }
    }

    /// `self` is the outer placement/instance transform, `incoming` the inner
    /// local transform. Translation never depends on incoming rotation (§3.11).
    pub fn compose(self, incoming: Self) -> Self {
        let mut rotation = [[0.0; 3]; 3];
        for (i, row) in rotation.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = self.rotation[i][0] * incoming.rotation[0][j]
                    + self.rotation[i][1] * incoming.rotation[1][j]
                    + self.rotation[i][2] * incoming.rotation[2][j];
            }
        }
        Self {
            version: 0,
            flags: self.flags | incoming.flags,
            rotation,
            translation: self.transform_point(incoming.translation),
            scale: self.scale * incoming.scale,
        }
    }

    /// Owner shortcuts test only zero versus nonzero, not individual bits.
    pub fn compose_record(self, local: Self) -> Self {
        let mut result = if local.flags == 0 {
            self
        } else if self.flags == 0 {
            local
        } else {
            let mut composed = self.compose(local);
            composed.scale = self.scale;
            composed.flags = self.flags | super::MODEL_EFFECT_TRANSFORM_FLAGS;
            composed
        };
        result.version = 0;
        result
    }

    pub fn transform_point(self, point: Vector3) -> Vector3 {
        let mut result = [0.0; 3];
        for (i, cell) in result.iter_mut().enumerate() {
            *cell = (self.rotation[i][0] * point[0] + self.rotation[i][1] * point[1] + self.rotation[i][2] * point[2])
                * self.scale
                + self.translation[i];
        }
        result
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}
