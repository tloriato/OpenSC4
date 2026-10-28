// SPDX-License-Identifier: GPL-3.0-or-later
//! Flammability's persisted grids and hotspot list (flammability §10).

use crate::store::archive::{
    fields::{array, count},
    ObjectReference,
};
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug)]
pub struct FlammabilityRecord {
    pub grids: [ObjectReference; 2],
    pub multiplier: f32,
    pub average: u8,
    pub hotspots: Vec<[i32; 3]>,
}

impl FlammabilityRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        if r.get_u16()? != 2 {
            return None;
        }
        let grids = array(r, |r| ObjectReference::read(r))?;
        let multiplier = r.get_f32()?;
        let average = r.get_u8()?;
        let n = count(r, 12)?;
        let mut hotspots = Vec::with_capacity(n);
        for _ in 0..n {
            hotspots.push(array(r, |r| r.get_i32())?);
        }
        Some(Self {
            grids,
            multiplier,
            average,
            hotspots,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(2);
        for reference in self.grids {
            reference.write(w);
        }
        w.put_f32(self.multiplier);
        w.put_u8(self.average);
        w.put_u32(self.hotspots.len() as u32);
        for hotspot in &self.hotspots {
            for v in hotspot {
                w.put_i32(*v);
            }
        }
        w.error() == 0
    }
}
