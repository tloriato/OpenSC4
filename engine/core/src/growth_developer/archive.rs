// SPDX-License-Identifier: GPL-3.0-or-later
//! Growth developer's saved fields, growth-developer §9. No grid resizing or
//! capacity recomputation takes place when reading an archive.
use crate::store::archive::{fields::array, ObjectReference};
use crate::stream::{InStream, OutStream};

#[derive(Clone, Debug)]
pub struct GrowthRecord {
    pub phantom_sum: i32,
    pub total_capacity: u32,
    pub grids: [ObjectReference; 5],
}

impl GrowthRecord {
    pub fn read(r: &mut dyn InStream) -> Option<Self> {
        if r.get_u16()? != 1 {
            return None;
        }
        Some(Self {
            phantom_sum: r.get_i32()?,
            total_capacity: r.get_u32()?,
            grids: array(r, ObjectReference::read)?,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(1);
        w.put_i32(self.phantom_sum);
        w.put_u32(self.total_capacity);
        for grid in self.grids {
            grid.write(w);
        }
        w.error() == 0
    }
}
