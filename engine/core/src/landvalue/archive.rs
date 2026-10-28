// SPDX-License-Identifier: GPL-3.0-or-later
//! Saved land-value fields (landvalue §10), before Init restarts the pipeline.

use crate::store::archive::{fields::array, ObjectReference};
use crate::stream::{InStream, OutStream};

#[derive(Clone, Debug)]
pub struct LandValueRecord {
    pub grids: [ObjectReference; 3],
    pub dirty: u8,
    pub average: u8,
    pub mean_altitude: f32,
}

impl LandValueRecord {
    pub fn read(r: &mut dyn InStream) -> Option<Self> {
        if r.get_u16()? != 1 {
            return None;
        }
        Some(Self {
            grids: array(r, |r| ObjectReference::read(r))?,
            dirty: r.get_u8()?,
            average: r.get_u8()?,
            mean_altitude: r.get_f32()?,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(1);
        for reference in self.grids {
            reference.write(w);
        }
        w.put_u8(self.dirty);
        w.put_u8(self.average);
        w.put_f32(self.mean_altitude);
        w.error() == 0
    }
}
