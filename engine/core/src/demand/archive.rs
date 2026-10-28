// SPDX-License-Identifier: GPL-3.0-or-later
//! Demand's saved fields (demand §12), without live graph linking or flag
//! normalization. References keep the class-store identity and file order.

use crate::store::archive::{
    fields::{array, count},
    ObjectReference,
};
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug)]
pub struct DemandRecord {
    pub version: u16,
    pub legacy: Option<[u32; 4]>,
    pub bonuses_relief: [f32; 4],
    pub nodes: Vec<(u32, ObjectReference)>,
    pub census: Vec<(u32, f32)>,
}

impl DemandRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        let version = r.get_u16()?;
        if version >= 3 {
            return None;
        }
        let legacy = if version < 2 {
            Some(array(r, |r| r.get_u32())?)
        } else {
            None
        };
        let bonuses_relief = array(r, |r| r.get_f32())?;
        let n = count(r, 8)?;
        let mut nodes = Vec::with_capacity(n);
        for _ in 0..n {
            nodes.push((r.get_u32()?, ObjectReference::read(r)?));
        }
        let n = count(r, 8)?;
        let mut census = Vec::with_capacity(n);
        for _ in 0..n {
            census.push((r.get_u32()?, r.get_f32()?));
        }
        Some(Self {
            version,
            legacy,
            bonuses_relief,
            nodes,
            census,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(self.version);
        if let Some(legacy) = self.legacy {
            for v in legacy {
                w.put_u32(v);
            }
        }
        for v in self.bonuses_relief {
            w.put_f32(v);
        }
        w.put_u32(self.nodes.len() as u32);
        for (key, reference) in &self.nodes {
            w.put_u32(*key);
            reference.write(w);
        }
        w.put_u32(self.census.len() as u32);
        for (key, value) in &self.census {
            w.put_u32(*key);
            w.put_f32(*value);
        }
        w.error() == 0
    }
}

#[derive(Clone, Debug)]
pub enum DemandCap {
    Percent(u8),
    Fraction(f32),
}

#[derive(Clone, Debug)]
pub struct DemandNodeRecord {
    pub version: u16,
    pub id: u32,
    pub supply_demand: [f32; 4],
    pub debug_lock: [f32; 2],
    pub active: [f32; 3],
    pub flag: u8,
    pub modifiers: [f32; 2],
    pub cap: DemandCap,
}

impl DemandNodeRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        let version = r.get_u16()?;
        if version >= 3 {
            return None;
        }
        Some(Self {
            version,
            id: r.get_u32()?,
            supply_demand: array(r, |r| r.get_f32())?,
            debug_lock: array(r, |r| r.get_f32())?,
            active: array(r, |r| r.get_f32())?,
            flag: r.get_u8()?,
            modifiers: array(r, |r| r.get_f32())?,
            cap: if version == 1 {
                DemandCap::Percent(r.get_u8()?)
            } else {
                DemandCap::Fraction(r.get_f32()?)
            },
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(self.version);
        w.put_u32(self.id);
        for v in self.supply_demand {
            w.put_f32(v);
        }
        for v in self.debug_lock {
            w.put_f32(v);
        }
        for v in self.active {
            w.put_f32(v);
        }
        w.put_u8(self.flag);
        for v in self.modifiers {
            w.put_f32(v);
        }
        match self.cap {
            DemandCap::Percent(v) => w.put_u8(v),
            DemandCap::Fraction(v) => w.put_f32(v),
        };
        w.error() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::ByteWriter;

    #[test]
    fn legacy_node_preserves_percent_and_noncanonical_flag() {
        let mut bytes = vec![1, 0, 9, 0, 0, 0];
        bytes.extend_from_slice(&[0; 36]);
        bytes.push(0xFF);
        bytes.extend_from_slice(&[0; 8]);
        bytes.push(67);
        let mut r = ByteReader::new(&bytes);
        let mut record = DemandNodeRecord::read(&mut r).unwrap();
        assert_eq!(r.remaining(), 0);
        let mut w = ByteWriter::new();
        assert!(record.write(&mut w));
        assert_eq!(w.into_bytes(), bytes);
        record.id = 10;
        let mut w = ByteWriter::new();
        assert!(record.write(&mut w));
        assert_eq!(w.into_bytes()[2], 10);
        assert!(DemandNodeRecord::read(&mut ByteReader::new(&bytes[..bytes.len() - 1])).is_none());
    }
}
