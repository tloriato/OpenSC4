// SPDX-License-Identifier: GPL-3.0-or-later
//! Crime's persisted incidents and iterator (crime §12), without visuals.

use crate::grid::CellIterator;
use crate::store::archive::{
    fields::{array, count},
    ObjectReference,
};
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug)]
pub struct IncidentRecord {
    pub lot: ObjectReference,
    pub kind: u32,
    pub name: Vec<u8>,
    pub icon: [u32; 3],
}

#[derive(Clone, Debug)]
pub struct CrimeRecord {
    pub grids: [ObjectReference; 2],
    pub iterator: CellIterator,
    pub unused: [u32; 2],
    pub average: f32,
    pub ordinance_factor: f32,
    pub monthly_count: u32,
    pub hotspot: [i32; 3],
    pub incidents: Vec<IncidentRecord>,
}

impl CrimeRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        if r.get_u16()? != 6 {
            return None;
        }
        let grids = array(r, |r| ObjectReference::read(r))?;
        let iterator = CellIterator::read(r)?;
        let unused = array(r, |r| r.get_u32())?;
        let average = r.get_f32()?;
        let ordinance_factor = r.get_f32()?;
        let monthly_count = r.get_u32()?;
        let hotspot = array(r, |r| r.get_i32())?;
        let n = count(r, 24)?;
        let mut incidents = Vec::with_capacity(n);
        for _ in 0..n {
            incidents.push(IncidentRecord {
                lot: ObjectReference::read(r)?,
                kind: r.get_u32()?,
                name: r.get_str()?,
                icon: array(r, |r| r.get_u32())?,
            });
        }
        Some(Self {
            grids,
            iterator,
            unused,
            average,
            ordinance_factor,
            monthly_count,
            hotspot,
            incidents,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(6);
        for reference in self.grids {
            reference.write(w);
        }
        self.iterator.write(w);
        for v in self.unused {
            w.put_u32(v);
        }
        w.put_f32(self.average);
        w.put_f32(self.ordinance_factor);
        w.put_u32(self.monthly_count);
        for v in self.hotspot {
            w.put_i32(v);
        }
        w.put_u32(self.incidents.len() as u32);
        for incident in &self.incidents {
            incident.lot.write(w);
            w.put_u32(incident.kind);
            w.put_str(&incident.name);
            for v in incident.icon {
                w.put_u32(v);
            }
        }
        w.error() == 0
    }
}
